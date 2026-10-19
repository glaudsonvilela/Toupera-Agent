//! Re-score every saved hotspot with local scenario evidence while preserving
//! an identity-matched, previously completed fixed-block measurement.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn read_json(path: &Path, limit: usize) -> Result<(Vec<u8>, Value), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!("{} exceeds size limit", path.display()));
    }
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    Ok((bytes, value))
}

fn lower_string<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {pointer}"))
}

pub fn reassess_cached_map(base_path: &Path, local_path: &Path) -> Result<Value, String> {
    let (base_bytes, mut base) = read_json(base_path, 5_000_000)?;
    let (local_bytes, local) = read_json(local_path, 2_000_000)?;
    if base.get("schema").and_then(Value::as_str) != Some("forensic-map-v1")
        || local.get("schema").and_then(Value::as_str) != Some("local-scenarios-v1")
    {
        return Err("expected forensic-map-v1 and local-scenarios-v1 reports".into());
    }
    if base
        .pointer("/identity/exact_onchain_runtime")
        .and_then(Value::as_bool)
        != Some(true)
        || base
            .pointer("/event_history/complete_from_deployment")
            .and_then(Value::as_bool)
            != Some(true)
        || base
            .pointer("/asset_flow_history/complete_from_deployment")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err("base report lacks completed identity or history evidence".into());
    }
    if base
        .get("local_scenario_evidence")
        .is_some_and(|value| !value.is_null())
    {
        return Err("base report already contains local scenario evidence".into());
    }
    let base_sha = hex::encode(Sha256::digest(&base_bytes));
    if lower_string(&local, "/identity/base_report_sha256")? != base_sha {
        return Err("local scenarios were not generated from this exact base report".into());
    }
    for (base_pointer, local_pointer) in [
        ("/identity/chain_id", "/identity/chain_id"),
        ("/identity/block_number", "/identity/block_number"),
    ] {
        if base.pointer(base_pointer) != local.pointer(local_pointer) {
            return Err(format!("identity mismatch at {base_pointer}"));
        }
    }
    for (base_pointer, local_pointer) in [
        ("/identity/address", "/identity/vault"),
        ("/identity/block_hash", "/identity/block_hash"),
        ("/identity/runtime_sha256", "/identity/runtime_sha256"),
    ] {
        if !lower_string(&base, base_pointer)?
            .eq_ignore_ascii_case(lower_string(&local, local_pointer)?)
        {
            return Err(format!("identity mismatch at {base_pointer}"));
        }
    }
    if local.pointer("/identity/asset") != base.pointer("/asset_flow_history/asset_address") {
        return Err("local scenario asset differs from base history".into());
    }
    let cases = local
        .get("cases")
        .and_then(Value::as_array)
        .ok_or("local scenario cases missing")?;
    if cases.len() > 128 {
        return Err("too many local scenario cases".into());
    }
    let mut added = 0usize;
    let mut limitations = Vec::new();
    let quarantine = base
        .get_mut("quarantine_cases")
        .and_then(Value::as_array_mut)
        .ok_or("base quarantine cases missing")?;
    for item in cases {
        if item.get("status").and_then(Value::as_str) != Some("completed") {
            continue;
        }
        let Some(operation) = item.get("operation").and_then(Value::as_str) else {
            continue;
        };
        if !["deposit", "mint", "withdraw", "redeem"].contains(&operation) {
            continue;
        }
        let Some(discrepancies) = item.get("discrepancies").and_then(Value::as_array) else {
            continue;
        };
        if discrepancies.is_empty() {
            continue;
        }
        let matched = quarantine.iter_mut().find(|case| {
            case.get("originating_function")
                .and_then(Value::as_str)
                .and_then(|value| value.split("::").last())
                .is_some_and(|value| value.starts_with(&format!("{operation}(")))
        });
        let Some(matched) = matched else {
            limitations.push(format!("no quarantine case maps {operation}"));
            continue;
        };
        let flags = matched
            .get_mut("review_flags")
            .and_then(Value::as_array_mut)
            .ok_or("base review_flags missing")?;
        for discrepancy in discrepancies.iter().take(8) {
            if let Some(text) = discrepancy.as_str().filter(|text| text.len() <= 500) {
                flags.push(Value::String(format!(
                    "local fork {operation} amount {}: {text}",
                    item.get("amount").and_then(Value::as_str).unwrap_or("?")
                )));
                added += 1;
            }
        }
        if matched
            .get("anomalies")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            matched["status"] = json!("needs_deep_review");
        }
    }
    if let Some(round_trip) = local
        .get("round_trip_discrepancies")
        .and_then(Value::as_array)
    {
        for discrepancy in round_trip.iter().take(8) {
            let Some(text) = discrepancy.as_str().filter(|text| text.len() <= 500) else {
                continue;
            };
            for operation in ["deposit", "redeem"] {
                if let Some(matched) = quarantine.iter_mut().find(|case| {
                    case.get("originating_function")
                        .and_then(Value::as_str)
                        .and_then(|value| value.split("::").last())
                        .is_some_and(|value| value.starts_with(&format!("{operation}(")))
                }) {
                    matched["review_flags"]
                        .as_array_mut()
                        .ok_or("base review_flags missing")?
                        .push(json!(format!("local round trip: {text}")));
                    if matched["anomalies"].as_array().is_some_and(Vec::is_empty) {
                        matched["status"] = json!("needs_deep_review");
                    }
                    added += 1;
                }
            }
        }
    }
    let number =
        |field: &str| -> usize { local.get(field).and_then(Value::as_u64).unwrap_or(0) as usize };
    if let Some(items) = local.get("limitations").and_then(Value::as_array) {
        limitations.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .take(8)
                .map(str::to_owned),
        );
    }
    if local
        .get("trace_limitations")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        limitations.push("at least one local trace is unavailable".into());
    }
    base["local_scenario_evidence"] = json!({
        "report_sha256":hex::encode(Sha256::digest(&local_bytes)),
        "completed_transactions":number("completed_transactions"),
        "reverted_transactions":number("reverted_transactions"),
        "rpc_failures":number("rpc_failures"),
        "unique_trace_jump_sites":number("unique_trace_jump_sites"),
        "unique_branch_outcomes":number("unique_branch_outcomes"),
        "jump_sites_with_both_outcomes":number("jump_sites_with_both_outcomes"),
        "review_flags_added":added,
        "limitations":limitations,
    });
    base["coverage"]["local_scenario_completed"] = json!(number("completed_transactions"));
    base["coverage"]["local_scenario_reverted"] = json!(number("reverted_transactions"));
    base["coverage"]["local_scenario_jump_sites"] = json!(number("unique_trace_jump_sites"));
    base["coverage"]["local_scenario_branch_outcomes"] = json!(number("unique_branch_outcomes"));
    let previous_reassessment = base
        .get("reassessment")
        .and_then(Value::as_array)
        .ok_or("base reassessment missing")?
        .clone();
    let quarantine = base["quarantine_cases"]
        .as_array()
        .ok_or("base quarantine cases missing")?
        .clone();
    let hotspots = base
        .get_mut("hotspots")
        .and_then(Value::as_array_mut)
        .ok_or("base hotspots missing")?;
    let mut reassessment = Vec::new();
    for hotspot in hotspots.iter_mut() {
        let name = format!(
            "{}::{}",
            hotspot
                .get("declaring_contract")
                .and_then(Value::as_str)
                .unwrap_or(""),
            hotspot
                .get("function")
                .and_then(Value::as_str)
                .unwrap_or("")
        );
        let Some(case) = quarantine
            .iter()
            .find(|case| case.get("originating_function").and_then(Value::as_str) == Some(&name))
        else {
            continue;
        };
        let baseline = previous_reassessment
            .iter()
            .find(|item| item.get("function").and_then(Value::as_str) == Some(&name))
            .and_then(|item| item.get("original_score").and_then(Value::as_u64))
            .or_else(|| hotspot.get("priority_score").and_then(Value::as_u64))
            .unwrap_or(0);
        let anomalies = case["anomalies"]
            .as_array()
            .is_some_and(|items| !items.is_empty());
        let review = case["review_flags"]
            .as_array()
            .is_some_and(|items| !items.is_empty());
        let added = if anomalies {
            20
        } else if review {
            5
        } else {
            0
        };
        let revised = (baseline + added).min(100);
        hotspot["priority_score"] = json!(revised);
        hotspot["priority_band"] = json!(if revised >= 40 {
            "high"
        } else if revised >= 25 {
            "medium"
        } else {
            "low"
        });
        if let Some(reasons) = hotspot.get_mut("reasons").and_then(Value::as_array_mut) {
            reasons.retain(|reason| {
                !reason
                    .get("feature")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name.starts_with("quarantine_"))
            });
            if added > 0 {
                reasons.push(json!({"feature":if anomalies {"quarantine_observed_anomaly"} else {"quarantine_receipt_question"},"points":added,"evidence":"local scenario or historical quarantine evidence requires review"}));
            }
        }
        reassessment.push(json!({
            "pass":3,
            "function":name,
            "original_score":baseline,
            "revised_score":revised,
            "quarantine_case_id":case.get("case_id").and_then(Value::as_str).unwrap_or(""),
            "change_reason":if anomalies {"observed anomaly promoted this path"} else if review {"review flags raised investigation priority; not a confirmed anomaly"} else {"no new discrepancy; priority retained, no claim of safety"},
        }));
    }
    hotspots.sort_by(|left, right| {
        right["priority_score"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&left["priority_score"].as_u64().unwrap_or(0))
            .then_with(|| {
                left["declaring_contract"]
                    .as_str()
                    .cmp(&right["declaring_contract"].as_str())
            })
            .then_with(|| left["function"].as_str().cmp(&right["function"].as_str()))
    });
    base["reassessment"] = Value::Array(reassessment);
    base["cached_reassessment"] = json!({
        "base_report_sha256":base_sha,
        "source_measurements_reused":true,
        "fixed_block_measurements_reused":true,
        "live_rpc_calls":0,
        "scope":"all saved hotspots re-scored from identity-matched local scenarios; source and chain measurements are reused",
    });
    Ok(base)
}
