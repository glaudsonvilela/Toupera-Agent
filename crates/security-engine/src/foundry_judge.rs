//! M7: local Foundry case preparation and a conservative reproduction gate.
//! A model suggestion is data, never executable Solidity.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::context_package::{ensure_private_cache, write_private};
use crate::prepare_review_plan;

const CASE_SCHEMA: &str = "m7-foundry-case-v1";
const RESULT_SCHEMA: &str = "m7-foundry-result-v1";
const TEST_FILE: &str = "test/Scenario.t.sol";
const REQUIRED_TESTS: [&str; 2] = ["testReproduce()", "testFuzzBoundaries(uint256)"];
const TEMPLATE: &str = r#"// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

// Replace this scaffold with a target-specific harness. Read case.json first.
// The assertion must fail on the expected safe behavior and pass only when the
// alleged behavior is observed. Use local contracts and meaningful inputs.
contract Scenario {
    function testReproduce() public pure {
        revert("TOUPERA_PENDING_HARNESS");
    }

    function testFuzzBoundaries(uint256) public pure {
        revert("TOUPERA_PENDING_HARNESS");
    }
}
"#;

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value).map_err(|error| format!("cannot encode M7 JSON: {error}"))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundryCase {
    pub schema: String,
    pub candidate_key: String,
    pub source_sha256: String,
    pub review_sha256: String,
    pub scaffold_sha256: String,
    pub hypothesis: String,
    pub assumptions: Vec<String>,
    pub relevant_paths: Vec<String>,
    pub suggested_test: String,
    pub required_tests: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HarnessApproval {
    pub reviewed: bool,
    pub reviewer_note: String,
    pub valid_calls_observed: bool,
    pub boundaries_covered: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct RejectionReview {
    pub reviewed: bool,
    pub reviewer: String,
    pub scope_reference: String,
    pub reason: String,
    pub assumptions_checked: bool,
    pub valid_inputs_checked: bool,
    pub failed_assertion_reviewed: bool,
    pub target_calls_reviewed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundryRejection {
    pub schema: String,
    pub candidate_key: String,
    pub status: String,
    pub source_sha256: String,
    pub harness_sha256: String,
    pub result_sha256: String,
    pub review_sha256: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FoundryPlan {
    pub schema: String,
    pub cases_created: usize,
    pub skipped_reviews: usize,
    pub case_directories: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundryResult {
    pub schema: String,
    pub candidate_key: String,
    pub state: String,
    pub source_sha256: String,
    pub harness_sha256: String,
    pub fuzz_seed: String,
    pub fuzz_runs: u32,
    pub runs: Vec<FoundryRun>,
    #[serde(default)]
    pub coverage: Option<FoundryCoverage>,
    pub caveat: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundryRun {
    pub run: u8,
    pub exit_success: bool,
    pub test_statuses: BTreeMap<String, String>,
    #[serde(default)]
    pub fuzz_runs_observed: Option<u64>,
    #[serde(default)]
    pub invariant_calls: Option<u64>,
    #[serde(default)]
    pub invariant_reverts: Option<u64>,
    pub json_path: String,
    pub trace_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundryCoverage {
    pub source: String,
    pub lines_found: u64,
    pub lines_hit: u64,
    pub branches_found: u64,
    pub branches_hit: u64,
    pub report_path: String,
    pub log_path: String,
    pub exit_success: bool,
}

fn get_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing M7 field {field}"))
}

/// Prepare private, inert cases. Never turn model text into Solidity.
pub fn prepare_foundry_cases(
    context: &Value,
    review: &Value,
    source_path: &Path,
    output_dir: &Path,
) -> Result<FoundryPlan, String> {
    if review.get("schema").and_then(Value::as_str) != Some("m6-review-v1") {
        return Err("M7 requires an m6-review-v1 report".into());
    }
    let package = context.get("package").unwrap_or(context);
    let expected_source = package
        .pointer("/digests/source_sha256")
        .and_then(Value::as_str)
        .ok_or("M5 package lacks source digest")?;
    let source = fs::read(source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let source_sha256 = digest(&source);
    if source_sha256 != expected_source {
        return Err("source bytes differ from the M5 package".into());
    }
    let plan = prepare_review_plan(context)?;
    let eligible: BTreeSet<_> = plan
        .requests
        .iter()
        .map(|item| item.candidate_key.as_str())
        .collect();
    let items = review
        .get("items")
        .and_then(Value::as_array)
        .ok_or("M6 report lacks items")?;
    let mut seen = BTreeSet::new();
    let mut cases = Vec::new();
    let mut skipped = 0;
    for item in items {
        let key = get_str(item, "candidate_key")?;
        if !seen.insert(key.to_owned()) {
            return Err(format!("duplicate M6 candidate key: {key}"));
        }
        if !eligible.contains(key) {
            return Err(format!(
                "M6 candidate key is absent from the M5 package: {key}"
            ));
        }
        let conclusion = item.get("conclusion").unwrap_or(&Value::Null);
        if item.get("status").and_then(Value::as_str) != Some("UNVERIFIED")
            || conclusion.get("disposition").and_then(Value::as_str) != Some("hypothesis")
            || item.get("error").is_some_and(|value| !value.is_null())
        {
            skipped += 1;
            continue;
        }
        if key.len() != 64 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("candidate key is not a SHA-256 hex digest".into());
        }
        let assumptions = conclusion["assumptions"]
            .as_array()
            .ok_or("hypothesis has no assumptions")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or("invalid assumption".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let relevant_paths = conclusion["relevant_paths"]
            .as_array()
            .ok_or("hypothesis has no relevant paths")?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or("invalid relevant path".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let hypothesis = get_str(conclusion, "hypothesis")?.trim();
        let suggested_test = get_str(conclusion, "suggested_test")?.trim();
        if hypothesis.is_empty()
            || suggested_test.is_empty()
            || assumptions.is_empty()
            || relevant_paths.is_empty()
            || assumptions.iter().any(|value| value.trim().is_empty())
            || relevant_paths.iter().any(|value| value.trim().is_empty())
        {
            return Err("M6 hypothesis is incomplete".into());
        }
        cases.push(FoundryCase {
            schema: CASE_SCHEMA.into(),
            candidate_key: key.to_owned(),
            source_sha256: source_sha256.clone(),
            review_sha256: digest(&json_bytes(item)?),
            scaffold_sha256: digest(TEMPLATE.as_bytes()),
            hypothesis: hypothesis.into(),
            assumptions,
            relevant_paths,
            suggested_test: suggested_test.into(),
            required_tests: REQUIRED_TESTS.iter().map(|value| (*value).into()).collect(),
        });
    }
    ensure_private_cache(output_dir)?;
    for case in &cases {
        let dir = output_dir.join(&case.candidate_key);
        if dir.exists() {
            return Err(format!("M7 case already exists: {}", dir.display()));
        }
    }
    let mut directories = Vec::new();
    for case in cases {
        let dir = output_dir.join(&case.candidate_key);
        ensure_private_cache(&dir)?;
        ensure_private_cache(&dir.join("src"))?;
        ensure_private_cache(&dir.join("test"))?;
        write_private(&dir.join("src/Target.sol"), &source)?;
        write_private(&dir.join(TEST_FILE), TEMPLATE.as_bytes())?;
        write_private(
            &dir.join("foundry.toml"),
            b"[profile.default]\nsrc = 'src'\ntest = 'test'\nout = 'out'\nlibs = []\nffi = false\nfs_permissions = []\n",
        )?;
        write_private(&dir.join("case.json"), &json_bytes(&case)?)?;
        write_private(
            &dir.join("harness.json"),
            &json_bytes(&HarnessApproval {
                reviewed: false,
                reviewer_note: String::new(),
                valid_calls_observed: false,
                boundaries_covered: false,
            })?,
        )?;
        write_private(
            &dir.join("rejection-review.json"),
            &json_bytes(&RejectionReview::default())?,
        )?;
        directories.push(dir.display().to_string());
    }
    Ok(FoundryPlan {
        schema: CASE_SCHEMA.into(),
        cases_created: directories.len(),
        skipped_reviews: skipped,
        case_directories: directories,
    })
}

fn read_private_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn require_absolute_executable(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() {
        return Err(format!(
            "binary path must be an absolute file: {}",
            path.display()
        ));
    }
    Ok(())
}

fn run_bounded(mut command: Command, timeout: Duration, output: &Path) -> Result<bool, String> {
    use std::fs::OpenOptions;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options
        .open(output)
        .map_err(|error| format!("cannot create {}: {error}", output.display()))?;
    let error_file = file
        .try_clone()
        .map_err(|error| format!("cannot clone output: {error}"))?;
    let mut child = command
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(error_file))
        .spawn()
        .map_err(|error| format!("cannot start forge: {error}"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("cannot wait for forge: {error}"))?
        {
            return Ok(status.success());
        }
        if start.elapsed() >= timeout {
            child
                .kill()
                .map_err(|error| format!("cannot stop timed-out forge: {error}"))?;
            child
                .wait()
                .map_err(|error| format!("cannot reap forge: {error}"))?;
            return Err("forge exceeded the 180-second case limit".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn test_results(json: &Value) -> BTreeMap<String, Value> {
    let mut found = BTreeMap::new();
    if let Some(suites) = json.as_object() {
        for (name, suite) in suites {
            if !name.ends_with("Scenario.t.sol:Scenario") {
                continue;
            }
            if let Some(results) = suite.get("test_results").and_then(Value::as_object) {
                for (name, result) in results {
                    found.insert(name.clone(), result.clone());
                }
            }
        }
    }
    found
}

fn statuses(results: &BTreeMap<String, Value>) -> BTreeMap<String, String> {
    results
        .iter()
        .filter_map(|(name, result)| {
            result
                .get("status")
                .and_then(Value::as_str)
                .map(|status| (name.clone(), status.into()))
        })
        .collect()
}

fn observed_metrics(results: &BTreeMap<String, Value>) -> (Option<u64>, Option<u64>, Option<u64>) {
    let fuzz_runs = results
        .get("testFuzzBoundaries(uint256)")
        .and_then(|result| result.pointer("/kind/Fuzz/runs"))
        .and_then(Value::as_u64);
    let mut calls = None;
    let mut reverts = None;
    for (name, result) in results {
        if !name.starts_with("invariant") {
            continue;
        }
        let Some(invariant) = result.pointer("/kind/Invariant") else {
            continue;
        };
        if let (Some(observed_calls), Some(observed_reverts)) = (
            invariant.get("calls").and_then(Value::as_u64),
            invariant.get("reverts").and_then(Value::as_u64),
        ) {
            calls = Some(calls.unwrap_or(0_u64).saturating_add(observed_calls));
            reverts = Some(reverts.unwrap_or(0_u64).saturating_add(observed_reverts));
        }
    }
    (fuzz_runs, calls, reverts)
}

pub(crate) fn target_coverage(
    lcov: &str,
    report_path: &Path,
    log_path: &Path,
    exit_success: bool,
) -> Option<FoundryCoverage> {
    let mut in_target = false;
    let mut target_seen = false;
    let mut lines = BTreeMap::<u64, u64>::new();
    let mut branches = BTreeMap::<String, bool>::new();
    for entry in lcov.lines() {
        if let Some(name) = entry.strip_prefix("SF:") {
            in_target = name == "Target.sol" || Path::new(name).ends_with("src/Target.sol");
            target_seen |= in_target;
            continue;
        }
        if !in_target {
            continue;
        }
        if let Some(data) = entry.strip_prefix("DA:") {
            let mut parts = data.split(',');
            if let (Some(number), Some(hits)) = (parts.next(), parts.next()) {
                if let (Ok(number), Ok(hits)) = (number.parse::<u64>(), hits.parse::<u64>()) {
                    let recorded = lines.entry(number).or_insert(0);
                    *recorded = (*recorded).max(hits);
                }
            }
        } else if let Some(data) = entry.strip_prefix("BRDA:") {
            let mut parts = data.split(',');
            if let (Some(line), Some(block), Some(branch), Some(taken)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            {
                let key = format!("{line}:{block}:{branch}");
                let hit = taken.parse::<u64>().is_ok_and(|count| count > 0);
                branches
                    .entry(key)
                    .and_modify(|value| *value |= hit)
                    .or_insert(hit);
            }
        } else if entry == "end_of_record" {
            in_target = false;
        }
    }
    if !target_seen {
        return None;
    }
    Some(FoundryCoverage {
        source: "src/Target.sol".into(),
        lines_found: lines.len() as u64,
        lines_hit: lines.values().filter(|hits| **hits > 0).count() as u64,
        branches_found: branches.len() as u64,
        branches_hit: branches.values().filter(|hit| **hit).count() as u64,
        report_path: report_path.display().to_string(),
        log_path: log_path.display().to_string(),
        exit_success,
    })
}

/// Execute only a reviewed local harness. Successful tests create a review
/// candidate, never a confirmed vulnerability or an automatic rejection.
pub fn judge_foundry_case(
    case_dir: &Path,
    forge_bin: &Path,
    solc_bin: &Path,
) -> Result<FoundryResult, String> {
    require_absolute_executable(forge_bin)?;
    require_absolute_executable(solc_bin)?;
    let case_dir = fs::canonicalize(case_dir)
        .map_err(|error| format!("cannot resolve M7 case directory: {error}"))?;
    ensure_private_cache(&case_dir)?;
    let case: FoundryCase = read_private_json(&case_dir.join("case.json"))?;
    if case.schema != CASE_SCHEMA || case.required_tests != REQUIRED_TESTS {
        return Err("unsupported M7 case schema or required tests".into());
    }
    let approval: HarnessApproval = read_private_json(&case_dir.join("harness.json"))?;
    if !approval.reviewed
        || approval.reviewer_note.trim().len() < 20
        || !approval.valid_calls_observed
        || !approval.boundaries_covered
    {
        return Err("harness needs a recorded review, valid calls and boundary coverage".into());
    }
    let source = fs::read(case_dir.join("src/Target.sol"))
        .map_err(|error| format!("cannot read target snapshot: {error}"))?;
    if digest(&source) != case.source_sha256 {
        return Err("target snapshot changed since M5".into());
    }
    let foundry_config = fs::read(case_dir.join("foundry.toml"))
        .map_err(|error| format!("cannot read Foundry configuration: {error}"))?;
    if foundry_config != b"[profile.default]\nsrc = 'src'\ntest = 'test'\nout = 'out'\nlibs = []\nffi = false\nfs_permissions = []\n" {
        return Err("Foundry configuration changed from the local-only template".into());
    }
    let harness = fs::read(case_dir.join(TEST_FILE))
        .map_err(|error| format!("cannot read Foundry harness: {error}"))?;
    let text = std::str::from_utf8(&harness).map_err(|_| "harness is not UTF-8")?;
    if digest(&harness) == case.scaffold_sha256 || text.contains("TOUPERA_PENDING_HARNESS") {
        return Err("Foundry harness is still a scaffold".into());
    }
    for signature in ["function testReproduce(", "function testFuzzBoundaries("] {
        if !text.contains(signature) {
            return Err(format!("harness is missing {signature}"));
        }
    }
    if ["createFork", "selectFork", "rpcUrl", ".ffi(", "vm.ffi("]
        .iter()
        .any(|needle| text.contains(needle))
    {
        return Err("harness contains a fork, RPC or FFI indicator".into());
    }
    let result_dir = case_dir.join("runs");
    if result_dir.exists() {
        return Err("M7 run directory already exists; keep prior evidence immutable".into());
    }
    ensure_private_cache(&result_dir)?;
    let seed = "0x5eed";
    let fuzz_runs = 256;
    let mut runs = Vec::new();
    for number in 1..=2_u8 {
        let json_path = result_dir.join(format!("run-{number}.json"));
        let trace_path = result_dir.join(format!("run-{number}.trace.txt"));
        let mut command = Command::new(forge_bin);
        command
            .arg("test")
            .arg("--offline")
            .arg("--use")
            .arg(solc_bin)
            .arg("--match-path")
            .arg(TEST_FILE)
            .arg("--fuzz-seed")
            .arg(seed)
            .arg("--fuzz-runs")
            .arg(fuzz_runs.to_string())
            .arg("--json-file")
            .arg(&json_path)
            .arg("-vvvv")
            .current_dir(&case_dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &case_dir)
            .env("FOUNDRY_FFI", "false")
            .env("FOUNDRY_OFFLINE", "true");
        let exit_success = run_bounded(command, Duration::from_secs(180), &trace_path)?;
        let (statuses, fuzz_runs_observed, invariant_calls, invariant_reverts) =
            match read_private_json::<Value>(&json_path) {
                Ok(json) => {
                    let results = test_results(&json);
                    let statuses = statuses(&results);
                    let (fuzz, calls, reverts) = observed_metrics(&results);
                    (statuses, fuzz, calls, reverts)
                }
                Err(_) => (BTreeMap::new(), None, None, None),
            };
        runs.push(FoundryRun {
            run: number,
            exit_success,
            test_statuses: statuses,
            fuzz_runs_observed,
            invariant_calls,
            invariant_reverts,
            json_path: json_path.display().to_string(),
            trace_path: trace_path.display().to_string(),
        });
    }
    let fuzz_passed = runs.iter().all(|run| {
        run.test_statuses
            .get("testFuzzBoundaries(uint256)")
            .is_some_and(|status| status == "Success")
            && run
                .fuzz_runs_observed
                .is_some_and(|observed| observed >= fuzz_runs as u64)
    });
    let tests_passed = runs.iter().all(|run| {
        run.exit_success
            && run.test_statuses.len() >= REQUIRED_TESTS.len()
            && run.test_statuses.values().all(|status| status == "Success")
            && REQUIRED_TESTS.iter().all(|name| {
                run.test_statuses
                    .get(*name)
                    .is_some_and(|status| status == "Success")
            })
    });
    let coverage = if fuzz_passed {
        let report_path = result_dir.join("coverage.info");
        let log_path = result_dir.join("coverage.trace.txt");
        let mut command = Command::new(forge_bin);
        command
            .arg("coverage")
            .arg("--report")
            .arg("lcov")
            .arg("--report-file")
            .arg(&report_path)
            .arg("--exclude-tests")
            .arg("--match-path")
            .arg(TEST_FILE)
            .arg("--match-test")
            .arg("^testFuzzBoundaries\\(uint256\\)$")
            .arg("--fuzz-seed")
            .arg(seed)
            .arg("--fuzz-runs")
            .arg(fuzz_runs.to_string())
            .arg("--offline")
            .arg("--use")
            .arg(solc_bin)
            .current_dir(&case_dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &case_dir)
            .env("FOUNDRY_FFI", "false")
            .env("FOUNDRY_OFFLINE", "true");
        let exit_success = run_bounded(command, Duration::from_secs(180), &log_path)?;
        fs::read_to_string(&report_path)
            .ok()
            .and_then(|lcov| target_coverage(&lcov, &report_path, &log_path, exit_success))
    } else {
        None
    };
    let reproduced = tests_passed
        && fuzz_passed
        && coverage
            .as_ref()
            .is_some_and(|metrics| metrics.exit_success && metrics.lines_hit > 0);
    let report = FoundryResult {
        schema: RESULT_SCHEMA.into(),
        candidate_key: case.candidate_key,
        state: if reproduced { "REPRODUCED_PENDING_REVIEW" } else { "INCONCLUSIVE" }.into(),
        source_sha256: case.source_sha256,
        harness_sha256: digest(&harness),
        fuzz_seed: seed.into(),
        fuzz_runs,
        runs,
        coverage,
        caveat: "Human review must verify that assertions demonstrate the alleged behavior; successful execution alone does not confirm a vulnerability. Failed execution does not reject a hypothesis.".into(),
    };
    write_private(&result_dir.join("result.json"), &json_bytes(&report)?)?;
    Ok(report)
}

/// Record a narrowly scoped, human-reviewed negative reproduction. A failing
/// test by itself is never evidence that a hypothesis is false.
pub fn reject_foundry_case(case_dir: &Path) -> Result<FoundryRejection, String> {
    let case_dir = fs::canonicalize(case_dir)
        .map_err(|error| format!("cannot resolve M7 case directory: {error}"))?;
    ensure_private_cache(&case_dir)?;
    let case: FoundryCase = read_private_json(&case_dir.join("case.json"))?;
    if case.schema != CASE_SCHEMA || case.required_tests != REQUIRED_TESTS {
        return Err("unsupported M7 case schema or required tests".into());
    }
    let source = fs::read(case_dir.join("src/Target.sol"))
        .map_err(|error| format!("cannot read target snapshot: {error}"))?;
    if digest(&source) != case.source_sha256 {
        return Err("target snapshot changed since M5".into());
    }
    let harness = fs::read(case_dir.join(TEST_FILE))
        .map_err(|error| format!("cannot read Foundry harness: {error}"))?;
    let result_path = case_dir.join("runs/result.json");
    let result_bytes =
        fs::read(&result_path).map_err(|error| format!("cannot read M7 result: {error}"))?;
    let result: FoundryResult = serde_json::from_slice(&result_bytes)
        .map_err(|error| format!("invalid M7 result: {error}"))?;
    if result.schema != RESULT_SCHEMA
        || result.state != "INCONCLUSIVE"
        || result.candidate_key != case.candidate_key
        || result.source_sha256 != case.source_sha256
        || result.harness_sha256 != digest(&harness)
        || result.runs.len() != 2
        || result.fuzz_runs < 256
    {
        return Err("M7 result does not match this case or a negative reproduction".into());
    }
    for (index, run) in result.runs.iter().enumerate() {
        let number = (index + 1) as u8;
        let json_path = case_dir.join(format!("runs/run-{number}.json"));
        let trace_path = case_dir.join(format!("runs/run-{number}.trace.txt"));
        if run.run != number
            || Path::new(&run.json_path) != json_path
            || Path::new(&run.trace_path) != trace_path
            || run.exit_success
            || !trace_path.is_file()
        {
            return Err(format!("M7 run {number} has inconsistent provenance"));
        }
        let raw: Value = read_private_json(&json_path)?;
        let tests = test_results(&raw);
        let actual = statuses(&tests);
        let (fuzz, calls, reverts) = observed_metrics(&tests);
        if actual != run.test_statuses
            || fuzz != run.fuzz_runs_observed
            || calls != run.invariant_calls
            || reverts != run.invariant_reverts
            || fuzz.is_none_or(|count| count < result.fuzz_runs as u64)
            || actual.get("testReproduce()") != Some(&"Failure".to_owned())
            || actual.get("testFuzzBoundaries(uint256)") != Some(&"Success".to_owned())
            || actual
                .iter()
                .any(|(name, status)| name != "testReproduce()" && status != "Success")
        {
            return Err(format!(
                "M7 run {number} is not a clean negative reproduction"
            ));
        }
    }
    let coverage = result
        .coverage
        .as_ref()
        .ok_or("negative case lacks coverage")?;
    let report_path = case_dir.join("runs/coverage.info");
    let log_path = case_dir.join("runs/coverage.trace.txt");
    if !coverage.exit_success
        || coverage.lines_hit == 0
        || Path::new(&coverage.report_path) != report_path
        || Path::new(&coverage.log_path) != log_path
        || !log_path.is_file()
    {
        return Err("negative case has no valid target coverage".into());
    }
    let lcov = fs::read_to_string(&report_path)
        .map_err(|error| format!("cannot read M7 coverage: {error}"))?;
    let observed = target_coverage(&lcov, &report_path, &log_path, true)
        .ok_or("coverage does not include target")?;
    if observed.lines_found != coverage.lines_found
        || observed.lines_hit != coverage.lines_hit
        || observed.branches_found != coverage.branches_found
        || observed.branches_hit != coverage.branches_hit
    {
        return Err("M7 coverage metrics differ from the LCOV artifact".into());
    }
    let review_path = case_dir.join("rejection-review.json");
    let review_bytes =
        fs::read(&review_path).map_err(|error| format!("cannot read rejection review: {error}"))?;
    let review: RejectionReview = serde_json::from_slice(&review_bytes)
        .map_err(|error| format!("invalid rejection review: {error}"))?;
    if !review.reviewed
        || review.reviewer.trim().is_empty()
        || review.scope_reference.trim().is_empty()
        || review.reason.trim().len() < 30
        || !review.assumptions_checked
        || !review.valid_inputs_checked
        || !review.failed_assertion_reviewed
        || !review.target_calls_reviewed
    {
        return Err("negative result requires a complete human review of scope, inputs, assumptions and assertion".into());
    }
    let rejection = FoundryRejection {
        schema: "m7-foundry-rejection-v1".into(),
        candidate_key: case.candidate_key,
        status: "REJECTED".into(),
        source_sha256: case.source_sha256,
        harness_sha256: digest(&harness),
        result_sha256: digest(&result_bytes),
        review_sha256: digest(&review_bytes),
        reason: review.reason,
    };
    write_private(
        &case_dir.join("runs/rejection.json"),
        &json_bytes(&rejection)?,
    )?;
    Ok(rejection)
}
