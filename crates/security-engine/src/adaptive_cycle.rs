//! Bounded feedback planner for local wCOP path exploration.
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn number_set(value: &Value) -> Result<BTreeSet<u64>, String> {
    value.as_array().ok_or("lista de PCs ausente")?.iter()
        .map(|item| item.as_u64().ok_or_else(|| "PC inválido".to_string())).collect()
}

fn candidate_id(value: &Value) -> Result<&str, String> {
    let id = value["id"].as_str().ok_or("candidato sem ID")?;
    if id.is_empty() || id.len() > 64 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
        return Err("ID de candidato inválido".into());
    }
    Ok(id)
}

fn valid_call(selector: &str, data: &str, sender: &str) -> bool {
    selector.len() == 10 && selector.starts_with("0x") && data.starts_with(selector)
        && data.len() <= 522 && data[2..].bytes().all(|c| c.is_ascii_hexdigit())
        && sender.len() == 42 && sender.starts_with("0x")
        && sender[2..].bytes().all(|c| c.is_ascii_hexdigit())
}

pub fn plan_adaptive_step(engine: &Value, state: &Value) -> Result<Value, String> {
    if state["format"] != 1 || state["address"] != "0x8a1d45e102e886510e891d2ec656a708991e2d76"
        || state["block"].as_u64().unwrap_or(0) == 0 {
        return Err("estado adaptativo fora do escopo".into());
    }
    let code = engine["normalized_hex"].as_str().ok_or("runtime ausente")?;
    let raw = hex::decode(code.trim_start_matches("0x")).map_err(|e| e.to_string())?;
    let hash = hex::encode(Sha256::digest(raw));
    if state["engine_code_sha256"].as_str() != Some(hash.as_str())
        || state["focus_engine_code_sha256"].as_str().is_some_and(|value| value != hash) {
        return Err("estado e runtime divergentes".into());
    }
    let candidates = state["candidates"].as_array().ok_or("candidatos ausentes")?;
    let attempts = state["attempts"].as_array().ok_or("histórico ausente")?;
    if candidates.is_empty() || candidates.len() > 256 || attempts.len() > 64 {
        return Err("limite do ciclo excedido".into());
    }
    let all_pcs: BTreeSet<u64> = engine["instructions"].as_array().ok_or("instruções ausentes")?
        .iter().map(|v| v["pc"].as_u64().ok_or_else(|| "instrução inválida".to_string()))
        .collect::<Result<_, _>>()?;
    let seen = number_set(&state["visited_pcs"])?;
    let baseline = number_set(&state["baseline_pcs"])?;
    if !seen.is_subset(&all_pcs) || !baseline.is_subset(&seen) {
        return Err("PCs fora do runtime".into());
    }
    let plateau = attempts.iter().rev().take_while(|row|
        row["new_pcs"].as_u64().unwrap_or(0) == 0
        && row["new_edges"].as_u64().unwrap_or(0) == 0
    ).count();
    let max_attempts = state["max_attempts"].as_u64().unwrap_or(32).clamp(1, 40) as usize;
    let untried_promising = candidates.iter().any(|candidate| {
        let is_new = candidate["id"].as_str().is_some_and(|id|
            !attempts.iter().any(|row| row["id"].as_str() == Some(id)));
        let family_new = candidate["family"].as_str().is_some_and(|family|
            !attempts.iter().any(|row| row["family"].as_str() == Some(family)));
        is_new && (candidate["setup"].as_array().is_some_and(|steps| !steps.is_empty())
            || candidate["derived_from"].is_string() || family_new)
    });
    if attempts.len() >= max_attempts || (plateau >= 16 && !untried_promising) {
        return Ok(json!({"format": 1, "status": "plateau", "attempts": attempts.len(),
            "plateau": plateau, "visited_pcs": seen.len(), "baseline_pcs": baseline.len(),
            "reason": "limite ou dezesseis tentativas sem ganho após opções novas"}));
    }
    let mut block_for = HashMap::<u64, String>::new();
    let mut block_pcs = HashMap::<String, Vec<u64>>::new();
    for block in engine["blocks"].as_array().ok_or("blocos ausentes")? {
        let id = block["id"].as_str().ok_or("ID de bloco ausente")?.to_owned();
        let mut pcs = Vec::new();
        for value in block["instruction_pcs"].as_array().ok_or("PCs de bloco ausentes")? {
            let pc = value.as_u64().ok_or("PC de bloco inválido")?;
            block_for.insert(pc, id.clone());
            pcs.push(pc);
        }
        block_pcs.insert(id, pcs);
    }
    let mut graph = HashMap::<String, Vec<String>>::new();
    for edge in engine["control_edges"].as_array().ok_or("arestas ausentes")? {
        if let (Some(from), Some(to)) = (edge["from"].as_str(), edge["to"].as_str()) {
            graph.entry(from.to_owned()).or_default().push(to.to_owned());
        }
    }
    let seen_blocks: HashSet<String> = block_for.iter()
        .filter(|(pc, _)| seen.contains(pc))
        .map(|(_, block)| block.clone()).collect();
    let frontier_sources: HashSet<String> = graph.iter()
        .filter(|(from, next)| seen_blocks.contains(*from)
            && next.iter().any(|to| !seen_blocks.contains(to)))
        .map(|(from, _)| from.clone()).collect();
    let mut entries = HashMap::<String, u64>::new();
    for selector in engine["selectors"].as_array().ok_or("seletores ausentes")? {
        if let (Some(key), Some(pc)) = (selector["selector"].as_str(),
                                      selector["destination_pc"]["pc"].as_u64()) {
            entries.insert(key.to_owned(), pc);
        }
    }
    let epoch = state["epoch"].as_u64().unwrap_or(0);
    let mut tried = HashSet::new();
    let mut family_n = HashMap::<String, usize>::new();
    let mut family_reward = HashMap::<String, f64>::new();
    let mut selector_n = HashMap::<String, usize>::new();
    let mut selector_blocks = HashMap::<String, HashSet<String>>::new();
    for row in attempts {
        if row["epoch"].as_u64() == Some(epoch) {
            if let Some(id) = row["id"].as_str() { tried.insert(id.to_owned()); }
        }
        if let Some(family) = row["family"].as_str() {
            *family_n.entry(family.to_owned()).or_default() += 1;
            let reward = (row["new_pcs"].as_u64().unwrap_or(0).min(80) as f64) * 0.45
                + (row["new_edges"].as_u64().unwrap_or(0).min(24) as f64) * 1.8
                + (row["new_uncertain_pcs"].as_u64().unwrap_or(0).min(15) as f64) * 0.7
                + if row["repeatable_deviation"] == true { 60.0 } else { 0.0 };
            *family_reward.entry(family.to_owned()).or_default() += reward;
        }
        if let Some(selector) = row["selector"].as_str() {
            *selector_n.entry(selector.to_owned()).or_default() += 1;
            if let Some(blocks) = row["blocks"].as_array() {
                for block in blocks.iter().filter_map(Value::as_str) {
                    selector_blocks.entry(selector.to_owned()).or_default().insert(block.to_owned());
                }
            }
        }
    }
    let mut reachable_cache = HashMap::<String, (usize, usize)>::new();
    let mut ids = HashSet::new();
    let mut choices = Vec::<(f64, String, Value, usize, usize)>::new();
    for candidate in candidates {
        let id = candidate_id(candidate)?.to_owned();
        if !ids.insert(id.clone()) { return Err("IDs repetidos".into()); }
        let selector = candidate["selector"].as_str().ok_or("seletor ausente")?;
        let family = candidate["family"].as_str().ok_or("família ausente")?;
        let data = candidate["data"].as_str().ok_or("calldata ausente")?;
        let sender = candidate["sender"].as_str().ok_or("conta ausente")?;
        if !valid_call(selector, data, sender) { return Err("candidato inválido".into()); }
        if let Some(setup) = candidate.get("setup") {
            let steps = setup.as_array().ok_or("sequência inválida")?;
            if steps.is_empty() || steps.len() > 2 { return Err("sequência fora do limite".into()); }
            for step in steps {
                let step_data = step["data"].as_str().ok_or("operação sem dados")?;
                let step_sender = step["sender"].as_str().ok_or("operação sem conta")?;
                let prefix = step_data.get(..10).ok_or("operação curta")?;
                if !["0x095ea7b3", "0x2f2ff15d", "0x40c10f19", "0x8456cb59"]
                    .contains(&prefix) || !valid_call(prefix, step_data, step_sender) {
                    return Err("operação preparatória inválida".into());
                }
            }
        }
        if tried.contains(&id) { continue; }
        let (unseen_reachable, reachable_frontiers) = *reachable_cache
            .entry(selector.to_owned()).or_insert_with(|| {
                let Some(entry_pc) = entries.get(selector) else { return (0, 0) };
                let Some(first) = block_for.get(entry_pc) else { return (0, 0) };
                let mut queue = VecDeque::from([first.clone()]);
                let mut reached = HashSet::new();
                let (mut count, mut frontiers) = (0, 0);
                while let Some(block) = queue.pop_front() {
                    if !reached.insert(block.clone()) || reached.len() > 600 { continue; }
                    count += block_pcs.get(&block).into_iter().flatten()
                        .filter(|pc| !seen.contains(pc)).count();
                    if frontier_sources.contains(&block) { frontiers += 1; }
                    if let Some(next) = graph.get(&block) { queue.extend(next.iter().cloned()); }
                }
                (count, frontiers)
            });
        let n = *family_n.get(family).unwrap_or(&0);
        let mean = family_reward.get(family).copied().unwrap_or(0.0) / (n.max(1) as f64);
        let selector_seen = *selector_n.get(selector).unwrap_or(&0);
        let adjacent_frontiers = selector_blocks.get(selector).map(|blocks|
            blocks.intersection(&frontier_sources).count()).unwrap_or(0);
        let setup_len = candidate["setup"].as_array().map(|steps| steps.len()).unwrap_or(0);
        let followed_gain = candidate["derived_from"].as_str().is_some_and(|parent|
            attempts.iter().any(|row| row["id"].as_str() == Some(parent)
                && (row["new_pcs"].as_u64().unwrap_or(0) > 0
                    || row["new_edges"].as_u64().unwrap_or(0) > 0)));
        let focus = &state["focus_by_selector"][selector];
        let focus_score = focus["score"].as_f64().unwrap_or(0.0).clamp(0.0, 50.0);
        let focus_main = focus["unseen_main_pcs"].as_u64().unwrap_or(0).min(60);
        let focus_open = focus["open_links"].as_u64().unwrap_or(0).min(30);
        let score = 18.0 / (1.0 + n as f64).sqrt()
            + mean.min(25.0) * 0.35
            + (unseen_reachable.min(500) as f64).sqrt() * 0.65
            + if selector_seen == 0 { 8.0 } else { 0.0 }
            + if plateau >= 4 && n == 0 { 15.0 } else { 0.0 }
            + if plateau >= 7 && candidate["boundary"] == true { 5.0 } else { 0.0 }
            + (reachable_frontiers.min(30) as f64).sqrt() * 1.5
            + (adjacent_frontiers.min(8) as f64) * 2.0
            + if setup_len > 0 { if plateau >= 5 { 26.0 } else { 11.0 } } else { 0.0 }
            + if followed_gain { 12.0 } else { 0.0 }
            + focus_score * 0.30 / (1.0 + selector_seen as f64 * 0.10)
            + (focus_main as f64).sqrt() * 0.8
            + (focus_open as f64).sqrt() * 0.7;
        choices.push((score, id, candidate.clone(), unseen_reachable, reachable_frontiers));
    }
    choices.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let Some((score, _, candidate, reachable, frontiers)) = choices.first() else {
        return Ok(json!({"format": 1, "status": "exhausted", "attempts": attempts.len(),
            "plateau": plateau, "visited_pcs": seen.len(), "reason": "candidatos desta rodada esgotados"}));
    };
    Ok(json!({"format": 1, "status": "next", "candidate": candidate,
        "score": (score * 100.0).round() / 100.0, "reachable_unseen_pcs": reachable,
        "reachable_frontiers": frontiers, "frontier_blocks_total": frontier_sources.len(),
        "attempts": attempts.len(), "plateau": plateau,
        "policy": "ganho observado, saídas adjacentes ainda abertas e sequências curtas locais"}))
}
