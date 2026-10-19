//! Bounded source-condition and observed-state planner for the next local round.
//! Source maps suggest candidates; only a local execution can observe behavior.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::Instant;

use alloy_primitives::U256;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::source::compile_source_ast_file;
use crate::{BytecodeValueSinkKind, ForensicInput};

#[derive(Clone, Debug, Serialize)]
pub struct ConditionProbe {
    pub pc: usize,
    pub source_file: String,
    pub function: String,
    pub operation: Option<String>,
    pub variable: String,
    pub variable_role: String,
    pub comparison: String,
    pub literal: String,
    pub value_when_true: Option<String>,
    pub value_when_false: Option<String>,
    pub observed_outcomes: Vec<String>,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BranchInfluence {
    pub pc: usize,
    pub origin_kinds: Vec<String>,
    pub origin_pcs: Vec<usize>,
    pub arithmetic_pcs: Vec<usize>,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StatePlan {
    pub setup_operation: String,
    pub target_operation: String,
    pub setup_amount: String,
    pub target_amount: String,
    pub shared_slots: usize,
    pub basis: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NumericRelation {
    pub sequence: String,
    pub operation: String,
    pub amount: String,
    pub field: String,
    pub expected_delta: String,
    pub observed_delta: String,
    pub equal: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PreviewRelation {
    pub operation: String,
    pub lower_amount: String,
    pub lower_preview: String,
    pub upper_amount: String,
    pub upper_preview: String,
    pub nondecreasing: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct AccountingGap {
    pub sequence: String,
    pub operation: String,
    pub before_assets_minus_balance: String,
    pub after_assets_minus_balance: String,
    pub changed: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForensicPlan {
    pub schema: String,
    pub forensic_report_sha256: String,
    pub lens_sha256: String,
    pub coverage_sha256: String,
    pub previous_scenarios_sha256: String,
    pub runtime_sha256: String,
    pub block_hash: String,
    pub fixed_contract: String,
    pub static_jump_sites: usize,
    pub one_sided_or_unseen_sites: usize,
    pub direct_conditions: usize,
    pub source_span_matches: usize,
    pub unresolved_sites: usize,
    pub probes: Vec<ConditionProbe>,
    pub branch_influences: Vec<BranchInfluence>,
    pub state_plans: Vec<StatePlan>,
    pub numeric_relations: Vec<NumericRelation>,
    pub preview_relations: Vec<PreviewRelation>,
    pub preview_reversals: usize,
    pub accounting_gaps: Vec<AccountingGap>,
    pub changed_accounting_gaps: usize,
    pub numeric_differences: usize,
    pub build_elapsed_ms: u128,
    pub next_round_recommended: bool,
    pub next_round_reason: String,
    pub limitations: Vec<String>,
}

#[derive(Clone)]
struct Condition {
    file_id: i64,
    source_file: String,
    start: usize,
    end: usize,
    function: String,
    variable: String,
    variable_role: String,
    operator: String,
    literal: U256,
    first_uint_parameter: bool,
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn read(path: &Path) -> Result<(Vec<u8>, Value), String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid JSON {}: {e}", path.display()))?;
    Ok((bytes, value))
}

fn span(node: &Value) -> Option<(usize, usize, i64)> {
    let mut parts = node["src"].as_str()?.split(':');
    let start = parts.next()?.parse::<usize>().ok()?;
    let length = parts.next()?.parse::<usize>().ok()?;
    let file = parts.next()?.parse::<i64>().ok()?;
    Some((start, start.checked_add(length)?, file))
}

fn children<'a>(node: &'a Value) -> Vec<&'a Value> {
    let mut result = Vec::new();
    if let Some(map) = node.as_object() {
        for value in map.values() {
            if value.get("nodeType").is_some() {
                result.push(value);
            }
            if let Some(items) = value.as_array() {
                result.extend(items.iter().filter(|v| v.get("nodeType").is_some()));
            }
        }
    }
    result
}

fn collect_roles(node: &Value, roles: &mut BTreeMap<i64, String>) {
    if node["nodeType"] == "VariableDeclaration" {
        if let Some(id) = node["id"].as_i64() {
            roles.insert(
                id,
                if node["stateVariable"] == true {
                    "state"
                } else {
                    "local"
                }
                .into(),
            );
        }
    }
    for child in children(node) {
        collect_roles(child, roles);
    }
}

fn collect_conditions(
    node: &Value,
    file: &str,
    function: &str,
    parameters: &BTreeSet<i64>,
    first: Option<i64>,
    roles: &BTreeMap<i64, String>,
    out: &mut Vec<Condition>,
) {
    let mut function = function.to_owned();
    let mut parameters = parameters.clone();
    let mut first = first;
    if node["nodeType"] == "FunctionDefinition" {
        function = node["name"].as_str().unwrap_or("").to_owned();
        parameters.clear();
        first = None;
        if let Some(items) = node["parameters"]["parameters"].as_array() {
            for (index, item) in items.iter().enumerate() {
                if let Some(id) = item["id"].as_i64() {
                    parameters.insert(id);
                    if index == 0
                        && item["typeDescriptions"]["typeString"]
                            .as_str()
                            .is_some_and(|t| t.starts_with("uint"))
                    {
                        first = Some(id);
                    }
                }
            }
        }
    }
    if node["nodeType"] == "BinaryOperation" {
        let mut operator = node["operator"].as_str().unwrap_or("");
        if matches!(operator, "<" | "<=" | ">" | ">=" | "==" | "!=") {
            let left = &node["leftExpression"];
            let right = &node["rightExpression"];
            let (identifier, literal) =
                if left["nodeType"] == "Identifier" && right["nodeType"] == "Literal" {
                    (left, right)
                } else if right["nodeType"] == "Identifier" && left["nodeType"] == "Literal" {
                    operator = match operator {
                        "<" => ">",
                        "<=" => ">=",
                        ">" => "<",
                        ">=" => "<=",
                        other => other,
                    };
                    (right, left)
                } else {
                    (&Value::Null, &Value::Null)
                };
            if literal["kind"] == "number" {
                if let (Some(id), Some(value), Some((start, end, file_id))) = (
                    identifier["referencedDeclaration"].as_i64(),
                    literal["value"]
                        .as_str()
                        .and_then(|s| s.parse::<U256>().ok()),
                    span(node),
                ) {
                    if roles.contains_key(&id) && start < end {
                        out.push(Condition {
                            file_id,
                            source_file: file.into(),
                            start,
                            end,
                            function: function.clone(),
                            variable: identifier["name"].as_str().unwrap_or("").into(),
                            variable_role: if parameters.contains(&id) {
                                "parameter".into()
                            } else {
                                roles.get(&id).cloned().unwrap_or_else(|| "unknown".into())
                            },
                            operator: operator.into(),
                            literal: value,
                            first_uint_parameter: first == Some(id),
                        });
                    }
                }
            }
        }
    }
    for child in children(node) {
        collect_conditions(child, file, &function, &parameters, first, roles, out);
    }
}

fn predicate(operator: &str, value: U256, literal: U256) -> bool {
    match operator {
        "<" => value < literal,
        "<=" => value <= literal,
        ">" => value > literal,
        ">=" => value >= literal,
        "==" => value == literal,
        "!=" => value != literal,
        _ => false,
    }
}

fn values(condition: &Condition) -> (Option<String>, Option<String>) {
    let mut candidates = vec![
        condition.literal.saturating_sub(U256::from(1)),
        condition.literal,
        condition.literal.saturating_add(U256::from(1)),
    ];
    candidates.sort();
    candidates.dedup();
    let true_value = candidates
        .iter()
        .find(|v| predicate(&condition.operator, **v, condition.literal))
        .map(ToString::to_string);
    let false_value = candidates
        .iter()
        .find(|v| !predicate(&condition.operator, **v, condition.literal))
        .map(ToString::to_string);
    (true_value, false_value)
}

fn runtime_pcs(runtime: &[u8]) -> Vec<usize> {
    let mut result = Vec::new();
    let mut pc = 0;
    while pc < runtime.len() {
        result.push(pc);
        let op = runtime[pc];
        pc += 1 + if (0x60..=0x7f).contains(&op) {
            usize::from(op - 0x5f)
        } else {
            0
        };
    }
    result
}

fn source_ranges(
    map: &str,
    runtime: &[u8],
) -> Result<BTreeMap<usize, (usize, usize, i64)>, String> {
    let pcs = runtime_pcs(runtime);
    let mut start = 0i64;
    let mut length = 0i64;
    let mut file = -1i64;
    let mut result = BTreeMap::new();
    for (pc, entry) in pcs.into_iter().zip(map.split(';')) {
        let mut fields = entry.split(':');
        if let Some(s) = fields.next().filter(|s| !s.is_empty()) {
            start = s.parse().map_err(|_| "invalid source map start")?;
        }
        if let Some(s) = fields.next().filter(|s| !s.is_empty()) {
            length = s.parse().map_err(|_| "invalid source map length")?;
        }
        if let Some(s) = fields.next().filter(|s| !s.is_empty()) {
            file = s.parse().map_err(|_| "invalid source map file")?;
        }
        if runtime[pc] == 0x57 && start >= 0 && length > 0 && file >= 0 {
            let begin = start as usize;
            let end = start.checked_add(length).ok_or("source map overflow")? as usize;
            result.insert(pc, (begin, end, file));
        }
    }
    Ok(result)
}

fn signed_delta(before: &str, after: &str) -> Option<String> {
    let a = before.parse::<U256>().ok()?;
    let b = after.parse::<U256>().ok()?;
    Some(if b >= a {
        (b - a).to_string()
    } else {
        format!("-{}", a - b)
    })
}

fn negative(value: U256) -> String {
    if value.is_zero() {
        "0".into()
    } else {
        format!("-{value}")
    }
}

fn numeric_relations(cases: &[Value]) -> Vec<NumericRelation> {
    let mut out = Vec::new();
    for case in cases.iter().filter(|c| c["status"] == "completed") {
        let Some(amount) = case["amount"].as_str().and_then(|s| s.parse::<U256>().ok()) else {
            continue;
        };
        let Some(preview) = case["preview"]
            .as_str()
            .and_then(|s| s.parse::<U256>().ok())
        else {
            continue;
        };
        let Some(before) = case["before"].as_object() else {
            continue;
        };
        let Some(after) = case["after"].as_object() else {
            continue;
        };
        let op = case["operation"].as_str().unwrap_or("");
        let (assets, shares, add) = match op {
            "deposit" => (amount, preview, true),
            "mint" => (preview, amount, true),
            "withdraw" => (amount, preview, false),
            "redeem" => (preview, amount, false),
            _ => continue,
        };
        let expected = [
            (
                "total_assets",
                if add {
                    assets.to_string()
                } else {
                    negative(assets)
                },
            ),
            (
                "total_supply",
                if add {
                    shares.to_string()
                } else {
                    negative(shares)
                },
            ),
            (
                "vault_asset_balance",
                if add {
                    assets.to_string()
                } else {
                    negative(assets)
                },
            ),
            (
                "actor_asset_balance",
                if add {
                    negative(assets)
                } else {
                    assets.to_string()
                },
            ),
            (
                "actor_share_balance",
                if add {
                    shares.to_string()
                } else {
                    negative(shares)
                },
            ),
        ];
        for (field, expected_delta) in expected {
            if let (Some(b), Some(a)) = (
                before.get(field).and_then(Value::as_str),
                after.get(field).and_then(Value::as_str),
            ) {
                if let Some(observed_delta) = signed_delta(b, a) {
                    out.push(NumericRelation {
                        sequence: case["sequence"].as_str().unwrap_or("").into(),
                        operation: op.into(),
                        amount: amount.to_string(),
                        field: field.into(),
                        equal: observed_delta == expected_delta,
                        expected_delta,
                        observed_delta,
                    });
                }
            }
        }
    }
    out
}

fn preview_relations(cases: &[Value]) -> Vec<PreviewRelation> {
    let mut by_op: BTreeMap<String, Vec<(U256, U256)>> = BTreeMap::new();
    for case in cases.iter().filter(|c| {
        c["sequence"]
            .as_str()
            .is_some_and(|s| s.starts_with("isolated:"))
    }) {
        let (Some(op), Some(amount), Some(preview)) = (
            case["operation"].as_str(),
            case["amount"].as_str().and_then(|s| s.parse::<U256>().ok()),
            case["preview"]
                .as_str()
                .and_then(|s| s.parse::<U256>().ok()),
        ) else {
            continue;
        };
        by_op.entry(op.into()).or_default().push((amount, preview));
    }
    let mut result = Vec::new();
    for (op, mut values) in by_op {
        values.sort_by_key(|pair| pair.0);
        values.dedup_by_key(|pair| pair.0);
        for pair in values.windows(2) {
            result.push(PreviewRelation {
                operation: op.clone(),
                lower_amount: pair[0].0.to_string(),
                lower_preview: pair[0].1.to_string(),
                upper_amount: pair[1].0.to_string(),
                upper_preview: pair[1].1.to_string(),
                nondecreasing: pair[0].1 <= pair[1].1,
            });
        }
    }
    result
}

fn accounting_gaps(cases: &[Value]) -> Vec<AccountingGap> {
    let mut result = Vec::new();
    for case in cases.iter().filter(|c| c["status"] == "completed") {
        let gap = |state: &Value| {
            signed_delta(
                state["vault_asset_balance"].as_str()?,
                state["total_assets"].as_str()?,
            )
        };
        let (Some(before), Some(after)) = (gap(&case["before"]), gap(&case["after"])) else {
            continue;
        };
        result.push(AccountingGap {
            sequence: case["sequence"].as_str().unwrap_or("").into(),
            operation: case["operation"].as_str().unwrap_or("").into(),
            changed: before != after,
            before_assets_minus_balance: before,
            after_assets_minus_balance: after,
        });
    }
    result
}

fn state_plans(cases: &[Value]) -> Vec<StatePlan> {
    let mut by_op: BTreeMap<String, Vec<(U256, BTreeSet<String>, BTreeSet<String>)>> =
        BTreeMap::new();
    for case in cases.iter().filter(|c| {
        c["status"] == "completed"
            && c["sequence"]
                .as_str()
                .is_some_and(|s| s.starts_with("isolated:"))
    }) {
        let op = case["operation"].as_str().unwrap_or("");
        if !matches!(op, "deposit" | "mint" | "withdraw" | "redeem") {
            continue;
        }
        let Some(amount) = case["amount"].as_str().and_then(|s| s.parse::<U256>().ok()) else {
            continue;
        };
        if amount.is_zero() || case["trace_steps"].is_null() {
            continue;
        }
        let reads = case["root_storage_reads"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let writes = case["root_storage_writes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        by_op
            .entry(op.into())
            .or_default()
            .push((amount, reads, writes));
    }
    let mut ranked = Vec::new();
    for (setup, setup_cases) in &by_op {
        for (target, target_cases) in &by_op {
            if setup == target {
                continue;
            }
            let mut best = None;
            for (setup_amount, _, writes) in setup_cases {
                for (target_amount, reads, _) in target_cases {
                    let shared = writes.intersection(reads).count();
                    if shared == 0 {
                        continue;
                    }
                    let candidate = (shared, *setup_amount, *target_amount);
                    if best.is_none_or(|old: (usize, U256, U256)| candidate > old) {
                        best = Some(candidate);
                    }
                }
            }
            if let Some((shared, setup_amount, target_amount)) = best {
                ranked.push(StatePlan {
                    setup_operation: setup.clone(),
                    target_operation: target.clone(),
                    setup_amount: setup_amount.to_string(),
                    target_amount: target_amount.to_string(),
                    shared_slots: shared,
                    basis: "root_slot_written_then_read_in_isolated_cases_not_causal_proof".into(),
                });
            }
        }
    }
    ranked.sort_by(|a, b| {
        b.shared_slots
            .cmp(&a.shared_slots)
            .then_with(|| a.setup_operation.cmp(&b.setup_operation))
            .then_with(|| a.target_operation.cmp(&b.target_operation))
    });
    ranked.truncate(8);
    ranked
}

pub fn build_forensic_plan(
    manifest_path: &Path,
    forensic_path: &Path,
    lens_path: &Path,
    scenarios_path: &Path,
    coverage_path: &Path,
) -> Result<ForensicPlan, String> {
    let start = Instant::now();
    let (_, manifest_json) = read(manifest_path)?;
    let manifest: ForensicInput =
        serde_json::from_value(manifest_json).map_err(|e| format!("invalid manifest: {e}"))?;
    let (forensic_bytes, forensic) = read(forensic_path)?;
    let (lens_bytes, lens) = read(lens_path)?;
    let (scenarios_bytes, scenarios) = read(scenarios_path)?;
    let (coverage_bytes, coverage) = read(coverage_path)?;
    let forensic_sha = digest(&forensic_bytes);
    let coverage_sha = digest(&coverage_bytes);
    let scenarios_sha = digest(&scenarios_bytes);
    if forensic["schema"] != "forensic-map-v1"
        || lens["schema"] != "variable-lens-v1"
        || scenarios["schema"] != "local-scenarios-v1"
        || coverage["schema"] != "local-coverage-v1"
        || lens["forensic_report_sha256"] != forensic_sha
        || lens["local_coverage_sha256"] != coverage_sha
        || scenarios["identity"]["base_report_sha256"] != forensic_sha
        || coverage["source_report_sha256"] != scenarios_sha
        || lens["identity"]["runtime_sha256"] != forensic["identity"]["runtime_sha256"]
        || scenarios["identity"]["block_hash"] != forensic["identity"]["block_hash"]
    {
        return Err("plan inputs do not describe the same fixed run".into());
    }
    let (comparison_bytes, comparison) = read(&manifest.comparison_solc_json_path)?;
    let runtime_hex = fs::read_to_string(&manifest.runtime_path)
        .map_err(|e| format!("cannot read runtime: {e}"))?;
    let runtime_hex = runtime_hex.trim().trim_start_matches("0x");
    let runtime = hex::decode(runtime_hex).map_err(|e| format!("invalid runtime hex: {e}"))?;
    if forensic["identity"]["comparison_json_sha256"] != digest(&comparison_bytes)
        || forensic["identity"]["runtime_sha256"] != digest(&runtime)
        || forensic["identity"]["exact_onchain_runtime"] != true
    {
        return Err("plan compiler or runtime identity differs".into());
    }
    let deployed = &comparison["contracts"][&manifest.comparison_source_name]
        [&manifest.contract_name]["evm"]["deployedBytecode"];
    let source_map = deployed["sourceMap"]
        .as_str()
        .ok_or("verified source map missing")?;
    let ranges = source_ranges(source_map, &runtime)?;
    let mut conditions = Vec::new();
    for (file, entry) in comparison["sources"]
        .as_object()
        .ok_or("verified AST sources missing")?
    {
        let mut roles = BTreeMap::new();
        collect_roles(&entry["ast"], &mut roles);
        collect_conditions(
            &entry["ast"],
            file,
            "",
            &BTreeSet::new(),
            None,
            &roles,
            &mut conditions,
        );
    }
    conditions
        .sort_by(|a, b| (&a.source_file, a.start, a.end).cmp(&(&b.source_file, b.start, b.end)));
    let mut probes = Vec::new();
    let mut target_sites = 0;
    let mut matched = 0;
    for origin in lens["source_coverage"]["pc_source_origins"]
        .as_array()
        .ok_or("source origins missing")?
    {
        let Some(pc) = origin["pc"].as_u64().map(|v| v as usize) else {
            continue;
        };
        let outcomes: Vec<String> = origin["observed_outcomes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        if outcomes.len() >= 2 {
            continue;
        }
        target_sites += 1;
        let Some((from, to, file_id)) = ranges.get(&pc).copied() else {
            continue;
        };
        if to.saturating_sub(from) > 128 {
            continue;
        }
        let candidates: Vec<&Condition> = conditions
            .iter()
            .filter(|c| {
                c.file_id == file_id
                    && c.start <= from
                    && to <= c.end
                    && c.function == origin["function"].as_str().unwrap_or("")
            })
            .collect();
        let Some(min_size) = candidates.iter().map(|c| c.end - c.start).min() else {
            continue;
        };
        let shortest: Vec<_> = candidates
            .into_iter()
            .filter(|c| c.end - c.start == min_size)
            .collect();
        let [condition] = shortest.as_slice() else {
            continue;
        };
        matched += 1;
        let (value_when_true, value_when_false) = values(condition);
        let operation = (condition.first_uint_parameter
            && matches!(
                condition.function.as_str(),
                "deposit" | "mint" | "withdraw" | "redeem"
            ))
        .then(|| condition.function.clone());
        probes.push(ConditionProbe {
            pc,
            source_file: condition.source_file.clone(),
            function: condition.function.clone(),
            operation,
            variable: condition.variable.clone(),
            variable_role: condition.variable_role.clone(),
            comparison: condition.operator.clone(),
            literal: condition.literal.to_string(),
            value_when_true,
            value_when_false,
            observed_outcomes: outcomes,
            confidence: "unique_narrow_verified_source_span_polarity_unknown".into(),
        });
    }
    let analysis = crate::reconstruct_bytecode_hex("forensic-plan-runtime", runtime_hex)?;
    let observed_by_pc: BTreeMap<usize, usize> = coverage["branch_observations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item["pc"].as_u64()? as usize,
                item["outcomes"].as_array()?.len(),
            ))
        })
        .collect();
    let mut branch_influences = Vec::new();
    for pc in analysis
        .instructions
        .iter()
        .filter(|item| item.opcode == 0x57)
        .map(|item| item.pc)
    {
        if observed_by_pc.get(&pc).copied().unwrap_or(0) >= 2 {
            continue;
        }
        let mut kinds = BTreeSet::new();
        let mut origins = BTreeSet::new();
        let mut arithmetic = BTreeSet::new();
        let mut uncertain = false;
        for flow in analysis.value_flows.iter().filter(|flow| {
            flow.sink_kind == BytecodeValueSinkKind::BranchCondition && flow.sink_pc.pc == pc
        }) {
            kinds.insert(format!("{:?}", flow.origin.kind));
            origins.insert(flow.origin.location.pc);
            arithmetic.extend(flow.arithmetic_pcs.iter().map(|item| item.pc));
            uncertain |= flow.confidence != crate::Confidence::Direct;
        }
        branch_influences.push(BranchInfluence {
            pc,
            origin_kinds: kinds.into_iter().collect(),
            origin_pcs: origins.into_iter().collect(),
            arithmetic_pcs: arithmetic.into_iter().collect(),
            confidence: if uncertain {
                "abstract_flow_inferred_or_merged"
            } else if analysis.value_flows.iter().any(|flow| {
                flow.sink_kind == BytecodeValueSinkKind::BranchCondition && flow.sink_pc.pc == pc
            }) {
                "abstract_flow_direct_origin_class"
            } else {
                "origin_unknown"
            }
            .into(),
        });
    }
    let cases = scenarios["cases"]
        .as_array()
        .ok_or("scenario cases missing")?;
    let numeric_relations = numeric_relations(cases);
    let preview_relations = preview_relations(cases);
    let preview_reversals = preview_relations
        .iter()
        .filter(|r| !r.nondecreasing)
        .count();
    let accounting_gaps = accounting_gaps(cases);
    let changed_accounting_gaps = accounting_gaps.iter().filter(|r| r.changed).count();
    let numeric_differences = numeric_relations.iter().filter(|r| !r.equal).count();
    let state_plans = state_plans(cases);
    let executable_probes = probes.iter().any(|probe| {
        probe.operation.is_some()
            && (probe.value_when_true.is_some() || probe.value_when_false.is_some())
    });
    let previous_state_plateau = scenarios["guidance"]["state_order_stop_reason"]
        == "four_sequences_without_new_target_outcome";
    let new_root_outcomes = scenarios["guidance"]["new_root_outcomes"]
        .as_array()
        .map_or(0, Vec::len);
    let (next_round_recommended, next_round_reason) =
        if numeric_differences > 0 || changed_accounting_gaps > 0 || preview_reversals > 0 {
            (true, "numeric_difference_requires_local_recheck")
        } else if executable_probes {
            (true, "direct_condition_values_available")
        } else if new_root_outcomes > 0 {
            (true, "new_root_result_observed")
        } else if !state_plans.is_empty() && !previous_state_plateau {
            (true, "observed_state_order_plans_not_yet_exhausted")
        } else {
            (false, "plateau_without_distinct_executable_probe")
        };
    Ok(ForensicPlan {
        schema: "forensic-plan-v1".into(), forensic_report_sha256: forensic_sha,
        lens_sha256: digest(&lens_bytes), coverage_sha256: coverage_sha,
        previous_scenarios_sha256: scenarios_sha,
        runtime_sha256: forensic["identity"]["runtime_sha256"].as_str().unwrap_or("").into(),
        block_hash: forensic["identity"]["block_hash"].as_str().unwrap_or("").into(),
        fixed_contract: forensic["identity"]["address"].as_str().unwrap_or("").into(),
        static_jump_sites: lens["source_coverage"]["runtime_static_jump_sites"].as_u64().unwrap_or(0) as usize,
        one_sided_or_unseen_sites: target_sites,
        direct_conditions: conditions.len(), source_span_matches: matched,
        unresolved_sites: target_sites.saturating_sub(matched),
        probes, branch_influences, state_plans, numeric_relations, preview_relations, preview_reversals, accounting_gaps, changed_accounting_gaps, numeric_differences,
        build_elapsed_ms: start.elapsed().as_millis(),
        next_round_recommended, next_round_reason: next_round_reason.into(),
        limitations: vec![
            "Abstract bytecode origin classes do not identify exact source variables or prove path feasibility".into(),
            "JUMPI source spans are compiler hints; a matched condition is not proven to be the runtime predicate or its branch polarity".into(),
            "Only direct first-parameter versus numeric-literal comparisons receive executable values; complex expressions remain unresolved".into(),
            "Slot overlap proposes transaction order but does not prove dynamic dependency or viable state".into(),
            "Numeric delta expectations assume ERC-4626-like accounting; token side effects can invalidate that expectation".into(),
            "Preview ordering compares isolated cases from the same initial snapshot; a reversal is an observation, not a finding".into(),
        ],
    })
}

#[derive(Serialize)]
pub struct SourceBenchRow {
    pub file: String,
    pub direct_conditions: usize,
    pub boundary_pairs: usize,
    pub executable_first_parameter_pairs: usize,
    pub elapsed_ms: u128,
}

pub fn benchmark_sources(solc: &Path, paths: &[&Path]) -> Result<Vec<SourceBenchRow>, String> {
    let mut rows = Vec::new();
    for path in paths {
        let start = Instant::now();
        let (name, _, output) = compile_source_ast_file(path, solc)?;
        let json: Value =
            serde_json::from_str(&output).map_err(|e| format!("invalid compiler JSON: {e}"))?;
        if json["errors"]
            .as_array()
            .is_some_and(|items| items.iter().any(|e| e["severity"] == "error"))
        {
            return Err(format!("compiler error for {}", path.display()));
        }
        let mut conditions = Vec::new();
        let mut roles = BTreeMap::new();
        collect_roles(&json["sources"][&name]["ast"], &mut roles);
        collect_conditions(
            &json["sources"][&name]["ast"],
            &name,
            "",
            &BTreeSet::new(),
            None,
            &roles,
            &mut conditions,
        );
        rows.push(SourceBenchRow {
            file: name,
            direct_conditions: conditions.len(),
            boundary_pairs: conditions
                .iter()
                .filter(|c| {
                    let (a, b) = values(c);
                    a.is_some() && b.is_some()
                })
                .count(),
            executable_first_parameter_pairs: conditions
                .iter()
                .filter(|c| {
                    let (a, b) = values(c);
                    c.first_uint_parameter && a.is_some() && b.is_some()
                })
                .count(),
            elapsed_ms: start.elapsed().as_millis(),
        });
    }
    Ok(rows)
}
