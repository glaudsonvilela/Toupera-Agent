//! Conservative source-level value tracing for scalar vault expressions.

use std::collections::HashMap;

use serde_json::Value;

use crate::source::{parse_span, placeholder_count};
use crate::{
    CallTarget, Confidence, EconomicFunction, Evidence, EvidenceLocation, FunctionId, StorageId,
    Uncertainty, UncertaintyKind, ValueFlow, ValueOrigin, ValueSink,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Trace {
    origin: Evidence<ValueOrigin>,
    calls: Vec<Evidence<FunctionId>>,
}

struct Summary {
    returns: Vec<Trace>,
    flows: Vec<ValueFlow>,
    unresolved: Vec<Uncertainty>,
    state_updates: HashMap<i64, Vec<Trace>>,
}

struct Engine<'a> {
    functions: HashMap<i64, &'a Value>,
    modifiers: HashMap<i64, &'a Value>,
    function_ids: &'a HashMap<i64, FunctionId>,
    state: &'a HashMap<i64, String>,
    file: &'a str,
    source: &'a str,
    source_id: i64,
}

pub(crate) fn populate_value_flows(
    records: &mut [EconomicFunction],
    members: &[Value],
    function_ids: &HashMap<i64, FunctionId>,
    state: &HashMap<i64, String>,
    file: &str,
    source: &str,
    source_id: i64,
) {
    let functions: HashMap<i64, &Value> = members
        .iter()
        .filter(|node| node["nodeType"].as_str() == Some("FunctionDefinition"))
        .filter_map(|node| Some((node["id"].as_i64()?, node)))
        .collect();
    let modifiers: HashMap<i64, &Value> = members
        .iter()
        .filter(|node| node["nodeType"].as_str() == Some("ModifierDefinition"))
        .filter_map(|node| Some((node["id"].as_i64()?, node)))
        .collect();
    let engine = Engine {
        functions,
        modifiers,
        function_ids,
        state,
        file,
        source,
        source_id,
    };
    for record in records {
        let function_id = engine.functions.iter().find_map(|(id, node)| {
            let matches_name = function_ids.get(id) == Some(&record.function);
            let same_location = node["nameLocation"]
                .as_str()
                .and_then(|span| parse_span(span, file, source, source_id))
                .is_some_and(|span| record.operation.location == EvidenceLocation::Source(span));
            (matches_name && same_location).then_some(*id)
        });
        if let Some(function_id) = function_id {
            let summary = engine.analyze(function_id, None, HashMap::new(), &[]);
            record.value_flows = summary.flows;
            record.unresolved.extend(summary.unresolved);
        }
    }
}

impl Engine<'_> {
    fn evidence<T>(&self, node: &Value, value: T, confidence: Confidence) -> Option<Evidence<T>> {
        let span = parse_span(
            node["src"].as_str()?,
            self.file,
            self.source,
            self.source_id,
        )?;
        Some(Evidence {
            value,
            location: EvidenceLocation::Source(span),
            confidence,
        })
    }

    fn fragment<'a>(&'a self, node: &Value) -> Option<&'a str> {
        let span = parse_span(
            node["src"].as_str()?,
            self.file,
            self.source,
            self.source_id,
        )?;
        span.slice(self.source)
    }

    fn analyze(
        &self,
        id: i64,
        arguments: Option<Vec<Vec<Trace>>>,
        state_before: HashMap<i64, Vec<Trace>>,
        stack: &[i64],
    ) -> Summary {
        let Some(function) = self.functions.get(&id).copied() else {
            return Summary {
                returns: Vec::new(),
                flows: Vec::new(),
                unresolved: Vec::new(),
                state_updates: HashMap::new(),
            };
        };
        let mut run = Run {
            engine: self,
            function,
            env: state_before,
            flows: Vec::new(),
            returns: Vec::new(),
            unresolved: Vec::new(),
            stack: stack.iter().copied().chain([id]).collect(),
            layers: Vec::new(),
            active_layer: None,
        };
        if let Some(invocations) = function["modifiers"].as_array() {
            for invocation in invocations {
                let definition = invocation["modifierName"]["referencedDeclaration"]
                    .as_i64()
                    .and_then(|id| self.modifiers.get(&id).copied());
                let body = definition.and_then(|definition| definition.get("body"));
                if let (Some(definition), Some(body)) = (definition, body) {
                    if placeholder_count(body) == 1 {
                        run.layers.push((invocation, definition));
                        continue;
                    }
                }
                run.layers.clear();
                run.unresolved.push(Uncertainty {
                    kind: UncertaintyKind::AmbiguousSemantics,
                    detail: "modifier value flow could not be composed".into(),
                    location: self
                        .evidence(invocation, (), Confidence::Direct)
                        .map(|item| item.location),
                });
                break;
            }
        }
        if let Some(parameters) = function["parameters"]["parameters"].as_array() {
            for (index, parameter) in parameters.iter().enumerate() {
                let Some(parameter_id) = parameter["id"].as_i64() else {
                    continue;
                };
                let origins = arguments
                    .as_ref()
                    .and_then(|items| items.get(index))
                    .cloned()
                    .unwrap_or_else(|| {
                        self.evidence(
                            parameter,
                            ValueOrigin::Parameter(
                                parameter["name"].as_str().unwrap_or("").to_owned(),
                            ),
                            Confidence::Direct,
                        )
                        .map(|origin| {
                            vec![Trace {
                                origin,
                                calls: Vec::new(),
                            }]
                        })
                        .unwrap_or_default()
                    });
                run.env.insert(parameter_id, origins);
            }
        }
        let continues = if run.layers.is_empty() {
            run.visit(&function["body"])
        } else {
            run.process_layer(0)
        };
        if continues {
            if let Some(returns) = function["returnParameters"]["parameters"].as_array() {
                let named: Vec<_> = returns
                    .iter()
                    .filter_map(|parameter| parameter["id"].as_i64())
                    .flat_map(|id| run.env.get(&id).cloned().unwrap_or_default())
                    .collect();
                run.emit(named.clone(), &function["body"], ValueSink::Return);
                run.returns.extend(named);
            }
        }
        dedup(&mut run.returns);
        run.flows.dedup();
        Summary {
            returns: run.returns,
            flows: run.flows,
            unresolved: run.unresolved,
            state_updates: run
                .env
                .into_iter()
                .filter(|(id, _)| self.state.contains_key(id))
                .collect(),
        }
    }
}

struct Run<'a, 'b> {
    engine: &'a Engine<'b>,
    function: &'b Value,
    env: HashMap<i64, Vec<Trace>>,
    flows: Vec<ValueFlow>,
    returns: Vec<Trace>,
    unresolved: Vec<Uncertainty>,
    stack: Vec<i64>,
    layers: Vec<(&'b Value, &'b Value)>,
    active_layer: Option<usize>,
}

impl Run<'_, '_> {
    fn process_layer(&mut self, index: usize) -> bool {
        let previous = self.active_layer;
        self.active_layer = Some(index);
        let (invocation, definition) = self.layers[index];
        if let (Some(parameters), Some(arguments)) = (
            definition["parameters"]["parameters"].as_array(),
            invocation["arguments"].as_array(),
        ) {
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if let Some(id) = parameter["id"].as_i64() {
                    let origins = self.eval(argument);
                    self.env.insert(id, origins);
                }
            }
        }
        let continues = self.visit(&definition["body"]);
        self.active_layer = previous;
        continues
    }

    fn emit(&mut self, origins: Vec<Trace>, node: &Value, sink: ValueSink) {
        let Some(sink) = self.engine.evidence(node, sink, Confidence::Direct) else {
            return;
        };
        for trace in origins {
            let flow = ValueFlow {
                origin: trace.origin,
                calls: trace.calls,
                sink: sink.clone(),
                confidence: Confidence::Inferred,
            };
            if !self.flows.contains(&flow) {
                self.flows.push(flow);
            }
        }
    }

    /// Returns whether control can continue after this statement.
    fn visit(&mut self, node: &Value) -> bool {
        match node["nodeType"].as_str() {
            Some("Block" | "UncheckedBlock") => {
                if let Some(statements) = node["statements"].as_array() {
                    for statement in statements {
                        if !self.visit(statement) {
                            return false;
                        }
                    }
                }
                true
            }
            Some("VariableDeclarationStatement") => {
                let origins = self.eval(&node["initialValue"]);
                if let Some(declarations) = node["declarations"].as_array() {
                    for declaration in declarations {
                        if let Some(id) = declaration["id"].as_i64() {
                            self.env.insert(id, origins.clone());
                        }
                    }
                }
                true
            }
            Some("ExpressionStatement") => {
                let expression = &node["expression"];
                if is_guard(expression) {
                    let condition = &expression["arguments"][0];
                    let origins = self.eval(condition);
                    self.emit(origins, condition, ValueSink::Guard);
                } else {
                    self.eval(expression);
                }
                true
            }
            Some("IfStatement") => {
                let condition = &node["condition"];
                let origins = self.eval(condition);
                self.emit(origins, condition, ValueSink::Guard);
                let before = self.env.clone();
                let true_continues = self.visit(&node["trueBody"]);
                let true_env = self.env.clone();
                self.env = before.clone();
                let false_continues = if node["falseBody"].is_null() {
                    true
                } else {
                    self.visit(&node["falseBody"])
                };
                let false_env = self.env.clone();
                self.env = match (true_continues, false_continues) {
                    (true, true) => merge_env(true_env, false_env),
                    (true, false) => true_env,
                    (false, true) => false_env,
                    (false, false) => before,
                };
                true_continues || false_continues
            }
            Some("WhileStatement" | "DoWhileStatement" | "ForStatement") => {
                if !node["initializationExpression"].is_null() {
                    self.visit(&node["initializationExpression"]);
                }
                let condition = &node["condition"];
                if !condition.is_null() {
                    let origins = self.eval(condition);
                    self.emit(origins, condition, ValueSink::Guard);
                }
                let before = self.env.clone();
                self.visit(&node["body"]);
                if !node["loopExpression"].is_null() {
                    self.visit(&node["loopExpression"]);
                }
                self.env = merge_env(before, self.env.clone());
                self.unresolved.push(Uncertainty {
                    kind: UncertaintyKind::AmbiguousSemantics,
                    detail: "loop value flow uses a one-iteration approximation".into(),
                    location: self
                        .engine
                        .evidence(node, (), Confidence::Direct)
                        .map(|item| item.location),
                });
                true
            }
            Some("Return") => {
                let origins = if node["expression"].is_null() {
                    self.function["returnParameters"]["parameters"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|parameter| parameter["id"].as_i64())
                        .flat_map(|id| self.env.get(&id).cloned().unwrap_or_default())
                        .collect()
                } else {
                    self.eval(&node["expression"])
                };
                self.emit(origins.clone(), node, ValueSink::Return);
                self.returns.extend(origins);
                false
            }
            Some("RevertStatement") => false,
            Some("PlaceholderStatement") if self.active_layer.is_some() => {
                let next = self.active_layer.expect("modifier layer was checked") + 1;
                if next < self.layers.len() {
                    self.process_layer(next);
                } else {
                    self.visit(&self.function["body"]);
                }
                true
            }
            Some("Break" | "Continue") => false,
            _ => {
                self.eval(node);
                true
            }
        }
    }

    fn eval(&mut self, node: &Value) -> Vec<Trace> {
        let mut origins = match node["nodeType"].as_str() {
            Some("Identifier") => self.identifier(node),
            Some("Assignment") => self.assignment(node),
            Some("FunctionCall") => self.call(node),
            Some("BinaryOperation") => {
                let mut values = self.eval(&node["leftExpression"]);
                values.extend(self.eval(&node["rightExpression"]));
                values
            }
            Some("Conditional") => {
                let condition = &node["condition"];
                let control = self.eval(condition);
                self.emit(control.clone(), condition, ValueSink::Guard);
                let mut values = control;
                values.extend(self.eval(&node["trueExpression"]));
                values.extend(self.eval(&node["falseExpression"]));
                values
            }
            Some("UnaryOperation") => self.unary(node),
            Some("MemberAccess") => self.eval(&node["expression"]),
            Some("IndexAccess") => {
                let mut values = self.eval(&node["baseExpression"]);
                values.extend(self.eval(&node["indexExpression"]));
                values
            }
            Some("TupleExpression") => node["components"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|component| self.eval(component))
                .collect(),
            _ => Vec::new(),
        };
        dedup(&mut origins);
        origins
    }

    fn identifier(&self, node: &Value) -> Vec<Trace> {
        let Some(id) = node["referencedDeclaration"].as_i64() else {
            return Vec::new();
        };
        if let Some(values) = self.env.get(&id) {
            return values.clone();
        }
        let Some(name) = self.engine.state.get(&id) else {
            return Vec::new();
        };
        self.engine
            .evidence(
                node,
                ValueOrigin::Storage(StorageId(name.clone())),
                Confidence::Direct,
            )
            .map(|origin| {
                vec![Trace {
                    origin,
                    calls: Vec::new(),
                }]
            })
            .unwrap_or_default()
    }

    fn assignment(&mut self, node: &Value) -> Vec<Trace> {
        let lhs = &node["leftHandSide"];
        let mut origins = self.eval(&node["rightHandSide"]);
        if node["operator"].as_str() != Some("=") {
            origins.extend(self.eval(lhs));
        }
        dedup(&mut origins);
        if let Some(id) = lhs["referencedDeclaration"].as_i64() {
            if let Some(name) = self.engine.state.get(&id) {
                self.emit(
                    origins.clone(),
                    lhs,
                    ValueSink::StorageWrite(StorageId(name.clone())),
                );
            }
            self.env.insert(id, origins.clone());
        } else if let Some((id, name)) = self.storage_target(lhs) {
            self.emit(
                origins.clone(),
                lhs,
                ValueSink::StorageWrite(StorageId(name)),
            );
            self.env.insert(id, origins.clone());
        }
        origins
    }

    fn unary(&mut self, node: &Value) -> Vec<Trace> {
        let sub = &node["subExpression"];
        let origins = self.eval(sub);
        if matches!(node["operator"].as_str(), Some("++" | "--")) {
            if let Some((id, name)) = self.storage_target(sub) {
                self.emit(
                    origins.clone(),
                    sub,
                    ValueSink::StorageWrite(StorageId(name)),
                );
                self.env.insert(id, origins.clone());
            }
        }
        origins
    }

    fn storage_target(&self, node: &Value) -> Option<(i64, String)> {
        let id = node["referencedDeclaration"].as_i64().or_else(|| {
            if node["nodeType"].as_str() == Some("IndexAccess") {
                node["baseExpression"]["referencedDeclaration"].as_i64()
            } else {
                None
            }
        })?;
        Some((id, self.engine.state.get(&id)?.clone()))
    }

    fn call(&mut self, node: &Value) -> Vec<Trace> {
        let arguments: Vec<_> = node["arguments"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|argument| self.eval(argument))
            .collect();
        let expression = &node["expression"];
        if let Some(id) = expression["referencedDeclaration"].as_i64() {
            if let Some(function_id) = self.engine.function_ids.get(&id) {
                if matches!(
                    function_id.0.split('(').next(),
                    Some("convertToShares" | "convertToAssets")
                ) {
                    if let Some(nodes) = node["arguments"].as_array() {
                        for (argument_node, values) in nodes.iter().zip(&arguments) {
                            self.emit(
                                values.clone(),
                                argument_node,
                                ValueSink::ConversionArgument(function_id.clone()),
                            );
                        }
                    }
                }
                if self.stack.contains(&id) || self.stack.len() >= 16 {
                    self.unresolved.push(Uncertainty {
                        kind: UncertaintyKind::AmbiguousSemantics,
                        detail: "recursive or overlong value-flow call path was truncated".into(),
                        location: self
                            .engine
                            .evidence(node, (), Confidence::Direct)
                            .map(|e| e.location),
                    });
                    return Vec::new();
                }
                let state_before = self
                    .env
                    .iter()
                    .filter(|(id, _)| self.engine.state.contains_key(id))
                    .map(|(id, values)| (*id, values.clone()))
                    .collect();
                let summary = self
                    .engine
                    .analyze(id, Some(arguments), state_before, &self.stack);
                self.unresolved.extend(summary.unresolved);
                let Some(call) =
                    self.engine
                        .evidence(node, function_id.clone(), Confidence::Direct)
                else {
                    return summary.returns;
                };
                for mut flow in summary.flows {
                    flow.calls.push(call.clone());
                    if !self.flows.contains(&flow) {
                        self.flows.push(flow);
                    }
                }
                self.env.extend(summary.state_updates);
                return summary
                    .returns
                    .into_iter()
                    .map(|mut trace| {
                        trace.calls.push(call.clone());
                        trace
                    })
                    .collect();
            }
        }
        if expression["nodeType"].as_str() == Some("MemberAccess") {
            let member = expression["memberName"].as_str().unwrap_or("");
            let receiver = &expression["expression"];
            let receiver_type = receiver["typeDescriptions"]["typeString"]
                .as_str()
                .unwrap_or("");
            let contract_call = receiver_type.starts_with("contract ");
            let low_level_call = matches!(member, "call" | "delegatecall" | "staticcall")
                && receiver_type.starts_with("address");
            if contract_call || low_level_call {
                let target = self.engine.fragment(expression).unwrap_or("").to_owned();
                let receiver_text = self.engine.fragment(receiver).unwrap_or("");
                let hint = format!("{receiver_text} {receiver_type}").to_ascii_lowercase();
                let oracle = matches!(
                    member,
                    "getPrice" | "latestPrice" | "latestRoundData" | "price"
                ) && (hint.contains("oracle")
                    || hint.contains("feed")
                    || hint.contains("aggregator"));
                let value = if oracle {
                    ValueOrigin::OracleCall(CallTarget(target))
                } else {
                    ValueOrigin::ExternalCall(CallTarget(target))
                };
                return self
                    .engine
                    .evidence(
                        node,
                        value,
                        if oracle {
                            Confidence::Inferred
                        } else {
                            Confidence::Direct
                        },
                    )
                    .map(|origin| {
                        vec![Trace {
                            origin,
                            calls: Vec::new(),
                        }]
                    })
                    .unwrap_or_default();
            }
        }
        if node["kind"].as_str() == Some("typeConversion") {
            return arguments.into_iter().flatten().collect();
        }
        self.unresolved.push(Uncertainty {
            kind: UncertaintyKind::AmbiguousSemantics,
            detail: "value flow through an unsupported call is unknown".into(),
            location: self
                .engine
                .evidence(node, (), Confidence::Direct)
                .map(|item| item.location),
        });
        Vec::new()
    }
}

fn is_guard(call: &Value) -> bool {
    call["nodeType"].as_str() == Some("FunctionCall")
        && matches!(
            call["expression"]["name"].as_str(),
            Some("require" | "assert")
        )
}

fn dedup<T: Eq>(items: &mut Vec<T>) {
    let mut index = 0;
    while index < items.len() {
        if items[..index].contains(&items[index]) {
            items.remove(index);
        } else {
            index += 1;
        }
    }
}

fn merge_env(
    mut left: HashMap<i64, Vec<Trace>>,
    right: HashMap<i64, Vec<Trace>>,
) -> HashMap<i64, Vec<Trace>> {
    for (id, values) in right {
        let entry = left.entry(id).or_default();
        entry.extend(values);
        dedup(entry);
    }
    left
}
