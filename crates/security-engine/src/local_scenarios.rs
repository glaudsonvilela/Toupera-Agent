//! Bounded, local-only vault scenarios on an Anvil fork of a verified block.
//! Transactions are sent exclusively to the child Anvil process on 127.0.0.1.

use std::collections::BTreeSet;
use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use alloy_primitives::U256;
use reqwest::blocking::Client;
use serde::Serialize;
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

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioState {
    pub total_assets: String,
    pub total_supply: String,
    pub vault_asset_balance: String,
    pub actor_asset_balance: String,
    pub actor_share_balance: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioCase {
    pub sequence: String,
    pub step: usize,
    pub operation: String,
    pub amount: String,
    pub preview: Option<String>,
    pub status: String,
    pub transaction_hash: Option<String>,
    pub gas_used: Option<u64>,
    pub before: Option<ScenarioState>,
    pub after: Option<ScenarioState>,
    pub trace_steps: Option<usize>,
    pub trace_jump_sites: Option<usize>,
    pub observed_jump_sites: Vec<String>,
    pub observed_branch_outcomes: Vec<String>,
    pub discrepancies: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalScenarioReport {
    pub schema: String,
    pub identity: LocalScenarioIdentity,
    pub execution_scope: String,
    pub cases: Vec<ScenarioCase>,
    pub completed_transactions: usize,
    pub reverted_transactions: usize,
    pub rpc_failures: usize,
    pub unique_trace_jump_sites: usize,
    pub unique_branch_outcomes: usize,
    pub jump_sites_with_both_outcomes: usize,
    pub trace_limitations: Vec<String>,
    pub round_trip_asset_delta: Option<String>,
    pub round_trip_discrepancies: Vec<String>,
    pub limitations: Vec<String>,
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
    match operation {
        Operation::Deposit | Operation::Mint => [
            assets.to_string(),
            shares.to_string(),
            format!("-{assets}"),
            shares.to_string(),
        ],
        Operation::Withdraw | Operation::Redeem => [
            if assets == U256::ZERO {
                "0".into()
            } else {
                format!("-{assets}")
            },
            if shares == U256::ZERO {
                "0".into()
            } else {
                format!("-{shares}")
            },
            assets.to_string(),
            if shares == U256::ZERO {
                "0".into()
            } else {
                format!("-{shares}")
            },
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

fn trace_steps(rpc: &Rpc, hash: &str) -> Result<(usize, Vec<String>, Vec<String>), String> {
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
    for log in logs {
        if log.get("op").and_then(Value::as_str) == Some("JUMPI") {
            let depth = log.get("depth").and_then(Value::as_u64).unwrap_or(0);
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
    Ok((
        logs.len(),
        jumps.into_iter().collect(),
        outcomes.into_iter().collect(),
    ))
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
) -> ScenarioCase {
    let mut case = ScenarioCase {
        sequence,
        step,
        operation: operation.name().into(),
        amount: amount.to_string(),
        preview: None,
        status: "not_started".into(),
        transaction_hash: None,
        gas_used: None,
        before: None,
        after: None,
        trace_steps: None,
        trace_jump_sites: None,
        observed_jump_sites: Vec::new(),
        observed_branch_outcomes: Vec::new(),
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
    match trace_steps(rpc, &hash) {
        Ok((steps, jumps, outcomes)) => {
            case.trace_steps = Some(steps);
            case.trace_jump_sites = Some(jumps.len());
            case.observed_jump_sites = jumps;
            case.observed_branch_outcomes = outcomes;
        }
        Err(error) => case
            .limitations
            .push(format!("local trace unavailable: {error}")),
    }
    match read_state(rpc, vault, asset, actor) {
        Ok(after) => {
            if case.status == "completed" {
                if let Some(preview) = preview {
                    case.discrepancies = compare_state(operation, amount, preview, &before, &after);
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
    let remote = Rpc::new(fork_rpc_url.to_owned())?;
    verify_rpc(&remote, &identity)?;
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
        U256::from(1),
        U256::from(7),
        asset_unit,
        asset_unit * U256::from(100),
    ];
    let share_amounts = [
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
    let completed_transactions = cases
        .iter()
        .filter(|case| case.status == "completed")
        .count();
    let reverted_transactions = cases
        .iter()
        .filter(|case| case.status == "reverted")
        .count();
    let rpc_failures = cases
        .iter()
        .filter(|case| case.status == "local_rpc_error" || case.status == "state_unavailable")
        .count();
    let jump_sites_with_both_outcomes = all_jumps
        .iter()
        .filter(|site| {
            all_outcomes.contains(&format!("{site}:taken"))
                && all_outcomes.contains(&format!("{site}:not_taken"))
        })
        .count();
    Ok(LocalScenarioReport {
        schema: SCHEMA.into(), identity,
        execution_scope: "child Anvil fork on 127.0.0.1; no transaction broadcast to source network".into(),
        cases, completed_transactions, reverted_transactions, rpc_failures,
        unique_trace_jump_sites: all_jumps.len(),
        unique_branch_outcomes: all_outcomes.len(),
        jump_sites_with_both_outcomes,
        trace_limitations,
        round_trip_asset_delta, round_trip_discrepancies,
        limitations: vec!["Jump sites are aggregate (depth,pc) positions across vault and asset calls, not source-level branch coverage or a completeness percentage".into(),"Scenario actor is the vault owner from the base report; results depend on that account's snapshot balances and approvals".into()],
    })
}
