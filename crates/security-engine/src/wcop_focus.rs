//! Read a pinned line map and choose where the next local reading has value.
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn pc_of(value: &str) -> Result<u64, String> {
    value.strip_prefix("pc:").ok_or("position prefix missing")?
        .parse::<u64>().map_err(|_| "position invalid".to_owned())
}

pub fn plan_wcop_focus(engine: &Value, map: &Value, base: &Value, extra: Option<&Value>) -> Result<Value, String> {
    let raw = hex::decode(engine["normalized_hex"].as_str().ok_or("runtime missing")?
        .trim_start_matches("0x")).map_err(|e| e.to_string())?;
    let code_hash = hex::encode(Sha256::digest(raw));
    let address = base["contract"].as_str().ok_or("contract missing")?;
    let block = base["block"].as_u64().ok_or("block missing")?;
    if map["format"] != 1 || map["contract"].as_str() != Some(address)
        || map["block"].as_u64() != Some(block)
        || map["engine_code_sha256"].as_str() != Some(code_hash.as_str())
        || base["engine_code_sha256"].as_str() != Some(code_hash.as_str()) {
        return Err("map and local readings differ".into());
    }
    if let Some(other) = extra {
        if other["contract"].as_str() != Some(address)
            || other["block"].as_u64() != Some(block)
            || other["engine_code_sha256"].as_str() != Some(code_hash.as_str()) {
            return Err("second reading differs".into());
        }
    }
    let all_pcs: HashSet<u64> = engine["instructions"].as_array().ok_or("instructions missing")?
        .iter().map(|row| row["pc"].as_u64().ok_or_else(|| "position invalid".to_owned()))
        .collect::<Result<_, _>>()?;
    let mut seen = HashSet::new();
    for reading in [Some(base), extra].into_iter().flatten() {
        for value in reading["pcs"].as_array().ok_or("positions missing")? {
            let pc = value.as_u64().ok_or("position invalid")?;
            if !all_pcs.contains(&pc) { return Err("observed position not in runtime".into()); }
            seen.insert(pc);
        }
    }
    if map["observed_pcs"].as_u64() != Some(seen.len() as u64) {
        return Err("line map has a different reading".into());
    }
    let mut locations = HashMap::<u64, (&str, &str)>::new();
    for row in map["pc_locations"].as_array().ok_or("line locations missing")? {
        let pc = row["pc"].as_u64().ok_or("mapped position invalid")?;
        let file = row["file"].as_str().ok_or("file missing")?;
        let kind = row["kind"].as_str().ok_or("kind missing")?;
        if !all_pcs.contains(&pc) || row["line"].as_u64().unwrap_or(0) == 0
            || !["source", "generated"].contains(&kind)
            || locations.insert(pc, (file, kind)).is_some() {
            return Err("line map invalid or repeated".into());
        }
    }
    if map["source_assigned_pcs"].as_u64() != Some(locations.len() as u64) {
        return Err("line count differs".into());
    }
    let mut block_for = HashMap::<u64, String>::new();
    let mut block_pcs = HashMap::<String, Vec<u64>>::new();
    for row in engine["blocks"].as_array().ok_or("blocks missing")? {
        let id = row["id"].as_str().ok_or("block id missing")?.to_owned();
        let pcs = row["instruction_pcs"].as_array().ok_or("block positions missing")?
            .iter().map(|pc| pc.as_u64().ok_or_else(|| "block position invalid".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        for pc in &pcs { block_for.insert(*pc, id.clone()); }
        block_pcs.insert(id, pcs);
    }
    let mut graph = HashMap::<String, Vec<String>>::new();
    let mut static_links = HashSet::<(String, String)>::new();
    for edge in engine["control_edges"].as_array().ok_or("links missing")? {
        let from = edge["from"].as_str().ok_or("from missing")?.to_owned();
        let to = edge["to"].as_str().ok_or("to missing")?.to_owned();
        graph.entry(from.clone()).or_default().push(to.clone());
        static_links.insert((from, to));
    }
    let mut observed_links = HashSet::<(String, String)>::new();
    for reading in [Some(base), extra].into_iter().flatten() {
        for edge in reading["observed_transitions"].as_array().ok_or("observed links missing")? {
            let pair = edge.as_array().ok_or("link invalid")?;
            if pair.len() != 2 { return Err("link length invalid".into()); }
            let from = pair[0].as_str().ok_or("link source invalid")?.to_owned();
            let to = pair[1].as_str().ok_or("link target invalid")?.to_owned();
            if !block_for.contains_key(&pc_of(&from)?) || !block_for.contains_key(&pc_of(&to)?) {
                return Err("observed link outside runtime".into());
            }
            observed_links.insert((from, to));
        }
    }
    let open_links: HashSet<_> = observed_links.difference(&static_links).cloned().collect();
    if map["observed_links_outside_static_map"].as_u64() != Some(open_links.len() as u64) {
        return Err("open link count differs".into());
    }
    let mut ranked = Vec::new();
    for row in map["links_to_review"].as_array().ok_or("open link list missing")? {
        let from = row["from"].as_str().ok_or("open link source missing")?;
        let to = row["to"].as_str().ok_or("open link target missing")?;
        if !open_links.contains(&(from.to_owned(), to.to_owned())) {
            return Err("listed link was not observed".into());
        }
        let from_pc = pc_of(from)?;
        let to_pc = pc_of(to)?;
        let source = locations.get(&from_pc).copied();
        let target = locations.get(&to_pc).copied();
        let in_main = [source, target].into_iter().flatten().any(|(file, kind)|
            kind == "source" && file == "src/LatamStable.sol");
        let generated = [source, target].into_iter().flatten().any(|(_, kind)| kind == "generated");
        let cross_file = source.zip(target).is_some_and(|((left, _), (right, _))| left != right);
        let score = 20 + if in_main { 25 } else { 0 }
            + if generated { 8 } else { 0 } + if cross_file { 5 } else { 0 };
        ranked.push(json!({"from": from, "to": to, "score": score,
            "from_source": row["from_source"], "to_source": row["to_source"],
            "reason": "observed locally; absent from the static link list"}));
    }
    if ranked.len() != open_links.len() { return Err("open link list incomplete".into()); }
    ranked.sort_by(|a,b| b["score"].as_u64().cmp(&a["score"].as_u64())
        .then_with(|| a["from"].as_str().cmp(&b["from"].as_str()))
        .then_with(|| a["to"].as_str().cmp(&b["to"].as_str())));
    let mut selector_focus = BTreeMap::<String, Value>::new();
    for row in engine["selectors"].as_array().ok_or("selectors missing")? {
        let Some(selector) = row["selector"].as_str() else { continue };
        let Some(entry_pc) = row["destination_pc"]["pc"].as_u64() else { continue };
        let Some(start) = block_for.get(&entry_pc) else { continue };
        let mut queue = VecDeque::from([start.clone()]);
        let mut reached = HashSet::new();
        while let Some(block) = queue.pop_front() {
            if !reached.insert(block.clone()) || reached.len() > 600 { continue; }
            if let Some(next) = graph.get(&block) { queue.extend(next.iter().cloned()); }
        }
        let mut unseen_source = 0;
        let mut unseen_main = 0;
        let mut unseen_generated = 0;
        for block in &reached {
            for pc in block_pcs.get(block).into_iter().flatten() {
                if seen.contains(pc) { continue; }
                if let Some((file, kind)) = locations.get(pc) {
                    if *kind == "generated" { unseen_generated += 1; }
                    else {
                        unseen_source += 1;
                        if *file == "src/LatamStable.sol" { unseen_main += 1; }
                    }
                }
            }
        }
        let open_count = open_links.iter().filter(|(from, _)| {
            pc_of(from).ok().and_then(|pc| block_for.get(&pc)).is_some_and(|block| reached.contains(block))
        }).count();
        let score = (unseen_source.min(500) as f64).sqrt()
            + (unseen_main.min(100) as f64).sqrt() * 2.0
            + (unseen_generated.min(500) as f64).sqrt() * 0.3
            + (open_count.min(30) as f64).sqrt() * 3.0;
        selector_focus.insert(selector.to_owned(), json!({"score": (score * 100.0).round()/100.0,
            "unseen_source_pcs": unseen_source, "unseen_main_pcs": unseen_main,
            "unseen_generated_pcs": unseen_generated, "open_links": open_count}));
    }
    Ok(json!({"format": 1, "contract": address, "block": block,
        "engine_code_sha256": code_hash, "observed_pcs": seen.len(),
        "open_links": open_links.len(), "selector_focus": selector_focus,
        "ranked_links": ranked, "limit": "rank orders local reading; it is not a finding"}))
}
