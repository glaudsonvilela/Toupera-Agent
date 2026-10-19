//! Evidence-linked sieve for residual wCOP runtime guards.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const REPLAYS: [&str; 4] = [
    "baseline-branch-replay.json",
    "branch-replay.json",
    "targeted-guards.json",
    "residual-guards.json",
];

fn read(job: &Path, name: &str, limit: u64) -> Result<Value, String> {
    let path = job.join(name);
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("{name}: {e}"))?;
    if !meta.file_type().is_file() || meta.len() > limit {
        return Err(format!("{name}: file outside limit"));
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{name}: {e}"))
}

fn digest(job: &Path, name: &str) -> Result<String, String> {
    let bytes = fs::read(job.join(name)).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn group(file: &str) -> (&'static str, u64, &'static str, &'static str) {
    if file.ends_with("/ECDSA.sol") {
        (
            "ecdsa_permit",
            55,
            "proxy_current",
            "Correlate each remaining PC with the v,r,s overload; InvalidSignatureLength is not returned by this entry path.",
        )
    } else if file.ends_with("/UUPSUpgradeable.sol") {
        (
            "uups_context",
            50,
            "proxy_vs_direct",
            "Keep direct-implementation and proxy delegatecall outcomes separate; inspect only a new repeatable slot or authorization violation.",
        )
    } else if file.ends_with("/ERC1967Utils.sol") {
        (
            "implementation_code",
            45,
            "proxy_current",
            "The zero-code branch follows UUPS validation; establish a concrete reachable code-state transition before replay.",
        )
    } else if file.ends_with("/Address.sol") {
        (
            "delegatecall_result",
            45,
            "proxy_current",
            "Check return-data bubbling and post-call target code on a bounded local mock; require a storage or authorization consequence.",
        )
    } else if file.ends_with("/Initializable.sol") {
        (
            "initialization_state",
            30,
            "fresh_state_only",
            "Measure a fresh local proxy separately; the pinned proxy is already initialized and direct implementation initialization reverted.",
        )
    } else if file.ends_with("/EIP712Upgradeable.sol") {
        (
            "eip712_storage",
            25,
            "other_state_only",
            "Separate empty-name/version and legacy-hash storage from the deployed nonempty domain; do not merge coverage.",
        )
    } else if file.ends_with("/ERC20Upgradeable.sol") {
        (
            "name_storage",
            20,
            "other_state_only",
            "The current short name follows the measured string-storage path; test long or empty strings only in prepared local state.",
        )
    } else {
        (
            "other_runtime",
            10,
            "unclassified",
            "Read the exact compiler location and precondition before executing a new case.",
        )
    }
}

pub fn sieve_wcop_forensic(job: &Path) -> Result<Value, String> {
    let engine = read(job, "engine-impl.json", 12_000_000)?;
    let map = read(job, "path-map.json", 8_000_000)?;
    let history = read(job, "history.json", 5_000_000)?;
    let classification = read(job, "residual-classification.json", 2_000_000)?;
    let historical = read(job, "historical-reconciliation.json", 2_000_000)?;
    let invariants = read(job, "invariant-sequences.json", 2_000_000)?;
    let clinical = read(job, "clinical-guards.json", 2_000_000)?;
    let code = hex::decode(
        engine["normalized_hex"]
            .as_str()
            .ok_or("runtime missing")?
            .trim_start_matches("0x"),
    )
    .map_err(|e| e.to_string())?;
    let code_hash = format!("{:x}", Sha256::digest(code));
    let address = history["address"].as_str().ok_or("address missing")?;
    let block = history["through_block"].as_u64().ok_or("block missing")?;
    let job_id = job
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or("job ID missing")?;
    if job_id.len() != 16 || !job_id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("job ID invalid".into());
    }
    for (name, value) in [
        (&"path-map.json", &map),
        (&"residual-classification.json", &classification),
        (&"historical-reconciliation.json", &historical),
        (&"invariant-sequences.json", &invariants),
        (&"clinical-guards.json", &clinical),
    ] {
        if value["engine_code_sha256"].as_str() != Some(code_hash.as_str())
            || value["block"].as_u64() != Some(block)
        {
            return Err(format!("{name}: runtime or block mismatch"));
        }
    }
    if classification["job_id"].as_str() != Some(job_id) {
        return Err("classification job mismatch".into());
    }
    for value in [&historical, &invariants, &clinical] {
        if value["job_id"].as_str() != Some(job_id) || value["contract"].as_str() != Some(address) {
            return Err("report job or contract mismatch".into());
        }
    }
    if historical["history_sha256"].as_str() != Some(digest(job, "history.json")?.as_str()) {
        return Err("historical report uses another history file".into());
    }
    let history_rows = historical["transactions"]
        .as_array()
        .ok_or("historical transactions missing")?;
    let matched: u64 = history_rows
        .iter()
        .map(|x| x["events_matched"].as_u64().unwrap_or(0))
        .sum();
    if historical["transaction_count"].as_u64() != Some(history_rows.len() as u64)
        || historical["matched_significant_events"].as_u64() != Some(matched)
        || matched == 0
        || history_rows.iter().any(|x| x["status"] != "ok")
    {
        return Err("historical event reconciliation incomplete".into());
    }
    let invariant_rows = invariants["cases"]
        .as_array()
        .ok_or("invariant cases missing")?;
    if invariants["all_snapshots_restored"] != true
        || invariants["scenarios"].as_u64() != Some(invariant_rows.len() as u64)
        || invariant_rows.is_empty()
        || invariant_rows
            .iter()
            .any(|x| x["checks"].as_array().is_none_or(|v| v.is_empty()))
    {
        return Err("invariant run incomplete".into());
    }
    let clinical_rows = clinical["cases"]
        .as_array()
        .ok_or("clinical cases missing")?;
    if clinical["cases_count"].as_u64() != Some(clinical_rows.len() as u64)
        || clinical_rows.is_empty()
        || clinical_rows
            .iter()
            .any(|x| x["expected_error"].as_str().is_some() && x["failed"] != true)
    {
        return Err("clinical run incomplete".into());
    }
    let mut source = BTreeMap::<u64, Value>::new();
    for row in map["pc_locations"]
        .as_array()
        .ok_or("source locations missing")?
    {
        let pc = row["pc"].as_u64().ok_or("source PC invalid")?;
        source.insert(pc, row.clone());
    }
    let mut static_jumpi = BTreeSet::<u64>::new();
    for row in engine["instructions"]
        .as_array()
        .ok_or("instructions missing")?
    {
        if row["mnemonic"] == "JUMPI" {
            static_jumpi.insert(row["pc"].as_u64().ok_or("JUMPI PC invalid")?);
        }
    }
    let mut observed = BTreeSet::<u64>::new();
    let mut input_hashes = BTreeMap::<String, String>::new();
    for name in REPLAYS {
        let value = read(job, name, 3_000_000)?;
        if value["engine_code_sha256"].as_str() != Some(code_hash.as_str())
            || value["block"].as_u64() != Some(block)
            || value["job_id"].as_str() != Some(job_id)
        {
            return Err(format!("{name}: replay identity mismatch"));
        }
        let recorded = classification["input_sha256"][name]
            .as_str()
            .ok_or_else(|| format!("{name}: classification digest missing"))?;
        let actual = digest(job, name)?;
        if recorded != actual {
            return Err(format!("{name}: classification uses another replay"));
        }
        input_hashes.insert(name.to_owned(), actual);
        for case in value["cases"].as_array().ok_or("replay cases missing")? {
            for jump in case["jumps"].as_array().ok_or("case jumps missing")? {
                observed.insert(jump["pc"].as_u64().ok_or("observed PC invalid")?);
            }
        }
    }
    if !observed.is_subset(&static_jumpi) {
        return Err("observed non-JUMPI PC".into());
    }
    let rows = classification["rows"]
        .as_array()
        .ok_or("classification rows missing")?;
    let mut counts = BTreeMap::<String, u64>::new();
    let mut grouped = BTreeMap::<String, (u64, &'static str, &'static str, Vec<Value>)>::new();
    for row in rows {
        let pc = row["pc"].as_u64().ok_or("classification PC invalid")?;
        if !observed.contains(&pc) {
            return Err("one-sided PC was never observed".into());
        }
        let category = row["category"].as_str().ok_or("category missing")?;
        *counts.entry(category.to_owned()).or_default() += 1;
        if category == "runtime_guard" {
            let location = row["source"].as_object().ok_or("runtime source missing")?;
            let file = location
                .get("file")
                .and_then(Value::as_str)
                .ok_or("runtime file missing")?;
            let (id, score, scope, next) = group(file);
            let group_row =
                grouped
                    .entry(id.to_owned())
                    .or_insert((score, scope, next, Vec::new()));
            group_row
                .3
                .push(json!({"pc": pc, "side": row["observed_side"],
                                    "file": file, "line": location.get("line")}));
        }
    }
    let declared = classification["one_side_categories"]
        .as_object()
        .ok_or("category counts missing")?;
    if classification["one_side_pcs"].as_u64() != Some(rows.len() as u64)
        || counts
            .iter()
            .any(|(name, count)| declared.get(name).and_then(Value::as_u64) != Some(*count))
        || counts.len() != declared.len()
        || classification["observed_jumpi_pcs"].as_u64() != Some(observed.len() as u64)
        || classification["static_jumpi_pcs"].as_u64() != Some(static_jumpi.len() as u64)
    {
        return Err("classification counts differ from cases".into());
    }
    let unseen: Vec<Value> = static_jumpi
        .difference(&observed)
        .map(|pc| json!({"pc": pc, "source": source.get(pc)}))
        .collect();
    if classification["unseen_static_jumpi_pcs"].as_u64() != Some(unseen.len() as u64) {
        return Err("unseen PC count differs".into());
    }
    let mut ranked: Vec<Value> = grouped
        .into_iter()
        .map(|(id, (score, scope, next, pcs))| {
            json!({"id": id, "score": score, "state_scope": scope, "pc_count": pcs.len(),
               "pcs": pcs, "next_check": next})
        })
        .collect();
    ranked.sort_by(|a, b| {
        b["score"]
            .as_u64()
            .cmp(&a["score"].as_u64())
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    let mut proof_hashes = input_hashes;
    for name in [
        "historical-reconciliation.json",
        "invariant-sequences.json",
        "clinical-guards.json",
        "residual-classification.json",
    ] {
        proof_hashes.insert(name.to_owned(), digest(job, name)?);
    }
    Ok(
        json!({"format": 1, "job_id": job_id, "contract": address, "block": block,
        "engine_code_sha256": code_hash, "state": "hold_no_confirmed_violation",
        "candidate_count": 0, "ready_for_report": false,
        "historical_receipt_transactions": history_rows.len(),
        "historical_events_matched": matched,
        "historical_storage_status": historical["historical_storage_status"],
        "local_invariant_scenarios": invariant_rows.len(),
        "static_jumpi_pcs": static_jumpi.len(), "observed_jumpi_pcs": observed.len(),
        "one_side_categories": counts, "runtime_guard_groups": ranked,
        "unseen_static_jumpi": unseen, "input_sha256": proof_hashes,
        "decision_rule": "A coverage gap or expected revert is not a candidate. Require a repeatable state or authorization violation with impact before reporting.",
        "limits": ["Historical transaction logs do not prove past storage values.",
                   "Fresh-state scenarios must remain separate from the pinned deployed proxy.",
                   "This sieve makes no external request and submits no transaction."]}),
    )
}
