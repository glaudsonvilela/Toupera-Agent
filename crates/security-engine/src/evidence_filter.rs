//! Deterministic, local evidence triage before an external review request.
//! Scores direct the next bounded scenario round; scores never promote a finding.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::llm_review::prepare_review_plan;

const SCHEMA: &str = "evidence-filter-v1";

#[derive(Serialize)]
pub struct FocusVariable {
    pub ast_id: i64,
    pub name: String,
    pub scope: String,
    pub score: i64,
    pub distance_to_economic_use: usize,
    pub reasons: Vec<String>,
}

#[derive(Serialize)]
pub struct FilterReport {
    pub schema: String,
    pub context_sha256: String,
    pub forensic_sha256: String,
    pub lens_sha256: String,
    pub scenarios_sha256: String,
    pub coverage_sha256: String,
    pub identity: Value,
    pub status: String,
    pub variable_focus: Vec<FocusVariable>,
    pub one_sided_source_pcs: Vec<u64>,
    pub next_scenario_families: Vec<String>,
    pub observed_discrepancy_cases: usize,
    pub reproducible_discrepancies: Vec<String>,
    pub eligible_candidate_keys: Vec<String>,
    pub limitations: Vec<String>,
}

fn read(path: &Path, label: &str) -> Result<(Vec<u8>, Value), String> {
    let bytes = fs::read(path).map_err(|error| format!("cannot read {label}: {error}"))?;
    let value =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid {label} JSON: {error}"))?;
    Ok((bytes, value))
}

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn required<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("missing {pointer}"))
}

fn same_identity(
    base: &Value,
    other: &Value,
    pointer: &str,
    other_key: &str,
) -> Result<(), String> {
    let expected = required(base, pointer)?;
    if other.get(other_key).and_then(Value::as_str) != Some(expected) {
        return Err(format!("identity mismatch at {pointer}"));
    }
    Ok(())
}

/// Consumes only fixed, already-recorded fork results. No RPC or model call occurs here.
pub fn filter_evidence(
    forensic_path: &Path,
    lens_path: &Path,
    scenarios_path: &Path,
    coverage_path: &Path,
    context_path: &Path,
) -> Result<FilterReport, String> {
    let (forensic_bytes, forensic) = read(forensic_path, "forensic report")?;
    let (lens_bytes, lens) = read(lens_path, "variable lens")?;
    let (scenario_bytes, scenarios) = read(scenarios_path, "scenario report")?;
    let (coverage_bytes, coverage) = read(coverage_path, "coverage report")?;
    let (context_bytes, context) = read(context_path, "context package")?;
    let forensic_hash = hash(&forensic_bytes);
    let lens_hash = hash(&lens_bytes);
    let scenarios_hash = hash(&scenario_bytes);
    let coverage_hash = hash(&coverage_bytes);
    let package = context.get("package").unwrap_or(&context);
    if forensic["schema"] != "forensic-map-v1"
        || lens["schema"] != "variable-lens-v1"
        || scenarios["schema"] != "local-scenarios-v1"
        || coverage["schema"] != "local-coverage-v1"
        || package["schema"] != "m5-context-v1"
    {
        return Err("unsupported evidence schema".into());
    }
    if forensic["identity"]["exact_onchain_runtime"] != true
        || lens["forensic_report_sha256"] != forensic_hash
        || scenarios["identity"]["base_report_sha256"] != forensic_hash
        || coverage["source_report_sha256"] != scenarios_hash
        || coverage["identity"]["base_report_sha256"] != forensic_hash
        || lens["local_coverage_sha256"] != coverage_hash
        || package["digests"]["forensic_json_sha256"] != forensic_hash
    {
        return Err("evidence hashes or runtime verification differ".into());
    }
    same_identity(
        &forensic,
        &lens["identity"],
        "/identity/runtime_sha256",
        "runtime_sha256",
    )?;
    same_identity(
        &forensic,
        &scenarios["identity"],
        "/identity/runtime_sha256",
        "runtime_sha256",
    )?;
    same_identity(
        &forensic,
        &coverage["identity"],
        "/identity/runtime_sha256",
        "runtime_sha256",
    )?;
    same_identity(
        &forensic,
        &lens["identity"],
        "/identity/block_hash",
        "block_hash",
    )?;
    same_identity(
        &forensic,
        &scenarios["identity"],
        "/identity/block_hash",
        "block_hash",
    )?;
    same_identity(
        &forensic,
        &coverage["identity"],
        "/identity/block_hash",
        "block_hash",
    )?;
    same_identity(&forensic, &lens["identity"], "/identity/address", "address")?;
    same_identity(
        &forensic,
        &scenarios["identity"],
        "/identity/address",
        "vault",
    )?;
    same_identity(
        &forensic,
        &coverage["identity"],
        "/identity/address",
        "vault",
    )?;
    for other in [
        &lens["identity"],
        &scenarios["identity"],
        &coverage["identity"],
    ] {
        if other["chain_id"] != forensic["identity"]["chain_id"]
            || other["block_number"] != forensic["identity"]["block_number"]
        {
            return Err("chain or fixed block differs across evidence".into());
        }
    }
    // M5 records normalized runtime (metadata stripped); M4 records exact onchain bytes.
    // The forensic JSON hash above is the binding between those representations.
    if package["digests"]["runtime_sha256"]
        .as_str()
        .filter(|digest| digest.len() == 64)
        .is_none()
    {
        return Err("context has no normalized runtime digest".into());
    }

    let economic_functions = [
        "deposit",
        "mint",
        "withdraw",
        "redeem",
        "totalassets",
        "converttoshares",
        "converttoassets",
        "previewdeposit",
        "previewmint",
        "previewwithdraw",
        "previewredeem",
        "maxdeposit",
        "maxmint",
        "maxwithdraw",
        "maxredeem",
    ];
    let mut incoming: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for edge in lens["edges"].as_array().ok_or("dependency edges missing")? {
        if let (Some(from), Some(to)) = (edge["from_ast_id"].as_i64(), edge["to_ast_id"].as_i64()) {
            incoming.entry(to).or_default().push(from);
        }
    }
    let mut distances = BTreeMap::<i64, usize>::new();
    let mut queue = VecDeque::new();
    for item in lens["variables"]
        .as_array()
        .ok_or("variable list missing")?
    {
        let Some(id) = item["ast_id"].as_i64() else {
            continue;
        };
        let direct = item["uses"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|use_site| {
                use_site["function"].as_str().is_some_and(|name| {
                    economic_functions.contains(&name.to_ascii_lowercase().as_str())
                })
            });
        if direct {
            distances.insert(id, 0);
            queue.push_back(id);
        }
    }
    for link in lens["state_transition_links"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let relevant = ["writer_function", "reader_function"].iter().any(|field| {
            link[*field].as_str().is_some_and(|name| {
                economic_functions.contains(&name.to_ascii_lowercase().as_str())
            })
        });
        if relevant {
            if let Some(id) = link["state_variable_ast_id"].as_i64() {
                if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(id) {
                    entry.insert(0);
                    queue.push_back(id);
                }
            }
        }
    }
    while let Some(id) = queue.pop_front() {
        let distance = distances[&id];
        if distance >= 4 {
            continue;
        }
        for predecessor in incoming.get(&id).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(*predecessor)
            {
                entry.insert(distance + 1);
                queue.push_back(*predecessor);
            }
        }
    }
    let mut focus = Vec::new();
    for item in lens["variables"]
        .as_array()
        .ok_or("variable list missing")?
    {
        let Some(ast_id) = item["ast_id"].as_i64() else {
            continue;
        };
        let Some(distance) = distances.get(&ast_id).copied() else {
            continue;
        };
        let score = item["priority_score"].as_i64().unwrap_or(0) + 24_i64 / (distance as i64 + 1);
        if score <= 0 {
            continue;
        }
        let mut reasons = item["priority_reasons"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .take(8)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if item["category"] == "persistent_state" {
            reasons.push("state_declaration".into());
        }
        reasons.push(format!("upstream_hops_to_economic_use:{distance}"));
        focus.push(FocusVariable {
            ast_id,
            name: item["name"].as_str().unwrap_or("").to_owned(),
            scope: item["scope"].as_str().unwrap_or("").to_owned(),
            score,
            distance_to_economic_use: distance,
            reasons,
        });
    }
    focus.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.ast_id.cmp(&b.ast_id)));
    focus.truncate(24);

    let mut one_sided = BTreeSet::new();
    for origin in lens["source_coverage"]["pc_source_origins"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if origin["observed_outcomes"]
            .as_array()
            .is_some_and(|outcomes| outcomes.len() < 2)
        {
            if let Some(pc) = origin["pc"].as_u64() {
                one_sided.insert(pc);
            }
        }
    }
    let mut families = BTreeSet::new();
    if one_sided.len() > 0 {
        families.insert("branch_boundary".to_owned());
    }
    if lens["state_transition_links"]
        .as_array()
        .is_some_and(|items| !items.is_empty())
    {
        families.insert("ordered_state".to_owned());
    }
    if lens["indexed_accesses"]
        .as_array()
        .is_some_and(|items| !items.is_empty())
    {
        families.insert("alternate_actor".to_owned());
    }
    if focus.iter().any(|v| {
        let name = v.name.to_ascii_lowercase();
        name.contains("asset") || name.contains("supply") || name.contains("balance")
    }) {
        families.insert("asset_state_change".to_owned());
    }

    // A single differing case is an observation, never a reproducible result.
    let mut by_behavior: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let mut discrepancy_cases = 0;
    let mut function_names = BTreeSet::new();
    for case in scenarios["cases"]
        .as_array()
        .ok_or("scenario cases missing")?
    {
        if case["status"] != "completed" || case["preflight"] != "call_completed" {
            continue;
        }
        let Some(sequence) = case["sequence"].as_str() else {
            continue;
        };
        let Some(operation) = case["operation"].as_str() else {
            continue;
        };
        let Some(discrepancies) = case["discrepancies"].as_array() else {
            continue;
        };
        if !discrepancies.is_empty() {
            discrepancy_cases += 1;
        }
        for item in discrepancies {
            if let Some(detail) = item.as_str().filter(|s| !s.is_empty() && s.len() <= 500) {
                by_behavior
                    .entry((operation.to_owned(), detail.to_owned()))
                    .or_default()
                    .insert(sequence.to_owned());
            }
        }
    }
    let mut reproducible = Vec::new();
    for ((operation, detail), sequences) in by_behavior {
        if sequences.len() >= 2 {
            function_names.insert(operation.clone());
            reproducible.push(format!("{operation}: {detail}"));
        }
    }
    // Infrastructure uncertainty blocks promotion even if a comparison differs.
    let sound_execution =
        scenarios["rpc_failures"] == 0 && scenarios["preflight_transaction_mismatches"] == 0;
    if !sound_execution {
        reproducible.clear();
        function_names.clear();
    }
    let plan = prepare_review_plan(&context)?;
    let keys = plan
        .requests
        .iter()
        .filter(|request| {
            function_names.iter().any(|name| {
                request.context["candidate"]["function"]
                    .as_str()
                    .is_some_and(|f| f.contains(name))
            })
        })
        .map(|request| request.candidate_key.clone())
        .collect::<Vec<_>>();
    let mut limitations = vec![
        "Priority scores guide local work; they do not establish behavior or completeness".into(),
    ];
    if !sound_execution {
        limitations.push("Execution uncertainty prevents external review promotion".into());
    }
    if reproducible.is_empty() {
        limitations.push("No repeated local discrepancy linked to an M5 candidate".into());
    }
    if !one_sided.is_empty() {
        limitations
            .push("One-sided source PCs are targeting hints, not path reachability proof".into());
    }
    Ok(FilterReport {
        schema: SCHEMA.into(),
        context_sha256: hash(&context_bytes),
        forensic_sha256: forensic_hash,
        lens_sha256: lens_hash,
        scenarios_sha256: scenarios_hash,
        coverage_sha256: coverage_hash,
        identity: forensic["identity"].clone(),
        status: if keys.is_empty() {
            "NO_ELIGIBLE_CANDIDATE"
        } else {
            "REVIEW_ELIGIBLE"
        }
        .into(),
        variable_focus: focus,
        one_sided_source_pcs: one_sided.into_iter().take(32).collect(),
        next_scenario_families: families.into_iter().collect(),
        observed_discrepancy_cases: discrepancy_cases,
        reproducible_discrepancies: reproducible,
        eligible_candidate_keys: keys,
        limitations,
    })
}

/// Validates the exact M5 package used to produce a filter and narrows M6 requests.
pub fn gate_review_plan(
    plan: &mut crate::llm_review::ReviewPlan,
    context_bytes: &[u8],
    filter: &Value,
) -> Result<(), String> {
    if filter["schema"] != SCHEMA || filter["context_sha256"] != hash(context_bytes) {
        return Err("filter does not match the exact M5 package".into());
    }
    let keys = filter["eligible_candidate_keys"]
        .as_array()
        .ok_or("filter lacks eligible candidate keys")?
        .iter()
        .map(|key| key.as_str().ok_or("invalid candidate key"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if (filter["status"] == "REVIEW_ELIGIBLE") != !keys.is_empty() {
        return Err("filter status disagrees with eligible keys".into());
    }
    let before = plan.requests.len();
    plan.requests
        .retain(|request| keys.contains(request.candidate_key.as_str()));
    if plan.requests.len() != keys.len() {
        return Err("filter refers to an unknown or duplicate candidate".into());
    }
    plan.eligible_candidates = plan.requests.len();
    plan.skipped_candidates += before - plan.requests.len();
    Ok(())
}
