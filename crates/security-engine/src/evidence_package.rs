//! M8: portable evidence snapshots and an explicit human report gate.
//! The evidence plan is safe to create before tests exist; it cannot confirm a finding.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::context_package::{ensure_private_cache, write_private};
use crate::{FoundryCase, FoundryResult};

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

fn foundry_statuses(json: &Value) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
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
                }
            }
        }
    }
    result
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
        let actual = foundry_statuses(&json);
        if actual != run.test_statuses
            || actual.len() != REQUIRED_TESTS.len()
            || !REQUIRED_TESTS
                .iter()
                .all(|name| actual.get(*name).is_some_and(|status| status == "Success"))
        {
            return Err("M7 test statuses do not match the captured Foundry JSON".into());
        }
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

/// Copy a case into a private, content-addressed evidence snapshot. It may
/// remain pending reproduction; planning never elevates the finding.
pub fn prepare_evidence_bundle(
    case_dir: &Path,
    output_dir: &Path,
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
        for name in [
            "runs/result.json",
            "runs/run-1.json",
            "runs/run-1.trace.txt",
            "runs/run-2.json",
            "runs/run-2.trace.txt",
        ] {
            copy_artifact(&case_dir.join(name), output_dir, name, &mut artifacts)?;
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
    }
    let facts = vec![
        EvidenceFact {
            statement: "The bundled Solidity source has the SHA-256 recorded in the M5 case".into(),
            artifact: "src/Target.sol".into(),
        },
        EvidenceFact {
            statement: format!("M7 evidence snapshot stage: {stage}"),
            artifact: if result_path.exists() {
                "runs/result.json"
            } else {
                "case.json"
            }
            .into(),
        },
    ];
    let manifest = EvidenceManifest {
        schema: SCHEMA.into(),
        candidate_key: case.candidate_key,
        stage,
        source_sha256: case.source_sha256,
        artifacts,
        facts,
        assumptions: case.assumptions,
        limitations: vec![
            "The M5/M7 source-first workflow covers one Solidity file without imports".into(),
            "Model hypotheses and successful Foundry tests require human semantic review".into(),
            "No production exploit or economic impact is inferred automatically".into(),
        ],
    };
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
