//! A small final sieve for what may deserve a compact external context.
use std::fs;
use std::path::Path;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn read(job: &Path, name: &str, max: u64) -> Result<Value, String> {
    let path = job.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !metadata.file_type().is_file() || metadata.len() > max {
        return Err(format!("{name}: input outside limit"));
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

pub fn sieve_wcop_context(job: &Path) -> Result<Value, String> {
    let decision = read(job, "decision.json", 2_000_000)?;
    let adaptive = read(job, "adaptive.json", 2_000_000)?;
    let focus = read(job, "focus-final.json", 5_000_000)?;
    let versions = read(job, "versions.json", 2_000_000)?;
    let address = decision["address"].as_str().ok_or("address missing")?;
    let block = decision["block_number"].as_u64().ok_or("block missing")?;
    if adaptive["contract"].as_str() != Some(address) || focus["contract"].as_str() != Some(address)
        || versions["address"].as_str() != Some(address)
        || adaptive["block"].as_u64() != Some(block) || focus["block"].as_u64() != Some(block)
        || versions["through_block"].as_u64() != Some(block) {
        return Err("results of different blocks".into());
    }
    let claims = decision["claims"].as_array().ok_or("decision claims missing")?;
    let required = ["source_map", "focus", "adaptive"];
    let versions_ok = claims.iter().any(|row| row["id"] == "versions"
        && (row["state"] == "OBSERVADO" || row["state"] == "LIMITADO"));
    let linked = decision["identity_ok"] == true && versions_ok
        && ["exact", "masked_immutables"].contains(&decision["source_binding"].as_str().unwrap_or(""))
        && required.iter().all(|id| claims.iter().any(|row|
            row["id"].as_str() == Some(id) && row["state"] == "OBSERVADO"));
    let mut candidates = Vec::new();
    for row in adaptive["deviations"].as_array().ok_or("local differences missing")? {
        let checks = row["checks"].as_array().ok_or("checks missing")?;
        if row["repeatable_locally"] != true
            || checks.is_empty()
            || checks.iter().any(|value| value.as_str().unwrap_or("").starts_with("local_execution_inconclusive")) {
            continue;
        }
        if candidates.len() == 8 { break; }
        candidates.push(json!({"candidate_id": row["candidate_id"], "checks": checks,
            "basis": "repeated only in the local copy; human review remains required"}));
    }
    let state = if !linked { "hold_identity" }
        else if !candidates.is_empty() { "ready_for_context" }
        else if adaptive["new_pcs"].as_u64().unwrap_or(0) > 0
            || focus["open_links"].as_u64().unwrap_or(0) > 0 { "local_review" }
        else { "hold_no_new_signal" };
    let decision_hash = format!("{:x}", Sha256::digest(
        fs::read(job.join("decision.json")).map_err(|e| e.to_string())?));
    Ok(json!({"format": 1, "address": address, "block": block,
        "decision_sha256": decision_hash, "state": state,
        "candidate_count": candidates.len(), "candidates": candidates,
        "external_context_allowed": state == "ready_for_context",
        "rule": "only a repeated local difference with linked identity may reach a compact context",
        "limit": "no external request is made by this step; a local result needs review"}))
}
