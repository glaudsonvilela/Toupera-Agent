//! Structural source CFG. Nodes are AST statements/expressions, not EVM basic blocks.

use serde_json::Value;

use crate::source::parse_span;
use crate::{
    BasicBlockId, ControlEdge, ControlEdgeKind, EvidenceLocation, Uncertainty, UncertaintyKind,
};

#[derive(Clone)]
struct Pending {
    from: BasicBlockId,
    kind: ControlEdgeKind,
    location: Option<EvidenceLocation>,
}

pub(crate) fn build_source_flow(
    body: &Value,
    file: &str,
    source: &str,
    source_id: i64,
) -> (Vec<ControlEdge>, Vec<Uncertainty>) {
    let mut builder = Builder {
        file,
        source,
        source_id,
        edges: Vec::new(),
        unresolved: Vec::new(),
    };
    let entry = Pending {
        from: BasicBlockId("entry".into()),
        kind: ControlEdgeKind::Next,
        location: builder.location(body),
    };
    let pending = builder.process(body, vec![entry]);
    builder.connect(pending, BasicBlockId("exit".into()), body);
    (builder.edges, builder.unresolved)
}

struct Builder<'a> {
    file: &'a str,
    source: &'a str,
    source_id: i64,
    edges: Vec<ControlEdge>,
    unresolved: Vec<Uncertainty>,
}

impl Builder<'_> {
    fn location(&self, node: &Value) -> Option<EvidenceLocation> {
        parse_span(
            node.get("src")?.as_str()?,
            self.file,
            self.source,
            self.source_id,
        )
        .map(EvidenceLocation::Source)
    }

    fn id(&mut self, node: &Value) -> Option<BasicBlockId> {
        let id = node.get("id").and_then(Value::as_i64);
        if id.is_none() || self.location(node).is_none() {
            self.unresolved.push(Uncertainty {
                kind: UncertaintyKind::MissingInput,
                detail: "AST node lacks a valid id or source span for the control graph".into(),
                location: self.location(node),
            });
        }
        id.filter(|_| self.location(node).is_some())
            .map(|id| BasicBlockId(format!("ast:{id}")))
    }

    fn connect(&mut self, incoming: Vec<Pending>, to: BasicBlockId, target: &Value) {
        let target_location = self.location(target);
        for edge in incoming {
            if let Some(location) = edge.location.or_else(|| target_location.clone()) {
                self.edges.push(ControlEdge {
                    from: edge.from,
                    to: to.clone(),
                    kind: edge.kind,
                    location,
                });
            } else {
                self.unresolved.push(Uncertainty {
                    kind: UncertaintyKind::MissingInput,
                    detail: "control edge has no valid source span".into(),
                    location: None,
                });
            }
        }
    }

    fn outgoing(&self, from: BasicBlockId, kind: ControlEdgeKind, at: &Value) -> Vec<Pending> {
        vec![Pending {
            from,
            kind,
            location: self.location(at),
        }]
    }

    fn process(&mut self, node: &Value, incoming: Vec<Pending>) -> Vec<Pending> {
        if incoming.is_empty() {
            return Vec::new();
        }
        match node.get("nodeType").and_then(Value::as_str) {
            Some("Block" | "UncheckedBlock") => {
                let mut pending = incoming;
                if let Some(statements) = node.get("statements").and_then(Value::as_array) {
                    for statement in statements {
                        pending = self.process(statement, pending);
                    }
                }
                pending
            }
            Some("IfStatement") => {
                let Some(id) = self.id(node) else {
                    return incoming;
                };
                self.connect(incoming, id.clone(), node);
                let condition = &node["condition"];
                let true_path = self.outgoing(id.clone(), ControlEdgeKind::True, condition);
                let false_path = self.outgoing(id, ControlEdgeKind::False, condition);
                let mut exits = self.process(&node["trueBody"], true_path);
                if node.get("falseBody").is_some_and(|body| !body.is_null()) {
                    exits.extend(self.process(&node["falseBody"], false_path));
                } else {
                    exits.extend(false_path);
                }
                exits
            }
            Some("WhileStatement") => {
                let Some(id) = self.id(node) else {
                    return incoming;
                };
                self.connect(incoming, id.clone(), node);
                let condition = &node["condition"];
                let body_exits = self.process(
                    &node["body"],
                    self.outgoing(id.clone(), ControlEdgeKind::True, condition),
                );
                let back_edges = body_exits
                    .into_iter()
                    .map(|edge| Pending {
                        from: edge.from,
                        kind: ControlEdgeKind::LoopBack,
                        location: edge.location,
                    })
                    .collect();
                self.connect(back_edges, id.clone(), condition);
                self.outgoing(id, ControlEdgeKind::False, condition)
            }
            Some(
                "ForStatement" | "DoWhileStatement" | "Break" | "Continue" | "TryStatement"
                | "InlineAssembly",
            ) => {
                self.unresolved.push(Uncertainty {
                    kind: UncertaintyKind::AmbiguousSemantics,
                    detail: format!(
                        "source control flow for {} is not modeled",
                        node["nodeType"].as_str().unwrap_or("unknown")
                    ),
                    location: self.location(node),
                });
                if let Some(id) = self.id(node) {
                    self.connect(incoming, id, node);
                }
                Vec::new()
            }
            Some("Return" | "RevertStatement") => self.plain(node, incoming, true),
            _ => self.plain(node, incoming, false),
        }
    }

    fn plain(&mut self, node: &Value, incoming: Vec<Pending>, terminal: bool) -> Vec<Pending> {
        let Some(id) = self.id(node) else {
            return incoming;
        };

        if let Some(condition) = single_conditional(node) {
            let Some(choice_id) = self.id(condition) else {
                return incoming;
            };
            self.connect(incoming, choice_id.clone(), condition);
            let guard = &condition["condition"];
            for (kind, branch) in [
                (ControlEdgeKind::True, &condition["trueExpression"]),
                (ControlEdgeKind::False, &condition["falseExpression"]),
            ] {
                if let Some(branch_id) = self.id(branch) {
                    self.connect(
                        self.outgoing(choice_id.clone(), kind, guard),
                        branch_id.clone(),
                        branch,
                    );
                    self.connect(
                        self.outgoing(branch_id, ControlEdgeKind::Next, branch),
                        id.clone(),
                        node,
                    );
                }
            }
        } else {
            if conditional_count(node) > 1 {
                self.unresolved.push(Uncertainty {
                    kind: UncertaintyKind::AmbiguousSemantics,
                    detail: "nested or multiple ternary expressions are not modeled".into(),
                    location: self.location(node),
                });
            }
            self.connect(incoming, id.clone(), node);
        }

        if terminal {
            self.connect(
                self.outgoing(id, ControlEdgeKind::Exit, node),
                BasicBlockId("exit".into()),
                node,
            );
            return Vec::new();
        }
        if let Some(guard) = guard_condition(node) {
            self.connect(
                self.outgoing(id.clone(), ControlEdgeKind::False, guard),
                BasicBlockId("exit".into()),
                node,
            );
            return self.outgoing(id, ControlEdgeKind::True, guard);
        }
        self.outgoing(id, ControlEdgeKind::Next, node)
    }
}

fn guard_condition(node: &Value) -> Option<&Value> {
    if node.get("nodeType")?.as_str()? != "ExpressionStatement" {
        return None;
    }
    let call = node.get("expression")?;
    if call.get("nodeType")?.as_str()? != "FunctionCall" {
        return None;
    }
    let name = call.get("expression")?.get("name")?.as_str()?;
    if name != "require" && name != "assert" {
        return None;
    }
    call.get("arguments")?.as_array()?.first()
}

fn conditional_count(node: &Value) -> usize {
    match node {
        Value::Array(items) => items.iter().map(conditional_count).sum(),
        Value::Object(fields) => {
            usize::from(fields.get("nodeType").and_then(Value::as_str) == Some("Conditional"))
                + fields
                    .iter()
                    .filter(|(key, _)| *key != "typeDescriptions")
                    .map(|(_, value)| conditional_count(value))
                    .sum::<usize>()
        }
        _ => 0,
    }
}

fn first_conditional(node: &Value) -> Option<&Value> {
    match node {
        Value::Array(items) => items.iter().find_map(first_conditional),
        Value::Object(fields) => {
            if fields.get("nodeType").and_then(Value::as_str) == Some("Conditional") {
                return Some(node);
            }
            fields
                .iter()
                .filter(|(key, _)| *key != "typeDescriptions")
                .find_map(|(_, value)| first_conditional(value))
        }
        _ => None,
    }
}

fn single_conditional(node: &Value) -> Option<&Value> {
    (conditional_count(node) == 1)
        .then(|| first_conditional(node))
        .flatten()
}
