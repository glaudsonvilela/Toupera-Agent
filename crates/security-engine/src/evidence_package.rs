//! M8: portable evidence snapshots and an explicit human report gate.
//! The evidence plan is safe to create before tests exist; it cannot confirm a finding.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::context_package::{ensure_private_cache, write_private};
use crate::assurance::guard_wcop_job;
use crate::foundry_judge::target_coverage;
use crate::{FoundryCase, FoundryRejection, FoundryResult, RejectionReview};

const SCHEMA: &str = "m8-evidence-v1";
const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
const REQUIRED_TESTS: [&str; 2] = ["testReproduce()", "testFuzzBoundaries(uint256)"];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvidenceFact {
    pub statement: String,
    pub artifact: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EvidenceManifest {
    pub schema: String,
    pub candidate_key: String,
    pub stage: String,
    pub source_sha256: String,
    pub artifacts: BTreeMap<String, String>,
    pub facts: Vec<EvidenceFact>,
    pub assumptions: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HumanEvidenceReview {
    pub approved_for_report: bool,
    pub reviewer: String,
    pub reviewed_at_utc: String,
    pub scope_authorized: bool,
    pub scope_reference: String,
    pub target_version: String,
    pub environment: String,
    pub forge_version: String,
    pub solc_version: String,
    pub reproduction_steps: String,
    pub title: String,
    pub summary: String,
    pub impact: String,
    pub prerequisites: Vec<String>,
    pub inferences: Vec<String>,
    pub additional_limitations: Vec<String>,
    pub assertions_reviewed: bool,
    pub reproduction_reviewed: bool,
    pub evidence_reviewed: bool,
    pub duplicate_checked: bool,
    pub duplicate_note: String,
}

impl Default for HumanEvidenceReview {
    fn default() -> Self {
        Self {
            approved_for_report: false,
            reviewer: String::new(),
            reviewed_at_utc: String::new(),
            scope_authorized: false,
            scope_reference: String::new(),
            target_version: String::new(),
            environment: String::new(),
            forge_version: String::new(),
            solc_version: String::new(),
            reproduction_steps: String::new(),
            title: String::new(),
            summary: String::new(),
            impact: String::new(),
            prerequisites: Vec::new(),
            inferences: Vec::new(),
            additional_limitations: Vec::new(),
            assertions_reviewed: false,
            reproduction_reviewed: false,
            evidence_reviewed: false,
            duplicate_checked: false,
            duplicate_note: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct EvidenceReady {
    pub schema: String,
    pub status: String,
    pub candidate_key: String,
    pub source_sha256: String,
    pub manifest_sha256: String,
    pub review_sha256: String,
    pub report_sha256: String,
    pub report_path: String,
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot encode evidence JSON: {error}"))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid {}: {error}", path.display()))
}

fn copy_artifact(
    from: &Path,
    bundle: &Path,
    name: &str,
    artifacts: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(from)
        .map_err(|error| format!("cannot inspect {}: {error}", from.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_ARTIFACT_BYTES {
        return Err(format!(
            "evidence artifact must be a regular file <=64 MiB: {}",
            from.display()
        ));
    }
    let bytes =
        fs::read(from).map_err(|error| format!("cannot read {}: {error}", from.display()))?;
    let to = bundle.join(name);
    if let Some(parent) = to.parent() {
        ensure_private_cache(parent)?;
    }
    write_private(&to, &bytes)?;
    artifacts.insert(name.into(), digest(&bytes));
    Ok(())
}

fn foundry_statuses(
    json: &Value,
) -> (
    BTreeMap<String, String>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
) {
    let mut result = BTreeMap::new();
    let mut fuzz_runs = None;
    let mut invariant_calls = None;
    let mut invariant_reverts = None;
    if let Some(suites) = json.as_object() {
        for (name, suite) in suites {
            if !name.ends_with("Scenario.t.sol:Scenario") {
                continue;
            }
            if let Some(tests) = suite.get("test_results").and_then(Value::as_object) {
                for (test, value) in tests {
                    if let Some(status) = value.get("status").and_then(Value::as_str) {
                        result.insert(test.clone(), status.into());
                    }
                    if test == "testFuzzBoundaries(uint256)" {
                        fuzz_runs = value.pointer("/kind/Fuzz/runs").and_then(Value::as_u64);
                    }
                    if test.starts_with("invariant") {
                        if let (Some(calls), Some(reverts)) = (
                            value
                                .pointer("/kind/Invariant/calls")
                                .and_then(Value::as_u64),
                            value
                                .pointer("/kind/Invariant/reverts")
                                .and_then(Value::as_u64),
                        ) {
                            invariant_calls =
                                Some(invariant_calls.unwrap_or(0_u64).saturating_add(calls));
                            invariant_reverts =
                                Some(invariant_reverts.unwrap_or(0_u64).saturating_add(reverts));
                        }
                    }
                }
            }
        }
    }
    (result, fuzz_runs, invariant_calls, invariant_reverts)
}

fn validate_reproduction(
    bundle: &Path,
    manifest: &EvidenceManifest,
) -> Result<FoundryResult, String> {
    let report: FoundryResult = read_json(&bundle.join("runs/result.json"))?;
    if report.schema != "m7-foundry-result-v1"
        || report.candidate_key != manifest.candidate_key
        || report.source_sha256 != manifest.source_sha256
        || report.state != "REPRODUCED_PENDING_REVIEW"
        || report.runs.len() != 2
        || report.fuzz_runs == 0
    {
        return Err("M7 reproduction report is missing or inconsistent".into());
    }
    let harness = fs::read(bundle.join("test/Scenario.t.sol"))
        .map_err(|error| format!("cannot read bundled harness: {error}"))?;
    if digest(&harness) != report.harness_sha256 {
        return Err("M7 harness hash differs from bundled harness".into());
    }
    for (index, run) in report.runs.iter().enumerate() {
        let number = index + 1;
        let json_name = format!("runs/run-{number}.json");
        let trace_name = format!("runs/run-{number}.trace.txt");
        if run.run != number as u8
            || !run.exit_success
            || !manifest.artifacts.contains_key(&json_name)
            || !manifest.artifacts.contains_key(&trace_name)
        {
            return Err("M7 run artifacts or exit status are incomplete".into());
        }
        let json: Value = read_json(&bundle.join(&json_name))?;
        let (actual, fuzz_runs, invariant_calls, invariant_reverts) = foundry_statuses(&json);
        if actual != run.test_statuses
            || actual.len() < REQUIRED_TESTS.len()
            || actual.values().any(|status| status != "Success")
            || fuzz_runs != run.fuzz_runs_observed
            || invariant_calls != run.invariant_calls
            || invariant_reverts != run.invariant_reverts
            || !fuzz_runs.is_some_and(|runs| runs >= report.fuzz_runs as u64)
            || !REQUIRED_TESTS
                .iter()
                .all(|name| actual.get(*name).is_some_and(|status| status == "Success"))
        {
            return Err("M7 test statuses do not match the captured Foundry JSON".into());
        }
    }
    let coverage = report.coverage.as_ref().ok_or("M7 coverage is absent")?;
    if !coverage.exit_success
        || coverage.lines_hit == 0
        || !manifest.artifacts.contains_key("runs/coverage.info")
        || !manifest.artifacts.contains_key("runs/coverage.trace.txt")
    {
        return Err("M7 target coverage is missing or empty".into());
    }
    let lcov = fs::read_to_string(bundle.join("runs/coverage.info"))
        .map_err(|error| format!("cannot read M7 coverage: {error}"))?;
    let actual_coverage = target_coverage(
        &lcov,
        &bundle.join("runs/coverage.info"),
        &bundle.join("runs/coverage.trace.txt"),
        true,
    )
    .ok_or("LCOV has no Target.sol record")?;
    if actual_coverage.lines_found != coverage.lines_found
        || actual_coverage.lines_hit != coverage.lines_hit
        || actual_coverage.branches_found != coverage.branches_found
        || actual_coverage.branches_hit != coverage.branches_hit
    {
        return Err("M7 coverage summary differs from bundled LCOV".into());
    }
    Ok(report)
}

fn verify_artifacts(bundle: &Path, manifest: &EvidenceManifest) -> Result<(), String> {
    for required in [
        "case.json",
        "src/Target.sol",
        "test/Scenario.t.sol",
        "foundry.toml",
    ] {
        if !manifest.artifacts.contains_key(required) {
            return Err(format!("evidence manifest lacks {required}"));
        }
    }
    if manifest
        .facts
        .iter()
        .any(|fact| !manifest.artifacts.contains_key(&fact.artifact))
    {
        return Err("evidence fact cites an artifact outside the manifest".into());
    }
    for (name, expected) in &manifest.artifacts {
        if name.starts_with('/') || name.split('/').any(|part| part == ".." || part.is_empty()) {
            return Err("evidence manifest contains an unsafe artifact path".into());
        }
        let path = bundle.join(name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if !metadata.is_file() || metadata.len() > MAX_ARTIFACT_BYTES {
            return Err(format!("invalid evidence artifact: {name}"));
        }
        let bytes = fs::read(&path).map_err(|error| format!("cannot read {name}: {error}"))?;
        if digest(&bytes) != *expected {
            return Err(format!("evidence artifact changed: {name}"));
        }
    }
    Ok(())
}

const GUARD_ARTIFACTS: [&str; 3] = [
    "guard/decision.json", "guard/job-manifest.json", "guard/compiler-input.json",
];

fn guard_main_source(input: &Value) -> Result<(&str, &str), String> {
    let selection = input["settings"]["outputSelection"].as_object()
        .ok_or("guard compiler selection missing")?;
    let main = selection.keys().find(|name| name.as_str() != "*")
        .ok_or("guard main source missing")?;
    let content = input["sources"][main]["content"].as_str()
        .ok_or("guard main source content missing")?;
    Ok((main, content))
}

fn verify_guard_snapshot(bundle: &Path, manifest: &EvidenceManifest) -> Result<(), String> {
    let present = GUARD_ARTIFACTS.iter()
        .filter(|name| manifest.artifacts.contains_key(**name)).count();
    if present == 0 {
        return Ok(());
    }
    if present != GUARD_ARTIFACTS.len() {
        return Err("incomplete guardian link".into());
    }
    let decision: Value = read_json(&bundle.join(GUARD_ARTIFACTS[0]))?;
    let job: Value = read_json(&bundle.join(GUARD_ARTIFACTS[1]))?;
    let input: Value = read_json(&bundle.join(GUARD_ARTIFACTS[2]))?;
    let job_bytes = fs::read(bundle.join(GUARD_ARTIFACTS[1]))
        .map_err(|error| error.to_string())?;
    if decision["identity_ok"] != true
        || decision["state"] != "escopo_vinculado"
        || !matches!(decision["source_binding"].as_str(), Some("exact" | "masked_immutables"))
        || decision["manifest_sha256"].as_str() != Some(digest(&job_bytes).as_str())
        || decision["address"] != job["address"]
        || decision["block_number"] != job["block_number"]
        || decision["block_hash"] != job["block_hash"]
        || decision["implementation"] != job["implementation"]
        || !decision["claims"].as_array().is_some_and(|claims| claims.iter().any(|claim|
            claim["id"] == "source"
            && matches!(claim["state"].as_str(), Some("OBSERVADO" | "RELACIONADO_COM_RESSALVA"))
        ))
    {
        return Err("guardian decision does not bind to the recorded target and source".into());
    }
    let (_, content) = guard_main_source(&input)?;
    if digest(content.as_bytes()) != manifest.source_sha256 {
        return Err("M7 source differs from guardian main source".into());
    }
    if job["artifacts"]["compiler-input.json"]["sha256"].as_str()
        != manifest.artifacts.get(GUARD_ARTIFACTS[2]).map(String::as_str)
    {
        return Err("guardian compiler input differs from job manifest".into());
    }
    Ok(())
}

/// Copy a case into a private, content-addressed evidence snapshot. It may
/// remain pending reproduction; planning never elevates the finding.
pub fn prepare_evidence_bundle(
    case_dir: &Path,
    output_dir: &Path,
) -> Result<EvidenceManifest, String> {
    prepare_evidence_bundle_with_guard(case_dir, output_dir, None)
}

pub fn prepare_evidence_bundle_with_guard(
    case_dir: &Path,
    output_dir: &Path,
    guard_job: Option<&Path>,
) -> Result<EvidenceManifest, String> {
    let case: FoundryCase = read_json(&case_dir.join("case.json"))?;
    if case.schema != "m7-foundry-case-v1"
        || case.candidate_key.len() != 64
        || !case
            .candidate_key
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("unsupported M7 case".into());
    }
    if output_dir.exists() {
        return Err(format!(
            "evidence bundle already exists: {}",
            output_dir.display()
        ));
    }
    if let Some(job) = guard_job {
        let current = guard_wcop_job(job)?;
        let saved: Value = read_json(&job.join("decision.json"))?;
        if current != saved || current["identity_ok"] != true
            || current["state"] != "escopo_vinculado"
        {
            return Err("guardian decision is stale or incomplete".into());
        }
        let input: Value = read_json(&job.join("compiler-input.json"))?;
        let (_, content) = guard_main_source(&input)?;
        if digest(content.as_bytes()) != case.source_sha256 {
            return Err("M7 source differs from guardian main source".into());
        }
    }
    ensure_private_cache(output_dir)?;
    let mut artifacts = BTreeMap::new();
    for name in [
        "case.json",
        "src/Target.sol",
        "test/Scenario.t.sol",
        "foundry.toml",
    ] {
        copy_artifact(&case_dir.join(name), output_dir, name, &mut artifacts)?;
    }
    if artifacts.get("src/Target.sol") != Some(&case.source_sha256) {
        return Err("case source hash differs from its M5 source hash".into());
    }
    let mut stage = "PENDING_REPRODUCTION".to_owned();
    let result_path = case_dir.join("runs/result.json");
    if result_path.exists() {
        copy_artifact(&result_path, output_dir, "runs/result.json", &mut artifacts)?;
        for name in [
            "runs/run-1.json",
            "runs/run-1.trace.txt",
            "runs/run-2.json",
            "runs/run-2.trace.txt",
            "runs/coverage.info",
            "runs/coverage.trace.txt",
        ] {
            if case_dir.join(name).exists() {
                copy_artifact(&case_dir.join(name), output_dir, name, &mut artifacts)?;
            }
        }
        let temporary = EvidenceManifest {
            schema: SCHEMA.into(),
            candidate_key: case.candidate_key.clone(),
            stage: String::new(),
            source_sha256: case.source_sha256.clone(),
            artifacts: artifacts.clone(),
            facts: Vec::new(),
            assumptions: Vec::new(),
            limitations: Vec::new(),
        };
        let report: FoundryResult = read_json(&output_dir.join("runs/result.json"))?;
        if report.state == "REPRODUCED_PENDING_REVIEW" {
            validate_reproduction(output_dir, &temporary)?;
            stage = "PENDING_HUMAN_REVIEW".into();
        } else if report.state != "INCONCLUSIVE" {
            return Err("M7 result has an unknown state".into());
        }
        if case_dir.join("runs/rejection.json").exists() {
            copy_artifact(
                &case_dir.join("rejection-review.json"),
                output_dir,
                "rejection-review.json",
                &mut artifacts,
            )?;
            copy_artifact(
                &case_dir.join("runs/rejection.json"),
                output_dir,
                "runs/rejection.json",
                &mut artifacts,
            )?;
            let rejection: FoundryRejection = read_json(&output_dir.join("runs/rejection.json"))?;
            let review: RejectionReview = read_json(&output_dir.join("rejection-review.json"))?;
            if stage != "PENDING_REPRODUCTION"
                || rejection.schema != "m7-foundry-rejection-v1"
                || rejection.status != "REJECTED"
                || rejection.candidate_key != case.candidate_key
                || rejection.source_sha256 != case.source_sha256
                || rejection.harness_sha256
                    != *artifacts
                        .get("test/Scenario.t.sol")
                        .ok_or("missing harness hash")?
                || rejection.result_sha256
                    != *artifacts
                        .get("runs/result.json")
                        .ok_or("missing result hash")?
                || rejection.review_sha256
                    != *artifacts
                        .get("rejection-review.json")
                        .ok_or("missing review hash")?
                || !review.reviewed
                || review.reviewer.trim().is_empty()
                || review.scope_reference.trim().is_empty()
                || review.reason.trim().len() < 30
                || !review.assumptions_checked
                || !review.valid_inputs_checked
                || !review.failed_assertion_reviewed
                || !review.target_calls_reviewed
                || rejection.reason != review.reason
            {
                return Err("M7 rejection does not match its evidence".into());
            }
            stage = "REJECTED".into();
        }
    }
    let mut facts = vec![
        EvidenceFact {
            statement: "The bundled Solidity source has the SHA-256 recorded in the M5 case".into(),
            artifact: "src/Target.sol".into(),
        },
        EvidenceFact {
            statement: format!("M7 evidence snapshot stage: {stage}"),
            artifact: if stage == "REJECTED" {
                "runs/rejection.json"
            } else if result_path.exists() {
                "runs/result.json"
            } else {
                "case.json"
            }
            .into(),
        },
    ];
    if stage == "PENDING_HUMAN_REVIEW" {
        let report: FoundryResult = read_json(&output_dir.join("runs/result.json"))?;
        if let Some(coverage) = report.coverage {
            facts.push(EvidenceFact {
                statement: format!(
                    "Foundry LCOV records {} of {} target lines hit and {} of {} branches hit",
                    coverage.lines_hit,
                    coverage.lines_found,
                    coverage.branches_hit,
                    coverage.branches_found
                ),
                artifact: "runs/coverage.info".into(),
            });
        }
    }
    let mut limitations = vec![
        "The M5/M7 source-first workflow covers one Solidity file without imports".into(),
        "Model hypotheses and successful Foundry tests require human semantic review".into(),
        "No production exploit or economic impact is inferred automatically".into(),
    ];
    if let Some(job) = guard_job {
        for (source, target) in [
            ("decision.json", GUARD_ARTIFACTS[0]),
            ("job-manifest.json", GUARD_ARTIFACTS[1]),
            ("compiler-input.json", GUARD_ARTIFACTS[2]),
        ] {
            copy_artifact(&job.join(source), output_dir, target, &mut artifacts)?;
        }
        let decision: Value = read_json(&output_dir.join(GUARD_ARTIFACTS[0]))?;
        facts.push(EvidenceFact {
            statement: format!(
                "Guardian linked Ethereum target {} at block {} ({}) with source binding {}",
                decision["address"].as_str().unwrap_or("?"),
                decision["block_number"].as_u64().unwrap_or(0),
                decision["block_hash"].as_str().unwrap_or("?"),
                decision["source_binding"].as_str().unwrap_or("?"),
            ),
            artifact: GUARD_ARTIFACTS[0].into(),
        });
        if decision["source_binding"] == "masked_immutables" {
            limitations.push("Immutable values were masked for bytecode comparison; their values were not individually confirmed".into());
        }
    } else {
        limitations.push("No guardian job is linked to this M8 evidence snapshot".into());
    }
    let manifest = EvidenceManifest {
        schema: SCHEMA.into(),
        candidate_key: case.candidate_key,
        stage,
        source_sha256: case.source_sha256,
        artifacts,
        facts,
        assumptions: case.assumptions,
        limitations,
    };
    verify_guard_snapshot(output_dir, &manifest)?;
    write_private(&output_dir.join("manifest.json"), &encode(&manifest)?)?;
    write_private(
        &output_dir.join("review.json"),
        &encode(&HumanEvidenceReview::default())?,
    )?;
    Ok(manifest)
}

fn required(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 4_096 {
        return Err(format!("human review requires a concise {label}"));
    }
    Ok(())
}

fn line(value: &str) -> String {
    value
        .replace(['\r', '\n'], " ")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {}\n", line(item)))
        .collect()
}

fn report_markdown(
    manifest: &EvidenceManifest,
    review: &HumanEvidenceReview,
    result: &FoundryResult,
) -> String {
    let mut text = format!(
        "# {}\n\nStatus: REPORT_READY (human-reviewed)\n\nCandidate: `{}`\nTarget version: {}\nSource SHA-256: `{}`\nScope reference: {}\nReviewer: {} ({})\nEnvironment: {}\n\n## Summary\n\n{}\n\n## Observed facts\n\n",
        line(&review.title),
        manifest.candidate_key,
        line(&review.target_version),
        manifest.source_sha256,
        line(&review.scope_reference),
        line(&review.reviewer),
        line(&review.reviewed_at_utc),
        line(&review.environment),
        line(&review.summary)
    );
    for fact in &manifest.facts {
        text.push_str(&format!(
            "- {} (`{}`)\n",
            line(&fact.statement),
            fact.artifact
        ));
    }
    text.push_str("- Both Foundry executions returned Success for `testReproduce()` and `testFuzzBoundaries(uint256)` (see `runs/run-1.json`, `runs/run-2.json` and their traces).\n");
    for run in &result.runs {
        text.push_str(&format!(
            "- Run {}: {} fuzz inputs recorded by Foundry.\n",
            run.run,
            run.fuzz_runs_observed.unwrap_or(0)
        ));
        if let (Some(calls), Some(reverts)) = (run.invariant_calls, run.invariant_reverts) {
            text.push_str(&format!(
                "- Run {} invariant campaign: {} handler calls, {} reverts.\n",
                run.run, calls, reverts
            ));
        }
    }
    text.push_str(
        "- No invariant call/revert rate is claimed when the harness has no invariant campaign.\n",
    );
    text.push_str("\n## Human inference and impact\n\n");
    text.push_str(&list(&review.inferences));
    text.push_str(&format!("\nImpact assessment: {}\n", line(&review.impact)));
    text.push_str("\n## Prerequisites and assumptions\n\n");
    text.push_str(&list(&review.prerequisites));
    text.push_str(&list(&manifest.assumptions));
    text.push_str("\n## Reproduction\n\nRun in an authorized, isolated local environment using the bundled `src/`, `test/` and `foundry.toml`.\n\n");
    text.push_str(&format!("`forge test --offline --use <local-solc-path> --match-path test/Scenario.t.sol --fuzz-seed {} --fuzz-runs {} -vvvv`\n\n", result.fuzz_seed, result.fuzz_runs));
    text.push_str(&format!("Recorded tool versions: Forge {}; solc {}. Compare output with both stored JSON runs and traces.\n", line(&review.forge_version), line(&review.solc_version)));
    text.push_str(&format!(
        "\nReviewer reproduction steps: {}\n",
        line(&review.reproduction_steps)
    ));
    text.push_str("\n## Limitations\n\n");
    text.push_str(&list(&manifest.limitations));
    text.push_str(&list(&review.additional_limitations));
    text.push_str(&format!(
        "\nDuplicate review: {}\n\nArtifact SHA-256 values are in `manifest.json`.\n",
        line(&review.duplicate_note)
    ));
    text
}

/// Finalize only after a positive M7 snapshot and complete human attestation.
pub fn finalize_evidence_bundle(bundle: &Path) -> Result<EvidenceReady, String> {
    ensure_private_cache(bundle)?;
    if bundle.join("ready.json").exists() || bundle.join("report.md").exists() {
        return Err("evidence bundle was already finalized".into());
    }
    let manifest_path = bundle.join("manifest.json");
    let review_path = bundle.join("review.json");
    let manifest: EvidenceManifest = read_json(&manifest_path)?;
    let review: HumanEvidenceReview = read_json(&review_path)?;
    if manifest.schema != SCHEMA || manifest.stage != "PENDING_HUMAN_REVIEW" {
        return Err("evidence bundle has no verified reproduction ready for human review".into());
    }
    verify_artifacts(bundle, &manifest)?;
    verify_guard_snapshot(bundle, &manifest)?;
    let case: FoundryCase = read_json(&bundle.join("case.json"))?;
    if case.candidate_key != manifest.candidate_key
        || case.source_sha256 != manifest.source_sha256
        || manifest.artifacts.get("src/Target.sol") != Some(&manifest.source_sha256)
    {
        return Err("M7 case identity differs from the M8 evidence manifest".into());
    }
    let result = validate_reproduction(bundle, &manifest)?;
    if !review.approved_for_report
        || !review.scope_authorized
        || !review.assertions_reviewed
        || !review.reproduction_reviewed
        || !review.evidence_reviewed
        || !review.duplicate_checked
    {
        return Err("scope, impact, duplication and evidence review gates are incomplete".into());
    }
    for (value, label) in [
        (&review.reviewer, "reviewer"),
        (&review.reviewed_at_utc, "review timestamp"),
        (&review.scope_reference, "authorized scope reference"),
        (&review.target_version, "target version"),
        (&review.environment, "execution environment"),
        (&review.forge_version, "Forge version"),
        (&review.solc_version, "solc version"),
        (&review.reproduction_steps, "reproduction steps"),
        (&review.title, "report title"),
        (&review.summary, "behavior summary"),
        (&review.impact, "impact assessment"),
        (&review.duplicate_note, "duplicate review note"),
    ] {
        required(value, label)?;
    }
    if review.inferences.is_empty()
        || review.prerequisites.is_empty()
        || review
            .inferences
            .iter()
            .chain(&review.prerequisites)
            .any(|value| value.trim().is_empty())
    {
        return Err("human review requires explicit inferences and prerequisites".into());
    }
    let report = report_markdown(&manifest, &review, &result);
    let report_bytes = report.as_bytes();
    let ready = EvidenceReady {
        schema: SCHEMA.into(),
        status: "REPORT_READY".into(),
        candidate_key: manifest.candidate_key,
        source_sha256: manifest.source_sha256,
        manifest_sha256: digest(&fs::read(&manifest_path).map_err(|error| error.to_string())?),
        review_sha256: digest(&fs::read(&review_path).map_err(|error| error.to_string())?),
        report_sha256: digest(report_bytes),
        report_path: bundle.join("report.md").display().to_string(),
    };
    write_private(&bundle.join("report.md"), report_bytes)?;
    write_private(&bundle.join("ready.json"), &encode(&ready)?)?;
    Ok(ready)
}
