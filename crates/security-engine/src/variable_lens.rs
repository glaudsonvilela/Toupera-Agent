//! Offline, source-backed view of variable declarations and syntactic value paths.
//! Runtime observations are kept as context until a verified PC-to-source mapping exists.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

use alloy_primitives::U256;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::SourceSpan;
use crate::forensic_probe::ForensicInput;
use crate::source::{compile_source_ast_text_with_evm_version, parse_span};

#[derive(Clone, Debug, Serialize)]
pub struct LayoutPosition {
    pub contract: String,
    pub slot: String,
    pub offset: u64,
    pub type_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariableUse {
    pub kind: String,
    pub contract: String,
    pub function: String,
    pub source_span: Option<SourceSpan>,
    pub expression: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariableRecord {
    pub ast_id: i64,
    pub name: String,
    pub declaring_contract: String,
    pub scope: String,
    pub declaration_role: String,
    pub category: String,
    pub type_name: String,
    pub mutability: String,
    pub data_location: String,
    pub declaration_span: Option<SourceSpan>,
    pub layout: Vec<LayoutPosition>,
    pub uses: Vec<VariableUse>,
    pub upstream_variable_ids: Vec<i64>,
    pub downstream_variable_ids: Vec<i64>,
    pub priority_score: u8,
    pub priority_reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariableEdge {
    pub from_ast_id: i64,
    pub to_ast_id: i64,
    pub relation: String,
    pub function: String,
    pub source_span: Option<SourceSpan>,
    pub expression: Option<String>,
    pub confidence: String,
    pub evidence_source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexedAccess {
    pub base_ast_ids: Vec<i64>,
    pub key_variable_ast_ids: Vec<i64>,
    pub key_expression: Option<String>,
    pub expression: Option<String>,
    pub function: String,
    pub contract: String,
    pub access: String,
    pub collection_kind: String,
    pub key_identity_kind: String,
    pub key_identity: Option<String>,
    pub parent_key_identities: Vec<Option<String>>,
    pub source_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, Serialize)]
pub struct KeyAliasRelation {
    pub left_access: usize,
    pub right_access: usize,
    pub base_ast_ids: Vec<i64>,
    pub relation: String,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct StateTransitionLink {
    pub state_variable_ast_id: i64,
    pub writer_contract: String,
    pub writer_function: String,
    pub reader_contract: String,
    pub reader_function: String,
    pub key_relation: String,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EffectBoundary {
    pub kind: String,
    pub contract: String,
    pub function: String,
    pub method: Option<String>,
    pub receiver_type: Option<String>,
    pub source_variable_ast_ids: Vec<i64>,
    pub source_span: Option<SourceSpan>,
    pub expression: Option<String>,
    pub effect_class: String,
    pub yul_operations: Vec<String>,
    pub literal_storage_slots: Vec<AssemblySlotUse>,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssemblySlotUse {
    pub operation: String,
    pub slot: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VerifiedVariable {
    pub ast_id: i64,
    pub name: String,
    pub source_file: Option<String>,
    pub declaration_start_byte: Option<usize>,
    pub declaration_end_byte: Option<usize>,
    pub declaring_contract: String,
    pub scope: String,
    pub type_name: String,
    pub state_variable: bool,
    pub observed_branch_pcs: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PcSourceHint {
    pub pc: usize,
    pub observed_outcomes: Vec<String>,
    pub verified_source_file: Option<String>,
    pub source_start_byte: Option<usize>,
    pub source_end_byte: Option<usize>,
    pub verified_variable_ast_ids: Vec<i64>,
    pub verified_variable_names: Vec<String>,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PcSourceOrigin {
    pub pc: usize,
    pub source_file: String,
    pub declaring_contract: String,
    pub function: String,
    pub function_ast_id: i64,
    pub origin_kind: String,
    pub observed_outcomes: Vec<String>,
    pub confidence: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FunctionSourceCoverage {
    pub source_file: String,
    pub declaring_contract: String,
    pub function: String,
    pub function_ast_id: i64,
    pub origin_kind: String,
    pub mapped_static_jump_sites: usize,
    pub observed_jump_sites: usize,
    pub observed_outcomes: usize,
    pub sites_with_both_outcomes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceCoverageSummary {
    pub runtime_static_jump_sites: usize,
    pub uniquely_mapped_static_jump_sites: usize,
    pub unattributed_static_jump_sites: usize,
    pub observed_attributed_jump_sites: usize,
    pub observed_attributed_outcomes: usize,
    pub function_source_coverage: Vec<FunctionSourceCoverage>,
    pub pc_source_origins: Vec<PcSourceOrigin>,
    pub denominator_kind: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct VariableLensReport {
    pub schema: String,
    pub identity: Value,
    pub analysis_source_sha256: String,
    pub forensic_report_sha256: String,
    pub local_coverage_sha256: String,
    pub variable_count: usize,
    pub category_counts: BTreeMap<String, usize>,
    pub declaration_role_counts: BTreeMap<String, usize>,
    pub dependency_edge_count: usize,
    pub cross_function_edge_count: usize,
    pub indexed_access_count: usize,
    pub indexed_accesses: Vec<IndexedAccess>,
    pub key_alias_relations: Vec<KeyAliasRelation>,
    pub key_alias_pairs_truncated: bool,
    pub state_transition_links: Vec<StateTransitionLink>,
    pub state_transition_links_truncated: bool,
    pub effect_boundaries: Vec<EffectBoundary>,
    pub ssa_dependency_edge_count: usize,
    pub ssa_unmatched_declaration_operands: usize,
    pub ssa_report_sha256: Option<String>,
    pub variables_without_source_use: usize,
    pub variables_with_read_without_writer: usize,
    pub observed_root_jump_sites: usize,
    pub observed_root_branch_outcomes: usize,
    pub pc_hints_with_variables: usize,
    pub pc_source_hints: Vec<PcSourceHint>,
    pub source_coverage: SourceCoverageSummary,
    pub verified_variables: Vec<VerifiedVariable>,
    pub variables: Vec<VariableRecord>,
    pub edges: Vec<VariableEdge>,
    pub limitations: Vec<String>,
}

struct Walker<'a> {
    source_name: &'a str,
    source: &'a str,
    source_id: i64,
    declarations: BTreeMap<i64, VariableRecord>,
    edges: Vec<VariableEdge>,
    indexed_accesses: Vec<IndexedAccess>,
    effect_boundaries: Vec<EffectBoundary>,
    functions: BTreeMap<i64, FunctionSummary>,
}

struct FunctionSummary {
    parameters: Vec<i64>,
    return_sources: Vec<i64>,
    visibility: String,
    mutability: String,
}

#[derive(Clone)]
struct Context {
    contract: String,
    function: String,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_json(path: &Path) -> Result<(Vec<u8>, Value), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid JSON {}: {error}", path.display()))?;
    Ok((bytes, value))
}

fn equals_ci(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn required_string<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, String> {
    let mut item = value;
    for key in path {
        item = item
            .get(*key)
            .ok_or_else(|| format!("missing {}", path.join(".")))?;
    }
    item.as_str()
        .ok_or_else(|| format!("invalid {}", path.join(".")))
}

fn verify_input(
    manifest: &ForensicInput,
    forensic_bytes: &[u8],
    forensic: &Value,
    coverage: &Value,
    source: &[u8],
) -> Result<(), String> {
    if manifest.schema != "forensic-input-v1"
        || forensic["schema"] != "forensic-map-v1"
        || coverage["schema"] != "local-coverage-v1"
    {
        return Err("incompatible manifest or report schema".into());
    }
    let identity = &forensic["identity"];
    let observed = &coverage["identity"];
    if identity["exact_onchain_runtime"] != true
        || identity["analysis_source_sha256"] != digest(source)
        || observed["base_report_sha256"] != digest(forensic_bytes)
        || identity["chain_id"] != manifest.chain_id
        || identity["block_number"] != manifest.block_number
        || !equals_ci(required_string(identity, &["address"])?, &manifest.address)
        || !equals_ci(
            required_string(identity, &["block_hash"])?,
            &manifest.block_hash,
        )
        || observed["chain_id"] != identity["chain_id"]
        || observed["block_number"] != identity["block_number"]
        || !equals_ci(required_string(observed, &["vault"])?, &manifest.address)
        || !equals_ci(
            required_string(observed, &["block_hash"])?,
            &manifest.block_hash,
        )
        || observed["runtime_sha256"] != identity["runtime_sha256"]
    {
        return Err("source, fixed block, runtime or report identity differs".into());
    }
    Ok(())
}

pub fn build_variable_lens(
    manifest_path: &Path,
    forensic_path: &Path,
    local_coverage_path: &Path,
    ssa_path: Option<&Path>,
) -> Result<VariableLensReport, String> {
    let (_, manifest_value) = read_json(manifest_path)?;
    let manifest: ForensicInput = serde_json::from_value(manifest_value)
        .map_err(|error| format!("invalid forensic manifest: {error}"))?;
    let (forensic_bytes, forensic) = read_json(forensic_path)?;
    let (coverage_bytes, coverage) = read_json(local_coverage_path)?;
    let source_bytes = fs::read(&manifest.analysis_source_path).map_err(|error| {
        format!(
            "cannot read {}: {error}",
            manifest.analysis_source_path.display()
        )
    })?;
    verify_input(
        &manifest,
        &forensic_bytes,
        &forensic,
        &coverage,
        &source_bytes,
    )?;
    let (comparison_bytes, comparison) = read_json(&manifest.comparison_solc_json_path)?;
    let runtime_text = fs::read_to_string(&manifest.runtime_path)
        .map_err(|error| format!("cannot read runtime: {error}"))?;
    let runtime_hex: String = runtime_text
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let runtime_hex = runtime_hex.strip_prefix("0x").unwrap_or(&runtime_hex);
    let runtime_bytes =
        hex::decode(runtime_hex).map_err(|error| format!("invalid runtime hex: {error}"))?;
    if forensic["identity"]["comparison_json_sha256"] != digest(&comparison_bytes)
        || forensic["identity"]["runtime_sha256"] != digest(&runtime_bytes)
        || (forensic["identity"]["exact_compiler_runtime"] != true
            && forensic["identity"]["immutable_masked_compiler_runtime"] != true)
    {
        return Err("compiler output or runtime differs from the fixed report".into());
    }
    let (pc_source_hints, verified_variables, source_coverage) =
        pc_hints(&manifest, &comparison, &coverage, &runtime_bytes)?;
    let source = std::str::from_utf8(&source_bytes)
        .map_err(|error| format!("source is not UTF-8: {error}"))?;
    let source_name = manifest
        .analysis_source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("analysis source has no UTF-8 file name")?;
    let output = compile_source_ast_text_with_evm_version(
        source_name,
        source,
        &manifest.solc_path,
        manifest.evm_version.as_deref(),
    )?;
    let solc: Value =
        serde_json::from_str(&output).map_err(|error| format!("invalid solc output: {error}"))?;
    if let Some(errors) = solc["errors"].as_array() {
        let failures: Vec<_> = errors
            .iter()
            .filter(|entry| entry["severity"] == "error")
            .filter_map(|entry| entry["formattedMessage"].as_str())
            .collect();
        if !failures.is_empty() {
            return Err(format!("solc reported errors: {}", failures.join("\n")));
        }
    }
    let source_entry = &solc["sources"][source_name];
    let source_id = source_entry["id"]
        .as_i64()
        .ok_or("solc source id is missing")?;
    let ast = &source_entry["ast"];
    if ast["src"]
        .as_str()
        .and_then(|src| parse_span(src, source_name, source, source_id))
        .is_none()
    {
        return Err("compiled AST does not match the source bytes".into());
    }
    let mut walker = Walker {
        source_name,
        source,
        source_id,
        declarations: BTreeMap::new(),
        edges: Vec::new(),
        indexed_accesses: Vec::new(),
        effect_boundaries: Vec::new(),
        functions: BTreeMap::new(),
    };
    let root = Context {
        contract: String::new(),
        function: String::new(),
    };
    walker.collect_declarations(ast, &root, "");
    walker.collect_functions(ast);
    walker.attach_layout(&solc);
    walker.walk(ast, &root, "read");
    let (ssa_added, ssa_unmatched, ssa_sha) = if let Some(path) = ssa_path {
        let (bytes, ssa) = read_json(path)?;
        if ssa["schema"] != "slither-ssa-v1" || ssa["source_sha256"] != digest(&source_bytes) {
            return Err("SlithIR SSA report does not match the analysis source".into());
        }
        let (added, unmatched) = walker.import_ssa(&ssa)?;
        (added, unmatched, Some(digest(&bytes)))
    } else {
        (0, 0, None)
    };
    let mut report = walker.finish(
        forensic["identity"].clone(),
        digest(&source_bytes),
        digest(&forensic_bytes),
        digest(&coverage_bytes),
        &coverage,
    )?;
    report.pc_hints_with_variables = pc_source_hints
        .iter()
        .filter(|hint| !hint.verified_variable_ast_ids.is_empty())
        .count();
    report.pc_source_hints = pc_source_hints;
    report.verified_variables = verified_variables;
    report.source_coverage = source_coverage;
    report.ssa_dependency_edge_count = ssa_added;
    report.ssa_unmatched_declaration_operands = ssa_unmatched;
    report.ssa_report_sha256 = ssa_sha;
    Ok(report)
}

impl Walker<'_> {
    fn node_span(&self, node: &Value) -> Option<SourceSpan> {
        parse_span(
            node["src"].as_str()?,
            self.source_name,
            self.source,
            self.source_id,
        )
    }

    fn expression(&self, node: &Value) -> Option<String> {
        let fragment = self.node_span(node)?.slice(self.source)?.trim();
        if fragment.is_empty() {
            return None;
        }
        Some(fragment.chars().take(180).collect())
    }

    fn context(node: &Value, parent: &Context) -> Context {
        let mut current = parent.clone();
        match node["nodeType"].as_str() {
            Some("ContractDefinition") => {
                current.contract = node["name"].as_str().unwrap_or("").into();
                current.function.clear();
            }
            Some("FunctionDefinition") | Some("ModifierDefinition") => {
                let name = node["name"].as_str().unwrap_or("");
                current.function = if name.is_empty() {
                    node["kind"].as_str().unwrap_or("anonymous").into()
                } else {
                    name.into()
                };
            }
            _ => {}
        }
        current
    }

    fn children<'a>(node: &'a Value) -> Vec<&'a Value> {
        let mut result = Vec::new();
        if let Some(object) = node.as_object() {
            for value in object.values() {
                match value {
                    Value::Object(_) if value.get("nodeType").is_some() => result.push(value),
                    Value::Array(items) => {
                        result.extend(items.iter().filter(|item| item.get("nodeType").is_some()))
                    }
                    _ => {}
                }
            }
        }
        result
    }

    fn collect_declarations(&mut self, node: &Value, parent: &Context, parent_type: &str) {
        let current = Self::context(node, parent);
        if node["nodeType"] == "VariableDeclaration" {
            if let Some(id) = node["id"].as_i64() {
                let state = node["stateVariable"] == true;
                let mutability = node["mutability"].as_str().unwrap_or("mutable");
                let category = if state {
                    match mutability {
                        "constant" => "constant",
                        "immutable" => "immutable",
                        _ if node["storageLocation"] == "transient" => "transient_state",
                        _ => "persistent_state",
                    }
                } else if node["storageLocation"] == "calldata" {
                    "calldata"
                } else if node["storageLocation"] == "memory" {
                    "memory"
                } else if node["storageLocation"] == "storage" {
                    "storage_reference"
                } else if current.function.is_empty() {
                    "declaration"
                } else {
                    "local_or_parameter"
                };
                self.declarations.insert(
                    id,
                    VariableRecord {
                        ast_id: id,
                        name: node["name"].as_str().unwrap_or("").into(),
                        declaring_contract: current.contract.clone(),
                        scope: current.function.clone(),
                        declaration_role: if parent_type == "ParameterList" {
                            "parameter"
                        } else if parent_type == "StructDefinition" {
                            "struct_member"
                        } else {
                            "variable"
                        }
                        .into(),
                        category: category.into(),
                        type_name: node["typeDescriptions"]["typeString"]
                            .as_str()
                            .unwrap_or("")
                            .into(),
                        mutability: mutability.into(),
                        data_location: node["storageLocation"].as_str().unwrap_or("default").into(),
                        declaration_span: self.node_span(node),
                        layout: Vec::new(),
                        uses: Vec::new(),
                        upstream_variable_ids: Vec::new(),
                        downstream_variable_ids: Vec::new(),
                        priority_score: 0,
                        priority_reasons: Vec::new(),
                    },
                );
            }
        }
        let kind = node["nodeType"].as_str().unwrap_or("");
        for child in Self::children(node) {
            self.collect_declarations(child, &current, kind);
        }
    }

    fn attach_layout(&mut self, solc: &Value) {
        let Some(contracts) = solc["contracts"][self.source_name].as_object() else {
            return;
        };
        for (contract, output) in contracts {
            let Some(storage) = output["storageLayout"]["storage"].as_array() else {
                continue;
            };
            for item in storage {
                let Some(id) = item["astId"].as_i64() else {
                    continue;
                };
                if let Some(record) = self.declarations.get_mut(&id) {
                    record.layout.push(LayoutPosition {
                        contract: contract.clone(),
                        slot: item["slot"].as_str().unwrap_or("").into(),
                        offset: item["offset"].as_u64().unwrap_or(0),
                        type_id: item["type"].as_str().unwrap_or("").into(),
                    });
                }
            }
        }
    }

    fn collect_functions(&mut self, node: &Value) {
        if node["nodeType"] == "FunctionDefinition" {
            if let Some(id) = node["id"].as_i64() {
                let parameters = node["parameters"]["parameters"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|item| item["id"].as_i64())
                    .collect();
                let mut returns = BTreeSet::new();
                if let Some(items) = node["returnParameters"]["parameters"].as_array() {
                    for item in items {
                        if let Some(id) = item["id"].as_i64() {
                            returns.insert(id);
                        }
                    }
                }
                fn return_refs(node: &Value, output: &mut BTreeSet<i64>) {
                    if node["nodeType"] == "Return" {
                        Walker::refs(&node["expression"], output);
                        return;
                    }
                    for child in Walker::children(node) {
                        return_refs(child, output);
                    }
                }
                return_refs(&node["body"], &mut returns);
                returns.retain(|id| self.declarations.contains_key(id));
                self.functions.insert(
                    id,
                    FunctionSummary {
                        parameters,
                        return_sources: returns.into_iter().collect(),
                        visibility: node["visibility"].as_str().unwrap_or("unknown").into(),
                        mutability: node["stateMutability"].as_str().unwrap_or("unknown").into(),
                    },
                );
            }
        }
        for child in Self::children(node) {
            self.collect_functions(child);
        }
    }

    fn call_return_sources(&self, call: &Value) -> Vec<i64> {
        if call["expression"]["nodeType"] != "Identifier" {
            return Vec::new();
        }
        call["expression"]["referencedDeclaration"]
            .as_i64()
            .and_then(|id| self.functions.get(&id))
            .map(|summary| summary.return_sources.clone())
            .unwrap_or_default()
    }

    fn connect_call_result(&mut self, targets: &BTreeSet<i64>, call: &Value, context: &Context) {
        if call["nodeType"] != "FunctionCall" {
            return;
        }
        for from in self.call_return_sources(call) {
            for to in targets {
                if !self.declarations.contains_key(to) || from == *to {
                    continue;
                }
                self.edges.push(VariableEdge {
                    from_ast_id: from,
                    to_ast_id: *to,
                    relation: "internal_return_to_result".into(),
                    function: context.function.clone(),
                    source_span: self.node_span(call),
                    expression: self.expression(call),
                    confidence: "syntactic_call_summary".into(),
                    evidence_source: "solc_ast".into(),
                });
            }
        }
    }

    fn connect_call_arguments(&mut self, call: &Value, context: &Context) {
        if call["expression"]["nodeType"] != "Identifier" {
            return;
        }
        let Some(id) = call["expression"]["referencedDeclaration"].as_i64() else {
            return;
        };
        let Some(summary) = self.functions.get(&id) else {
            return;
        };
        let parameters = summary.parameters.clone();
        let Some(arguments) = call["arguments"].as_array() else {
            return;
        };
        for (argument, to) in arguments.iter().zip(parameters) {
            if !self.declarations.contains_key(&to) {
                continue;
            }
            let mut refs = BTreeSet::new();
            Self::refs(argument, &mut refs);
            for from in refs {
                if !self.declarations.contains_key(&from) || from == to {
                    continue;
                }
                self.edges.push(VariableEdge {
                    from_ast_id: from,
                    to_ast_id: to,
                    relation: "internal_argument_to_parameter".into(),
                    function: context.function.clone(),
                    source_span: self.node_span(argument),
                    expression: self.expression(argument),
                    confidence: "syntactic_call_summary".into(),
                    evidence_source: "solc_ast".into(),
                });
            }
        }
    }

    fn record_effect_call(&mut self, node: &Value, context: &Context) {
        let member = &node["expression"];
        if member["nodeType"] != "MemberAccess" {
            return;
        }
        let method = member["memberName"].as_str().unwrap_or("");
        let receiver_type = member["expression"]["typeDescriptions"]["typeString"]
            .as_str()
            .unwrap_or("");
        let referenced = member["referencedDeclaration"].as_i64();
        let summary = referenced.and_then(|id| self.functions.get(&id));
        let (kind, effect_class) = if matches!(method, "call" | "delegatecall" | "staticcall")
            && receiver_type.starts_with("address")
        {
            (
                "low_level_call",
                if method == "staticcall" {
                    "read_or_revert"
                } else {
                    "unknown_external_or_storage_effect"
                },
            )
        } else if matches!(method, "send" | "transfer")
            && receiver_type.starts_with("address payable")
        {
            ("native_value_call", "external_state_change_possible")
        } else if receiver_type.starts_with("contract ")
            && !summary
                .is_some_and(|item| matches!(item.visibility.as_str(), "internal" | "private"))
        {
            (
                "contract_call",
                if summary.is_some_and(|item| matches!(item.mutability.as_str(), "view" | "pure")) {
                    "read_or_revert"
                } else {
                    "external_state_change_possible"
                },
            )
        } else {
            return;
        };
        let mut refs = BTreeSet::new();
        Self::refs(node, &mut refs);
        refs.retain(|id| self.declarations.contains_key(id));
        self.effect_boundaries.push(EffectBoundary {
            kind: kind.into(),
            contract: context.contract.clone(),
            function: context.function.clone(),
            method: Some(method.into()),
            receiver_type: Some(receiver_type.into()),
            source_variable_ast_ids: refs.into_iter().collect(),
            source_span: self.node_span(node),
            expression: self.expression(node),
            effect_class: effect_class.into(),
            yul_operations: Vec::new(),
            literal_storage_slots: Vec::new(),
            confidence: "source_call_boundary".into(),
        });
    }

    fn record_assembly(&mut self, node: &Value, context: &Context) {
        fn operations(
            node: &Value,
            output: &mut BTreeSet<String>,
            slots: &mut Vec<AssemblySlotUse>,
        ) {
            if node["nodeType"] == "YulFunctionCall" {
                if let Some(name) = node["functionName"]["name"].as_str() {
                    output.insert(name.into());
                    if matches!(name, "sload" | "sstore" | "tload" | "tstore") {
                        if let Some(slot) =
                            node["arguments"].as_array().and_then(|args| args.first())
                        {
                            if slot["nodeType"] == "YulLiteral" {
                                if let Some(value) = slot["value"].as_str() {
                                    let parsed = if let Some(hex) = value.strip_prefix("0x") {
                                        U256::from_str_radix(hex, 16)
                                    } else {
                                        U256::from_str_radix(value, 10)
                                    };
                                    if let Ok(parsed) = parsed {
                                        slots.push(AssemblySlotUse {
                                            operation: name.into(),
                                            slot: parsed.to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for child in Walker::children(node) {
                operations(child, output, slots);
            }
        }
        let mut names = BTreeSet::new();
        let mut slots = Vec::new();
        operations(node, &mut names, &mut slots);
        slots.sort_by(|a, b| (&a.operation, &a.slot).cmp(&(&b.operation, &b.slot)));
        slots.dedup_by(|a, b| a.operation == b.operation && a.slot == b.slot);
        self.effect_boundaries.push(EffectBoundary {
            kind: "inline_assembly".into(),
            contract: context.contract.clone(),
            function: context.function.clone(),
            method: None,
            receiver_type: None,
            source_variable_ast_ids: Vec::new(),
            source_span: self.node_span(node),
            expression: self.expression(node),
            effect_class: if names.iter().any(|name| {
                matches!(
                    name.as_str(),
                    "sstore"
                        | "tstore"
                        | "call"
                        | "delegatecall"
                        | "create"
                        | "create2"
                        | "selfdestruct"
                )
            }) {
                "storage_or_external_effect_possible"
            } else {
                "opaque_assembly_boundary"
            }
            .into(),
            yul_operations: names.into_iter().collect(),
            literal_storage_slots: slots,
            confidence: "source_assembly_boundary".into(),
        });
    }

    fn refs(node: &Value, output: &mut BTreeSet<i64>) {
        if let Some(id) = node["referencedDeclaration"].as_i64() {
            if id >= 0 {
                output.insert(id);
            }
        }
        for child in Self::children(node) {
            Self::refs(child, output);
        }
    }

    fn targets(node: &Value, output: &mut BTreeSet<i64>) {
        match node["nodeType"].as_str() {
            Some("TupleExpression") => {
                if let Some(items) = node["components"].as_array() {
                    for item in items {
                        Self::targets(item, output);
                    }
                }
            }
            Some("IndexAccess") | Some("IndexRangeAccess") => {
                Self::targets(&node["baseExpression"], output)
            }
            _ => {
                if let Some(id) = node["referencedDeclaration"].as_i64() {
                    if id >= 0 {
                        output.insert(id);
                    }
                }
            }
        }
    }

    fn record(&mut self, id: i64, kind: &str, node: &Value, context: &Context) {
        let source_span = self.node_span(node);
        let expression = self.expression(node);
        if let Some(variable) = self.declarations.get_mut(&id) {
            variable.uses.push(VariableUse {
                kind: kind.into(),
                contract: context.contract.clone(),
                function: context.function.clone(),
                source_span,
                expression,
            });
        }
    }

    fn key_identity(node: &Value) -> (String, Option<String>) {
        match node["nodeType"].as_str() {
            Some("Literal") => {
                let kind = node["kind"].as_str().unwrap_or("");
                let value = node["value"].as_str().unwrap_or("");
                if kind == "number" {
                    let value = value.replace('_', "");
                    let parsed = if let Some(hex) = value.strip_prefix("0x") {
                        U256::from_str_radix(hex, 16)
                    } else {
                        U256::from_str_radix(&value, 10)
                    };
                    if let Ok(parsed) = parsed {
                        return ("fixed_literal".into(), Some(format!("number:{parsed}")));
                    }
                }
                if kind == "bool" && matches!(value, "true" | "false") {
                    return ("fixed_literal".into(), Some(format!("bool:{value}")));
                }
                if matches!(kind, "string" | "unicodeString" | "hexString") {
                    if let Some(hex) = node["hexValue"].as_str() {
                        return ("fixed_literal".into(), Some(format!("bytes:{hex}")));
                    }
                }
            }
            Some("Identifier") => {
                if let Some(id) = node["referencedDeclaration"].as_i64().filter(|id| *id >= 0) {
                    return (
                        "symbolic_declaration".into(),
                        Some(format!("declaration:{id}")),
                    );
                }
            }
            Some("MemberAccess") => {
                if node["memberName"] == "sender" && node["expression"]["name"] == "msg" {
                    return ("transaction_actor".into(), Some("msg.sender".into()));
                }
                if let Some(id) = node["referencedDeclaration"].as_i64().filter(|id| *id >= 0) {
                    return (
                        "symbolic_declaration".into(),
                        Some(format!("declaration:{id}")),
                    );
                }
            }
            _ => {}
        }
        ("unresolved_expression".into(), None)
    }

    fn parent_key_identities(node: &Value) -> Vec<Option<String>> {
        if node["nodeType"] != "IndexAccess" {
            return Vec::new();
        }
        let mut result = Self::parent_key_identities(&node["baseExpression"]);
        result.push(Self::key_identity(&node["indexExpression"]).1);
        result
    }

    fn record_index(&mut self, node: &Value, context: &Context, access: &str) {
        if node["nodeType"] != "IndexAccess" {
            return;
        }
        let mut base = BTreeSet::new();
        Self::targets(&node["baseExpression"], &mut base);
        let mut keys = BTreeSet::new();
        Self::refs(&node["indexExpression"], &mut keys);
        let kind = base
            .iter()
            .filter_map(|id| self.declarations.get(id))
            .map(|record| record.type_name.as_str())
            .find_map(|name| {
                if name.contains("mapping(") {
                    Some("mapping")
                } else if name.contains('[') {
                    Some("array")
                } else {
                    None
                }
            })
            .unwrap_or("indexed_unknown");
        self.indexed_accesses.push(IndexedAccess {
            base_ast_ids: base.into_iter().collect(),
            key_variable_ast_ids: keys
                .into_iter()
                .filter(|id| self.declarations.contains_key(id))
                .collect(),
            key_expression: self.expression(&node["indexExpression"]),
            expression: self.expression(node),
            function: context.function.clone(),
            contract: context.contract.clone(),
            access: access.into(),
            collection_kind: kind.into(),
            key_identity_kind: Self::key_identity(&node["indexExpression"]).0,
            key_identity: Self::key_identity(&node["indexExpression"]).1,
            parent_key_identities: Self::parent_key_identities(&node["baseExpression"]),
            source_span: self.node_span(node),
        });
    }

    fn record_index_chain(&mut self, node: &Value, context: &Context, access: &str) {
        if node["nodeType"] != "IndexAccess" {
            return;
        }
        self.record_index(node, context, access);
        self.walk(&node["indexExpression"], context, "index_key");
        self.record_index_chain(&node["baseExpression"], context, access);
    }

    fn connect(
        &mut self,
        targets: &BTreeSet<i64>,
        origin: &Value,
        context: &Context,
        relation: &str,
    ) {
        let mut sources = BTreeSet::new();
        Self::refs(origin, &mut sources);
        let span = self.node_span(origin);
        let expression = self.expression(origin);
        for from in sources {
            if !self.declarations.contains_key(&from) {
                continue;
            }
            for to in targets {
                if !self.declarations.contains_key(to) {
                    continue;
                }
                self.edges.push(VariableEdge {
                    from_ast_id: from,
                    to_ast_id: *to,
                    relation: relation.into(),
                    function: context.function.clone(),
                    source_span: span.clone(),
                    expression: expression.clone(),
                    confidence: "direct_syntax".into(),
                    evidence_source: "solc_ast".into(),
                });
            }
        }
    }

    fn walk(&mut self, node: &Value, parent: &Context, mode: &str) {
        let current = Self::context(node, parent);
        match node["nodeType"].as_str() {
            Some("Assignment") => {
                let left = &node["leftHandSide"];
                let right = &node["rightHandSide"];
                let mut targets = BTreeSet::new();
                Self::targets(left, &mut targets);
                for id in &targets {
                    self.record(*id, "write", left, &current);
                    if node["operator"] != "=" {
                        self.record(*id, "read", left, &current);
                    }
                }
                self.connect(&targets, right, &current, "assignment");
                self.connect_call_result(&targets, right, &current);
                self.walk(right, &current, "read");
                if left["nodeType"] == "IndexAccess" {
                    self.record_index_chain(left, &current, "write");
                }
                return;
            }
            Some("IndexAccess") => {
                self.record_index(node, &current, mode);
            }
            Some("VariableDeclarationStatement") => {
                let mut targets = BTreeSet::new();
                if let Some(items) = node["declarations"].as_array() {
                    for item in items {
                        if let Some(id) = item["id"].as_i64() {
                            targets.insert(id);
                            self.record(id, "write", item, &current);
                        }
                    }
                }
                if !node["initialValue"].is_null() {
                    self.connect(&targets, &node["initialValue"], &current, "initialization");
                    self.connect_call_result(&targets, &node["initialValue"], &current);
                    self.walk(&node["initialValue"], &current, "read");
                }
                return;
            }
            Some("VariableDeclaration") => {
                if let (Some(id), Some(value)) = (node["id"].as_i64(), node.get("value")) {
                    if !value.is_null() {
                        let targets = BTreeSet::from([id]);
                        self.record(id, "write", node, &current);
                        self.connect(&targets, value, &current, "initialization");
                        self.connect_call_result(&targets, value, &current);
                        self.walk(value, &current, "read");
                    }
                }
                return;
            }
            Some("UnaryOperation")
                if matches!(node["operator"].as_str(), Some("++" | "--" | "delete")) =>
            {
                let operand = &node["subExpression"];
                let mut targets = BTreeSet::new();
                Self::targets(operand, &mut targets);
                for id in targets {
                    self.record(id, "write", node, &current);
                    if node["operator"] != "delete" {
                        self.record(id, "read", node, &current);
                    }
                }
                return;
            }
            Some(
                "IfStatement" | "WhileStatement" | "DoWhileStatement" | "ForStatement"
                | "Conditional",
            ) => {
                if let Some(condition) = node.get("condition") {
                    self.walk(condition, &current, "guard_read");
                }
                for child in Self::children(node) {
                    if node.get("condition") != Some(child) {
                        self.walk(child, &current, mode);
                    }
                }
                return;
            }
            Some("Return") => {
                if let Some(expression) = node.get("expression") {
                    self.walk(expression, &current, "return_read");
                }
                return;
            }
            Some("FunctionCall") => {
                self.connect_call_arguments(node, &current);
                self.record_effect_call(node, &current);
                let callee = node["expression"]["name"].as_str().unwrap_or("");
                let kind = if matches!(callee, "require" | "assert") {
                    "guard_read"
                } else {
                    "call_argument"
                };
                if let Some(arguments) = node["arguments"].as_array() {
                    for argument in arguments {
                        self.walk(argument, &current, kind);
                    }
                }
                self.walk(&node["expression"], &current, "call_target_read");
                return;
            }
            Some("InlineAssembly") => {
                self.record_assembly(node, &current);
                return;
            }
            Some("Identifier" | "MemberAccess") => {
                if let Some(id) = node["referencedDeclaration"].as_i64() {
                    self.record(id, mode, node, &current);
                }
            }
            _ => {}
        }
        for child in Self::children(node) {
            self.walk(child, &current, mode);
        }
    }

    fn import_ssa(&mut self, report: &Value) -> Result<(usize, usize), String> {
        let operations = report["operations"]
            .as_array()
            .ok_or("SlithIR SSA operations are missing")?;
        if report["operation_count"].as_u64() != Some(operations.len() as u64) {
            return Err("SlithIR SSA operation count differs".into());
        }
        let declaration_positions: BTreeMap<(usize, String), i64> = self
            .declarations
            .iter()
            .filter_map(|(id, record)| {
                Some((
                    (
                        record.declaration_span.as_ref()?.start_byte(),
                        record.name.clone(),
                    ),
                    *id,
                ))
            })
            .collect();
        let mut anchors: BTreeMap<String, i64> = BTreeMap::new();
        let mut unmatched = BTreeSet::new();
        let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for operation in operations {
            let write = &operation["write"];
            let write_key = write["key"]
                .as_str()
                .ok_or("SlithIR SSA write key is missing")?;
            anchor_ssa_operand(write, &declaration_positions, &mut anchors, &mut unmatched);
            let reads = operation["reads"]
                .as_array()
                .ok_or("SlithIR SSA reads are missing")?;
            for read in reads {
                let read_key = read["key"]
                    .as_str()
                    .ok_or("SlithIR SSA read key is missing")?;
                anchor_ssa_operand(read, &declaration_positions, &mut anchors, &mut unmatched);
                graph
                    .entry(read_key.into())
                    .or_default()
                    .insert(write_key.into());
            }
        }
        let mut new_edges = BTreeSet::new();
        for (start_key, from_id) in &anchors {
            let mut visited = BTreeSet::new();
            let mut queue = VecDeque::from([start_key.clone()]);
            while let Some(current) = queue.pop_front() {
                if let Some(next) = graph.get(&current) {
                    for key in next {
                        if !visited.insert(key.clone()) {
                            continue;
                        }
                        if let Some(to_id) = anchors.get(key) {
                            if to_id != from_id {
                                let function = start_key.split(':').nth(1).unwrap_or("").to_owned();
                                new_edges.insert((*from_id, *to_id, function));
                            }
                        }
                        queue.push_back(key.clone());
                    }
                }
            }
        }
        let count = new_edges.len();
        for (from, to, function) in new_edges {
            self.edges.push(VariableEdge {
                from_ast_id: from,
                to_ast_id: to,
                relation: "slithir_ssa_dependency".into(),
                function,
                source_span: None,
                expression: None,
                confidence: "inferred_ssa".into(),
                evidence_source: "slither_ssa".into(),
            });
        }
        Ok((count, unmatched.len()))
    }

    fn finish(
        mut self,
        identity: Value,
        source_sha: String,
        forensic_sha: String,
        coverage_sha: String,
        coverage: &Value,
    ) -> Result<VariableLensReport, String> {
        self.indexed_accesses.sort_by(|a, b| {
            (
                &a.base_ast_ids,
                &a.key_variable_ast_ids,
                &a.function,
                &a.expression,
                &a.access,
            )
                .cmp(&(
                    &b.base_ast_ids,
                    &b.key_variable_ast_ids,
                    &b.function,
                    &b.expression,
                    &b.access,
                ))
        });
        self.indexed_accesses.dedup_by(|a, b| {
            a.base_ast_ids == b.base_ast_ids
                && a.key_variable_ast_ids == b.key_variable_ast_ids
                && a.contract == b.contract
                && a.function == b.function
                && a.expression == b.expression
                && a.access == b.access
        });
        let (key_alias_relations, key_alias_pairs_truncated) = key_aliases(&self.indexed_accesses);
        self.effect_boundaries.sort_by(|a, b| {
            (&a.contract, &a.function, &a.kind, &a.expression).cmp(&(
                &b.contract,
                &b.function,
                &b.kind,
                &b.expression,
            ))
        });
        self.effect_boundaries.dedup_by(|a, b| {
            a.contract == b.contract
                && a.function == b.function
                && a.kind == b.kind
                && a.expression == b.expression
                && a.source_span == b.source_span
        });
        self.edges.sort_by(|a, b| {
            (
                a.from_ast_id,
                a.to_ast_id,
                &a.relation,
                &a.function,
                &a.expression,
            )
                .cmp(&(
                    b.from_ast_id,
                    b.to_ast_id,
                    &b.relation,
                    &b.function,
                    &b.expression,
                ))
        });
        self.edges.dedup_by(|a, b| {
            a.from_ast_id == b.from_ast_id
                && a.to_ast_id == b.to_ast_id
                && a.relation == b.relation
                && a.function == b.function
                && a.expression == b.expression
        });
        let mut forward: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
        let mut backward: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
        for edge in &self.edges {
            forward
                .entry(edge.from_ast_id)
                .or_default()
                .insert(edge.to_ast_id);
            backward
                .entry(edge.to_ast_id)
                .or_default()
                .insert(edge.from_ast_id);
        }
        let mut category_counts = BTreeMap::new();
        let mut declaration_role_counts = BTreeMap::new();
        let mut without_use = 0;
        let mut read_without_writer = 0;
        for (id, record) in &mut self.declarations {
            record.uses.sort_by(|a, b| {
                (&a.kind, &a.contract, &a.function, &a.expression).cmp(&(
                    &b.kind,
                    &b.contract,
                    &b.function,
                    &b.expression,
                ))
            });
            record.uses.dedup_by(|a, b| {
                a.kind == b.kind
                    && a.contract == b.contract
                    && a.function == b.function
                    && a.expression == b.expression
                    && a.source_span == b.source_span
            });
            record.upstream_variable_ids = closure(*id, &backward);
            record.downstream_variable_ids = closure(*id, &forward);
            *category_counts.entry(record.category.clone()).or_insert(0) += 1;
            *declaration_role_counts
                .entry(record.declaration_role.clone())
                .or_insert(0) += 1;
            if record.uses.is_empty() {
                without_use += 1;
            }
            let has_read = record.uses.iter().any(|item| item.kind.contains("read"));
            let has_write = record.uses.iter().any(|item| item.kind == "write");
            if has_read && !has_write && record.category == "persistent_state" {
                read_without_writer += 1;
            }
            let mut score = 0u8;
            if record.category == "persistent_state" {
                score += 35;
                record.priority_reasons.push("persistent_state".into());
            } else if record.category == "immutable" {
                score += 25;
                record.priority_reasons.push("immutable_state".into());
            }
            if has_write {
                score += 15;
                record.priority_reasons.push("observed_source_write".into());
            }
            if record.uses.iter().any(|item| item.kind == "guard_read") {
                score += 15;
                record.priority_reasons.push("used_in_condition".into());
            }
            if record.uses.iter().any(|item| item.kind == "call_argument") {
                score += 10;
                record.priority_reasons.push("passed_to_call".into());
            }
            if !record.upstream_variable_ids.is_empty()
                && !record.downstream_variable_ids.is_empty()
            {
                score += 10;
                record
                    .priority_reasons
                    .push("intermediate_value_path".into());
            }
            record.priority_score = score;
        }
        let (state_transition_links, state_transition_links_truncated) = state_transitions(
            &self.declarations,
            &self.indexed_accesses,
            &key_alias_relations,
        );
        let observed_root_jump_sites = coverage["vault_jump_sites_observed"]
            .as_u64()
            .ok_or("local coverage lacks observed vault jump sites")?
            as usize;
        let observed_root_branch_outcomes = coverage["vault_branch_outcomes_observed"]
            .as_u64()
            .ok_or("local coverage lacks observed vault outcomes")?
            as usize;
        Ok(VariableLensReport {
            schema: "variable-lens-v1".into(),
            identity,
            analysis_source_sha256: source_sha,
            forensic_report_sha256: forensic_sha,
            local_coverage_sha256: coverage_sha,
            variable_count: self.declarations.len(),
            category_counts,
            declaration_role_counts,
            dependency_edge_count: self.edges.len(),
            cross_function_edge_count: self.edges.iter().filter(|edge| edge.relation.starts_with("internal_")).count(),
            indexed_access_count: self.indexed_accesses.len(),
            indexed_accesses: self.indexed_accesses,
            key_alias_relations,
            key_alias_pairs_truncated,
            state_transition_links,
            state_transition_links_truncated,
            effect_boundaries: self.effect_boundaries,
            ssa_dependency_edge_count: 0,
            ssa_unmatched_declaration_operands: 0,
            ssa_report_sha256: None,
            variables_without_source_use: without_use,
            variables_with_read_without_writer: read_without_writer,
            observed_root_jump_sites,
            observed_root_branch_outcomes,
            pc_hints_with_variables: 0,
            pc_source_hints: Vec::new(),
            source_coverage: SourceCoverageSummary {
                runtime_static_jump_sites: 0,
                uniquely_mapped_static_jump_sites: 0,
                unattributed_static_jump_sites: 0,
                observed_attributed_jump_sites: 0,
                observed_attributed_outcomes: 0,
                function_source_coverage: Vec::new(),
                pc_source_origins: Vec::new(),
                denominator_kind: "uniquely_source_mapped_static_jump_sites_not_reachable_paths".into(),
            },
            verified_variables: Vec::new(),
            variables: self.declarations.into_values().collect(),
            edges: self.edges,
            limitations: vec![
                "Dependencies are syntactic AST paths; reachability and cross-call value propagation are not established".into(),
                "The flattened analysis source is not the verified multifile deployment bundle; layout and variable names are source-backed but PC/slot correspondence to the deployed runtime is not asserted".into(),
                "Observed branch PCs have tentative verified-source range hints; a source range can include variables unrelated to the branch predicate".into(),
                "Verified-source AST IDs and flattened-analysis AST IDs belong to separate compilations and are never merged by name alone".into(),
                "Mapping keys and array indices are parameterized families, not an enumeration of every possible element".into(),
                "Assembly, proxies, delegatecall, external effects and path feasibility require separate analysis".into(),
                "Priority is an explainable review ordering, not a probability of a problem".into(),
            ],
        })
    }
}

fn anchor_ssa_operand(
    operand: &Value,
    positions: &BTreeMap<(usize, String), i64>,
    anchors: &mut BTreeMap<String, i64>,
    unmatched: &mut BTreeSet<String>,
) {
    let (Some(key), Some(name), Some(start)) = (
        operand["key"].as_str(),
        operand["name"].as_str(),
        operand["start"].as_u64(),
    ) else {
        return;
    };
    if let Some(id) = positions.get(&(start as usize, name.to_owned())) {
        anchors.insert(key.into(), *id);
    } else {
        unmatched.insert(key.into());
    }
}

fn closure(start: i64, edges: &BTreeMap<i64, BTreeSet<i64>>) -> Vec<i64> {
    let mut found = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        if let Some(next) = edges.get(&current) {
            for id in next {
                if *id != start && found.insert(*id) {
                    queue.push_back(*id);
                }
            }
        }
    }
    found.into_iter().collect()
}

fn is_fixed_key(value: &str) -> bool {
    value.starts_with("number:") || value.starts_with("bool:") || value.starts_with("bytes:")
}

fn alias_relation(left: &IndexedAccess, right: &IndexedAccess) -> (&'static str, &'static str) {
    let mut left_keys = left.parent_key_identities.clone();
    left_keys.push(left.key_identity.clone());
    let mut right_keys = right.parent_key_identities.clone();
    right_keys.push(right.key_identity.clone());
    if left_keys.len() != right_keys.len() {
        return ("different_index_depth", "structural");
    }
    if left_keys.iter().zip(&right_keys).any(|(a, b)| {
        matches!((a, b), (Some(a), Some(b)) if is_fixed_key(a) && is_fixed_key(b) && a != b)
    }) {
        return ("distinct_fixed_keys", "proven_for_normalized_literals");
    }
    if left_keys
        .iter()
        .zip(&right_keys)
        .all(|(a, b)| matches!((a, b), (Some(a), Some(b)) if a == b && is_fixed_key(a)))
    {
        return ("same_fixed_key", "proven_for_normalized_literals");
    }
    if left_keys
        .iter()
        .zip(&right_keys)
        .all(|(a, b)| matches!((a, b), (Some(a), Some(b)) if a == b))
    {
        return ("same_symbolic_key", "possible_same_value_only");
    }
    ("may_overlap", "unresolved_symbolic_values")
}

fn key_aliases(accesses: &[IndexedAccess]) -> (Vec<KeyAliasRelation>, bool) {
    const MAX_PAIRS: usize = 20_000;
    let mut output = Vec::new();
    for left in 0..accesses.len() {
        for right in left + 1..accesses.len() {
            if accesses[left].base_ast_ids.is_empty()
                || accesses[left].base_ast_ids != accesses[right].base_ast_ids
            {
                continue;
            }
            if output.len() == MAX_PAIRS {
                return (output, true);
            }
            let (relation, confidence) = alias_relation(&accesses[left], &accesses[right]);
            output.push(KeyAliasRelation {
                left_access: left,
                right_access: right,
                base_ast_ids: accesses[left].base_ast_ids.clone(),
                relation: relation.into(),
                confidence: confidence.into(),
            });
        }
    }
    (output, false)
}

fn transition_key_relation(
    id: i64,
    writer: &VariableUse,
    reader: &VariableUse,
    accesses: &[IndexedAccess],
    aliases: &[KeyAliasRelation],
) -> String {
    let writes: BTreeSet<usize> = accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| {
            access.base_ast_ids.contains(&id)
                && access.contract == writer.contract
                && access.function == writer.function
                && access.access == "write"
        })
        .map(|(index, _)| index)
        .collect();
    let reads: BTreeSet<usize> = accesses
        .iter()
        .enumerate()
        .filter(|(_, access)| {
            access.base_ast_ids.contains(&id)
                && access.contract == reader.contract
                && access.function == reader.function
                && access.access != "write"
        })
        .map(|(index, _)| index)
        .collect();
    if writes.is_empty() || reads.is_empty() {
        return "element_key_unresolved".into();
    }
    let mut relations = BTreeSet::new();
    for alias in aliases {
        if (writes.contains(&alias.left_access) && reads.contains(&alias.right_access))
            || (writes.contains(&alias.right_access) && reads.contains(&alias.left_access))
        {
            relations.insert(alias.relation.as_str());
        }
    }
    if relations.contains("same_fixed_key") {
        "same_fixed_key".into()
    } else if relations.contains("same_symbolic_key") {
        "same_symbolic_key_possible".into()
    } else if relations.len() == 1 && relations.contains("distinct_fixed_keys") {
        "distinct_fixed_keys".into()
    } else {
        "element_key_may_overlap".into()
    }
}

fn state_transitions(
    declarations: &BTreeMap<i64, VariableRecord>,
    accesses: &[IndexedAccess],
    aliases: &[KeyAliasRelation],
) -> (Vec<StateTransitionLink>, bool) {
    const MAX_LINKS: usize = 10_000;
    let mut output = Vec::new();
    let mut unique = BTreeSet::new();
    for (id, record) in declarations {
        if record.category != "persistent_state" {
            continue;
        }
        let writers: Vec<_> = record
            .uses
            .iter()
            .filter(|item| item.kind == "write" && !item.function.is_empty())
            .collect();
        let readers: Vec<_> = record
            .uses
            .iter()
            .filter(|item| item.kind != "write" && !item.function.is_empty())
            .collect();
        for writer in &writers {
            for reader in &readers {
                let key_relation =
                    if record.type_name.contains("mapping(") || record.type_name.contains('[') {
                        transition_key_relation(*id, writer, reader, accesses, aliases)
                    } else {
                        "same_state_variable".into()
                    };
                let key = (
                    *id,
                    writer.contract.clone(),
                    writer.function.clone(),
                    reader.contract.clone(),
                    reader.function.clone(),
                    key_relation.clone(),
                );
                if !unique.insert(key) {
                    continue;
                }
                if output.len() == MAX_LINKS {
                    return (output, true);
                }
                output.push(StateTransitionLink {
                    state_variable_ast_id: *id,
                    writer_contract: writer.contract.clone(),
                    writer_function: writer.function.clone(),
                    reader_contract: reader.contract.clone(),
                    reader_function: reader.function.clone(),
                    key_relation,
                    confidence: "possible_transaction_order_from_source".into(),
                });
            }
        }
    }
    (output, false)
}

#[derive(Clone, Copy)]
struct SourceRange {
    start: usize,
    end: usize,
    file_id: i64,
}

struct VerifiedFunctionRange {
    ast_id: i64,
    source_file: String,
    contract: String,
    function: String,
    kind: String,
    range: SourceRange,
}

fn verified_function_ranges(
    node: &Value,
    parent: &Context,
    files: &BTreeMap<i64, String>,
    output: &mut Vec<VerifiedFunctionRange>,
) {
    let current = Walker::context(node, parent);
    if matches!(
        node["nodeType"].as_str(),
        Some("FunctionDefinition" | "ModifierDefinition")
    ) {
        if let (Some(ast_id), Some(range)) = (node["id"].as_i64(), raw_range(node)) {
            if let Some(source_file) = files.get(&range.file_id) {
                output.push(VerifiedFunctionRange {
                    ast_id,
                    source_file: source_file.clone(),
                    contract: current.contract.clone(),
                    function: current.function.clone(),
                    kind: if node["nodeType"] == "ModifierDefinition" {
                        "modifier"
                    } else {
                        "function"
                    }
                    .into(),
                    range,
                });
            }
        }
    }
    for child in Walker::children(node) {
        verified_function_ranges(child, &current, files, output);
    }
}

fn raw_range(node: &Value) -> Option<SourceRange> {
    let mut parts = node["src"].as_str()?.split(':');
    let start = parts.next()?.parse::<usize>().ok()?;
    let len = parts.next()?.parse::<usize>().ok()?;
    let file_id = parts.next()?.parse::<i64>().ok()?;
    Some(SourceRange {
        start,
        end: start.checked_add(len)?,
        file_id,
    })
}

fn verified_references(
    node: &Value,
    context: &Context,
    files: &BTreeMap<i64, String>,
    declarations: &mut BTreeMap<i64, VerifiedVariable>,
    references: &mut Vec<(i64, SourceRange)>,
) {
    let current = Walker::context(node, context);
    match node["nodeType"].as_str() {
        Some("VariableDeclaration") => {
            if let (Some(id), Some(name)) = (node["id"].as_i64(), node["name"].as_str()) {
                let range = raw_range(node);
                declarations.insert(
                    id,
                    VerifiedVariable {
                        ast_id: id,
                        name: name.into(),
                        source_file: range.and_then(|item| files.get(&item.file_id).cloned()),
                        declaration_start_byte: range.map(|item| item.start),
                        declaration_end_byte: range.map(|item| item.end),
                        declaring_contract: current.contract.clone(),
                        scope: current.function.clone(),
                        type_name: node["typeDescriptions"]["typeString"]
                            .as_str()
                            .unwrap_or("")
                            .into(),
                        state_variable: node["stateVariable"] == true,
                        observed_branch_pcs: Vec::new(),
                    },
                );
            }
        }
        Some("Identifier" | "MemberAccess") => {
            if let (Some(id), Some(range)) =
                (node["referencedDeclaration"].as_i64(), raw_range(node))
            {
                references.push((id, range));
            }
        }
        _ => {}
    }
    for child in Walker::children(node) {
        verified_references(child, &current, files, declarations, references);
    }
}

fn decoded_source_map(text: &str) -> Result<Vec<Option<SourceRange>>, String> {
    let mut start = 0i64;
    let mut len = 0i64;
    let mut file_id = -1i64;
    let mut result = Vec::new();
    for item in text.split(';') {
        let fields: Vec<_> = item.split(':').collect();
        for (index, target) in [&mut start, &mut len, &mut file_id].into_iter().enumerate() {
            if let Some(value) = fields.get(index) {
                if !value.is_empty() {
                    *target = value
                        .parse::<i64>()
                        .map_err(|_| "invalid solc source map")?;
                }
            }
        }
        result.push(if start >= 0 && len > 0 && file_id >= 0 {
            Some(SourceRange {
                start: start as usize,
                end: start.checked_add(len).ok_or("source map range overflow")? as usize,
                file_id,
            })
        } else {
            None
        });
    }
    Ok(result)
}

fn instruction_pcs(bytes: &[u8]) -> Vec<usize> {
    let mut pcs = Vec::new();
    let mut pc = 0;
    while pc < bytes.len() {
        pcs.push(pc);
        let opcode = bytes[pc];
        pc += 1 + if (0x60..=0x7f).contains(&opcode) {
            usize::from(opcode - 0x5f)
        } else {
            0
        };
    }
    pcs
}

fn pc_hints(
    manifest: &ForensicInput,
    comparison: &Value,
    coverage: &Value,
    runtime_bytes: &[u8],
) -> Result<
    (
        Vec<PcSourceHint>,
        Vec<VerifiedVariable>,
        SourceCoverageSummary,
    ),
    String,
> {
    let deployed = &comparison["contracts"][&manifest.comparison_source_name]
        [&manifest.contract_name]["evm"]["deployedBytecode"];
    let compiled_hex = deployed["object"]
        .as_str()
        .ok_or("verified compiler runtime is missing")?;
    let compiled = hex::decode(compiled_hex)
        .map_err(|error| format!("verified runtime is not plain hex: {error}"))?;
    if compiled.len() != runtime_bytes.len() {
        return Err("compiler and fixed runtime lengths differ".into());
    }
    let map_text = deployed["sourceMap"]
        .as_str()
        .ok_or("verified compiler source map is missing")?;
    let ranges = decoded_source_map(map_text)?;
    let compiled_pcs = instruction_pcs(&compiled);
    let onchain_pcs = instruction_pcs(runtime_bytes);
    if ranges.len() > compiled_pcs.len() || compiled_pcs != onchain_pcs {
        return Err("compiler and fixed runtime instruction positions differ".into());
    }
    let mut map_by_pc = BTreeMap::new();
    for (index, range) in ranges.into_iter().enumerate() {
        map_by_pc.insert(compiled_pcs[index], range);
    }
    let mut files = BTreeMap::new();
    let mut declarations = BTreeMap::new();
    let mut references = Vec::new();
    let source_entries = comparison["sources"]
        .as_object()
        .ok_or("verified compiler AST sources are missing")?;
    for (name, entry) in source_entries {
        if let Some(id) = entry["id"].as_i64() {
            files.insert(id, name.clone());
        }
    }
    let root = Context {
        contract: String::new(),
        function: String::new(),
    };
    let mut function_ranges = Vec::new();
    for entry in source_entries.values() {
        verified_function_ranges(&entry["ast"], &root, &files, &mut function_ranges);
        verified_references(
            &entry["ast"],
            &root,
            &files,
            &mut declarations,
            &mut references,
        );
    }
    let observations = coverage["branch_observations"]
        .as_array()
        .ok_or("local coverage branch observations are missing")?;
    let mut hints = Vec::new();
    for observation in observations {
        let pc = observation["pc"]
            .as_u64()
            .ok_or("invalid observed branch PC")? as usize;
        if compiled.get(pc) != Some(&0x57) || runtime_bytes.get(pc) != Some(&0x57) {
            return Err(format!("observed PC {pc} is not a JUMPI in both runtimes"));
        }
        let range = map_by_pc.get(&pc).copied().flatten();
        let mut ids = BTreeSet::new();
        let too_broad = range.is_some_and(|item| item.end.saturating_sub(item.start) > 256);
        if let Some(range) = range.filter(|_| !too_broad) {
            for (id, reference) in &references {
                if reference.file_id == range.file_id
                    && reference.start >= range.start
                    && reference.end <= range.end
                    && declarations.contains_key(id)
                {
                    ids.insert(*id);
                }
            }
        }
        let names: BTreeSet<String> = ids
            .iter()
            .filter_map(|id| declarations.get(id).map(|record| record.name.clone()))
            .collect();
        for id in &ids {
            if let Some(record) = declarations.get_mut(id) {
                record.observed_branch_pcs.push(pc);
            }
        }
        let outcomes = observation["outcomes"]
            .as_array()
            .ok_or("branch observation has no outcomes")?
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect();
        hints.push(PcSourceHint {
            pc,
            observed_outcomes: outcomes,
            verified_source_file: range.and_then(|item| files.get(&item.file_id).cloned()),
            source_start_byte: range.map(|item| item.start),
            source_end_byte: range.map(|item| item.end),
            verified_variable_ast_ids: ids.into_iter().collect(),
            verified_variable_names: names.into_iter().collect(),
            confidence: if range.is_none() {
                "unmapped"
            } else if too_broad {
                "broad_source_range"
            } else {
                "tentative_source_range"
            }
            .into(),
        });
    }
    let observed_by_pc: BTreeMap<usize, Vec<String>> = observations
        .iter()
        .filter_map(|item| {
            Some((
                item["pc"].as_u64()? as usize,
                item["outcomes"]
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            ))
        })
        .collect();
    let static_pcs: Vec<usize> = onchain_pcs
        .into_iter()
        .filter(|pc| runtime_bytes.get(*pc) == Some(&0x57))
        .collect();
    let mut origins = Vec::new();
    let mut by_function: BTreeMap<i64, FunctionSourceCoverage> = BTreeMap::new();
    for pc in &static_pcs {
        let Some(range) = map_by_pc.get(pc).copied().flatten() else {
            continue;
        };
        if range.end.saturating_sub(range.start) > 256 {
            continue;
        }
        let candidates: Vec<_> = function_ranges
            .iter()
            .filter(|function| {
                function.range.file_id == range.file_id
                    && range.start >= function.range.start
                    && range.end <= function.range.end
            })
            .collect();
        let [function] = candidates.as_slice() else {
            continue;
        };
        let outcomes = observed_by_pc.get(pc).cloned().unwrap_or_default();
        origins.push(PcSourceOrigin {
            pc: *pc,
            source_file: function.source_file.clone(),
            declaring_contract: function.contract.clone(),
            function: function.function.clone(),
            function_ast_id: function.ast_id,
            origin_kind: function.kind.clone(),
            observed_outcomes: outcomes.clone(),
            confidence: "verified_source_origin_not_runtime_caller".into(),
        });
        let entry = by_function
            .entry(function.ast_id)
            .or_insert_with(|| FunctionSourceCoverage {
                source_file: function.source_file.clone(),
                declaring_contract: function.contract.clone(),
                function: function.function.clone(),
                function_ast_id: function.ast_id,
                origin_kind: function.kind.clone(),
                mapped_static_jump_sites: 0,
                observed_jump_sites: 0,
                observed_outcomes: 0,
                sites_with_both_outcomes: 0,
            });
        entry.mapped_static_jump_sites += 1;
        entry.observed_jump_sites += usize::from(!outcomes.is_empty());
        entry.observed_outcomes += outcomes.len();
        entry.sites_with_both_outcomes += usize::from(outcomes.len() == 2);
    }
    let source_coverage = SourceCoverageSummary {
        runtime_static_jump_sites: static_pcs.len(),
        uniquely_mapped_static_jump_sites: origins.len(),
        unattributed_static_jump_sites: static_pcs.len() - origins.len(),
        observed_attributed_jump_sites: origins
            .iter()
            .filter(|origin| !origin.observed_outcomes.is_empty())
            .count(),
        observed_attributed_outcomes: origins
            .iter()
            .map(|origin| origin.observed_outcomes.len())
            .sum(),
        function_source_coverage: by_function.into_values().collect(),
        pc_source_origins: origins,
        denominator_kind: "uniquely_source_mapped_static_jump_sites_not_reachable_paths".into(),
    };
    Ok((hints, declarations.into_values().collect(), source_coverage))
}
