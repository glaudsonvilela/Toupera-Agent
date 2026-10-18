//! Source-backed reconstruction from solc's Standard JSON AST output.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Map, Value, json};

use crate::cfg::build_source_flow;
use crate::{
    ArithmeticExpression, CallTarget, Confidence, ContractId, EconomicFunction, EconomicOperation,
    Evidence, EvidenceLocation, FunctionId, SourceSpan, StorageId, Uncertainty, UncertaintyKind,
};

/// Compiles one explicitly supplied Solidity source file locally, then reconstructs its IR.
/// Imports and multi-file projects are not supported in this first source-backed slice.
pub fn reconstruct_source_file(
    source_path: &Path,
    solc_path: &Path,
) -> Result<Vec<EconomicFunction>, String> {
    let source = fs::read_to_string(source_path)
        .map_err(|error| format!("cannot read {}: {error}", source_path.display()))?;
    let source_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "source path has no UTF-8 file name".to_owned())?;

    let mut sources = Map::new();
    sources.insert(source_name.to_owned(), json!({ "content": source }));
    let input = json!({
        "language": "Solidity",
        "sources": sources,
        "settings": { "outputSelection": { "*": { "": ["ast"] } } }
    });

    let mut compiler = Command::new(solc_path)
        .arg("--standard-json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", solc_path.display()))?;
    compiler
        .stdin
        .take()
        .ok_or("cannot open solc stdin")?
        .write_all(input.to_string().as_bytes())
        .map_err(|error| format!("cannot send source to solc: {error}"))?;
    let output = compiler
        .wait_with_output()
        .map_err(|error| format!("cannot read solc output: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "solc failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let json = std::str::from_utf8(&output.stdout)
        .map_err(|error| format!("solc output is not UTF-8: {error}"))?;
    reconstruct_from_ast_json(json, source_name, &source)
}

/// Reads solc Standard JSON output for a single source whose content is supplied separately.
pub fn reconstruct_from_ast_json(
    standard_json: &str,
    source_name: &str,
    source: &str,
) -> Result<Vec<EconomicFunction>, String> {
    let output: Value = serde_json::from_str(standard_json)
        .map_err(|error| format!("invalid solc JSON: {error}"))?;
    if let Some(errors) = output.get("errors").and_then(Value::as_array) {
        let fatal: Vec<_> = errors
            .iter()
            .filter(|item| item.get("severity").and_then(Value::as_str) == Some("error"))
            .filter_map(|item| item.get("formattedMessage").and_then(Value::as_str))
            .collect();
        if !fatal.is_empty() {
            return Err(format!("solc reported errors: {}", fatal.join("\n")));
        }
    }
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
    if ast
        .get("src")
        .and_then(Value::as_str)
        .and_then(|src| parse_span(src, source_name, source, source_id))
        .is_none()
    {
        return Err("source content or source id does not match AST offsets".into());
    }
    let nodes = ast
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("solc source entry has no AST nodes")?;

    let mut functions = Vec::new();
    for contract in nodes
        .iter()
        .filter(|node| node_type(node) == Some("ContractDefinition"))
    {
        if contract.get("contractKind").and_then(Value::as_str) != Some("contract") {
            continue;
        }
        let contract_name = contract
            .get("name")
            .and_then(Value::as_str)
            .ok_or("contract has no name")?;
        let Some(members) = contract.get("nodes").and_then(Value::as_array) else {
            continue;
        };
        let state: HashMap<i64, String> = members
            .iter()
            .filter(|node| node_type(node) == Some("VariableDeclaration"))
            .filter(|node| node.get("stateVariable").and_then(Value::as_bool) == Some(true))
            .filter_map(|node| {
                Some((
                    node.get("id")?.as_i64()?,
                    node.get("name")?.as_str()?.to_owned(),
                ))
            })
            .collect();
        let function_ids: HashMap<i64, FunctionId> = members
            .iter()
            .filter(|node| node_type(node) == Some("FunctionDefinition"))
            .filter_map(|node| Some((node.get("id")?.as_i64()?, function_id(node)?)))
            .collect();

        for function in members
            .iter()
            .filter(|node| node_type(node) == Some("FunctionDefinition"))
        {
            let Some(body) = function.get("body").filter(|body| !body.is_null()) else {
                continue;
            };
            let Some(id) = function_id(function) else {
                continue;
            };
            let name = function.get("name").and_then(Value::as_str).unwrap_or("");
            let name_src = function
                .get("nameLocation")
                .and_then(Value::as_str)
                .filter(|location| !location.starts_with("-1:"))
                .or_else(|| function.get("src").and_then(Value::as_str));
            let Some(name_span) =
                name_src.and_then(|src| parse_span(src, source_name, source, source_id))
            else {
                return Err(format!(
                    "invalid source location for {contract_name}.{name}"
                ));
            };
            if !name.is_empty() && name_span.slice(source) != Some(name) {
                return Err(format!("source text does not match function name {name}"));
            }
            let operation = Evidence {
                value: operation_for_name(name),
                location: EvidenceLocation::Source(name_span),
                confidence: Confidence::Inferred,
            };
            let mut ir = EconomicFunction::new(ContractId(contract_name.to_owned()), id, operation);
            if contract
                .get("baseContracts")
                .and_then(Value::as_array)
                .is_some_and(|bases| !bases.is_empty())
            {
                ir.unresolved.push(Uncertainty {
                    kind: UncertaintyKind::MissingInput,
                    detail: "inherited state and functions are not resolved in this source slice"
                        .into(),
                    location: None,
                });
            }
            if let Some(modifiers) = function.get("modifiers").and_then(Value::as_array) {
                for modifier in modifiers {
                    if let Some(evidence) = evidence_for_node(
                        modifier,
                        source_name,
                        source,
                        source_id,
                        source_fragment(modifier, source_name, source, source_id)
                            .unwrap_or_default()
                            .to_owned(),
                        Confidence::Direct,
                    ) {
                        ir.modifiers.push(evidence);
                    }
                }
                if !modifiers.is_empty() {
                    ir.unresolved.push(Uncertainty {
                        kind: UncertaintyKind::AmbiguousSemantics,
                        detail: "modifier body is not expanded into this function's dependencies"
                            .into(),
                        location: None,
                    });
                }
            }
            let mut walker = Walker {
                source_name,
                source,
                source_id,
                state: &state,
                function_ids: &function_ids,
                ir: &mut ir,
            };
            walker.visit(body, Access::Read, false);
            let (edges, uncertainties) = build_source_flow(body, source_name, source, source_id);
            ir.control_edges = edges;
            ir.unresolved.extend(uncertainties);
            functions.push(ir);
        }
    }
    Ok(functions)
}

fn function_id(node: &Value) -> Option<FunctionId> {
    let kind = node.get("kind")?.as_str()?;
    if kind == "constructor" {
        return None;
    }
    if kind == "fallback" || kind == "receive" {
        return Some(FunctionId(format!("{kind}()")));
    }
    let name = node.get("name")?.as_str()?;
    if name.is_empty() {
        return None;
    }
    let parameters = node.get("parameters")?.get("parameters")?.as_array()?;
    let types: Vec<&str> = parameters
        .iter()
        .map(|parameter| {
            parameter
                .get("typeDescriptions")
                .and_then(|item| item.get("typeString"))
                .and_then(Value::as_str)
                .unwrap_or("?")
        })
        .collect();
    Some(FunctionId(format!("{name}({})", types.join(","))))
}

fn operation_for_name(name: &str) -> EconomicOperation {
    match name {
        "deposit" => EconomicOperation::Deposit,
        "mint" => EconomicOperation::Mint,
        "withdraw" => EconomicOperation::Withdraw,
        "redeem" => EconomicOperation::Redeem,
        "previewDeposit" => EconomicOperation::PreviewDeposit,
        "previewMint" => EconomicOperation::PreviewMint,
        "previewWithdraw" => EconomicOperation::PreviewWithdraw,
        "previewRedeem" => EconomicOperation::PreviewRedeem,
        "convertToShares" => EconomicOperation::ConvertToShares,
        "convertToAssets" => EconomicOperation::ConvertToAssets,
        "totalAssets" => EconomicOperation::TotalAssets,
        "totalSupply" => EconomicOperation::TotalSupply,
        "getPrice" | "latestPrice" => EconomicOperation::OraclePrice,
        _ => EconomicOperation::Other,
    }
}

fn node_type(node: &Value) -> Option<&str> {
    node.get("nodeType").and_then(Value::as_str)
}

pub(crate) fn parse_span(
    src: &str,
    file: &str,
    source: &str,
    source_id: i64,
) -> Option<SourceSpan> {
    let mut parts = src.split(':');
    let start = parts.next()?.parse::<usize>().ok()?;
    let length = parts.next()?.parse::<usize>().ok()?;
    let id = parts.next()?.parse::<i64>().ok()?;
    if id != source_id || parts.next().is_some() {
        return None;
    }
    let span = SourceSpan::new(file, start, start.checked_add(length)?)?;
    span.slice(source)?;
    Some(span)
}

fn source_fragment<'a>(
    node: &Value,
    file: &str,
    source: &'a str,
    source_id: i64,
) -> Option<&'a str> {
    let src = node.get("src")?.as_str()?;
    parse_span(src, file, source, source_id)?.slice(source)
}

fn evidence_for_node<T>(
    node: &Value,
    file: &str,
    source: &str,
    source_id: i64,
    value: T,
    confidence: Confidence,
) -> Option<Evidence<T>> {
    let span = parse_span(node.get("src")?.as_str()?, file, source, source_id)?;
    Some(Evidence {
        value,
        location: EvidenceLocation::Source(span),
        confidence,
    })
}

#[derive(Clone, Copy)]
enum Access {
    Read,
    Write,
    ReadWrite,
}

struct Walker<'a> {
    source_name: &'a str,
    source: &'a str,
    source_id: i64,
    state: &'a HashMap<i64, String>,
    function_ids: &'a HashMap<i64, FunctionId>,
    ir: &'a mut EconomicFunction,
}

impl Walker<'_> {
    fn evidence<T>(&self, node: &Value, value: T, confidence: Confidence) -> Option<Evidence<T>> {
        evidence_for_node(
            node,
            self.source_name,
            self.source,
            self.source_id,
            value,
            confidence,
        )
    }

    fn fragment<'a>(&'a self, node: &Value) -> Option<&'a str> {
        source_fragment(node, self.source_name, self.source, self.source_id)
    }

    fn visit(&mut self, node: &Value, access: Access, within_arithmetic: bool) {
        match node {
            Value::Array(items) => {
                for item in items {
                    self.visit(item, access, within_arithmetic);
                }
            }
            Value::Object(fields) => match node_type(node) {
                Some("Identifier") => self.visit_identifier(node, access),
                Some("Assignment") => {
                    let operator = node.get("operator").and_then(Value::as_str).unwrap_or("=");
                    if operator != "=" {
                        if let Some(text) = self.fragment(node).map(str::to_owned) {
                            if let Some(evidence) =
                                self.evidence(node, ArithmeticExpression(text), Confidence::Direct)
                            {
                                self.ir.arithmetic.push(evidence);
                            }
                        }
                    }
                    let lhs_access = if operator == "=" {
                        Access::Write
                    } else {
                        Access::ReadWrite
                    };
                    self.visit(&node["leftHandSide"], lhs_access, false);
                    self.visit(&node["rightHandSide"], Access::Read, false);
                }
                Some("UnaryOperation") => {
                    let operator = node.get("operator").and_then(Value::as_str).unwrap_or("");
                    let operand_access = if operator == "++" || operator == "--" {
                        Access::ReadWrite
                    } else {
                        Access::Read
                    };
                    self.visit(&node["subExpression"], operand_access, within_arithmetic);
                }
                Some("IndexAccess") => {
                    self.visit(&node["baseExpression"], access, within_arithmetic);
                    self.visit(&node["indexExpression"], Access::Read, within_arithmetic);
                }
                Some("MemberAccess") => {
                    self.visit(&node["expression"], access, within_arithmetic);
                }
                Some("FunctionCall") => {
                    self.visit_call(node);
                    self.visit(&node["expression"], Access::Read, within_arithmetic);
                    self.visit(&node["arguments"], Access::Read, within_arithmetic);
                }
                Some("BinaryOperation") => {
                    let operator = node.get("operator").and_then(Value::as_str).unwrap_or("");
                    let arithmetic = matches!(operator, "+" | "-" | "*" | "/" | "%" | "**");
                    if arithmetic && !within_arithmetic {
                        if let Some(text) = self.fragment(node).map(str::to_owned) {
                            if let Some(evidence) =
                                self.evidence(node, ArithmeticExpression(text), Confidence::Direct)
                            {
                                self.ir.arithmetic.push(evidence);
                            }
                        }
                    }
                    self.visit(
                        &node["leftExpression"],
                        Access::Read,
                        within_arithmetic || arithmetic,
                    );
                    self.visit(
                        &node["rightExpression"],
                        Access::Read,
                        within_arithmetic || arithmetic,
                    );
                }
                Some("Conditional") => {
                    self.record_condition(&node["condition"]);
                    self.visit(&node["condition"], Access::Read, false);
                    self.visit(&node["trueExpression"], Access::Read, false);
                    self.visit(&node["falseExpression"], Access::Read, false);
                }
                Some("IfStatement") => {
                    self.record_condition(&node["condition"]);
                    self.visit(&node["condition"], Access::Read, false);
                    self.visit(&node["trueBody"], Access::Read, false);
                    self.visit(&node["falseBody"], Access::Read, false);
                }
                Some("ForStatement" | "WhileStatement" | "DoWhileStatement") => {
                    self.record_condition(&node["condition"]);
                    for (key, value) in fields {
                        if key != "condition" {
                            self.visit(value, Access::Read, false);
                        }
                    }
                    self.visit(&node["condition"], Access::Read, false);
                }
                _ => {
                    for (key, value) in fields {
                        if key != "typeDescriptions" && key != "documentation" {
                            self.visit(value, access, within_arithmetic);
                        }
                    }
                }
            },
            _ => {}
        }
    }

    fn visit_identifier(&mut self, node: &Value, access: Access) {
        let Some(name) = node
            .get("referencedDeclaration")
            .and_then(Value::as_i64)
            .and_then(|id| self.state.get(&id))
            .cloned()
        else {
            return;
        };
        let Some(evidence) = self.evidence(node, StorageId(name), Confidence::Direct) else {
            return;
        };
        match access {
            Access::Read => self.ir.reads.push(evidence),
            Access::Write => self.ir.writes.push(evidence),
            Access::ReadWrite => {
                self.ir.reads.push(evidence.clone());
                self.ir.writes.push(evidence);
            }
        }
    }

    fn record_condition(&mut self, node: &Value) {
        if let Some(text) = self.fragment(node).map(str::to_owned) {
            if let Some(evidence) = self.evidence(node, text, Confidence::Direct) {
                self.ir.control_conditions.push(evidence);
            }
        }
    }

    fn visit_call(&mut self, node: &Value) {
        let expression = &node["expression"];
        if let Some(id) = expression
            .get("referencedDeclaration")
            .and_then(Value::as_i64)
        {
            if let Some(function) = self.function_ids.get(&id) {
                if let Some(evidence) = self.evidence(node, function.clone(), Confidence::Direct) {
                    self.ir.related_functions.push(evidence);
                }
            }
        }
        if node_type(expression) == Some("Identifier")
            && expression.get("name").and_then(Value::as_str) == Some("require")
        {
            if let Some(condition) = node
                .get("arguments")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
            {
                self.record_condition(condition);
            }
        }
        if node_type(expression) != Some("MemberAccess") {
            return;
        }
        let member = expression
            .get("memberName")
            .and_then(Value::as_str)
            .unwrap_or("");
        let receiver = &expression["expression"];
        let receiver_type = receiver
            .get("typeDescriptions")
            .and_then(|item| item.get("typeString"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let contract_call = receiver_type.starts_with("contract ");
        let low_level_call = matches!(member, "call" | "delegatecall" | "staticcall")
            && receiver_type.starts_with("address");
        if !contract_call && !low_level_call {
            return;
        }
        let Some(target) = self.fragment(expression).map(str::to_owned) else {
            return;
        };
        let Some(call) = self.evidence(node, CallTarget(target), Confidence::Direct) else {
            return;
        };
        self.ir.external_calls.push(call.clone());

        let receiver_text = self.fragment(receiver).unwrap_or("").to_owned();
        let hint = format!("{receiver_text} {receiver_type}").to_ascii_lowercase();
        let price_method = matches!(
            member,
            "getPrice" | "latestPrice" | "latestRoundData" | "price"
        );
        if price_method
            && (hint.contains("oracle") || hint.contains("feed") || hint.contains("aggregator"))
        {
            let mut oracle = call;
            oracle.confidence = Confidence::Inferred;
            self.ir.oracle_dependencies.push(oracle);
        }
        if member == "delegatecall" {
            if let Some(implementation) =
                self.evidence(receiver, CallTarget(receiver_text), Confidence::Tentative)
            {
                self.ir.delegatecall_targets.push(implementation);
                self.ir.unresolved.push(Uncertainty {
                    kind: UncertaintyKind::AmbiguousSemantics,
                    detail: "delegatecall target may be a proxy implementation; resolve before asserting".into(),
                    location: self.evidence(node, (), Confidence::Direct).map(|e| e.location),
                });
            }
        }
    }
}
