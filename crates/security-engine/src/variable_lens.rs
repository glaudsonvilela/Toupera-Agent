//! Offline, source-backed view of variable declarations and syntactic value paths.
//! Runtime observations are kept as context until a verified PC-to-source mapping exists.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

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
    pub variables_without_observed_use: usize,
    pub variables_with_read_without_writer: usize,
    pub observed_root_jump_sites: usize,
    pub observed_root_branch_outcomes: usize,
    pub pc_hints_with_variables: usize,
    pub pc_source_hints: Vec<PcSourceHint>,
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
    let pc_source_hints = pc_hints(&manifest, &comparison, &coverage, &runtime_bytes)?;
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
    };
    let root = Context {
        contract: String::new(),
        function: String::new(),
    };
    walker.collect_declarations(ast, &root, "");
    walker.attach_layout(&solc);
    walker.walk(ast, &root, "read");
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
                function: context.function.clone(),
                source_span,
                expression,
            });
        }
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
                self.walk(right, &current, "read");
                if left["nodeType"] == "IndexAccess" {
                    self.walk(&left["indexExpression"], &current, "index_key");
                }
                return;
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

    fn finish(
        mut self,
        identity: Value,
        source_sha: String,
        forensic_sha: String,
        coverage_sha: String,
        coverage: &Value,
    ) -> Result<VariableLensReport, String> {
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
                (&a.kind, &a.function, &a.expression).cmp(&(&b.kind, &b.function, &b.expression))
            });
            record.uses.dedup_by(|a, b| {
                a.kind == b.kind
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
            variables_without_observed_use: without_use,
            variables_with_read_without_writer: read_without_writer,
            observed_root_jump_sites,
            observed_root_branch_outcomes,
            pc_hints_with_variables: 0,
            pc_source_hints: Vec::new(),
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

#[derive(Clone, Copy)]
struct SourceRange {
    start: usize,
    end: usize,
    file_id: i64,
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
    declarations: &mut BTreeMap<i64, String>,
    references: &mut Vec<(i64, SourceRange)>,
) {
    match node["nodeType"].as_str() {
        Some("VariableDeclaration") => {
            if let (Some(id), Some(name)) = (node["id"].as_i64(), node["name"].as_str()) {
                declarations.insert(id, name.into());
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
        verified_references(child, declarations, references);
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
) -> Result<Vec<PcSourceHint>, String> {
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
        verified_references(&entry["ast"], &mut declarations, &mut references);
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
            .filter_map(|id| declarations.get(id).cloned())
            .collect();
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
    Ok(hints)
}
