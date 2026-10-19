//! Reconcile published version events with bounded block snapshots.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub fn reconcile_wcop_versions(history: &Value, raw: &Value, current_hex: &str) -> Result<Value, String> {
    let address = history["address"].as_str().ok_or("address missing")?;
    let block = history["through_block"].as_u64().ok_or("block missing")?;
    if raw["format"] != 1 || raw["chain_id"] != 1 || raw["address"].as_str() != Some(address)
        || raw["through_block"].as_u64() != Some(block)
        || history["complete_event_range"] != true {
        return Err("timeline and readings differ".into());
    }
    let current_hash = hex::encode(Sha256::digest(
        hex::decode(current_hex.trim().trim_start_matches("0x")).map_err(|e| e.to_string())?));
    let snapshots = raw["snapshots"].as_array().ok_or("snapshots missing")?;
    if snapshots.is_empty() || snapshots.len() > 34 { return Err("snapshot limit".into()); }
    let mut by_block = HashMap::new();
    let mut previous = 0;
    for row in snapshots {
        let n = row["block"].as_u64().ok_or("snapshot block missing")?;
        if n <= previous || n > block || row["block_hash"].as_str().is_none_or(|v| v.len() != 66)
            || (row["available"] == true && row["code_sha256"].as_str().is_none_or(|v| v.len() != 64)) {
            return Err("snapshot order or identity invalid".into());
        }
        by_block.insert(n, row);
        previous = n;
    }
    if previous != block || snapshots.last().unwrap()["code_sha256"].as_str() != Some(current_hash.as_str()) {
        return Err("last version differs from pinned code".into());
    }
    let events = raw["upgrade_events"].as_array().ok_or("version events missing")?;
    if events.len() > 16 { return Err("version event limit".into()); }
    let history_events: Vec<_> = history["timeline"].as_array().ok_or("history missing")?
        .iter().filter(|row| row["kind"] == "upgraded").collect();
    if history_events.len() != events.len() { return Err("event count differs".into()); }
    let last_event_address = events.last().and_then(|row| row["implementation"].as_str());
    let current_address = snapshots.last().unwrap()["implementation"].as_str();
    let current_slot_matches_last_event = last_event_address.is_none_or(|value| Some(value) == current_address);
    let mut changes = Vec::new();
    for (event, known) in events.iter().zip(history_events.iter()) {
        if event["block"] != known["block"] || event["tx"] != known["tx"]
            || event["log_index"] != known["log_index"]
            || event["implementation"] != known["implementation"] {
            return Err("version event differs from history".into());
        }
        let n = event["block"].as_u64().ok_or("event block invalid")?;
        let after = by_block.get(&n).ok_or("version block not read")?;
        let declared = event["implementation"].as_str().ok_or("version address missing")?;
        let agrees = if after["available"] == true {
            Some(after["implementation"].as_str() == Some(declared))
        } else { None };
        let before = if n > raw["creation_block"].as_u64().unwrap_or(n) {
            by_block.get(&(n - 1)).copied()
        } else { None };
        let before_hash = before.and_then(|row| row["code_sha256"].as_str());
        let after_hash = after["code_sha256"].as_str();
        changes.push(json!({"block": n, "tx": event["tx"], "implementation": declared,
            "slot_matches_event": agrees, "before_code_sha256": before_hash,
            "after_code_sha256": after_hash,
            "code_changed": before_hash.is_some_and(|v| Some(v) != after_hash),
            "before_supply_raw": before.and_then(|row| row["total_supply_raw"].as_str()),
            "after_supply_raw": after["total_supply_raw"],
            "before_roles_seen": before.map(|row| &row["roles_seen_active"]),
            "after_roles_seen": after["roles_seen_active"],
            "role_read_gaps": after["role_reads_incomplete"]}));
    }
    let mismatches = changes.iter().filter(|row| row["slot_matches_event"] == false).count();
    let unverified = changes.iter().filter(|row| row["slot_matches_event"].is_null()).count();
    let gap_count: usize = snapshots.iter().map(|row|
        row["role_reads_incomplete"].as_array().map(|v| v.len()).unwrap_or(0)).sum();
    Ok(json!({"format": 1, "address": address, "through_block": block,
        "current_code_sha256": current_hash, "snapshot_count": snapshots.len(),
        "change_count": changes.len(), "slot_event_mismatches": mismatches,
        "unverified_event_blocks": unverified,
        "current_slot_matches_last_event": current_slot_matches_last_event,
        "role_read_gaps": gap_count, "changes": changes,
        "state": if mismatches > 0 || !current_slot_matches_last_event { "needs_review" }
            else if unverified > 0 { "limited_historical_state" }
            else { "consistent_in_read_blocks" },
        "limit": "block-level snapshots do not prove every intermediate state or account"}))
}
