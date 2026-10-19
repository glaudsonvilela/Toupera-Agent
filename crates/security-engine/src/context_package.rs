//! Content-addressed M5 context packages. The cache is local and stores only
//! bounded slices, not full repository content or any network responses.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::source::{function_id, parse_span};
use crate::{
    ContractId, EconomicAnalysisMetrics, EconomicCandidate, EconomicFunction, EvidenceLocation,
    FindingKind, FunctionId, SourceSpan, Uncertainty, analyze_economic_functions,
    compile_source_ast_text_with_evm_version, normalized_runtime_hex, reconstruct_from_ast_json,
};

const PACKAGE_SCHEMA: &str = "m5-context-v1";
const MAX_FUNCTION_SOURCE: usize = 4_096;
const MAX_RELATED_SOURCE: usize = 1_536;
const MAX_EVIDENCE_SOURCE: usize = 384;
const MAX_EVIDENCE_ITEMS: usize = 12;
const MAX_RELATED_FUNCTIONS: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactDigests {
    pub source_sha256: String,
    pub compiler_sha256: String,
    pub engine_sha256: String,
    pub ast_sha256: String,
    pub runtime_sha256: Option<String>,
    pub slither_json_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evm_version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FunctionDigest {
    pub contract: ContractId,
    pub function: FunctionId,
    pub source_sha256: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvidenceExcerpt {
    pub span: SourceSpan,
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FunctionExcerpt {
    pub contract: ContractId,
    pub function: FunctionId,
    pub source_sha256: String,
    pub span: SourceSpan,
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContextSlice {
    pub candidate: EconomicCandidate,
    pub function: FunctionExcerpt,
    pub related_functions: Vec<FunctionExcerpt>,
    pub evidence_excerpts: Vec<EvidenceExcerpt>,
    pub storage_reads: Vec<String>,
    pub storage_writes: Vec<String>,
    pub arithmetic: Vec<String>,
    pub oracle_targets: Vec<String>,
    pub calls: Vec<String>,
    pub invariant_questions: Vec<String>,
    pub unresolved: Vec<Uncertainty>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContextPackage {
    pub schema: String,
    pub source_name: String,
    pub digests: ArtifactDigests,
    pub function_digests: Vec<FunctionDigest>,
    pub economic_metrics: EconomicAnalysisMetrics,
    pub candidate_count: usize,
    pub slices: Vec<ContextSlice>,
    pub unresolved: Vec<Uncertainty>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CachedContextPackage {
    pub cache_hit: bool,
    pub cache_key: String,
    pub package: Value,
}

#[derive(Clone)]
struct SourceFunction {
    digest: FunctionDigest,
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn bounded(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_owned(), false);
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

fn source_functions(
    ast: &Value,
    source_id: i64,
    source_name: &str,
    source: &str,
) -> Result<Vec<SourceFunction>, String> {
    let nodes = ast
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("solc AST has no nodes")?;
    let mut records = Vec::new();
    for contract in nodes.iter().filter(|node| {
        node.get("nodeType").and_then(Value::as_str) == Some("ContractDefinition")
            && node.get("contractKind").and_then(Value::as_str) == Some("contract")
    }) {
        let Some(contract_name) = contract.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(members) = contract.get("nodes").and_then(Value::as_array) else {
            continue;
        };
        for member in members.iter().filter(|node| {
            node.get("nodeType").and_then(Value::as_str) == Some("FunctionDefinition")
                && node.get("body").is_some_and(|body| !body.is_null())
        }) {
            let Some(function) = function_id(member) else {
                continue;
            };
            let span = parse_span(
                member
                    .get("src")
                    .and_then(Value::as_str)
                    .ok_or("function AST has no source span")?,
                source_name,
                source,
                source_id,
            )
            .ok_or("function AST span does not match source")?;
            let text = span
                .slice(source)
                .ok_or("function source span is invalid")?;
            records.push(SourceFunction {
                digest: FunctionDigest {
                    contract: ContractId(contract_name.to_owned()),
                    function,
                    source_sha256: sha256(text.as_bytes()),
                    span,
                },
            });
        }
    }
    Ok(records)
}

fn excerpt(record: &SourceFunction, source: &str, limit: usize) -> FunctionExcerpt {
    let full = record.digest.span.slice(source).unwrap_or("");
    let (text, truncated) = bounded(full, limit);
    FunctionExcerpt {
        contract: record.digest.contract.clone(),
        function: record.digest.function.clone(),
        source_sha256: record.digest.source_sha256.clone(),
        span: record.digest.span.clone(),
        text,
        truncated,
    }
}

fn unique_limited<I>(items: I, limit: usize) -> (Vec<String>, bool)
where
    I: IntoIterator<Item = String>,
{
    let mut unique = BTreeSet::new();
    let mut truncated = false;
    for item in items {
        if unique.len() >= limit && !unique.contains(&item) {
            truncated = true;
            continue;
        }
        unique.insert(item);
    }
    (unique.into_iter().collect(), truncated)
}

fn invariant_questions(kind: FindingKind) -> Vec<String> {
    let questions: &[&str] = match kind {
        FindingKind::AccountingDrift => &[
            "For each successful entry/exit, do token balance and share supply changes reconcile after fees?",
            "Can a revert or external call leave accounting partially updated?",
        ],
        FindingKind::PriceSourceAsymmetry | FindingKind::OracleFallbackMismatch => &[
            "Do entry, exit and preview paths use the same validated price under the same state?",
            "What happens when the primary feed is stale, zero or unavailable?",
        ],
        FindingKind::DecimalMismatch => &[
            "Are asset/share units and decimal scales reciprocal across both conversion directions?",
        ],
        FindingKind::RoundingAsymmetry | FindingKind::PreviewExecutionMismatch => &[
            "For identical state and input, does preview bound or match execution as the protocol specifies?",
        ],
        FindingKind::ShareInflation => &[
            "Can a direct donation before or after the first mint change shares received by a later depositor?",
        ],
        FindingKind::FeeBypass => {
            &["Do preview and execution include the same fee policy at boundary amounts?"]
        }
    };
    questions.iter().map(|item| (*item).to_owned()).collect()
}

fn build_slice(
    candidate: EconomicCandidate,
    functions: &[EconomicFunction],
    records: &[SourceFunction],
    source: &str,
) -> Option<ContextSlice> {
    let record = records.iter().find(|item| {
        item.digest.contract == candidate.contract && item.digest.function == candidate.function
    })?;
    let ir = functions
        .iter()
        .find(|item| item.contract == candidate.contract && item.function == candidate.function)?;
    let function = excerpt(record, source, MAX_FUNCTION_SOURCE);
    let mut truncated = function.truncated;

    let mut selected = BTreeSet::new();
    let mut related_functions = Vec::new();
    for call in &ir.resolved_calls {
        selected.insert((call.value.contract.0.clone(), call.value.function.0.clone()));
    }
    for evidence in &candidate.evidence {
        if let EvidenceLocation::Source(span) = evidence {
            for other in records {
                if other.digest.span.file() == span.file()
                    && other.digest.span.start_byte() <= span.start_byte()
                    && other.digest.span.end_byte() >= span.end_byte()
                    && (other.digest.contract != candidate.contract
                        || other.digest.function != candidate.function)
                {
                    selected.insert((
                        other.digest.contract.0.clone(),
                        other.digest.function.0.clone(),
                    ));
                }
            }
        }
    }
    for (contract, function_id) in selected {
        if contract == candidate.contract.0 && function_id == candidate.function.0 {
            continue;
        }
        if related_functions.len() >= MAX_RELATED_FUNCTIONS {
            truncated = true;
            break;
        }
        if let Some(other) = records.iter().find(|item| {
            item.digest.contract.0 == contract && item.digest.function.0 == function_id
        }) {
            let item = excerpt(other, source, MAX_RELATED_SOURCE);
            truncated |= item.truncated;
            related_functions.push(item);
        }
    }

    let mut evidence_excerpts = Vec::new();
    for evidence in &candidate.evidence {
        if evidence_excerpts.len() >= MAX_EVIDENCE_ITEMS {
            truncated = true;
            break;
        }
        if let EvidenceLocation::Source(span) = evidence {
            if let Some(text) = span.slice(source) {
                let (text, shortened) = bounded(text, MAX_EVIDENCE_SOURCE);
                truncated |= shortened;
                evidence_excerpts.push(EvidenceExcerpt {
                    span: span.clone(),
                    text,
                    truncated: shortened,
                });
            }
        }
    }

    let (storage_reads, a) = unique_limited(ir.reads.iter().map(|item| item.value.0.clone()), 16);
    let (storage_writes, b) = unique_limited(ir.writes.iter().map(|item| item.value.0.clone()), 16);
    let (arithmetic, c) = unique_limited(
        ir.arithmetic
            .iter()
            .map(|item| bounded(&item.value.0, 256).0),
        16,
    );
    let (oracle_targets, d) = unique_limited(
        ir.oracle_dependencies
            .iter()
            .map(|item| item.value.0.clone()),
        8,
    );
    let (calls, e) = unique_limited(
        ir.resolved_calls
            .iter()
            .map(|item| format!("{}.{}", item.value.contract.0, item.value.function.0)),
        12,
    );
    truncated |= a || b || c || d || e;
    Some(ContextSlice {
        invariant_questions: invariant_questions(candidate.kind),
        candidate,
        function,
        related_functions,
        evidence_excerpts,
        storage_reads,
        storage_writes,
        arithmetic,
        oracle_targets,
        calls,
        unresolved: ir.unresolved.clone(),
        truncated,
    })
}

/// Build a local package. `runtime_hex` is fingerprinted only; it is not
/// asserted to correspond to the source without a byte-for-byte solc match.
pub fn build_context_package(
    source_name: &str,
    source: &str,
    solc_json: &str,
    compiler_sha256: &str,
    engine_sha256: &str,
    slither_json: Option<&str>,
    runtime_hex: Option<&str>,
) -> Result<ContextPackage, String> {
    let output: Value =
        serde_json::from_str(solc_json).map_err(|error| format!("invalid solc JSON: {error}"))?;
    let source_entry = output
        .get("sources")
        .and_then(|sources| sources.get(source_name))
        .ok_or_else(|| format!("solc output has no AST for {source_name}"))?;
    let source_id = source_entry
        .get("id")
        .and_then(Value::as_i64)
        .ok_or("solc source entry has no numeric id")?;
    let ast = source_entry
        .get("ast")
        .ok_or("solc source entry has no AST")?;
    let ast_bytes =
        serde_json::to_vec(ast).map_err(|error| format!("cannot encode AST: {error}"))?;
    let functions = reconstruct_from_ast_json(solc_json, source_name, source)?;
    let records = source_functions(ast, source_id, source_name, source)?;
    let analysis = analyze_economic_functions(&functions, source_name, source, slither_json)?;
    let runtime_sha256 = runtime_hex
        .map(|value| {
            let normalized = normalized_runtime_hex(value)?;
            let bytes = hex::decode(normalized)
                .map_err(|error| format!("invalid normalized bytecode: {error}"))?;
            Ok::<_, String>(sha256(&bytes))
        })
        .transpose()?;
    let digests = ArtifactDigests {
        source_sha256: sha256(source.as_bytes()),
        compiler_sha256: compiler_sha256.to_owned(),
        engine_sha256: engine_sha256.to_owned(),
        ast_sha256: sha256(&ast_bytes),
        runtime_sha256,
        slither_json_sha256: slither_json.map(|value| sha256(value.as_bytes())),
        evm_version: None,
    };
    let mut slices = Vec::new();
    let mut unresolved = analysis.unresolved;
    for candidate in analysis.candidates {
        match build_slice(candidate.clone(), &functions, &records, source) {
            Some(slice) => slices.push(slice),
            None => unresolved.push(Uncertainty {
                kind: crate::UncertaintyKind::MissingInput,
                detail: format!(
                    "candidate {}.{} has no matching source function span",
                    candidate.contract.0, candidate.function.0
                ),
                location: None,
            }),
        }
    }
    let candidate_count = slices.len();
    Ok(ContextPackage {
        schema: PACKAGE_SCHEMA.into(),
        source_name: source_name.into(),
        digests,
        function_digests: records.into_iter().map(|item| item.digest).collect(),
        economic_metrics: analysis.metrics,
        candidate_count,
        slices,
        unresolved,
    })
}

#[cfg(unix)]
pub(crate) fn ensure_private_cache(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if !dir.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder
            .create(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    }
    let metadata = fs::symlink_metadata(dir)
        .map_err(|error| format!("cannot inspect {}: {error}", dir.display()))?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err("context cache must be a private directory (mode 0700)".into());
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn ensure_private_cache(_dir: &Path) -> Result<(), String> {
    Err("M5 cache currently requires Unix filesystem permissions".into())
}

pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&temp)
        .map_err(|error| format!("cannot create {}: {error}", temp.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("cannot write cache: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("cannot sync cache: {error}"))?;
    fs::rename(&temp, path).map_err(|error| format!("cannot publish cache: {error}"))?;
    Ok(())
}

/// Read or build the package. Cache hits skip solc, IR reconstruction, M4 and
/// slicing. The key includes exact input and compiler bytes plus this schema.
pub fn load_or_build_context_package(
    source_path: &Path,
    solc_path: &Path,
    cache_dir: &Path,
    slither_json: Option<&str>,
    runtime_hex: Option<&str>,
) -> Result<CachedContextPackage, String> {
    load_or_build_context_package_with_evm_version(
        source_path,
        solc_path,
        cache_dir,
        slither_json,
        runtime_hex,
        None,
    )
}

pub fn load_or_build_context_package_with_evm_version(
    source_path: &Path,
    solc_path: &Path,
    cache_dir: &Path,
    slither_json: Option<&str>,
    runtime_hex: Option<&str>,
    evm_version: Option<&str>,
) -> Result<CachedContextPackage, String> {
    let source = fs::read_to_string(source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let source_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("source path has no UTF-8 file name")?;
    let compiler = fs::read(solc_path)
        .map_err(|error| format!("cannot read compiler {}: {error}", solc_path.display()))?;
    let compiler_sha256 = sha256(&compiler);
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot locate analysis engine: {error}"))?;
    let engine = fs::read(&executable).map_err(|error| {
        format!(
            "cannot read analysis engine {}: {error}",
            executable.display()
        )
    })?;
    let engine_sha256 = sha256(&engine);
    let normalized_runtime = runtime_hex.map(normalized_runtime_hex).transpose()?;
    let runtime_sha256 = normalized_runtime
        .as_ref()
        .map(|value| {
            hex::decode(value)
                .map(|bytes| sha256(&bytes))
                .map_err(|error| format!("invalid normalized bytecode: {error}"))
        })
        .transpose()?;
    let slither_sha256 = slither_json.map(|value| sha256(value.as_bytes()));
    let key = sha256(
        format!(
            "{PACKAGE_SCHEMA}\n{}\n{compiler_sha256}\n{engine_sha256}\n{}\n{}\n{}\n{}",
            source_name,
            sha256(source.as_bytes()),
            slither_sha256.as_deref().unwrap_or("-"),
            runtime_sha256.as_deref().unwrap_or("-"),
            evm_version.unwrap_or("-")
        )
        .as_bytes(),
    );
    ensure_private_cache(cache_dir)?;
    let path = cache_dir.join(format!("{key}.json"));
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
                    return Err(
                        "context cache entry must be a private regular file (mode 0600)".into(),
                    );
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect cache {}: {error}", path.display())),
    }
    match fs::read_to_string(&path) {
        Ok(cached) => {
            if let Ok(package) = serde_json::from_str::<Value>(&cached) {
                if package.get("schema").and_then(Value::as_str) == Some(PACKAGE_SCHEMA)
                    && package
                        .pointer("/digests/source_sha256")
                        .and_then(Value::as_str)
                        == Some(sha256(source.as_bytes()).as_str())
                    && package
                        .pointer("/digests/compiler_sha256")
                        .and_then(Value::as_str)
                        == Some(compiler_sha256.as_str())
                    && package
                        .pointer("/digests/engine_sha256")
                        .and_then(Value::as_str)
                        == Some(engine_sha256.as_str())
                    && package
                        .pointer("/digests/runtime_sha256")
                        .and_then(Value::as_str)
                        == runtime_sha256.as_deref()
                    && package
                        .pointer("/digests/slither_json_sha256")
                        .and_then(Value::as_str)
                        == slither_sha256.as_deref()
                    && package
                        .pointer("/digests/evm_version")
                        .and_then(Value::as_str)
                        == evm_version
                {
                    return Ok(CachedContextPackage {
                        cache_hit: true,
                        cache_key: key,
                        package,
                    });
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot read cache {}: {error}", path.display())),
    }
    let solc_json =
        compile_source_ast_text_with_evm_version(source_name, &source, solc_path, evm_version)?;
    let mut built = build_context_package(
        source_name,
        &source,
        &solc_json,
        &compiler_sha256,
        &engine_sha256,
        slither_json,
        runtime_hex,
    )?;
    built.digests.evm_version = evm_version.map(str::to_owned);
    let package = serde_json::to_value(&built)
        .map_err(|error| format!("cannot encode context package: {error}"))?;
    let bytes = serde_json::to_vec(&package)
        .map_err(|error| format!("cannot encode cache entry: {error}"))?;
    write_private(&path, &bytes)?;
    Ok(CachedContextPackage {
        cache_hit: false,
        cache_key: key,
        package,
    })
}
