//! Bounded, local-only vault scenarios on an Anvil fork of a verified block.
//! Transactions are sent exclusively to the child Anvil process on 127.0.0.1.

use std::collections::BTreeSet;
use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use crate::adaptive_chip::{AdaptiveChip, AdaptiveChipReport};
use alloy_primitives::U256;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as Sha2Digest, Sha256};
use sha3::{Digest as Sha3Digest, Keccak256};

const SCHEMA: &str = "local-scenarios-v1";

#[derive(Clone, Debug, Serialize)]
pub struct LocalScenarioIdentity {
    pub chain_id: u64,
    pub vault: String,
    pub asset: String,
    pub actor: String,
    pub block_number: u64,
    pub block_hash: String,
    pub runtime_sha256: String,
    pub base_report_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScenarioState {
    pub total_assets: String,
    pub total_supply: String,
    pub vault_asset_balance: String,
    pub actor_asset_balance: String,
    pub actor_share_balance: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScenarioCase {
    pub sequence: String,
    pub step: usize,
    pub actor: String,
    pub operation: String,
    pub amount: String,
    pub expectation: String,
    pub preview: Option<String>,
    pub preflight: String,
    pub preflight_detail: Option<String>,
    pub status: String,
    pub transaction_hash: Option<String>,
    pub gas_used: Option<u64>,
    pub before: Option<ScenarioState>,
    pub after: Option<ScenarioState>,
    pub trace_steps: Option<usize>,
    pub trace_jump_sites: Option<usize>,
    pub observed_jump_sites: Vec<String>,
    pub observed_branch_outcomes: Vec<String>,
    pub root_storage_reads: Vec<String>,
    pub root_storage_writes: Vec<String>,
    pub root_call_targets: Vec<String>,
    pub child_trace_steps: usize,
    pub discrepancies: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObservedSequenceTransition {
    pub sequence: String,
    pub from_step: usize,
    pub to_step: usize,
    pub actor: String,
    pub state_continuity: bool,
    pub changed_fields: Vec<String>,
    pub prior_writes_read_next: Vec<String>,
    pub evidence_class: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddressMappingSlotCandidate {
    pub key_address: String,
    pub base_slot: u64,
    pub observed_slot: String,
    pub read_cases: usize,
    pub write_cases: usize,
    pub evidence_class: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalScenarioReport {
    pub schema: String,
    pub identity: LocalScenarioIdentity,
    pub execution_scope: String,
    pub cases: Vec<ScenarioCase>,
    pub observed_sequence_transitions: Vec<ObservedSequenceTransition>,
    pub address_mapping_slot_candidates: Vec<AddressMappingSlotCandidate>,
    pub observed_root_call_targets: Vec<String>,
    pub cases_with_child_trace: usize,
    pub mapping_slot_search_exclusive_limit: u64,
    pub completed_transactions: usize,
    pub reverted_transactions: usize,
    pub rpc_failures: usize,
    pub preflight_completed: usize,
    pub preflight_rejected_or_unavailable: usize,
    pub preflight_transaction_mismatches: usize,
    pub unique_trace_jump_sites: usize,
    pub unique_branch_outcomes: usize,
    pub jump_sites_with_both_outcomes: usize,
    pub vault_runtime_jump_sites: usize,
    pub vault_jump_sites_observed: usize,
    pub vault_branch_outcomes_observed: usize,
    pub vault_jump_sites_with_both_outcomes: usize,
    pub trace_limitations: Vec<String>,
    pub round_trip_asset_delta: Option<String>,
    pub round_trip_discrepancies: Vec<String>,
    pub limitations: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance: Option<GuidedScenarioEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GuidedScenarioEvidence {
    pub variable_lens_sha256: String,
    pub prior_coverage_sha256: String,
    pub prior_root_outcome_count: usize,
    pub focus_variable_ast_ids: Vec<i64>,
    pub selected_families: Vec<String>,
    pub target_source_pcs: Vec<usize>,
    pub target_source_functions: Vec<String>,
    pub selection_basis: String,
    pub executed_sequences: Vec<String>,
    pub new_root_outcomes: Vec<String>,
    pub new_root_jump_sites: Vec<usize>,
    pub feedback_case_budget: usize,
    pub feedback_cases_executed: usize,
    pub feedback_stop_reason: Option<String>,
    pub source_boundary_cases: usize,
    pub preview_boundary_cases: usize,
    pub preview_boundary_queries: usize,
    pub state_order_sequences_executed: usize,
    pub state_order_sequence_budget: usize,
    pub state_order_stop_reason: Option<String>,
    pub adaptive_chip: Option<AdaptiveChipReport>,
}

struct Rpc {
    url: String,
    client: Client,
}

impl Rpc {
    fn new(url: String) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { url, client })
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let response = self
            .client
            .post(&self.url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .map_err(|error| format!("{method}: {error}"))?;
        let status = response.status();
        let body: Value = response
            .json()
            .map_err(|error| format!("{method}: invalid JSON response: {error}"))?;
        if !status.is_success() {
            return Err(format!("{method}: HTTP {status}: {body}"));
        }
        if let Some(error) = body.get("error") {
            return Err(format!("{method}: {error}"));
        }
        body.get("result")
            .cloned()
            .ok_or_else(|| format!("{method}: missing result"))
    }
}

struct AnvilChild(Child);

impl Drop for AnvilChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Deposit,
    Mint,
    Withdraw,
    Redeem,
}

impl Operation {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "deposit" => Some(Self::Deposit),
            "mint" => Some(Self::Mint),
            "withdraw" => Some(Self::Withdraw),
            "redeem" => Some(Self::Redeem),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Deposit => "deposit",
            Self::Mint => "mint",
            Self::Withdraw => "withdraw",
            Self::Redeem => "redeem",
        }
    }

    fn preview(self) -> &'static str {
        match self {
            Self::Deposit => "previewDeposit(uint256)",
            Self::Mint => "previewMint(uint256)",
            Self::Withdraw => "previewWithdraw(uint256)",
            Self::Redeem => "previewRedeem(uint256)",
        }
    }

    fn signature(self) -> &'static str {
        match self {
            Self::Deposit => "deposit(uint256,address)",
            Self::Mint => "mint(uint256,address)",
            Self::Withdraw => "withdraw(uint256,address,address)",
            Self::Redeem => "redeem(uint256,address,address)",
        }
    }
}

fn required_string<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {pointer} in base report"))
}

fn validate_address(address: &str) -> Result<String, String> {
    if address.len() != 42
        || !address.starts_with("0x")
        || !address[2..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(format!("invalid address {address}"));
    }
    Ok(address.to_ascii_lowercase())
}

fn hex_u64(value: &str) -> Result<u64, String> {
    u64::from_str_radix(value.trim_start_matches("0x"), 16).map_err(|error| error.to_string())
}

fn hex_u256(value: &str) -> Result<U256, String> {
    let bytes = hex::decode(value.trim_start_matches("0x")).map_err(|error| error.to_string())?;
    if bytes.len() != 32 {
        return Err("eth_call did not return one ABI uint256 word".into());
    }
    Ok(U256::from_be_slice(&bytes))
}

fn selector(signature: &str) -> String {
    hex::encode(&Keccak256::digest(signature.as_bytes())[..4])
}

fn word(value: U256) -> String {
    hex::encode(value.to_be_bytes::<32>())
}

fn address_word(address: &str) -> String {
    format!("{:0>64}", &address[2..])
}

fn address_mapping_slot(address: &str, base_slot: u64) -> Result<String, String> {
    let address = validate_address(address)?;
    let mut preimage = [0u8; 64];
    let key = hex::decode(&address[2..]).map_err(|error| error.to_string())?;
    preimage[12..32].copy_from_slice(&key);
    preimage[32..64].copy_from_slice(&U256::from(base_slot).to_be_bytes::<32>());
    Ok(format!("0x{}", hex::encode(Keccak256::digest(preimage))))
}

fn calldata(signature: &str, amount: Option<U256>, addresses: &[&str]) -> String {
    let mut data = format!("0x{}", selector(signature));
    if let Some(amount) = amount {
        data.push_str(&word(amount));
    }
    for address in addresses {
        data.push_str(&address_word(address));
    }
    data
}

fn view_u256(rpc: &Rpc, address: &str, data: &str) -> Result<U256, String> {
    let raw = rpc.call("eth_call", json!([{"to":address,"data":data},"latest"]))?;
    let raw = raw.as_str().ok_or("eth_call result is not hex")?;
    hex_u256(raw)
}

fn preview_change_boundary(
    rpc: &Rpc,
    vault: &str,
    operation: Operation,
    pivot: U256,
    window: U256,
) -> Result<(Option<U256>, usize), String> {
    if window == U256::ZERO {
        return Ok((None, 0));
    }
    let mut queries = 0_usize;
    let preview = |amount| {
        view_u256(
            rpc,
            vault,
            &calldata(operation.preview(), Some(amount), &[]),
        )
    };
    let base = preview(pivot)?;
    queries += 1;
    let upper = pivot.saturating_add(window);
    let upper_value = preview(upper)?;
    queries += 1;
    if upper_value <= base {
        return Ok((None, queries));
    }
    let mut low = pivot + U256::from(1);
    let mut high = upper;
    while low < high && queries < 40 {
        let mid = low + (high - low) / U256::from(2);
        let value = preview(mid)?;
        queries += 1;
        if value > base {
            high = mid;
        } else {
            low = mid + U256::from(1);
        }
    }
    if low != high {
        return Ok((None, queries));
    }
    let prior = preview(low - U256::from(1))?;
    let at = preview(low)?;
    queries += 2;
    Ok(((prior == base && at > base).then_some(low), queries))
}

fn read_state(rpc: &Rpc, vault: &str, asset: &str, actor: &str) -> Result<ScenarioState, String> {
    let total_assets = view_u256(rpc, vault, &calldata("totalAssets()", None, &[]))?;
    let total_supply = view_u256(rpc, vault, &calldata("totalSupply()", None, &[]))?;
    let vault_asset_balance =
        view_u256(rpc, asset, &calldata("balanceOf(address)", None, &[vault]))?;
    let actor_asset_balance =
        view_u256(rpc, asset, &calldata("balanceOf(address)", None, &[actor]))?;
    let actor_share_balance =
        view_u256(rpc, vault, &calldata("balanceOf(address)", None, &[actor]))?;
    Ok(ScenarioState {
        total_assets: total_assets.to_string(),
        total_supply: total_supply.to_string(),
        vault_asset_balance: vault_asset_balance.to_string(),
        actor_asset_balance: actor_asset_balance.to_string(),
        actor_share_balance: actor_share_balance.to_string(),
    })
}

fn signed_delta(before: &str, after: &str) -> Result<String, String> {
    let before = before.parse::<U256>().map_err(|error| error.to_string())?;
    let after = after.parse::<U256>().map_err(|error| error.to_string())?;
    Ok(if after >= before {
        (after - before).to_string()
    } else {
        format!("-{}", before - after)
    })
}

fn expected_deltas(operation: Operation, amount: U256, preview: U256) -> [String; 4] {
    let (assets, shares) = match operation {
        Operation::Deposit => (amount, preview),
        Operation::Mint => (preview, amount),
        Operation::Withdraw => (amount, preview),
        Operation::Redeem => (preview, amount),
    };
    let negative = |value: U256| {
        if value == U256::ZERO {
            "0".into()
        } else {
            format!("-{value}")
        }
    };
    match operation {
        Operation::Deposit | Operation::Mint => [
            assets.to_string(),
            shares.to_string(),
            negative(assets),
            shares.to_string(),
        ],
        Operation::Withdraw | Operation::Redeem => [
            negative(assets),
            negative(shares),
            assets.to_string(),
            negative(shares),
        ],
    }
}

fn compare_state(
    operation: Operation,
    amount: U256,
    preview: U256,
    before: &ScenarioState,
    after: &ScenarioState,
) -> Vec<String> {
    let expected = expected_deltas(operation, amount, preview);
    let fields = [
        (
            "vault asset balance",
            &before.vault_asset_balance,
            &after.vault_asset_balance,
            &expected[0],
        ),
        (
            "total supply",
            &before.total_supply,
            &after.total_supply,
            &expected[1],
        ),
        (
            "actor asset balance",
            &before.actor_asset_balance,
            &after.actor_asset_balance,
            &expected[2],
        ),
        (
            "actor share balance",
            &before.actor_share_balance,
            &after.actor_share_balance,
            &expected[3],
        ),
        (
            "totalAssets()",
            &before.total_assets,
            &after.total_assets,
            &expected[0],
        ),
    ];
    fields
        .iter()
        .filter_map(
            |(name, before, after, expected)| match signed_delta(before, after) {
                Ok(observed) if &observed == *expected => None,
                Ok(observed) => Some(format!(
                    "{name}: expected delta {expected}, observed {observed}"
                )),
                Err(error) => Some(format!("{name}: cannot compare state: {error}")),
            },
        )
        .collect()
}

fn send_local(rpc: &Rpc, from: &str, to: &str, data: &str) -> Result<(String, Value), String> {
    if !rpc.url.starts_with("http://127.0.0.1:") {
        return Err("state-changing RPC is restricted to child Anvil on 127.0.0.1".into());
    }
    let hash = rpc.call(
        "eth_sendTransaction",
        json!([{"from":from,"to":to,"data":data,"gas":"0x2dc6c0"}]),
    )?;
    let hash = hash
        .as_str()
        .ok_or("eth_sendTransaction returned no hash")?
        .to_owned();
    for _ in 0..40 {
        let receipt = rpc.call("eth_getTransactionReceipt", json!([hash]))?;
        if !receipt.is_null() {
            return Ok((hash, receipt));
        }
        sleep(Duration::from_millis(250));
    }
    Err(format!("local receipt unavailable for {hash}"))
}

fn snapshot(rpc: &Rpc) -> Result<String, String> {
    rpc.call("evm_snapshot", json!([]))?
        .as_str()
        .map(str::to_owned)
        .ok_or("local snapshot ID missing".into())
}

fn revert_snapshot(rpc: &Rpc, id: &str) -> Result<(), String> {
    if rpc.call("evm_revert", json!([id]))? != Value::Bool(true) {
        return Err("local fork snapshot could not be restored".into());
    }
    Ok(())
}

fn append_guided_case(
    cases: &mut Vec<ScenarioCase>,
    jumps: &mut BTreeSet<String>,
    outcomes: &mut BTreeSet<String>,
    trace_limitations: &mut Vec<String>,
    guidance: &mut GuidedScenarioEvidence,
    case: ScenarioCase,
) {
    jumps.extend(case.observed_jump_sites.iter().cloned());
    outcomes.extend(case.observed_branch_outcomes.iter().cloned());
    if case.trace_steps.is_none() {
        trace_limitations.push(format!(
            "{} step {}: local trace missing",
            case.sequence, case.step
        ));
    }
    if !guidance.executed_sequences.contains(&case.sequence) {
        guidance.executed_sequences.push(case.sequence.clone());
    }
    cases.push(case);
}

fn observed_root_outcomes(outcomes: &BTreeSet<String>, target_pcs: &[usize]) -> usize {
    outcomes
        .iter()
        .filter(|outcome| {
            let Some(rest) = outcome.strip_prefix("1:") else {
                return false;
            };
            let Some((pc, _)) = rest.split_once(':') else {
                return false;
            };
            target_pcs.is_empty() || pc.parse::<usize>().is_ok_and(|pc| target_pcs.contains(&pc))
        })
        .count()
}

struct TraceEvidence {
    steps: usize,
    jumps: Vec<String>,
    outcomes: Vec<String>,
    root_storage_reads: Vec<String>,
    root_storage_writes: Vec<String>,
    root_call_targets: Vec<String>,
    child_steps: usize,
}

fn trace_steps(rpc: &Rpc, hash: &str) -> Result<TraceEvidence, String> {
    let value = rpc.call(
        "debug_traceTransaction",
        json!([hash,{"disableMemory":true,"disableStack":false,"disableStorage":true}]),
    )?;
    let logs = value
        .get("structLogs")
        .and_then(Value::as_array)
        .ok_or("trace has no structLogs")?;
    let mut jumps = BTreeSet::new();
    let mut outcomes = BTreeSet::new();
    let mut root_storage_reads = BTreeSet::new();
    let mut root_storage_writes = BTreeSet::new();
    let mut root_call_targets = BTreeSet::new();
    let mut child_steps = 0_usize;
    for log in logs {
        let depth = log.get("depth").and_then(Value::as_u64).unwrap_or(0);
        if depth > 1 {
            child_steps += 1;
        }
        if depth == 1 {
            if let Some(kind) = log.get("op").and_then(Value::as_str) {
                if matches!(kind, "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE") {
                    if let Some(target) = log
                        .get("stack")
                        .and_then(Value::as_array)
                        .and_then(|stack| stack.iter().rev().nth(1))
                        .and_then(Value::as_str)
                        .and_then(|word| hex_u256_padded(word).ok())
                    {
                        let bytes = target.to_be_bytes::<32>();
                        root_call_targets.insert(format!("0x{}", hex::encode(&bytes[12..])));
                    }
                }
                if matches!(kind, "SLOAD" | "SSTORE") {
                    if let Some(slot) = log
                        .get("stack")
                        .and_then(Value::as_array)
                        .and_then(|stack| stack.last())
                        .and_then(Value::as_str)
                        .and_then(|word| hex_u256_padded(word).ok())
                    {
                        let slot = format!("0x{}", hex::encode(slot.to_be_bytes::<32>()));
                        if kind == "SLOAD" {
                            root_storage_reads.insert(slot);
                        } else {
                            root_storage_writes.insert(slot);
                        }
                    }
                }
            }
        }
        if log.get("op").and_then(Value::as_str) == Some("JUMPI") {
            let pc = log.get("pc").and_then(Value::as_u64).unwrap_or(0);
            jumps.insert(format!("{depth}:{pc}"));
            if let Some(stack) = log.get("stack").and_then(Value::as_array) {
                if stack.len() >= 2 {
                    if let Some(condition) = stack[stack.len() - 2].as_str() {
                        if let Ok(condition) = hex_u256_padded(condition) {
                            outcomes.insert(format!(
                                "{depth}:{pc}:{}",
                                if condition == U256::ZERO {
                                    "not_taken"
                                } else {
                                    "taken"
                                }
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(TraceEvidence {
        steps: logs.len(),
        jumps: jumps.into_iter().collect(),
        outcomes: outcomes.into_iter().collect(),
        root_storage_reads: root_storage_reads.into_iter().collect(),
        root_storage_writes: root_storage_writes.into_iter().collect(),
        root_call_targets: root_call_targets.into_iter().collect(),
        child_steps,
    })
}

fn hex_u256_padded(value: &str) -> Result<U256, String> {
    let raw = value.trim_start_matches("0x");
    if raw.is_empty() {
        return Err("empty EVM stack word".into());
    }
    let padded = if raw.len() % 2 == 0 {
        raw.to_owned()
    } else {
        format!("0{raw}")
    };
    let bytes = hex::decode(padded).map_err(|error| error.to_string())?;
    if bytes.len() > 32 {
        return Err("EVM stack word exceeds 32 bytes".into());
    }
    Ok(U256::from_be_slice(&bytes))
}

fn run_case(
    rpc: &Rpc,
    vault: &str,
    asset: &str,
    actor: &str,
    operation: Operation,
    amount: U256,
    sequence: String,
    step: usize,
    expectation: &str,
) -> ScenarioCase {
    let mut case = ScenarioCase {
        sequence,
        step,
        actor: actor.into(),
        operation: operation.name().into(),
        amount: amount.to_string(),
        expectation: expectation.into(),
        preview: None,
        preflight: "not_run".into(),
        preflight_detail: None,
        status: "not_started".into(),
        transaction_hash: None,
        gas_used: None,
        before: None,
        after: None,
        trace_steps: None,
        trace_jump_sites: None,
        observed_jump_sites: Vec::new(),
        observed_branch_outcomes: Vec::new(),
        root_storage_reads: Vec::new(),
        root_storage_writes: Vec::new(),
        root_call_targets: Vec::new(),
        child_trace_steps: 0,
        discrepancies: Vec::new(),
        limitations: Vec::new(),
    };
    let before = match read_state(rpc, vault, asset, actor) {
        Ok(value) => value,
        Err(error) => {
            case.status = "state_unavailable".into();
            case.limitations.push(error);
            return case;
        }
    };
    case.before = Some(before.clone());
    let preview_data = calldata(operation.preview(), Some(amount), &[]);
    let preview = match view_u256(rpc, vault, &preview_data) {
        Ok(value) => {
            case.preview = Some(value.to_string());
            Some(value)
        }
        Err(error) => {
            case.limitations
                .push(format!("preview unavailable: {error}"));
            None
        }
    };
    let data = match operation {
        Operation::Deposit | Operation::Mint => {
            calldata(operation.signature(), Some(amount), &[actor])
        }
        Operation::Withdraw | Operation::Redeem => {
            calldata(operation.signature(), Some(amount), &[actor, actor])
        }
    };
    match rpc.call(
        "eth_call",
        json!([{"from":actor,"to":vault,"data":data},"latest"]),
    ) {
        Ok(_) => case.preflight = "call_completed".into(),
        Err(error) => {
            case.preflight = "call_rejected_or_unavailable".into();
            case.preflight_detail = Some(error.chars().take(240).collect());
        }
    }
    let (hash, receipt) = match send_local(rpc, actor, vault, &data) {
        Ok(value) => value,
        Err(error) => {
            case.status = "local_rpc_error".into();
            case.limitations.push(error);
            return case;
        }
    };
    case.transaction_hash = Some(hash.clone());
    case.gas_used = receipt
        .get("gasUsed")
        .and_then(Value::as_str)
        .and_then(|raw| hex_u64(raw).ok());
    let receipt_status = receipt
        .get("status")
        .and_then(Value::as_str)
        .and_then(|raw| hex_u64(raw).ok());
    case.status = match receipt_status {
        Some(1) => "completed",
        Some(0) => "reverted",
        _ => "receipt_status_missing",
    }
    .into();
    if case.preflight == "call_completed" && case.status == "reverted" {
        case.discrepancies
            .push("preflight completed but local transaction reverted".into());
    } else if case.preflight == "call_rejected_or_unavailable" && case.status == "completed" {
        case.discrepancies
            .push("preflight rejected or was unavailable but local transaction completed".into());
    }
    if expectation == "above_reported_limit" && case.status == "completed" {
        case.discrepancies
            .push("operation completed above the reported limit; review limit semantics".into());
    }
    match trace_steps(rpc, &hash) {
        Ok(trace) => {
            case.trace_steps = Some(trace.steps);
            case.trace_jump_sites = Some(trace.jumps.len());
            case.observed_jump_sites = trace.jumps;
            case.observed_branch_outcomes = trace.outcomes;
            case.root_storage_reads = trace.root_storage_reads;
            case.root_storage_writes = trace.root_storage_writes;
            case.root_call_targets = trace.root_call_targets;
            case.child_trace_steps = trace.child_steps;
        }
        Err(error) => case
            .limitations
            .push(format!("local trace unavailable: {error}")),
    }
    match read_state(rpc, vault, asset, actor) {
        Ok(after) => {
            if case.status == "completed" {
                if let Some(preview) = preview {
                    case.discrepancies
                        .extend(compare_state(operation, amount, preview, &before, &after));
                } else {
                    case.limitations
                        .push("completed operation has no preview for exact comparison".into());
                }
            }
            case.after = Some(after);
        }
        Err(error) => case
            .limitations
            .push(format!("post-state unavailable: {error}")),
    }
    case
}

fn verify_rpc(rpc: &Rpc, identity: &LocalScenarioIdentity) -> Result<(), String> {
    let chain = rpc.call("eth_chainId", json!([]))?;
    let chain = hex_u64(chain.as_str().ok_or("chain ID is not hex")?)?;
    if chain != identity.chain_id {
        return Err(format!("chain ID mismatch: {chain}"));
    }
    let block_tag = format!("0x{:x}", identity.block_number);
    let block = rpc.call("eth_getBlockByNumber", json!([block_tag, false]))?;
    let hash = block
        .get("hash")
        .and_then(Value::as_str)
        .ok_or("fixed block hash missing")?;
    if !hash.eq_ignore_ascii_case(&identity.block_hash) {
        return Err("fixed block hash mismatch".into());
    }
    let code = rpc.call("eth_getCode", json!([identity.vault, block_tag]))?;
    let code = code.as_str().ok_or("fixed runtime missing")?;
    let bytes = hex::decode(code.trim_start_matches("0x")).map_err(|error| error.to_string())?;
    let digest = hex::encode(Sha256::digest(&bytes));
    if digest != identity.runtime_sha256 {
        return Err("fixed runtime hash mismatch".into());
    }
    let asset_raw = rpc.call(
        "eth_call",
        json!([{"to":identity.vault,"data":calldata("asset()",None,&[])},block_tag]),
    )?;
    let asset_raw = asset_raw.as_str().ok_or("asset() did not return hex")?;
    let asset_word =
        hex::decode(asset_raw.trim_start_matches("0x")).map_err(|error| error.to_string())?;
    if asset_word.len() != 32 || format!("0x{}", hex::encode(&asset_word[12..])) != identity.asset {
        return Err("asset() differs from the base report asset address".into());
    }
    Ok(())
}

pub fn run_local_scenarios(
    report_path: &Path,
    fork_rpc_url: &str,
    anvil_path: &Path,
) -> Result<LocalScenarioReport, String> {
    run_scenarios(report_path, None, None, None, fork_rpc_url, anvil_path)
}

pub fn run_guided_scenarios(
    report_path: &Path,
    lens_path: &Path,
    prior_coverage_path: &Path,
    fork_rpc_url: &str,
    anvil_path: &Path,
    previous_report_path: Option<&Path>,
) -> Result<LocalScenarioReport, String> {
    run_scenarios(
        report_path,
        Some(lens_path),
        Some(prior_coverage_path),
        previous_report_path,
        fork_rpc_url,
        anvil_path,
    )
}

fn run_scenarios(
    report_path: &Path,
    lens_path: Option<&Path>,
    prior_coverage_path: Option<&Path>,
    previous_report_path: Option<&Path>,
    fork_rpc_url: &str,
    anvil_path: &Path,
) -> Result<LocalScenarioReport, String> {
    let bytes =
        fs::read(report_path).map_err(|error| format!("cannot read base report: {error}"))?;
    let base_report_sha256 = hex::encode(Sha256::digest(&bytes));
    let base: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid base report: {error}"))?;
    if base.get("schema").and_then(Value::as_str) != Some("forensic-map-v1") {
        return Err("expected forensic-map-v1 base report".into());
    }
    if base
        .pointer("/identity/exact_onchain_runtime")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("base report lacks exact onchain runtime identity".into());
    }
    let (guidance, prior_outcomes, source_boundaries) = if let (Some(path), Some(coverage_path)) =
        (lens_path, prior_coverage_path)
    {
        let lens_bytes =
            fs::read(path).map_err(|error| format!("cannot read variable lens: {error}"))?;
        let lens: Value = serde_json::from_slice(&lens_bytes)
            .map_err(|error| format!("invalid variable lens: {error}"))?;
        if lens["schema"] != "variable-lens-v1"
            || lens["forensic_report_sha256"] != base_report_sha256
            || lens["identity"]["runtime_sha256"] != base["identity"]["runtime_sha256"]
            || lens["identity"]["block_hash"] != base["identity"]["block_hash"]
            || lens["identity"]["address"] != base["identity"]["address"]
        {
            return Err("variable lens and fixed base report identities differ".into());
        }
        let coverage_bytes = fs::read(coverage_path)
            .map_err(|error| format!("cannot read prior coverage: {error}"))?;
        let coverage: Value = serde_json::from_slice(&coverage_bytes)
            .map_err(|error| format!("invalid prior coverage: {error}"))?;
        let coverage_sha = hex::encode(Sha256::digest(&coverage_bytes));
        if coverage["schema"] != "local-coverage-v1"
            || lens["local_coverage_sha256"] != coverage_sha
            || coverage["identity"]["base_report_sha256"] != base_report_sha256
            || coverage["identity"]["runtime_sha256"] != base["identity"]["runtime_sha256"]
        {
            return Err("prior coverage does not match the variable lens and fixed report".into());
        }
        let mut prior_outcomes = BTreeSet::new();
        for branch in coverage["branch_observations"]
            .as_array()
            .ok_or("prior coverage has no branch observations")?
        {
            let pc = branch["pc"].as_u64().ok_or("prior branch PC is invalid")?;
            for outcome in branch["outcomes"]
                .as_array()
                .ok_or("prior branch outcomes are missing")?
            {
                let direction = outcome
                    .as_str()
                    .ok_or("prior branch direction is invalid")?;
                if !matches!(direction, "taken" | "not_taken") {
                    return Err("prior branch direction is invalid".into());
                }
                prior_outcomes.insert(format!("{pc}:{direction}"));
            }
        }
        let variables = lens["variables"]
            .as_array()
            .ok_or("variable lens has no variables")?;
        let mut ids = BTreeSet::new();
        let mut families = BTreeSet::new();
        for variable in variables {
            let name = variable["name"].as_str().unwrap_or("").to_ascii_lowercase();
            let typ = variable["type_name"].as_str().unwrap_or("");
            let Some(id) = variable["ast_id"].as_i64() else {
                continue;
            };
            if name.contains("cap") || name.contains("limit") {
                ids.insert(id);
                families.insert("reported_limits".to_owned());
            }
            if typ.starts_with("mapping(address") || name.contains("balance") {
                ids.insert(id);
                families.insert("alternate_actor".to_owned());
            }
            if name.contains("asset") || name.contains("supply") {
                ids.insert(id);
                families.insert("asset_state_change".to_owned());
            }
        }
        let mut target_pcs = BTreeSet::new();
        let mut target_functions = BTreeSet::new();
        if let Some(origins) = lens["source_coverage"]["pc_source_origins"].as_array() {
            for origin in origins {
                let function = origin["function"].as_str().unwrap_or("");
                let observed = origin["observed_outcomes"].as_array().map_or(0, Vec::len);
                if observed < 2
                    && matches!(
                        function,
                        "deposit"
                            | "mint"
                            | "withdraw"
                            | "redeem"
                            | "maxDeposit"
                            | "maxMint"
                            | "maxWithdraw"
                            | "maxRedeem"
                    )
                {
                    if let Some(pc) = origin["pc"].as_u64() {
                        target_pcs.insert(pc as usize);
                        target_functions.insert(function.to_owned());
                    }
                }
            }
        }
        if !target_functions.is_empty()
            && lens["state_transition_links"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        {
            families.insert("ordered_state".into());
        }
        let mut source_boundaries = Vec::new();
        for hint in lens["numeric_boundaries"].as_array().into_iter().flatten() {
            let Some(operation) = hint["function"].as_str().and_then(Operation::from_name) else {
                continue;
            };
            let Some(boundary) = hint["literal_value"]
                .as_str()
                .and_then(|text| text.parse::<U256>().ok())
            else {
                continue;
            };
            for (label, amount) in [
                ("below", boundary.saturating_sub(U256::from(1))),
                ("at", boundary),
                ("above", boundary.saturating_add(U256::from(1))),
            ] {
                source_boundaries.push((operation, amount, label.to_owned()));
            }
        }
        source_boundaries.sort_by(|a, b| a.0.name().cmp(b.0.name()).then_with(|| a.1.cmp(&b.1)));
        source_boundaries.dedup_by(|a, b| a.0.name() == b.0.name() && a.1 == b.1);
        source_boundaries.truncate(12);
        (Some(GuidedScenarioEvidence {
            variable_lens_sha256: hex::encode(Sha256::digest(&lens_bytes)),
            prior_coverage_sha256: coverage_sha,
            prior_root_outcome_count: prior_outcomes.len(),
            focus_variable_ast_ids: ids.into_iter().collect(),
            selected_families: families.into_iter().collect(),
            target_source_pcs: target_pcs.into_iter().collect(),
            target_source_functions: target_functions.into_iter().collect(),
            selection_basis: "Source variable names/types and one-sided or unobserved source-origin PCs select bounded local families; neither proves path reachability".into(),
            executed_sequences: Vec::new(),
            new_root_outcomes: Vec::new(),
            new_root_jump_sites: Vec::new(),
            feedback_case_budget: 16,
            feedback_cases_executed: 0,
            feedback_stop_reason: None,
            source_boundary_cases: 0,
            preview_boundary_cases: 0,
            preview_boundary_queries: 0,
            state_order_sequences_executed: 0,
            state_order_sequence_budget: 8,
            state_order_stop_reason: None,
            adaptive_chip: None,
        }), prior_outcomes, source_boundaries)
    } else if lens_path.is_none() && prior_coverage_path.is_none() {
        (None, BTreeSet::new(), Vec::new())
    } else {
        return Err("guided scenarios require both variable lens and prior coverage".into());
    };
    let chain_id = base
        .pointer("/identity/chain_id")
        .and_then(Value::as_u64)
        .ok_or("base chain ID missing")?;
    let block_number = base
        .pointer("/identity/block_number")
        .and_then(Value::as_u64)
        .ok_or("base block number missing")?;
    let identity = LocalScenarioIdentity {
        chain_id,
        vault: validate_address(required_string(&base, "/identity/address")?)?,
        asset: validate_address(required_string(&base, "/asset_flow_history/asset_address")?)?,
        actor: validate_address(required_string(&base, "/event_history/last_owner")?)?,
        block_number,
        block_hash: required_string(&base, "/identity/block_hash")?.to_ascii_lowercase(),
        runtime_sha256: required_string(&base, "/identity/runtime_sha256")?.to_owned(),
        base_report_sha256,
    };
    let mut chip = if guidance.is_some() {
        Some(AdaptiveChip::load(
            previous_report_path,
            prior_coverage_path.ok_or("guided prior coverage missing")?,
            &identity.base_report_sha256,
            &identity.runtime_sha256,
            &identity.block_hash,
            &prior_outcomes,
        )?)
    } else {
        if previous_report_path.is_some() {
            return Err("previous report requires guided scenarios".into());
        }
        None
    };
    let remote = Rpc::new(fork_rpc_url.to_owned())?;
    verify_rpc(&remote, &identity)?;
    let runtime_raw = remote.call(
        "eth_getCode",
        json!([identity.vault, format!("0x{:x}", block_number)]),
    )?;
    let runtime_hex = runtime_raw
        .as_str()
        .ok_or("remote vault runtime is not hex")?;
    let runtime_analysis =
        crate::reconstruct_bytecode_hex("local-scenarios-vault-runtime", runtime_hex)?;
    let vault_jump_pcs: BTreeSet<usize> = runtime_analysis
        .instructions
        .iter()
        .filter(|item| item.opcode == 0x57)
        .map(|item| item.pc)
        .collect();
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    drop(listener);
    let child = Command::new(anvil_path)
        .args([
            "--fork-url",
            fork_rpc_url,
            "--fork-block-number",
            &block_number.to_string(),
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--auto-impersonate",
            "--steps-tracing",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start Anvil: {error}"))?;
    let mut child = AnvilChild(child);
    let local = Rpc::new(format!("http://127.0.0.1:{port}"))?;
    let mut ready = false;
    for _ in 0..120 {
        if local.call("eth_chainId", json!([])).is_ok() {
            ready = true;
            break;
        }
        if child
            .0
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("Anvil exited before the local fork became available".into());
        }
        sleep(Duration::from_millis(250));
    }
    if !ready {
        return Err("Anvil local fork did not become available within 30 seconds".into());
    }
    verify_rpc(&local, &identity)?;
    local.call(
        "anvil_setBalance",
        json!([identity.actor, "0x56bc75e2d63100000"]),
    )?;
    local.call("anvil_impersonateAccount", json!([identity.actor]))?;
    let approve = calldata("approve(address,uint256)", None, &[&identity.vault]);
    let approve = format!("{approve}{}", word(U256::MAX));
    let (_, approval_receipt) = send_local(&local, &identity.actor, &identity.asset, &approve)?;
    if approval_receipt.get("status").and_then(Value::as_str) != Some("0x1") {
        return Err("local asset approval did not complete".into());
    }
    let baseline = read_state(&local, &identity.vault, &identity.asset, &identity.actor)?;
    let asset_unit = base
        .pointer("/onchain_state/asset.decimals()/decoded")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value <= 18)
        .map(|decimals| U256::from(10u64).pow(U256::from(decimals)))
        .unwrap_or(U256::from(1_000_000u64));
    let supply = baseline
        .total_supply
        .parse::<U256>()
        .map_err(|error| error.to_string())?;
    let mut cases = Vec::new();
    let mut trace_limitations = Vec::new();
    let mut all_jumps = BTreeSet::new();
    let mut all_outcomes = BTreeSet::new();
    let asset_amounts = [
        U256::ZERO,
        U256::from(1),
        U256::from(7),
        asset_unit,
        asset_unit * U256::from(100),
    ];
    let share_amounts = [
        U256::ZERO,
        U256::from(1),
        U256::from(7),
        U256::from(1_000_000),
        (supply / U256::from(1000)).max(U256::from(1)),
    ];
    for (operation, amounts) in [
        (Operation::Deposit, asset_amounts),
        (Operation::Mint, share_amounts),
        (Operation::Withdraw, asset_amounts),
        (Operation::Redeem, share_amounts),
    ] {
        for amount in amounts {
            let snapshot = local.call("evm_snapshot", json!([]))?;
            let snapshot = snapshot
                .as_str()
                .ok_or("local snapshot ID missing")?
                .to_owned();
            let case = run_case(
                &local,
                &identity.vault,
                &identity.asset,
                &identity.actor,
                operation,
                amount,
                format!("isolated:{}:{amount}", operation.name()),
                1,
                "state_comparison",
            );
            all_jumps.extend(case.observed_jump_sites.iter().cloned());
            all_outcomes.extend(case.observed_branch_outcomes.iter().cloned());
            if case.trace_steps.is_none() {
                trace_limitations.push(format!(
                    "{} {amount}: local trace missing",
                    operation.name()
                ));
            }
            cases.push(case);
            let reverted = local.call("evm_revert", json!([snapshot]))?;
            if reverted != Value::Bool(true) {
                return Err("local fork snapshot could not be restored".into());
            }
        }
    }
    let mut scenario_limitations = Vec::new();
    let mut limit_cases = Vec::new();
    match view_u256(
        &local,
        &identity.vault,
        &calldata("maxWithdraw(address)", None, &[&identity.actor]),
    ) {
        Ok(limit) => match limit.checked_add(U256::from(1)) {
            Some(above) => limit_cases.push((Operation::Withdraw, above)),
            None => scenario_limitations
                .push("maxWithdraw is uint256 max; no higher input exists".into()),
        },
        Err(error) => scenario_limitations.push(format!(
            "maxWithdraw unavailable for boundary sample: {error}"
        )),
    }
    match view_u256(
        &local,
        &identity.vault,
        &calldata("maxRedeem(address)", None, &[&identity.actor]),
    ) {
        Ok(shares) => match shares.checked_add(U256::from(1)) {
            Some(above) => limit_cases.push((Operation::Redeem, above)),
            None => {
                scenario_limitations.push("maxRedeem is uint256 max; no higher input exists".into())
            }
        },
        Err(error) => scenario_limitations.push(format!(
            "maxRedeem unavailable for boundary sample: {error}"
        )),
    }
    for (operation, amount) in limit_cases {
        let snapshot = local.call("evm_snapshot", json!([]))?;
        let snapshot = snapshot
            .as_str()
            .ok_or("local snapshot ID missing")?
            .to_owned();
        let case = run_case(
            &local,
            &identity.vault,
            &identity.asset,
            &identity.actor,
            operation,
            amount,
            format!("above_limit:{}:{amount}", operation.name()),
            1,
            "above_reported_limit",
        );
        all_jumps.extend(case.observed_jump_sites.iter().cloned());
        all_outcomes.extend(case.observed_branch_outcomes.iter().cloned());
        if case.trace_steps.is_none() {
            trace_limitations.push(format!(
                "{} boundary {amount}: local trace missing",
                operation.name()
            ));
        }
        cases.push(case);
        if local.call("evm_revert", json!([snapshot]))? != Value::Bool(true) {
            return Err("local fork boundary snapshot could not be restored".into());
        }
    }
    let round_trip_snapshot = snapshot(&local)?;
    let round_trip_before = read_state(&local, &identity.vault, &identity.asset, &identity.actor)?;
    let deposit = run_case(
        &local,
        &identity.vault,
        &identity.asset,
        &identity.actor,
        Operation::Deposit,
        asset_unit,
        "deposit_then_redeem".into(),
        1,
        "state_comparison",
    );
    all_jumps.extend(deposit.observed_jump_sites.iter().cloned());
    all_outcomes.extend(deposit.observed_branch_outcomes.iter().cloned());
    if deposit.trace_steps.is_none() {
        trace_limitations.push("deposit_then_redeem step 1: local trace missing".into());
    }
    let minted = deposit
        .before
        .as_ref()
        .zip(deposit.after.as_ref())
        .and_then(|(before, after)| {
            signed_delta(&before.actor_share_balance, &after.actor_share_balance).ok()
        })
        .and_then(|value| value.parse::<U256>().ok());
    cases.push(deposit);
    let mut round_trip_discrepancies = Vec::new();
    if let Some(minted) = minted.filter(|value| *value > U256::ZERO) {
        let redeem = run_case(
            &local,
            &identity.vault,
            &identity.asset,
            &identity.actor,
            Operation::Redeem,
            minted,
            "deposit_then_redeem".into(),
            2,
            "state_comparison",
        );
        all_jumps.extend(redeem.observed_jump_sites.iter().cloned());
        all_outcomes.extend(redeem.observed_branch_outcomes.iter().cloned());
        if redeem.trace_steps.is_none() {
            trace_limitations.push("deposit_then_redeem step 2: local trace missing".into());
        }
        cases.push(redeem);
    } else {
        round_trip_discrepancies
            .push("deposit did not mint observable shares; second step omitted".into());
    }
    let round_trip_after = read_state(&local, &identity.vault, &identity.asset, &identity.actor)?;
    let round_trip_asset_delta = Some(signed_delta(
        &round_trip_before.actor_asset_balance,
        &round_trip_after.actor_asset_balance,
    )?);
    if round_trip_asset_delta
        .as_deref()
        .is_some_and(|value| !value.starts_with('-') && value != "0")
    {
        round_trip_discrepancies.push(format!(
            "actor asset balance increased over deposit/redeem sequence by {}",
            round_trip_asset_delta.as_deref().unwrap_or("?")
        ));
    }
    revert_snapshot(&local, &round_trip_snapshot)?;
    let mut guidance = guidance;
    if let Some(focus) = guidance.as_mut() {
        chip.as_mut()
            .ok_or("guided chip missing")?
            .seed_baseline(&cases);
        if focus
            .selected_families
            .iter()
            .any(|family| family == "reported_limits")
        {
            for (operation, signature, available) in [
                (
                    Operation::Deposit,
                    "maxDeposit(address)",
                    baseline.actor_asset_balance.as_str(),
                ),
                (
                    Operation::Mint,
                    "maxMint(address)",
                    baseline.actor_asset_balance.as_str(),
                ),
                (
                    Operation::Withdraw,
                    "maxWithdraw(address)",
                    baseline.vault_asset_balance.as_str(),
                ),
                (
                    Operation::Redeem,
                    "maxRedeem(address)",
                    baseline.actor_share_balance.as_str(),
                ),
            ] {
                let limit = match view_u256(
                    &local,
                    &identity.vault,
                    &calldata(signature, None, &[&identity.actor]),
                ) {
                    Ok(limit) => limit,
                    Err(error) => {
                        scenario_limitations
                            .push(format!("{signature} unavailable for guided range: {error}"));
                        continue;
                    }
                };
                let budget = available
                    .parse::<U256>()
                    .map_err(|error| error.to_string())?;
                let required = if matches!(operation, Operation::Mint) && limit != U256::MAX {
                    view_u256(
                        &local,
                        &identity.vault,
                        &calldata("previewMint(uint256)", Some(limit), &[]),
                    )
                    .unwrap_or(U256::MAX)
                } else {
                    limit
                };
                if limit == U256::MAX || required > budget || limit == U256::ZERO {
                    scenario_limitations.push(format!("{signature} limit outside local actor budget; guided near-limit transactions omitted"));
                    continue;
                }
                for (label, amount) in [
                    ("below", limit - U256::from(1)),
                    ("at", limit),
                    ("above", limit + U256::from(1)),
                ] {
                    let id = snapshot(&local)?;
                    let case = run_case(
                        &local,
                        &identity.vault,
                        &identity.asset,
                        &identity.actor,
                        operation,
                        amount,
                        format!("guided_limit:{}:{label}", operation.name()),
                        1,
                        if label == "above" {
                            "above_reported_limit"
                        } else {
                            "state_comparison"
                        },
                    );
                    append_guided_case(
                        &mut cases,
                        &mut all_jumps,
                        &mut all_outcomes,
                        &mut trace_limitations,
                        focus,
                        case,
                    );
                    revert_snapshot(&local, &id)?;
                }
            }
        }
        if focus
            .selected_families
            .iter()
            .any(|family| family == "alternate_actor")
        {
            let id = snapshot(&local)?;
            let second = "0x000000000000000000000000000000000000beef";
            local.call("anvil_setBalance", json!([second, "0x56bc75e2d63100000"]))?;
            local.call("anvil_impersonateAccount", json!([second]))?;
            let transfer_amount = asset_unit * U256::from(2);
            let transfer = format!(
                "0x{}{}{}",
                selector("transfer(address,uint256)"),
                address_word(second),
                word(transfer_amount)
            );
            match send_local(&local, &identity.actor, &identity.asset, &transfer) {
                Ok((_, receipt)) if receipt["status"] == "0x1" => {
                    let approve = format!(
                        "{}{}",
                        calldata("approve(address,uint256)", None, &[&identity.vault]),
                        word(U256::MAX)
                    );
                    let (_, receipt) = send_local(&local, second, &identity.asset, &approve)?;
                    if receipt["status"] == "0x1" {
                        let deposit = run_case(
                            &local,
                            &identity.vault,
                            &identity.asset,
                            second,
                            Operation::Deposit,
                            asset_unit,
                            "guided_actor:deposit_then_redeem".into(),
                            1,
                            "state_comparison",
                        );
                        let minted = deposit
                            .before
                            .as_ref()
                            .zip(deposit.after.as_ref())
                            .and_then(|(before, after)| {
                                signed_delta(
                                    &before.actor_share_balance,
                                    &after.actor_share_balance,
                                )
                                .ok()
                            })
                            .and_then(|value| value.parse::<U256>().ok());
                        append_guided_case(
                            &mut cases,
                            &mut all_jumps,
                            &mut all_outcomes,
                            &mut trace_limitations,
                            focus,
                            deposit,
                        );
                        if let Some(minted) = minted.filter(|value| *value > U256::ZERO) {
                            let redeem = run_case(
                                &local,
                                &identity.vault,
                                &identity.asset,
                                second,
                                Operation::Redeem,
                                minted,
                                "guided_actor:deposit_then_redeem".into(),
                                2,
                                "state_comparison",
                            );
                            append_guided_case(
                                &mut cases,
                                &mut all_jumps,
                                &mut all_outcomes,
                                &mut trace_limitations,
                                focus,
                                redeem,
                            );
                        }
                    } else {
                        scenario_limitations
                            .push("second local actor approval did not complete".into());
                    }
                }
                Ok(_) => scenario_limitations
                    .push("local asset transfer to second actor did not complete".into()),
                Err(error) => scenario_limitations
                    .push(format!("second actor preparation unavailable: {error}")),
            }
            revert_snapshot(&local, &id)?;
        }
        if focus
            .selected_families
            .iter()
            .any(|family| family == "asset_state_change")
        {
            let id = snapshot(&local)?;
            let donation = format!(
                "0x{}{}{}",
                selector("transfer(address,uint256)"),
                address_word(&identity.vault),
                word(asset_unit)
            );
            match send_local(&local, &identity.actor, &identity.asset, &donation) {
                Ok((_, receipt)) if receipt["status"] == "0x1" => {
                    let deposit = run_case(
                        &local,
                        &identity.vault,
                        &identity.asset,
                        &identity.actor,
                        Operation::Deposit,
                        asset_unit,
                        "guided_asset_state:deposit_then_redeem".into(),
                        1,
                        "state_comparison",
                    );
                    let minted = deposit
                        .before
                        .as_ref()
                        .zip(deposit.after.as_ref())
                        .and_then(|(before, after)| {
                            signed_delta(&before.actor_share_balance, &after.actor_share_balance)
                                .ok()
                        })
                        .and_then(|value| value.parse::<U256>().ok());
                    append_guided_case(
                        &mut cases,
                        &mut all_jumps,
                        &mut all_outcomes,
                        &mut trace_limitations,
                        focus,
                        deposit,
                    );
                    if let Some(minted) = minted.filter(|value| *value > U256::ZERO) {
                        let redeem = run_case(
                            &local,
                            &identity.vault,
                            &identity.asset,
                            &identity.actor,
                            Operation::Redeem,
                            minted,
                            "guided_asset_state:deposit_then_redeem".into(),
                            2,
                            "state_comparison",
                        );
                        append_guided_case(
                            &mut cases,
                            &mut all_jumps,
                            &mut all_outcomes,
                            &mut trace_limitations,
                            focus,
                            redeem,
                        );
                    }
                }
                Ok(_) => {
                    scenario_limitations.push("local asset balance change did not complete".into())
                }
                Err(error) => scenario_limitations
                    .push(format!("local asset balance change unavailable: {error}")),
            }
            revert_snapshot(&local, &id)?;
        }
        if focus
            .selected_families
            .iter()
            .any(|family| family == "ordered_state")
        {
            let id = snapshot(&local)?;
            let withdraw = run_case(
                &local,
                &identity.vault,
                &identity.asset,
                &identity.actor,
                Operation::Withdraw,
                asset_unit,
                "guided_order:withdraw_then_deposit".into(),
                1,
                "state_comparison",
            );
            let completed = withdraw.status == "completed";
            append_guided_case(
                &mut cases,
                &mut all_jumps,
                &mut all_outcomes,
                &mut trace_limitations,
                focus,
                withdraw,
            );
            if completed {
                let deposit = run_case(
                    &local,
                    &identity.vault,
                    &identity.asset,
                    &identity.actor,
                    Operation::Deposit,
                    asset_unit,
                    "guided_order:withdraw_then_deposit".into(),
                    2,
                    "state_comparison",
                );
                append_guided_case(
                    &mut cases,
                    &mut all_jumps,
                    &mut all_outcomes,
                    &mut trace_limitations,
                    focus,
                    deposit,
                );
            }
            revert_snapshot(&local, &id)?;
        }
        // The chip compares normalized behavior, skips repeated inputs and
        // ranks bounded alternatives using previously observed transitions.
        let chip = chip.as_mut().ok_or("guided chip missing")?;
        let share_scale = (supply / U256::from(1000)).max(U256::from(1));
        let actor_budget = baseline
            .actor_asset_balance
            .parse::<U256>()
            .unwrap_or(U256::ZERO);
        let share_budget = (supply / U256::from(100)).max(U256::from(1));
        for (operation, amount, label) in &source_boundaries {
            if focus.feedback_cases_executed >= focus.feedback_case_budget || chip.plateau() >= 8 {
                break;
            }
            let available = if matches!(operation, Operation::Deposit | Operation::Withdraw) {
                actor_budget
            } else {
                share_budget
            };
            let scale = if matches!(operation, Operation::Deposit | Operation::Withdraw) {
                asset_unit
            } else {
                share_scale
            };
            if *amount > available || !chip.consider(operation.name(), *amount, scale) {
                continue;
            }
            let id = snapshot(&local)?;
            let case = run_case(
                &local,
                &identity.vault,
                &identity.asset,
                &identity.actor,
                *operation,
                *amount,
                format!("guided_source:{}:{label}:{amount}", operation.name()),
                1,
                "state_comparison",
            );
            chip.observe(&case, &focus.target_source_pcs);
            append_guided_case(
                &mut cases,
                &mut all_jumps,
                &mut all_outcomes,
                &mut trace_limitations,
                focus,
                case,
            );
            focus.feedback_cases_executed += 1;
            focus.source_boundary_cases += 1;
            revert_snapshot(&local, &id)?;
        }
        for operation in [Operation::Mint, Operation::Redeem] {
            if focus.feedback_cases_executed >= focus.feedback_case_budget || chip.plateau() >= 8 {
                break;
            }
            let pivot = share_scale;
            let window = (share_scale / U256::from(1000)).max(U256::from(2));
            match preview_change_boundary(&local, &identity.vault, operation, pivot, window) {
                Ok((Some(boundary), queries)) => {
                    focus.preview_boundary_queries += queries;
                    for amount in [
                        boundary.saturating_sub(U256::from(1)),
                        boundary,
                        boundary.saturating_add(U256::from(1)),
                    ] {
                        if focus.feedback_cases_executed >= focus.feedback_case_budget
                            || chip.plateau() >= 8
                        {
                            break;
                        }
                        if amount > share_budget
                            || !chip.consider(operation.name(), amount, share_scale)
                        {
                            continue;
                        }
                        let id = snapshot(&local)?;
                        let case = run_case(
                            &local,
                            &identity.vault,
                            &identity.asset,
                            &identity.actor,
                            operation,
                            amount,
                            format!("guided_preview:{}:{amount}", operation.name()),
                            1,
                            "state_comparison",
                        );
                        chip.observe(&case, &focus.target_source_pcs);
                        append_guided_case(
                            &mut cases,
                            &mut all_jumps,
                            &mut all_outcomes,
                            &mut trace_limitations,
                            focus,
                            case,
                        );
                        focus.feedback_cases_executed += 1;
                        focus.preview_boundary_cases += 1;
                        revert_snapshot(&local, &id)?;
                    }
                }
                Ok((None, queries)) => {
                    focus.preview_boundary_queries += queries;
                    scenario_limitations.push(format!(
                        "{} preview change not located in bounded local window",
                        operation.name()
                    ));
                }
                Err(error) => scenario_limitations.push(format!(
                    "{} preview observation unavailable: {error}",
                    operation.name()
                )),
            }
        }
        let candidates = chip.candidates(
            asset_unit,
            share_scale,
            &focus.target_source_functions,
            focus.feedback_case_budget.saturating_mul(4),
        );
        for (name, amount) in candidates {
            if focus.feedback_cases_executed >= focus.feedback_case_budget {
                focus.feedback_stop_reason = Some("case_budget_reached".into());
                break;
            }
            if chip.plateau() >= 8 {
                focus.feedback_stop_reason =
                    Some("eight_cases_without_new_behavior_or_target_outcome".into());
                break;
            }
            let available = if matches!(name.as_str(), "deposit" | "withdraw") {
                actor_budget
            } else {
                share_budget
            };
            if amount > available {
                continue;
            }
            let Some(operation) = Operation::from_name(&name) else {
                continue;
            };
            let id = snapshot(&local)?;
            let case = run_case(
                &local,
                &identity.vault,
                &identity.asset,
                &identity.actor,
                operation,
                amount,
                format!("guided_feedback:{}:{amount}", operation.name()),
                1,
                "state_comparison",
            );
            chip.observe(&case, &focus.target_source_pcs);
            append_guided_case(
                &mut cases,
                &mut all_jumps,
                &mut all_outcomes,
                &mut trace_limitations,
                focus,
                case,
            );
            focus.feedback_cases_executed += 1;
            revert_snapshot(&local, &id)?;
        }
        if focus.feedback_stop_reason.is_none() {
            focus.feedback_stop_reason = Some(
                if chip.plateau() >= 8 {
                    "eight_cases_without_new_behavior_or_target_outcome"
                } else if focus.feedback_cases_executed >= focus.feedback_case_budget {
                    "case_budget_reached"
                } else {
                    "no_more_distinct_candidates"
                }
                .into(),
            );
        }
        let planned_sequences = [
            (
                "state_deposit",
                None,
                Operation::Deposit,
                U256::ZERO,
                asset_unit + U256::from(1),
                asset_unit,
            ),
            (
                "order_withdraw_deposit",
                Some(Operation::Withdraw),
                Operation::Deposit,
                asset_unit,
                asset_unit + U256::from(1),
                U256::ZERO,
            ),
            (
                "state_mint",
                None,
                Operation::Mint,
                U256::ZERO,
                share_scale + U256::from(1),
                asset_unit,
            ),
            (
                "order_deposit_mint",
                Some(Operation::Deposit),
                Operation::Mint,
                asset_unit + U256::from(1),
                share_scale,
                U256::ZERO,
            ),
            (
                "state_withdraw",
                None,
                Operation::Withdraw,
                U256::ZERO,
                asset_unit,
                asset_unit + U256::from(1),
            ),
            (
                "order_mint_redeem",
                Some(Operation::Mint),
                Operation::Redeem,
                share_scale,
                share_scale,
                U256::ZERO,
            ),
            (
                "order_redeem_mint",
                Some(Operation::Redeem),
                Operation::Mint,
                share_scale,
                share_scale + U256::from(1),
                U256::ZERO,
            ),
            (
                "order_deposit_redeem",
                Some(Operation::Deposit),
                Operation::Redeem,
                asset_unit + U256::from(1),
                share_scale,
                U256::ZERO,
            ),
        ];
        let mut sequence_plateau = 0_usize;
        for (label, setup, target, setup_amount, target_amount, donation_amount) in
            planned_sequences
        {
            if focus.state_order_sequences_executed >= focus.state_order_sequence_budget {
                focus.state_order_stop_reason = Some("sequence_budget_reached".into());
                break;
            }
            if sequence_plateau >= 4 {
                focus.state_order_stop_reason =
                    Some("four_sequences_without_new_target_outcome".into());
                break;
            }
            let id = snapshot(&local)?;
            let before_count = observed_root_outcomes(&all_outcomes, &focus.target_source_pcs);
            let sequence = format!("guided_cycle:{label}");
            let mut ready = true;
            if donation_amount > U256::ZERO {
                let donation = format!(
                    "0x{}{}{}",
                    selector("transfer(address,uint256)"),
                    address_word(&identity.vault),
                    word(donation_amount)
                );
                ready = match send_local(&local, &identity.actor, &identity.asset, &donation) {
                    Ok((_, receipt)) => receipt["status"] == "0x1",
                    Err(error) => {
                        scenario_limitations.push(format!(
                            "{sequence} local state change unavailable: {error}"
                        ));
                        false
                    }
                };
            }
            if ready {
                if let Some(setup_operation) = setup {
                    let setup_case = run_case(
                        &local,
                        &identity.vault,
                        &identity.asset,
                        &identity.actor,
                        setup_operation,
                        setup_amount,
                        sequence.clone(),
                        1,
                        "state_comparison",
                    );
                    ready = setup_case.status == "completed";
                    append_guided_case(
                        &mut cases,
                        &mut all_jumps,
                        &mut all_outcomes,
                        &mut trace_limitations,
                        focus,
                        setup_case,
                    );
                }
            }
            if ready {
                let target_case = run_case(
                    &local,
                    &identity.vault,
                    &identity.asset,
                    &identity.actor,
                    target,
                    target_amount,
                    sequence,
                    if setup.is_some() { 2 } else { 1 },
                    "state_comparison",
                );
                append_guided_case(
                    &mut cases,
                    &mut all_jumps,
                    &mut all_outcomes,
                    &mut trace_limitations,
                    focus,
                    target_case,
                );
            }
            focus.state_order_sequences_executed += 1;
            revert_snapshot(&local, &id)?;
            sequence_plateau =
                if observed_root_outcomes(&all_outcomes, &focus.target_source_pcs) > before_count {
                    0
                } else {
                    sequence_plateau + 1
                };
        }
        if focus.state_order_stop_reason.is_none() {
            focus.state_order_stop_reason = Some("sequence_budget_reached".into());
        }
    }
    let completed_transactions = cases
        .iter()
        .filter(|case| case.status == "completed")
        .count();
    let observed_sequence_transitions = cases
        .windows(2)
        .filter_map(|pair| {
            let [previous, next] = pair else { return None };
            if previous.sequence != next.sequence
                || previous.actor != next.actor
                || previous.step.checked_add(1) != Some(next.step)
                || previous.status != "completed"
            {
                return None;
            }
            let (Some(before), Some(after), Some(next_before)) =
                (&previous.before, &previous.after, &next.before)
            else {
                return None;
            };
            let changed_fields = [
                ("total_assets", &before.total_assets, &after.total_assets),
                ("total_supply", &before.total_supply, &after.total_supply),
                (
                    "vault_asset_balance",
                    &before.vault_asset_balance,
                    &after.vault_asset_balance,
                ),
                (
                    "actor_asset_balance",
                    &before.actor_asset_balance,
                    &after.actor_asset_balance,
                ),
                (
                    "actor_share_balance",
                    &before.actor_share_balance,
                    &after.actor_share_balance,
                ),
            ]
            .into_iter()
            .filter(|(_, before, after)| before != after)
            .map(|(name, _, _)| name.into())
            .collect();
            Some(ObservedSequenceTransition {
                sequence: previous.sequence.clone(),
                from_step: previous.step,
                to_step: next.step,
                actor: previous.actor.clone(),
                state_continuity: after.total_assets == next_before.total_assets
                    && after.total_supply == next_before.total_supply
                    && after.vault_asset_balance == next_before.vault_asset_balance
                    && after.actor_asset_balance == next_before.actor_asset_balance
                    && after.actor_share_balance == next_before.actor_share_balance,
                changed_fields,
                prior_writes_read_next: previous
                    .root_storage_writes
                    .iter()
                    .filter(|slot| next.root_storage_reads.contains(slot))
                    .cloned()
                    .collect(),
                evidence_class: "observed_in_child_anvil_sequence".into(),
            })
        })
        .collect();
    const MAPPING_SLOT_SEARCH_LIMIT: u64 = 64;
    let actors: BTreeSet<_> = cases.iter().map(|case| case.actor.clone()).collect();
    let mut address_mapping_slot_candidates = Vec::new();
    for actor in actors {
        for base_slot in 0..MAPPING_SLOT_SEARCH_LIMIT {
            let observed_slot = address_mapping_slot(&actor, base_slot)?;
            let read_cases = cases
                .iter()
                .filter(|case| case.root_storage_reads.contains(&observed_slot))
                .count();
            let write_cases = cases
                .iter()
                .filter(|case| case.root_storage_writes.contains(&observed_slot))
                .count();
            if read_cases + write_cases > 0 {
                address_mapping_slot_candidates.push(AddressMappingSlotCandidate {
                    key_address: actor.clone(),
                    base_slot,
                    observed_slot,
                    read_cases,
                    write_cases,
                    evidence_class:
                        "root_slot_matches_address_mapping_formula_not_source_variable_identity"
                            .into(),
                });
            }
        }
    }
    let reverted_transactions = cases
        .iter()
        .filter(|case| case.status == "reverted")
        .count();
    let rpc_failures = cases
        .iter()
        .filter(|case| case.status == "local_rpc_error" || case.status == "state_unavailable")
        .count();
    let preflight_completed = cases
        .iter()
        .filter(|case| case.preflight == "call_completed")
        .count();
    let preflight_rejected_or_unavailable = cases
        .iter()
        .filter(|case| case.preflight == "call_rejected_or_unavailable")
        .count();
    let preflight_transaction_mismatches = cases
        .iter()
        .filter(|case| {
            case.discrepancies
                .iter()
                .any(|item| item.starts_with("preflight "))
        })
        .count();
    let observed_root_call_targets: Vec<String> = cases
        .iter()
        .flat_map(|case| case.root_call_targets.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let cases_with_child_trace = cases
        .iter()
        .filter(|case| case.child_trace_steps > 0)
        .count();
    let jump_sites_with_both_outcomes = all_jumps
        .iter()
        .filter(|site| {
            all_outcomes.contains(&format!("{site}:taken"))
                && all_outcomes.contains(&format!("{site}:not_taken"))
        })
        .count();
    let vault_observed_pcs: BTreeSet<usize> = all_jumps
        .iter()
        .filter_map(|site| {
            site.strip_prefix("1:")
                .and_then(|pc| pc.parse::<usize>().ok())
        })
        .filter(|pc| vault_jump_pcs.contains(pc))
        .collect();
    let vault_outcomes: BTreeSet<String> = all_outcomes
        .iter()
        .filter_map(|outcome| outcome.strip_prefix("1:"))
        .filter_map(|outcome| {
            let (pc, direction) = outcome.split_once(':')?;
            let pc = pc.parse::<usize>().ok()?;
            vault_jump_pcs
                .contains(&pc)
                .then(|| format!("{pc}:{direction}"))
        })
        .collect();
    let vault_jump_sites_with_both_outcomes = vault_observed_pcs
        .iter()
        .filter(|pc| {
            vault_outcomes.contains(&format!("{pc}:taken"))
                && vault_outcomes.contains(&format!("{pc}:not_taken"))
        })
        .count();
    if let Some(focus) = guidance.as_mut() {
        focus.new_root_outcomes = vault_outcomes
            .difference(&prior_outcomes)
            .cloned()
            .collect();
        let prior_pcs: BTreeSet<usize> = prior_outcomes
            .iter()
            .filter_map(|item| item.split_once(':').and_then(|(pc, _)| pc.parse().ok()))
            .collect();
        focus.new_root_jump_sites = vault_observed_pcs.difference(&prior_pcs).copied().collect();
        focus.adaptive_chip = chip.take().map(|chip| {
            chip.finish(
                focus
                    .feedback_stop_reason
                    .as_deref()
                    .unwrap_or("not_started"),
            )
        });
    }
    Ok(LocalScenarioReport {
        schema: SCHEMA.into(),
        identity,
        execution_scope:
            "child Anvil fork on 127.0.0.1; no transaction broadcast to source network".into(),
        cases,
        observed_sequence_transitions,
        address_mapping_slot_candidates,
        observed_root_call_targets,
        cases_with_child_trace,
        mapping_slot_search_exclusive_limit: MAPPING_SLOT_SEARCH_LIMIT,
        completed_transactions,
        reverted_transactions,
        rpc_failures,
        preflight_completed,
        preflight_rejected_or_unavailable,
        preflight_transaction_mismatches,
        unique_trace_jump_sites: all_jumps.len(),
        unique_branch_outcomes: all_outcomes.len(),
        jump_sites_with_both_outcomes,
        vault_runtime_jump_sites: vault_jump_pcs.len(),
        vault_jump_sites_observed: vault_observed_pcs.len(),
        vault_branch_outcomes_observed: vault_outcomes.len(),
        vault_jump_sites_with_both_outcomes,
        trace_limitations,
        round_trip_asset_delta,
        round_trip_discrepancies,
        limitations: {
            scenario_limitations.extend(["Aggregate (depth,pc) positions include vault and asset calls. Vault-only counts use depth 1 and exact runtime JUMPI PCs; the static denominator includes paths that may be unreachable, so this is not source-level or reachable-path coverage".into(),"Scenario actor is the vault owner from the base report; results depend on that account's snapshot balances and approvals".into()]);
            scenario_limitations
        },
        guidance,
    })
}
