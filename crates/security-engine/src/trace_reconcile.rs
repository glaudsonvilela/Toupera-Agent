//! Bind observed local-fork program counters to one reconstructed runtime.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub fn reconcile_trace(engine: &Value, trace: &Value) -> Result<Value, String> {
    let encoded = engine["normalized_hex"].as_str().ok_or("motor sem runtime normalizado")?;
    let bytes = hex::decode(encoded.trim_start_matches("0x")).map_err(|e| e.to_string())?;
    let digest = hex::encode(Sha256::digest(bytes));
    if trace["engine_code_sha256"].as_str() != Some(digest.as_str()) {
        return Err("identidade do runtime e da leitura local diverge".into());
    }
    let instructions = engine["instructions"].as_array().ok_or("motor sem instruções")?;
    let instruction_pcs: BTreeSet<u64> = instructions.iter()
        .map(|item| item["pc"].as_u64().ok_or("PC do motor inválido"))
        .collect::<Result<_, _>>()?;
    let pcs: BTreeSet<u64> = trace["pcs"].as_array().ok_or("leitura sem PCs")?.iter()
        .map(|item| item.as_u64().ok_or("PC observado inválido"))
        .collect::<Result<_, _>>()?;
    if !pcs.is_subset(&instruction_pcs) {
        return Err("PC observado ausente do runtime reconstruído".into());
    }
    if trace["observed_instruction_pcs"].as_u64() != Some(pcs.len() as u64) {
        return Err("contagem de PCs observados divergente".into());
    }
    let blocks = engine["blocks"].as_array().ok_or("motor sem blocos")?;
    let mut block_for = BTreeMap::<u64, String>::new();
    let mut block_ids = BTreeSet::new();
    for block in blocks {
        let id = block["id"].as_str().ok_or("bloco sem identificador")?.to_owned();
        block_ids.insert(id.clone());
        for pc in block["instruction_pcs"].as_array().ok_or("bloco sem instruções")? {
            block_for.insert(pc.as_u64().ok_or("PC de bloco inválido")?, id.clone());
        }
    }
    let visited_blocks: BTreeSet<String> = pcs.iter()
        .filter_map(|pc| block_for.get(pc).cloned()).collect();
    let static_edges: BTreeSet<(String, String)> = engine["control_edges"].as_array()
        .ok_or("motor sem arestas")?.iter()
        .filter_map(|item| Some((item["from"].as_str()?.to_owned(),
                                 item["to"].as_str()?.to_owned())))
        .collect();
    let mut observed_edges = BTreeSet::new();
    for edge in trace["observed_transitions"].as_array().ok_or("leitura sem transições")? {
        let parts = edge.as_array().ok_or("transição inválida")?;
        if parts.len() != 2 { return Err("transição incompleta".into()); }
        let from = parts[0].as_str().ok_or("origem de transição inválida")?.to_owned();
        let to = parts[1].as_str().ok_or("destino de transição inválido")?.to_owned();
        if !block_ids.contains(&from) || !block_ids.contains(&to) {
            return Err("transição fora dos blocos reconstruídos".into());
        }
        observed_edges.insert((from, to));
    }
    let unmatched: Vec<_> = observed_edges.difference(&static_edges)
        .take(30).map(|(from, to)| json!({"from": from, "to": to})).collect();
    let mut observed_uncertainties = Vec::new();
    let mut observed_by_reason = BTreeMap::<String, usize>::new();
    for item in engine["unresolved"].as_array().ok_or("motor sem dúvidas")? {
        let Some(pc) = item["location"]["Bytecode"]["pc"].as_u64() else { continue };
        if !pcs.contains(&pc) { continue }
        let reason = item["detail"].as_str().unwrap_or("motivo não descrito");
        *observed_by_reason.entry(reason.to_owned()).or_default() += 1;
        if reason.contains("fork, which was not supplied") { continue }
        let priority = if reason.contains("storage key") { 4 }
            else if reason.contains("jump destination") { 3 }
            else if reason.contains("stack states") || reason.contains("memory word") { 2 }
            else { 1 };
        observed_uncertainties.push((priority, pc, reason.to_owned()));
    }
    observed_uncertainties.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let candidates: Vec<_> = observed_uncertainties.iter().take(30)
        .map(|(priority, pc, reason)| json!({
            "pc": pc, "question": reason, "priority": priority,
            "basis": "dúvida em PC observado em chamada isolada; exige revisão, não é falha confirmada"
        })).collect();
    let cases = trace["cases"].as_array().ok_or("leitura sem casos")?;
    let errors = cases.iter().filter(|case| case.get("error").is_some()).count();
    let reverted = cases.iter().filter(|case| case["failed"] == true).count();
    let matched_edges = observed_edges.intersection(&static_edges).count();
    Ok(json!({
        "contract": trace["contract"], "block": trace["block"],
        "engine_code_sha256": digest,
        "calls": cases.len(), "trace_errors": errors, "reverted_calls": reverted,
        "observed_instruction_pcs": pcs.len(), "disassembled_instruction_pcs": instruction_pcs.len(),
        "observed_blocks": visited_blocks.len(), "engine_blocks": blocks.len(),
        "observed_transitions": observed_edges.len(), "matched_static_edges": matched_edges,
        "unmatched_transitions": observed_edges.len() - matched_edges,
        "unmatched_examples": unmatched,
        "observed_uncertainties": observed_by_reason.values().sum::<usize>(),
        "observed_uncertainties_by_reason": observed_by_reason,
        "selected_count": candidates.len(), "selected": candidates,
        "interpretation": "chamadas e sequência limitada em cópia local do bloco; PCs restantes podem ser alcançáveis em outros estados; transições não mapeadas indicam lacuna do mapa estático, não falha do contrato"
    }))
}
