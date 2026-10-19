//! Reproducible, read-only forensic map for one verified EVM runtime.
//! Scores prioritize investigation; they are not vulnerability probabilities.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as Sha2Digest, Sha256};
use sha3::{Digest as Sha3Digest, Keccak256};

use crate::{
    BytecodeAnalysis, BytecodeSourceComparison, EconomicFunction, EconomicOperation,
    EvidenceLocation, SlitherSignal, SourceSpan, ValueOrigin, ValueSink,
    analyze_economic_functions, compare_with_solc_contract,
    compile_source_ast_text_with_evm_version, reconstruct_bytecode_hex, reconstruct_from_ast_json,
};

const SCHEMA: &str = "forensic-map-v1";
const MAX_HISTORY_BLOCKS: u64 = 20_000;
const HISTORY_CHUNK: u64 = 2_000;

#[derive(Clone, Debug, Deserialize)]
pub struct ForensicInput {
    pub schema: String,
    pub analysis_source_path: PathBuf,
    pub solc_path: PathBuf,
    pub evm_version: Option<String>,
    pub runtime_path: PathBuf,
    pub comparison_solc_json_path: PathBuf,
    pub comparison_source_name: String,
    pub contract_name: String,
    pub rpc_url: String,
    #[serde(default)]
    pub rpc_fallback_urls: Vec<String>,
    pub chain_id: u64,
    pub address: String,
    pub block_number: u64,
    pub block_hash: String,
    pub slither_json_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForensicIdentity {
    pub chain_id: u64,
    pub address: String,
    pub block_number: u64,
    pub block_hash: String,
    pub rpc_host: String,
    pub rpc_fallback_used: bool,
    pub runtime_sha256: String,
    pub analysis_source_sha256: String,
    pub comparison_json_sha256: String,
    pub exact_onchain_runtime: bool,
    pub exact_compiler_runtime: bool,
    pub immutable_masked_compiler_runtime: bool,
    pub comparison_source_name: String,
    pub analysis_source_name: String,
    pub analysis_source_is_verified_bundle: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HotspotReason {
    pub feature: String,
    pub points: u8,
    pub evidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForensicHotspot {
    pub declaring_contract: String,
    pub function: String,
    pub selector: Option<String>,
    pub entry_pc: Option<usize>,
    pub source_span: Option<SourceSpan>,
    pub priority_score: u8,
    pub priority_band: String,
    pub reasons: Vec<HotspotReason>,
    pub slither_checks: Vec<String>,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    pub external_calls: Vec<String>,
    pub internal_calls: Vec<String>,
    pub reachable_functions: Vec<String>,
    pub transitive_reads: Vec<String>,
    pub transitive_writes: Vec<String>,
    pub transitive_external_calls: Vec<String>,
    pub arithmetic: Vec<String>,
    pub branch_count: usize,
    pub unresolved_count: usize,
    pub investigation_questions: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProbeValue {
    pub raw_hex: Option<String>,
    pub decoded: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoundaryProbe {
    pub method: String,
    pub input: String,
    pub related_hotspot_score: u8,
    pub result: ProbeValue,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForensicEvent {
    pub name: String,
    pub block_number: u64,
    pub transaction_hash: String,
    pub log_index: String,
    pub data: String,
    pub topics: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventHistory {
    pub deployment_block: Option<u64>,
    pub scanned_from_block: Option<u64>,
    pub complete_from_deployment: bool,
    pub event_counts: BTreeMap<String, usize>,
    pub events: Vec<ForensicEvent>,
    pub omitted_events: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QuarantineCase {
    pub case_id: String,
    pub focus: String,
    pub originating_function: String,
    pub trigger_score: u8,
    pub trigger_reasons: Vec<String>,
    pub source_readers: Vec<String>,
    pub source_writers: Vec<String>,
    pub variable_access_evidence: Vec<VariableAccess>,
    pub source_value_flows: usize,
    pub value_flow_evidence: Vec<QuarantineFlow>,
    pub bytecode_effect_pcs: Vec<usize>,
    pub bytecode_value_sink_pcs: Vec<usize>,
    pub bytecode_summary_incomplete: Option<bool>,
    pub boundary_probe_count: usize,
    pub boundary_reverts: usize,
    pub anomalies: Vec<String>,
    pub status: String,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariableAccess {
    pub function: String,
    pub access: String,
    pub source_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Serialize)]
pub struct QuarantineFlow {
    pub function: String,
    pub origin: String,
    pub sink: String,
    pub calls: Vec<String>,
    pub origin_span: Option<SourceSpan>,
    pub sink_span: Option<SourceSpan>,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Reassessment {
    pub pass: u8,
    pub function: String,
    pub original_score: u8,
    pub revised_score: u8,
    pub quarantine_case_id: String,
    pub change_reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ForensicReport {
    pub schema: String,
    pub identity: ForensicIdentity,
    pub source_function_count: usize,
    pub bytecode_selector_count: usize,
    pub bytecode_unresolved_count: usize,
    pub compiler_comparison: BytecodeSourceComparison,
    pub economic_candidate_count: usize,
    pub slither_detector_counts: BTreeMap<String, usize>,
    pub slither_unmapped_count: usize,
    pub hotspots: Vec<ForensicHotspot>,
    pub quarantine_cases: Vec<QuarantineCase>,
    pub reassessment: Vec<Reassessment>,
    pub onchain_state: BTreeMap<String, ProbeValue>,
    pub boundary_probes: Vec<BoundaryProbe>,
    pub observed_relations: Vec<String>,
    pub event_history: EventHistory,
    pub unresolved: Vec<String>,
}

struct Rpc {
    client: Client,
    url: String,
}

impl Rpc {
    fn new(url: &str) -> Result<Self, String> {
        if !(url.starts_with("https://")
            || url.starts_with("http://127.0.0.1:")
            || url.starts_with("http://localhost:"))
        {
            return Err("RPC URL must use HTTPS or local HTTP".into());
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| format!("cannot create RPC client: {error}"))?;
        Ok(Self {
            client,
            url: url.into(),
        })
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let response = self
            .client
            .post(&self.url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .map_err(|error| format!("{method} transport error: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("{method} HTTP {}", response.status()));
        }
        let value: Value = response
            .json()
            .map_err(|error| format!("{method} invalid JSON: {error}"))?;
        if let Some(error) = value.get("error") {
            return Err(format!("{method} RPC error: {error}"));
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| format!("{method} missing result"))
    }

    fn code(&self, address: &str, block: u64) -> Result<String, String> {
        self.call("eth_getCode", json!([address, block_tag(block)]))?
            .as_str()
            .map(str::to_owned)
            .ok_or("eth_getCode result is not hex".into())
    }

    fn view(&self, address: &str, data: &str, block: u64) -> Result<String, String> {
        self.call(
            "eth_call",
            json!([{"to":address,"data":data}, block_tag(block)]),
        )?
        .as_str()
        .map(str::to_owned)
        .ok_or("eth_call result is not hex".into())
    }
}

fn block_tag(block: u64) -> String {
    format!("0x{block:x}")
}

fn hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    hex::decode(raw).map_err(|error| format!("invalid hex: {error}"))
}

fn hex_u64(value: &str) -> Result<u64, String> {
    u64::from_str_radix(value.strip_prefix("0x").unwrap_or(value), 16)
        .map_err(|error| format!("invalid hex integer: {error}"))
}

fn validate_address(value: &str) -> Result<String, String> {
    if value.len() != 42
        || !value.starts_with("0x")
        || !value[2..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("address must be 0x followed by 40 hex digits".into());
    }
    Ok(value.to_ascii_lowercase())
}

fn decode_word(raw: &str) -> Option<[u8; 32]> {
    let bytes = hex_bytes(raw).ok()?;
    bytes.try_into().ok()
}

fn as_u128(raw: &str) -> Option<u128> {
    let word = decode_word(raw)?;
    if word[..16].iter().any(|byte| *byte != 0) {
        return None;
    }
    Some(u128::from_be_bytes(word[16..].try_into().ok()?))
}

fn as_address(raw: &str) -> Option<String> {
    let word = decode_word(raw)?;
    if word[..12].iter().any(|byte| *byte != 0) {
        return None;
    }
    Some(format!("0x{}", hex::encode(&word[12..])))
}

fn encode_u256(value: u128) -> String {
    format!("{value:064x}")
}
fn encode_address(value: &str) -> String {
    format!("{:0>64}", &value[2..])
}

fn probe_call(
    rpc: &Rpc,
    address: &str,
    selector: &str,
    argument: Option<&str>,
    block: u64,
    kind: &str,
) -> ProbeValue {
    let data = format!(
        "0x{}{}",
        selector.trim_start_matches("0x"),
        argument.unwrap_or("")
    );
    match rpc.view(address, &data, block) {
        Ok(raw) => {
            let decoded = match kind {
                "address" => as_address(&raw),
                "uint" => as_u128(&raw).map(|value| value.to_string()),
                _ => None,
            };
            ProbeValue {
                raw_hex: Some(raw),
                decoded,
                error: None,
            }
        }
        Err(error) => ProbeValue {
            raw_hex: None,
            decoded: None,
            error: Some(error),
        },
    }
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn function_name(signature: &str) -> &str {
    signature.split('(').next().unwrap_or(signature)
}

fn critical(name: &str) -> bool {
    matches!(
        name,
        "deposit"
            | "mint"
            | "withdraw"
            | "redeem"
            | "previewDeposit"
            | "previewMint"
            | "previewWithdraw"
            | "previewRedeem"
            | "convertToShares"
            | "convertToAssets"
            | "totalAssets"
            | "totalSupply"
            | "maxDeposit"
            | "maxMint"
            | "maxWithdraw"
            | "maxRedeem"
            | "_deposit"
            | "_withdraw"
            | "_transferIn"
            | "_transferOut"
            | "_convertToShares"
            | "_convertToAssets"
            | "_mint"
            | "_burn"
            | "distributeYield"
            | "setDepositCap"
    )
}

fn strings<T>(items: &[crate::Evidence<T>], render: impl Fn(&T) -> String) -> Vec<String> {
    items
        .iter()
        .map(|item| render(&item.value))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

struct Closure {
    reachable: Vec<String>,
    reads: Vec<String>,
    writes: Vec<String>,
    external_calls: Vec<String>,
    unresolved_edges: usize,
}

fn call_closure(start: &EconomicFunction, functions: &[EconomicFunction]) -> Closure {
    let mut queue = VecDeque::from([(start, 0_usize)]);
    let mut seen = BTreeSet::new();
    let mut reachable = BTreeSet::new();
    let mut reads = BTreeSet::new();
    let mut writes = BTreeSet::new();
    let mut external_calls = BTreeSet::new();
    let mut unresolved_edges = 0;
    while let Some((current, depth)) = queue.pop_front() {
        let identity = format!("{}::{}", current.contract.0, current.function.0);
        if !seen.insert(identity.clone()) {
            continue;
        }
        if depth > 0 {
            reachable.insert(identity);
            reads.extend(current.reads.iter().map(|item| item.value.0.clone()));
            writes.extend(current.writes.iter().map(|item| item.value.0.clone()));
            external_calls.extend(
                current
                    .external_calls
                    .iter()
                    .map(|item| item.value.0.clone()),
            );
        }
        for call in &current.resolved_calls {
            if depth >= 8 {
                unresolved_edges += 1;
                continue;
            }
            if let Some(next) = functions.iter().find(|function| {
                function.contract == call.value.contract && function.function == call.value.function
            }) {
                queue.push_back((next, depth + 1));
            } else {
                unresolved_edges += 1;
            }
        }
    }
    Closure {
        reachable: reachable.into_iter().collect(),
        reads: reads.into_iter().collect(),
        writes: writes.into_iter().collect(),
        external_calls: external_calls.into_iter().collect(),
        unresolved_edges,
    }
}

fn hotspot_map(
    functions: &[EconomicFunction],
    comparison: &BytecodeSourceComparison,
    target: &str,
    slither_signals: &[SlitherSignal],
) -> Vec<ForensicHotspot> {
    let mut result = Vec::new();
    for function in functions {
        let name = function_name(&function.function.0);
        if !critical(name) && function.contract.0 != target {
            continue;
        }
        let closure = call_closure(function, functions);
        let methods: Vec<_> = comparison
            .matched_methods
            .iter()
            .filter(|method| function_name(&method.signature) == name)
            .collect();
        let method = if methods.len() == 1
            && (function.contract.0 == target
                || !functions.iter().any(|other| {
                    other.contract.0 == target && function_name(&other.function.0) == name
                })) {
            methods.first().copied()
        } else {
            None
        };
        let mut reasons = Vec::new();
        let mut add = |feature: &str, points: u8, evidence: String| {
            reasons.push(HotspotReason {
                feature: feature.into(),
                points,
                evidence,
            });
        };
        let source_span = match &function.operation.location {
            EvidenceLocation::Source(span) => Some(span.clone()),
            _ => None,
        };
        let slither_checks: Vec<String> = source_span
            .as_ref()
            .map(|function_span| {
                slither_signals
                    .iter()
                    .filter(|signal| {
                        signal.location.as_ref().is_some_and(|span| {
                            span.file() == function_span.file()
                                && span.start_byte() < function_span.end_byte()
                                && span.end_byte() > function_span.start_byte()
                        })
                    })
                    .map(|signal| signal.check.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default();
        if matches!(
            function.operation.value,
            EconomicOperation::Deposit
                | EconomicOperation::Mint
                | EconomicOperation::Withdraw
                | EconomicOperation::Redeem
        ) {
            add(
                "asset_or_share_transition",
                15,
                format!("{:?}", function.operation.value),
            );
        }
        if !function.writes.is_empty() {
            add(
                "storage_write",
                12,
                format!("{} direct writes", function.writes.len()),
            );
        }
        if !function.external_calls.is_empty() {
            add(
                "external_call",
                15,
                format!("{} direct calls", function.external_calls.len()),
            );
        }
        if !function.arithmetic.is_empty() {
            add(
                "arithmetic",
                10,
                format!("{} expressions", function.arithmetic.len()),
            );
        }
        if !function.control_conditions.is_empty() {
            add(
                "branches_or_guards",
                8,
                format!("{} conditions", function.control_conditions.len()),
            );
        }
        if !function.reachability_paths.is_empty() {
            add(
                "downstream_state_or_conversion",
                8,
                format!("{} paths", function.reachability_paths.len()),
            );
        }
        if !closure.writes.is_empty() {
            add(
                "reachable_storage_write",
                8,
                format!("{} transitive storage fields", closure.writes.len()),
            );
        }
        if !closure.external_calls.is_empty() {
            add(
                "reachable_external_call",
                8,
                format!("{} transitive call targets", closure.external_calls.len()),
            );
        }
        if closure.unresolved_edges > 0 {
            add(
                "unresolved_call_edges",
                5,
                format!("{} unresolved call edges", closure.unresolved_edges),
            );
        }
        if !slither_checks.is_empty() {
            add("slither_detector", 4, slither_checks.join(", "));
        }
        if !function.unresolved.is_empty() {
            add(
                "unresolved_semantics",
                8,
                format!("{} uncertainties", function.unresolved.len()),
            );
        }
        if !function.modifiers.is_empty() {
            add(
                "access_or_modifier_logic",
                6,
                format!("{} modifiers", function.modifiers.len()),
            );
        }
        if matches!(
            name,
            "maxDeposit"
                | "maxMint"
                | "previewDeposit"
                | "previewMint"
                | "previewWithdraw"
                | "previewRedeem"
                | "_convertToShares"
                | "_convertToAssets"
        ) {
            add("boundary_or_rounding_surface", 10, name.into());
        }
        let score: u8 = reasons
            .iter()
            .map(|reason| reason.points as u16)
            .sum::<u16>()
            .min(100) as u8;
        let band = if score >= 40 {
            "high"
        } else if score >= 25 {
            "medium"
        } else {
            "low"
        };
        let mut questions = Vec::new();
        if matches!(name, "deposit" | "mint" | "_deposit" | "_transferIn") {
            questions
                .push("Do minted shares reflect assets actually received from the token?".into());
        }
        if matches!(name, "withdraw" | "redeem" | "_withdraw" | "_transferOut") {
            questions.push("Can every burned share amount be paid out under token behavior and balance changes?".into());
        }
        if name.contains("convert") || name.starts_with("preview") || name.starts_with("max") {
            questions.push("Do zero, one-unit, cap-boundary and donation states preserve rounding and limit relations?".into());
        }
        if !function.modifiers.is_empty() {
            questions
                .push("Are modifier guards applied on every reachable state-changing path?".into());
        }
        result.push(ForensicHotspot {
            declaring_contract: function.contract.0.clone(),
            function: function.function.0.clone(),
            selector: method.map(|item| item.selector.clone()),
            entry_pc: method.map(|item| item.destination_pc.pc),
            source_span,
            priority_score: score,
            priority_band: band.into(),
            reasons,
            slither_checks,
            reads: strings(&function.reads, |item| item.0.clone()),
            writes: strings(&function.writes, |item| item.0.clone()),
            external_calls: strings(&function.external_calls, |item| item.0.clone()),
            internal_calls: strings(&function.resolved_calls, |item| {
                format!("{}::{}", item.contract.0, item.function.0)
            }),
            reachable_functions: closure.reachable,
            transitive_reads: closure.reads,
            transitive_writes: closure.writes,
            transitive_external_calls: closure.external_calls,
            arithmetic: strings(&function.arithmetic, |item| item.0.clone()),
            branch_count: function.control_conditions.len(),
            unresolved_count: function.unresolved.len(),
            investigation_questions: questions,
        });
    }
    result.sort_by(|a, b| {
        b.priority_score
            .cmp(&a.priority_score)
            .then_with(|| a.declaring_contract.cmp(&b.declaring_contract))
            .then_with(|| a.function.cmp(&b.function))
    });
    result
}

fn related_probe(name: &str, method: &str) -> bool {
    let observed = function_name(method);
    if name == observed {
        return true;
    }
    match name {
        "_convertToShares" => matches!(
            observed,
            "convertToShares" | "previewDeposit" | "previewWithdraw"
        ),
        "_convertToAssets" => matches!(
            observed,
            "convertToAssets" | "previewMint" | "previewRedeem"
        ),
        "_deposit" => matches!(observed, "previewDeposit" | "previewMint"),
        "_withdraw" => matches!(observed, "previewWithdraw" | "previewRedeem"),
        _ => false,
    }
}

fn historical_relations(
    history: &EventHistory,
    state: &BTreeMap<String, ProbeValue>,
) -> (Vec<String>, Vec<String>) {
    let mut relations = Vec::new();
    let mut anomalies = Vec::new();
    if !history.complete_from_deployment || history.error.is_some() || history.omitted_events > 0 {
        return (relations, anomalies);
    }
    let zero_topic = format!("0x{}", "00".repeat(32));
    let mut minted = 0_u128;
    let mut burned = 0_u128;
    let mut complete = true;
    for event in &history.events {
        if event.name != "Transfer" {
            continue;
        }
        let Some(amount) = as_u128(&event.data) else {
            complete = false;
            break;
        };
        if event
            .topics
            .get(1)
            .is_some_and(|topic| topic.eq_ignore_ascii_case(&zero_topic))
        {
            let Some(value) = minted.checked_add(amount) else {
                complete = false;
                break;
            };
            minted = value;
        }
        if event
            .topics
            .get(2)
            .is_some_and(|topic| topic.eq_ignore_ascii_case(&zero_topic))
        {
            let Some(value) = burned.checked_add(amount) else {
                complete = false;
                break;
            };
            burned = value;
        }
    }
    if complete {
        if let (Some(expected), Some(actual)) = (
            minted.checked_sub(burned),
            state
                .get("totalSupply()")
                .and_then(|value| value.raw_hex.as_deref())
                .and_then(as_u128),
        ) {
            let relation = format!(
                "share Transfer mint-burn={expected} versus totalSupply={actual}; equal={}",
                expected == actual
            );
            if expected != actual {
                anomalies.push(relation.clone());
            }
            relations.push(relation);
        }
    }
    if let (Some(event), Some(actual)) = (
        history
            .events
            .iter()
            .rev()
            .find(|event| event.name == "DepositCapSet"),
        state
            .get("depositCap()")
            .and_then(|value| value.raw_hex.as_deref())
            .and_then(as_u128),
    ) {
        if let Some(expected) = as_u128(&event.data) {
            let relation = format!(
                "last DepositCapSet={expected} versus depositCap={actual}; equal={}",
                expected == actual
            );
            if expected != actual {
                anomalies.push(relation.clone());
            }
            relations.push(relation);
        }
    }
    if let (Some(event), Some(actual)) = (
        history
            .events
            .iter()
            .rev()
            .find(|event| event.name == "OwnershipTransferred"),
        state
            .get("owner()")
            .and_then(|value| value.decoded.as_deref()),
    ) {
        if let Some(topic) = event.topics.get(2) {
            if let Some(expected) = as_address(topic) {
                let relation = format!(
                    "last OwnershipTransferred owner={expected} versus current owner={actual}; equal={}",
                    expected.eq_ignore_ascii_case(actual)
                );
                if !expected.eq_ignore_ascii_case(actual) {
                    anomalies.push(relation.clone());
                }
                relations.push(relation);
            }
        }
    }
    (relations, anomalies)
}

fn numeric_anomalies(
    state: &BTreeMap<String, ProbeValue>,
    probes: &[BoundaryProbe],
) -> Vec<String> {
    let mut anomalies = Vec::new();
    let number = |key: &str| {
        state
            .get(key)
            .and_then(|value| value.raw_hex.as_deref())
            .and_then(as_u128)
    };
    if let (Some(total), Some(balance)) =
        (number("totalAssets()"), number("asset.balanceOf(vault)"))
    {
        if total != balance {
            anomalies.push(format!(
                "totalAssets={total} differs from asset.balanceOf(vault)={balance}"
            ));
        }
    }
    if let (Some(cap), Some(total), Some(max)) = (
        number("depositCap()"),
        number("totalAssets()"),
        number("maxDeposit(address)"),
    ) {
        if cap != u128::MAX && max != cap.saturating_sub(total) {
            anomalies.push(format!(
                "maxDeposit={max} differs from cap headroom={}",
                cap.saturating_sub(total)
            ));
        }
    }
    let sample = |name: &str, input: &str| {
        probes
            .iter()
            .find(|probe| probe.method == name && probe.input == input)
            .and_then(|probe| probe.result.raw_hex.as_deref())
            .and_then(as_u128)
    };
    for amount in probes
        .iter()
        .map(|probe| probe.input.as_str())
        .collect::<BTreeSet<_>>()
    {
        if amount == "0" {
            continue;
        }
        if let Some(0) = sample("previewDeposit(uint256)", amount) {
            anomalies.push(format!(
                "positive deposit input {amount} previews zero shares"
            ));
        }
        if let (Some(ceil), Some(floor)) = (
            sample("previewWithdraw(uint256)", amount),
            sample("convertToShares(uint256)", amount),
        ) {
            if ceil < floor {
                anomalies.push(format!(
                    "input {amount}: previewWithdraw {ceil} < convertToShares {floor}"
                ));
            }
        }
        if let (Some(ceil), Some(floor)) = (
            sample("previewMint(uint256)", amount),
            sample("convertToAssets(uint256)", amount),
        ) {
            if ceil < floor {
                anomalies.push(format!(
                    "input {amount}: previewMint {ceil} < convertToAssets {floor}"
                ));
            }
        }
    }
    anomalies.extend(monotonicity_observations(probes).1);
    anomalies
}

fn monotonicity_observations(probes: &[BoundaryProbe]) -> (Vec<String>, Vec<String>) {
    let mut by_method: BTreeMap<String, Vec<(u128, u128)>> = BTreeMap::new();
    for probe in probes {
        if let (Ok(input), Some(output)) = (
            probe.input.parse::<u128>(),
            probe.result.raw_hex.as_deref().and_then(as_u128),
        ) {
            by_method
                .entry(probe.method.clone())
                .or_default()
                .push((input, output));
        }
    }
    let mut relations = Vec::new();
    let mut anomalies = Vec::new();
    for (method, mut points) in by_method {
        points.sort_by_key(|(input, _)| *input);
        let mut violations = 0;
        for pair in points.windows(2) {
            if pair[1].1 < pair[0].1 {
                violations += 1;
                anomalies.push(format!(
                    "{method} decreases from {} at input {} to {} at input {}",
                    pair[0].1, pair[0].0, pair[1].1, pair[1].0
                ));
            }
        }
        relations.push(format!(
            "{method}: {} successful sampled inputs; monotonicity violations={violations}",
            points.len()
        ));
    }
    (relations, anomalies)
}

fn focus_weight(name: &str) -> u8 {
    let lower = name.to_ascii_lowercase();
    if lower.contains("supply") {
        12
    } else if lower.contains("cap") {
        11
    } else if lower.contains("price") || lower.contains("oracle") {
        10
    } else if lower.contains("balance") {
        9
    } else if lower.contains("fee") {
        8
    } else if lower.contains("allowance") {
        7
    } else if lower.contains("asset") {
        6
    } else {
        0
    }
}

fn quarantine_focus(hotspot: &ForensicHotspot) -> String {
    if matches!(function_name(&hotspot.function), "maxDeposit" | "maxMint") {
        if let Some(cap) = hotspot
            .reads
            .iter()
            .chain(hotspot.transitive_reads.iter())
            .find(|item| item.to_ascii_lowercase().contains("cap"))
        {
            return cap.clone();
        }
    }
    let candidates = hotspot
        .writes
        .iter()
        .map(|name| (40_u8, name))
        .chain(hotspot.transitive_writes.iter().map(|name| (30_u8, name)))
        .chain(hotspot.reads.iter().map(|name| (20_u8, name)))
        .chain(hotspot.transitive_reads.iter().map(|name| (10_u8, name)));
    candidates
        .max_by(|(left_weight, left), (right_weight, right)| {
            (*left_weight + focus_weight(left))
                .cmp(&(*right_weight + focus_weight(right)))
                .then_with(|| right.cmp(left))
        })
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| hotspot.function.clone())
}

fn quarantine_cases(
    hotspots: &[ForensicHotspot],
    functions: &[EconomicFunction],
    bytecode: &BytecodeAnalysis,
    state: &BTreeMap<String, ProbeValue>,
    probes: &[BoundaryProbe],
    history: &EventHistory,
) -> Vec<QuarantineCase> {
    let mut cases = Vec::new();
    let mut anomalies = numeric_anomalies(state, probes);
    anomalies.extend(historical_relations(history, state).1);
    for hotspot in hotspots
        .iter()
        .filter(|item| item.priority_score >= 25)
        .take(24)
    {
        let focus = quarantine_focus(hotspot);
        let mut readers = BTreeSet::new();
        let mut writers = BTreeSet::new();
        let mut value_flows = 0;
        let mut access_evidence = Vec::new();
        let mut flow_evidence = Vec::new();
        for function in functions {
            let identity = format!("{}::{}", function.contract.0, function.function.0);
            for item in function.reads.iter().filter(|item| item.value.0 == focus) {
                readers.insert(identity.clone());
                if access_evidence.len() < 64 {
                    access_evidence.push(VariableAccess {
                        function: identity.clone(),
                        access: "read".into(),
                        source_span: match &item.location {
                            EvidenceLocation::Source(span) => Some(span.clone()),
                            _ => None,
                        },
                    });
                }
            }
            for item in function.writes.iter().filter(|item| item.value.0 == focus) {
                writers.insert(identity.clone());
                if access_evidence.len() < 64 {
                    access_evidence.push(VariableAccess {
                        function: identity.clone(),
                        access: "write".into(),
                        source_span: match &item.location {
                            EvidenceLocation::Source(span) => Some(span.clone()),
                            _ => None,
                        },
                    });
                }
            }
            for flow in function.value_flows.iter().filter(|flow| {
                matches!(&flow.origin.value, ValueOrigin::Storage(item) if item.0 == focus)
                    || matches!(&flow.sink.value, ValueSink::StorageWrite(item) if item.0 == focus)
            }) {
                value_flows += 1;
                if flow_evidence.len() < 32 {
                    flow_evidence.push(QuarantineFlow {
                        function: identity.clone(),
                        origin: format!("{:?}", flow.origin.value),
                        sink: format!("{:?}", flow.sink.value),
                        calls: flow.calls.iter().map(|call| call.value.0.clone()).collect(),
                        origin_span: match &flow.origin.location {
                            EvidenceLocation::Source(span) => Some(span.clone()),
                            _ => None,
                        },
                        sink_span: match &flow.sink.location {
                            EvidenceLocation::Source(span) => Some(span.clone()),
                            _ => None,
                        },
                        confidence: format!("{:?}", flow.confidence),
                    });
                }
            }
        }
        let entry = hotspot.selector.as_ref().and_then(|selector| {
            bytecode
                .entry_summaries
                .iter()
                .find(|item| &item.selector == selector)
        });
        let effects = entry
            .map(|item| {
                item.effects
                    .iter()
                    .map(|effect| effect.location.pc)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .take(32)
                    .collect()
            })
            .unwrap_or_default();
        let sinks = entry
            .map(|item| {
                item.value_flows
                    .iter()
                    .map(|flow| flow.sink_pc.pc)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .take(32)
                    .collect()
            })
            .unwrap_or_default();
        let related: Vec<_> = probes
            .iter()
            .filter(|probe| related_probe(function_name(&hotspot.function), &probe.method))
            .collect();
        let reverts = related
            .iter()
            .filter(|probe| probe.result.error.is_some())
            .count();
        let name = function_name(&hotspot.function);
        let relevant_anomalies: Vec<_> = anomalies
            .iter()
            .filter(|item| {
                (item.contains("totalAssets")
                    && matches!(
                        name,
                        "totalAssets"
                            | "deposit"
                            | "mint"
                            | "withdraw"
                            | "redeem"
                            | "_deposit"
                            | "_withdraw"
                    ))
                    || (item.contains("maxDeposit")
                        && matches!(name, "maxDeposit" | "maxMint" | "deposit" | "mint"))
                    || (item.contains("previewDeposit")
                        && matches!(name, "previewDeposit" | "deposit" | "_convertToShares"))
                    || (item.contains("previewWithdraw")
                        && matches!(name, "previewWithdraw" | "withdraw" | "_convertToShares"))
                    || (item.contains("previewMint")
                        && matches!(name, "previewMint" | "mint" | "_convertToAssets"))
                    || (item.contains("totalSupply")
                        && matches!(
                            name,
                            "totalSupply" | "_mint" | "_burn" | "deposit" | "withdraw"
                        ))
                    || (item.contains("DepositCapSet")
                        && matches!(name, "setDepositCap" | "maxDeposit" | "maxMint"))
                    || (item.contains("OwnershipTransferred")
                        && matches!(name, "distributeYield" | "setDepositCap"))
            })
            .cloned()
            .collect();
        let mut limitations = Vec::new();
        if entry.is_none() {
            limitations.push(
                "No independent bytecode entry summary for this internal or unresolved function"
                    .into(),
            );
        }
        if entry.is_some_and(|item| item.incomplete) {
            limitations.push("Bytecode entry summary has unresolved control flow".into());
        }
        if history.error.is_some() || !history.complete_from_deployment {
            limitations.push("Historical event coverage is partial".into());
        }
        if related.is_empty() {
            limitations.push("No read-only boundary probe directly covers this function".into());
        }
        if reverts > 0 {
            limitations.push(format!(
                "{reverts} read-only boundary calls reverted; context and cause require review"
            ));
        }
        let status = if !relevant_anomalies.is_empty() {
            "anomaly_observed"
        } else if !related.is_empty() {
            "sampled_no_anomaly_observed"
        } else if entry.is_some() {
            "static_crosscheck_only"
        } else {
            "inconclusive"
        };
        let case_id = sha256(
            format!(
                "{}::{}::{focus}",
                hotspot.declaring_contract, hotspot.function
            )
            .as_bytes(),
        )[..16]
            .to_owned();
        cases.push(QuarantineCase {
            case_id,
            focus,
            originating_function: format!("{}::{}", hotspot.declaring_contract, hotspot.function),
            trigger_score: hotspot.priority_score,
            trigger_reasons: hotspot
                .reasons
                .iter()
                .map(|item| item.feature.clone())
                .collect(),
            source_readers: readers.into_iter().collect(),
            source_writers: writers.into_iter().collect(),
            variable_access_evidence: access_evidence,
            source_value_flows: value_flows,
            value_flow_evidence: flow_evidence,
            bytecode_effect_pcs: effects,
            bytecode_value_sink_pcs: sinks,
            bytecode_summary_incomplete: entry.map(|item| item.incomplete),
            boundary_probe_count: related.len(),
            boundary_reverts: reverts,
            anomalies: relevant_anomalies,
            status: status.into(),
            limitations,
        });
    }
    cases
}

fn reevaluate_map(
    functions: &[EconomicFunction],
    comparison: &BytecodeSourceComparison,
    target: &str,
    slither_signals: &[SlitherSignal],
    cases: &[QuarantineCase],
) -> (Vec<ForensicHotspot>, Vec<Reassessment>) {
    // Engine 1 starts from its source/bytecode map again. Engine 2 may only add
    // observed anomalies, never a confidence bonus for a normal sample.
    let mut map = hotspot_map(functions, comparison, target, slither_signals);
    let mut reassessment = Vec::new();
    for hotspot in &mut map {
        let identity = format!("{}::{}", hotspot.declaring_contract, hotspot.function);
        if let Some(case) = cases
            .iter()
            .find(|case| case.originating_function == identity)
        {
            let original = hotspot.priority_score;
            let added = if case.anomalies.is_empty() { 0 } else { 20 };
            hotspot.priority_score = original.saturating_add(added).min(100);
            hotspot.priority_band = if hotspot.priority_score >= 40 {
                "high"
            } else if hotspot.priority_score >= 25 {
                "medium"
            } else {
                "low"
            }
            .into();
            if added > 0 {
                hotspot.reasons.push(HotspotReason {
                    feature: "quarantine_observed_anomaly".into(),
                    points: added,
                    evidence: case.anomalies.join("; "),
                });
            }
            reassessment.push(Reassessment {
                pass: 2,
                function: identity,
                original_score: original,
                revised_score: hotspot.priority_score,
                quarantine_case_id: case.case_id.clone(),
                change_reason: if added > 0 {
                    "observed anomaly promoted this path".into()
                } else {
                    "no new anomaly; score retained, no claim of safety".into()
                },
            });
        }
    }
    map.sort_by(|a, b| {
        b.priority_score
            .cmp(&a.priority_score)
            .then_with(|| a.declaring_contract.cmp(&b.declaring_contract))
            .then_with(|| a.function.cmp(&b.function))
    });
    (map, reassessment)
}

fn topic(signature: &str) -> String {
    format!("0x{}", hex::encode(Keccak256::digest(signature.as_bytes())))
}

fn earliest_available_block(error: &str) -> Option<u64> {
    error
        .split("earliest available ")
        .nth(1)?
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

fn event_history(rpc: &Rpc, address: &str, block: u64) -> EventHistory {
    let mut history = EventHistory {
        deployment_block: None,
        scanned_from_block: None,
        complete_from_deployment: false,
        event_counts: BTreeMap::new(),
        events: Vec::new(),
        omitted_events: 0,
        error: None,
    };
    let outcome = (|| -> Result<(), String> {
        let mut lo = match rpc.code(address, 0) {
            Ok(_) => 0,
            Err(error) => {
                earliest_available_block(&error).unwrap_or_else(|| block.saturating_sub(8_000))
            }
        };
        if lo > block {
            return Err("RPC history starts after the requested block".into());
        }
        let code_at_lower = hex_bytes(&rpc.code(address, lo)?)?;
        if !code_at_lower.is_empty() {
            history.scanned_from_block = Some(lo.max(block.saturating_sub(MAX_HISTORY_BLOCKS - 1)));
        } else {
            let mut hi = block;
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if hex_bytes(&rpc.code(address, mid)?)?.is_empty() {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            if hex_bytes(&rpc.code(address, lo)?)?.is_empty() {
                return Err("runtime absent at requested block".into());
            }
            history.deployment_block = Some(lo);
        }
        let from = history
            .scanned_from_block
            .unwrap_or_else(|| lo.max(block.saturating_sub(MAX_HISTORY_BLOCKS - 1)));
        history.scanned_from_block = Some(from);
        history.complete_from_deployment = history.deployment_block == Some(from);
        let known: BTreeMap<String, String> = [
            ("Deposit(address,address,uint256,uint256)", "Deposit"),
            (
                "Withdraw(address,address,address,uint256,uint256)",
                "Withdraw",
            ),
            (
                "YieldDistributed(address,uint256,uint256)",
                "YieldDistributed",
            ),
            ("DepositCapSet(uint256)", "DepositCapSet"),
            (
                "OwnershipTransferred(address,address)",
                "OwnershipTransferred",
            ),
            ("Transfer(address,address,uint256)", "Transfer"),
            ("Approval(address,address,uint256)", "Approval"),
        ]
        .into_iter()
        .map(|(signature, name)| (topic(signature), name.into()))
        .collect();
        let mut start = from;
        while start <= block {
            let end = block.min(start.saturating_add(HISTORY_CHUNK - 1));
            let logs = rpc.call(
                "eth_getLogs",
                json!([{"address":address,"fromBlock":block_tag(start),"toBlock":block_tag(end)}]),
            )?;
            let logs = logs
                .as_array()
                .ok_or("eth_getLogs result is not an array")?;
            for log in logs {
                let topics: Vec<String> = log
                    .get("topics")
                    .and_then(Value::as_array)
                    .ok_or("log has no topics")?
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
                let first = topics
                    .first()
                    .map(String::as_str)
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let name = known
                    .get(&first)
                    .cloned()
                    .unwrap_or_else(|| format!("unknown:{first}"));
                *history.event_counts.entry(name.clone()).or_default() += 1;
                if history.events.len() < 200 {
                    history.events.push(ForensicEvent {
                        name,
                        block_number: hex_u64(
                            log.get("blockNumber")
                                .and_then(Value::as_str)
                                .ok_or("log missing blockNumber")?,
                        )?,
                        transaction_hash: log
                            .get("transactionHash")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        log_index: log
                            .get("logIndex")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .into(),
                        data: log.get("data").and_then(Value::as_str).unwrap_or("").into(),
                        topics,
                    });
                } else {
                    history.omitted_events += 1;
                }
            }
            if end == block {
                break;
            }
            start = end + 1;
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        history.error = Some(error);
    }
    history
}

pub fn run_forensic_probe(input: &ForensicInput) -> Result<ForensicReport, String> {
    if input.schema != "forensic-input-v1" {
        return Err("expected forensic-input-v1 manifest".into());
    }
    let address = validate_address(&input.address)?;
    if input.block_hash.len() != 66
        || !input.block_hash.starts_with("0x")
        || !input.block_hash[2..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("manifest block_hash must be a 32-byte hex value".into());
    }
    let source = read(&input.analysis_source_path)?;
    let source_name = input
        .analysis_source_path
        .file_name()
        .and_then(|item| item.to_str())
        .ok_or("analysis source needs UTF-8 file name")?;
    let runtime_text = read(&input.runtime_path)?;
    let runtime_bytes = hex_bytes(runtime_text.trim())?;
    let comparison_json = read(&input.comparison_solc_json_path)?;
    let slither = input
        .slither_json_path
        .as_ref()
        .map(|path| read(path))
        .transpose()?;
    let ast_json = compile_source_ast_text_with_evm_version(
        source_name,
        &source,
        &input.solc_path,
        input.evm_version.as_deref(),
    )?;
    let functions = reconstruct_from_ast_json(&ast_json, source_name, &source)?;
    let analysis =
        reconstruct_bytecode_hex(&input.runtime_path.display().to_string(), &runtime_text)?;
    let comparison = compare_with_solc_contract(
        &analysis,
        &comparison_json,
        &input.comparison_source_name,
        &input.contract_name,
    )?;
    if comparison.normalized_runtime_match != Some(true)
        && comparison.immutable_masked_runtime_match != Some(true)
    {
        return Err(
            "compiler runtime does not match the supplied runtime outside immutable references"
                .into(),
        );
    }
    let mut selected = None;
    let mut rpc_failures = 0;
    for (index, url) in std::iter::once(&input.rpc_url)
        .chain(input.rpc_fallback_urls.iter())
        .enumerate()
    {
        let attempt = (|| -> Result<(Rpc, u64, String, String), String> {
            let rpc = Rpc::new(url)?;
            let chain = rpc.call("eth_chainId", json!([]))?;
            let chain = hex_u64(chain.as_str().ok_or("eth_chainId is not hex")?)?;
            if chain != input.chain_id {
                return Err("chain ID mismatch".into());
            }
            let header = rpc.call(
                "eth_getBlockByNumber",
                json!([block_tag(input.block_number), false]),
            )?;
            let block_hash = header
                .get("hash")
                .and_then(Value::as_str)
                .ok_or("block header has no hash")?
                .to_owned();
            if !block_hash.eq_ignore_ascii_case(&input.block_hash) {
                return Err("fixed block hash mismatch".into());
            }
            let onchain = hex_bytes(&rpc.code(&address, input.block_number)?)?;
            if onchain != runtime_bytes {
                return Err("runtime differs from fixed-block eth_getCode".into());
            }
            let host = reqwest::Url::parse(url)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".into());
            Ok((rpc, chain, block_hash, host))
        })();
        match attempt {
            Ok((rpc, chain, block_hash, host)) => {
                selected = Some((rpc, chain, block_hash, host, index > 0));
                break;
            }
            Err(_) => rpc_failures += 1,
        }
    }
    let (rpc, chain, block_hash, rpc_host, rpc_fallback_used) = selected.ok_or_else(|| {
        format!(
            "all {} RPC endpoints failed chain, block, or runtime verification",
            rpc_failures
        )
    })?;
    let economic =
        analyze_economic_functions(&functions, source_name, &source, slither.as_deref())?;
    let mut detector_counts = BTreeMap::new();
    for signal in &economic.slither_signals {
        *detector_counts.entry(signal.check.clone()).or_insert(0) += 1;
    }
    let initial_hotspots = hotspot_map(
        &functions,
        &comparison,
        &input.contract_name,
        &economic.slither_signals,
    );
    let output: Value = serde_json::from_str(&comparison_json)
        .map_err(|error| format!("invalid comparison JSON: {error}"))?;
    let methods = output
        .pointer(&format!(
            "/contracts/{}/{}/evm/methodIdentifiers",
            input
                .comparison_source_name
                .replace('~', "~0")
                .replace('/', "~1"),
            input.contract_name.replace('~', "~0").replace('/', "~1")
        ))
        .and_then(Value::as_object)
        .ok_or("comparison JSON has no methodIdentifiers")?;
    let method_selector = |name: &str| methods.get(name).and_then(Value::as_str);
    let mut state = BTreeMap::new();
    for (signature, kind) in [
        ("owner()", "address"),
        ("asset()", "address"),
        ("depositCap()", "uint"),
        ("totalAssets()", "uint"),
        ("totalSupply()", "uint"),
        ("decimals()", "uint"),
        ("maxDeposit(address)", "uint"),
        ("maxMint(address)", "uint"),
    ] {
        if let Some(selector) = method_selector(signature) {
            let argument = signature
                .contains("address)")
                .then(|| encode_address(&address));
            state.insert(
                signature.into(),
                probe_call(
                    &rpc,
                    &address,
                    selector,
                    argument.as_deref(),
                    input.block_number,
                    kind,
                ),
            );
        }
    }
    let asset = state
        .get("asset()")
        .and_then(|value| value.decoded.as_deref())
        .map(str::to_owned);
    if let Some(asset) = asset.as_deref() {
        state.insert(
            "asset.balanceOf(vault)".into(),
            probe_call(
                &rpc,
                asset,
                "70a08231",
                Some(&encode_address(&address)),
                input.block_number,
                "uint",
            ),
        );
        state.insert(
            "asset.decimals()".into(),
            probe_call(&rpc, asset, "313ce567", None, input.block_number, "uint"),
        );
        match rpc.code(asset, input.block_number) {
            Ok(code) => match hex_bytes(&code) {
                Ok(bytes) => {
                    state.insert(
                        "asset.code_sha256".into(),
                        ProbeValue {
                            raw_hex: None,
                            decoded: Some(sha256(&bytes)),
                            error: None,
                        },
                    );
                }
                Err(error) => {
                    state.insert(
                        "asset.code_sha256".into(),
                        ProbeValue {
                            raw_hex: None,
                            decoded: None,
                            error: Some(error),
                        },
                    );
                }
            },
            Err(error) => {
                state.insert(
                    "asset.code_sha256".into(),
                    ProbeValue {
                        raw_hex: None,
                        decoded: None,
                        error: Some(error),
                    },
                );
            }
        }
    }
    let number = |key: &str| {
        state
            .get(key)
            .and_then(|value| value.raw_hex.as_deref())
            .and_then(as_u128)
    };
    let mut relations = Vec::new();
    if let (Some(total), Some(balance)) =
        (number("totalAssets()"), number("asset.balanceOf(vault)"))
    {
        relations.push(format!(
            "At block {}, totalAssets={} and asset.balanceOf(vault)={}; equal={}",
            input.block_number,
            total,
            balance,
            total == balance
        ));
    }
    if let (Some(cap), Some(total), Some(max)) = (
        number("depositCap()"),
        number("totalAssets()"),
        number("maxDeposit(address)"),
    ) {
        let expected = if cap == u128::MAX {
            None
        } else {
            Some(cap.saturating_sub(total))
        };
        if let Some(expected) = expected {
            relations.push(format!(
                "maxDeposit={} versus max(cap-totalAssets,0)={}; equal={}",
                max,
                expected,
                max == expected
            ));
        }
    }
    if let (Some(vault_decimals), Some(asset_decimals)) =
        (number("decimals()"), number("asset.decimals()"))
    {
        relations.push(format!(
            "vault decimals={} and asset decimals={}; observed offset={}",
            vault_decimals,
            asset_decimals,
            vault_decimals.saturating_sub(asset_decimals)
        ));
    }
    let unit = number("asset.decimals()")
        .and_then(|value| u32::try_from(value).ok())
        .and_then(|decimals| 10_u128.checked_pow(decimals));
    let mut samples = BTreeSet::from([0_u128, 1_u128, 2_u128, 3_u128, 7_u128, 10_u128]);
    if let Some(unit) = unit {
        samples.insert(unit.saturating_sub(1));
        samples.insert(unit);
        samples.insert(unit.saturating_add(1));
    }
    if let Some(headroom) = number("maxDeposit(address)") {
        if headroom <= 1_000_000_000_000_000_000 {
            samples.insert(headroom.saturating_sub(1));
            samples.insert(headroom);
            samples.insert(headroom.saturating_add(1));
        }
    }
    if let Some(total) = number("totalAssets()") {
        if total <= 1_000_000_000_000_000_000 {
            samples.insert(total);
        }
    }
    let mut probes = Vec::new();
    for amount in samples {
        for signature in [
            "previewDeposit(uint256)",
            "previewMint(uint256)",
            "previewWithdraw(uint256)",
            "previewRedeem(uint256)",
            "convertToShares(uint256)",
            "convertToAssets(uint256)",
        ] {
            let Some(selector) = method_selector(signature) else {
                continue;
            };
            let name = function_name(signature);
            let related_score = initial_hotspots
                .iter()
                .filter(|item| function_name(&item.function) == name)
                .map(|item| item.priority_score)
                .max()
                .unwrap_or(0);
            probes.push(BoundaryProbe {
                method: signature.into(),
                input: amount.to_string(),
                related_hotspot_score: related_score,
                result: probe_call(
                    &rpc,
                    &address,
                    selector,
                    Some(&encode_u256(amount)),
                    input.block_number,
                    "uint",
                ),
            });
        }
    }
    probes.sort_by(|a, b| {
        b.related_hotspot_score
            .cmp(&a.related_hotspot_score)
            .then_with(|| a.method.cmp(&b.method))
            .then_with(|| a.input.cmp(&b.input))
    });
    relations.extend(monotonicity_observations(&probes).0);
    for amount in [1_u128, unit.unwrap_or(1)] {
        let get = |method: &str| {
            probes
                .iter()
                .find(|probe| probe.method == method && probe.input == amount.to_string())
                .and_then(|probe| probe.result.raw_hex.as_deref())
                .and_then(as_u128)
        };
        if let (Some(preview), Some(convert)) = (
            get("previewDeposit(uint256)"),
            get("convertToShares(uint256)"),
        ) {
            relations.push(format!("at input {amount}, previewDeposit={preview} and convertToShares={convert}; equal={}", preview == convert));
        }
        if let (Some(preview), Some(convert)) = (
            get("previewWithdraw(uint256)"),
            get("convertToShares(uint256)"),
        ) {
            relations.push(format!("at input {amount}, previewWithdraw={preview} and convertToShares={convert}; ceil>=floor={}", preview >= convert));
        }
        if let (Some(preview), Some(convert)) =
            (get("previewMint(uint256)"), get("convertToAssets(uint256)"))
        {
            relations.push(format!("at input {amount}, previewMint={preview} and convertToAssets={convert}; ceil>=floor={}", preview >= convert));
        }
    }
    let history = event_history(&rpc, &address, input.block_number);
    let (historical_observations, _) = historical_relations(&history, &state);
    relations.extend(historical_observations);
    let quarantine_cases = quarantine_cases(
        &initial_hotspots,
        &functions,
        &analysis,
        &state,
        &probes,
        &history,
    );
    let (mut hotspots, reassessment) = reevaluate_map(
        &functions,
        &comparison,
        &input.contract_name,
        &economic.slither_signals,
        &quarantine_cases,
    );
    let mut unresolved = Vec::new();
    if source_name != input.comparison_source_name {
        unresolved.push("Analysis source is a transformed single-file view; runtime identity is established by the separate compiler bundle, not by these source bytes".into());
    }
    if !history.complete_from_deployment {
        unresolved.push("Event chronology does not cover every block since deployment".into());
    }
    if let Some(error) = &history.error {
        unresolved.push(format!("Event history incomplete: {error}"));
    }
    if economic.candidates.is_empty() {
        unresolved.push("M4 produced no candidate; hotspot priority remains available and does not imply safety".into());
    }
    if rpc_fallback_used {
        unresolved.push(format!("primary RPC failed fixed-block verification; selected {rpc_host} after {rpc_failures} failed endpoint(s)"));
    }
    if comparison.unmatched_source_methods.len() > 0
        || comparison.unmatched_bytecode_selectors.len() > 0
    {
        unresolved.push("Not all method selectors crosswalked between source and bytecode".into());
    }
    for item in &mut hotspots {
        if item.selector.is_none() && item.function.starts_with('_') {
            item.investigation_questions.push("Internal function has no independent ABI entry PC; inspect reachable callers and call paths".into());
        }
    }
    Ok(ForensicReport {
        schema: SCHEMA.into(),
        identity: ForensicIdentity {
            chain_id: chain,
            address,
            block_number: input.block_number,
            block_hash,
            rpc_host,
            rpc_fallback_used,
            runtime_sha256: sha256(&runtime_bytes),
            analysis_source_sha256: sha256(source.as_bytes()),
            comparison_json_sha256: sha256(comparison_json.as_bytes()),
            exact_onchain_runtime: true,
            exact_compiler_runtime: comparison.normalized_runtime_match == Some(true),
            immutable_masked_compiler_runtime: comparison.immutable_masked_runtime_match
                == Some(true),
            comparison_source_name: input.comparison_source_name.clone(),
            analysis_source_name: source_name.into(),
            analysis_source_is_verified_bundle: source_name == input.comparison_source_name,
        },
        source_function_count: functions.len(),
        bytecode_selector_count: analysis.selectors.len(),
        bytecode_unresolved_count: analysis.unresolved.len(),
        compiler_comparison: comparison,
        economic_candidate_count: economic.candidates.len(),
        slither_detector_counts: detector_counts,
        slither_unmapped_count: economic
            .slither_signals
            .iter()
            .filter(|signal| !signal.mapped_to_candidate)
            .count(),
        hotspots,
        quarantine_cases,
        reassessment,
        onchain_state: state,
        boundary_probes: probes,
        observed_relations: relations,
        event_history: history,
        unresolved,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BoundaryProbe, ProbeValue, earliest_available_block, hotspot_map, numeric_anomalies,
        quarantine_focus,
    };
    use crate::{
        BytecodeSourceComparison, Confidence, ContractId, EconomicFunction, EconomicOperation,
        Evidence, EvidenceLocation, FunctionId, ResolvedFunction, SourceSpan, StorageId,
    };
    use std::collections::BTreeMap;

    #[test]
    fn maps_storage_through_internal_call_and_chooses_supply_for_quarantine() {
        let location = EvidenceLocation::Source(SourceSpan::new("Target.sol", 0, 10).unwrap());
        let mut entry = EconomicFunction::new(
            ContractId("Target".into()),
            FunctionId("deposit(uint256,address)".into()),
            Evidence {
                value: EconomicOperation::Deposit,
                location: location.clone(),
                confidence: Confidence::Direct,
            },
        );
        entry.resolved_calls.push(Evidence {
            value: ResolvedFunction {
                contract: ContractId("Target".into()),
                function: FunctionId("_deposit()".into()),
            },
            location: location.clone(),
            confidence: Confidence::Inferred,
        });
        let mut internal = EconomicFunction::new(
            ContractId("Target".into()),
            FunctionId("_deposit()".into()),
            Evidence {
                value: EconomicOperation::Other,
                location: location.clone(),
                confidence: Confidence::Direct,
            },
        );
        internal.writes.push(Evidence {
            value: StorageId("_totalSupply".into()),
            location,
            confidence: Confidence::Direct,
        });
        let comparison = BytecodeSourceComparison {
            source_name: "Target.sol".into(),
            contract_name: "Target".into(),
            normalized_runtime_match: Some(true),
            immutable_masked_runtime_match: None,
            immutable_values: Vec::new(),
            matched_methods: Vec::new(),
            unmatched_source_methods: Vec::new(),
            unmatched_bytecode_selectors: Vec::new(),
            unresolved: Vec::new(),
        };
        let map = hotspot_map(&[entry, internal], &comparison, "Target", &[]);
        let deposit = map
            .iter()
            .find(|item| item.function.starts_with("deposit("))
            .unwrap();
        assert!(deposit.transitive_writes.contains(&"_totalSupply".into()));
        assert_eq!(quarantine_focus(deposit), "_totalSupply");
    }

    #[test]
    fn parser_recovers_pruned_history_floor() {
        assert_eq!(
            earliest_available_block(
                "pruned history unavailable: requested 1, earliest available 11934576\"}"
            ),
            Some(11934576)
        );
    }

    #[test]
    fn positive_zero_share_preview_stays_anomaly() {
        let probes = vec![BoundaryProbe {
            method: "previewDeposit(uint256)".into(),
            input: "1".into(),
            related_hotspot_score: 40,
            result: ProbeValue {
                raw_hex: Some(format!("0x{:064x}", 0)),
                decoded: Some("0".into()),
                error: None,
            },
        }];
        let anomalies = numeric_anomalies(&BTreeMap::new(), &probes);
        assert_eq!(anomalies.len(), 1);
        assert!(anomalies[0].contains("zero shares"));
    }
}
