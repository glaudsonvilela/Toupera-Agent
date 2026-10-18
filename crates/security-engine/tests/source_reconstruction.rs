use security_engine::{
    Confidence, ControlEdge, ControlEdgeKind, EconomicOperation, Evidence, EvidenceLocation,
    reconstruct_from_ast_json,
};

const VAULT_SOURCE: &str = include_str!("../../../tests/fixtures/VaultWithOracle.sol");
const VAULT_AST: &str = include_str!("../../../tests/fixtures/VaultWithOracle.ast.json");
const PROXY_SOURCE: &str = include_str!("../../../tests/fixtures/VaultProxyFixture.sol");
const PROXY_AST: &str = include_str!("../../../tests/fixtures/VaultProxyFixture.ast.json");
const FLOW_SOURCE: &str = include_str!("../../../tests/fixtures/VaultFlowFixture.sol");
const FLOW_AST: &str = include_str!("../../../tests/fixtures/VaultFlowFixture.ast.json");
const LOOP_SOURCE: &str = include_str!("../../../tests/fixtures/VaultLoopFixture.sol");
const LOOP_AST: &str = include_str!("../../../tests/fixtures/VaultLoopFixture.ast.json");

fn source_text<'a, T>(evidence: &Evidence<T>, source: &'a str, file: &str) -> &'a str {
    let EvidenceLocation::Source(span) = &evidence.location else {
        panic!("source reconstruction must have source provenance");
    };
    assert_eq!(span.file(), file);
    span.slice(source).expect("evidence must resolve to source")
}

fn edge_text<'a>(edge: &ControlEdge, source: &'a str, file: &str) -> &'a str {
    let EvidenceLocation::Source(span) = &edge.location else {
        panic!("source control edge must have source provenance");
    };
    assert_eq!(span.file(), file);
    span.slice(source)
        .expect("edge span must resolve to source")
}

#[test]
fn extracts_vault_economics_from_solc_ast() {
    let records = reconstruct_from_ast_json(VAULT_AST, "VaultWithOracle.sol", VAULT_SOURCE)
        .expect("fixture AST must be accepted");
    assert_eq!(records.len(), 2);

    let deposit = records
        .iter()
        .find(|item| item.function.0 == "deposit(uint256)")
        .unwrap();
    assert_eq!(deposit.operation.value, EconomicOperation::Deposit);
    assert_eq!(deposit.operation.confidence, Confidence::Inferred);
    assert_eq!(
        source_text(&deposit.operation, VAULT_SOURCE, "VaultWithOracle.sol"),
        "deposit"
    );
    assert!(
        deposit
            .reads
            .iter()
            .any(|item| item.value.0 == "totalAssets")
    );
    assert!(
        deposit
            .writes
            .iter()
            .any(|item| item.value.0 == "totalAssets")
    );
    assert!(deposit.external_calls.iter().any(|item| {
        item.value.0 == "oracle.latestPrice"
            && source_text(item, VAULT_SOURCE, "VaultWithOracle.sol") == "oracle.latestPrice()"
    }));
    assert_eq!(deposit.oracle_dependencies.len(), 1);
    assert_eq!(
        deposit.oracle_dependencies[0].confidence,
        Confidence::Inferred
    );
    assert!(deposit.arithmetic.iter().any(|item| {
        source_text(item, VAULT_SOURCE, "VaultWithOracle.sol")
            == "assets * totalSupply / totalAssets"
    }));
    assert!(
        deposit
            .control_conditions
            .iter()
            .any(|item| item.value == "totalSupply == 0")
    );

    let redeem = records
        .iter()
        .find(|item| item.function.0 == "redeem(uint256)")
        .unwrap();
    assert_eq!(redeem.operation.value, EconomicOperation::Redeem);
    assert!(redeem.oracle_dependencies.is_empty());
    assert!(
        redeem
            .writes
            .iter()
            .any(|item| item.value.0 == "totalSupply")
    );
    assert!(redeem.arithmetic.iter().any(|item| {
        source_text(item, VAULT_SOURCE, "VaultWithOracle.sol")
            == "shares * totalAssets / totalSupply"
    }));
}

#[test]
fn records_delegatecall_as_a_tentative_proxy_relation() {
    let records = reconstruct_from_ast_json(PROXY_AST, "VaultProxyFixture.sol", PROXY_SOURCE)
        .expect("proxy fixture AST must be accepted");
    let fallback = records
        .iter()
        .find(|item| item.function.0 == "fallback()")
        .expect("fallback must be reconstructed");
    assert_eq!(fallback.operation.value, EconomicOperation::Other);
    assert_eq!(fallback.delegatecall_targets.len(), 1);
    assert_eq!(fallback.delegatecall_targets[0].value.0, "implementation");
    assert_eq!(
        fallback.delegatecall_targets[0].confidence,
        Confidence::Tentative
    );
    assert_eq!(
        source_text(
            &fallback.delegatecall_targets[0],
            PROXY_SOURCE,
            "VaultProxyFixture.sol"
        ),
        "implementation"
    );
    assert_eq!(fallback.unresolved.len(), 1);
}

#[test]
fn links_internal_calls_modifiers_and_branch_conditions() {
    let records = reconstruct_from_ast_json(FLOW_AST, "VaultFlowFixture.sol", FLOW_SOURCE)
        .expect("flow fixture AST must be accepted");
    let conversion = records
        .iter()
        .find(|item| item.function.0 == "convertToShares(uint256)")
        .unwrap();
    assert_eq!(
        conversion.operation.value,
        EconomicOperation::ConvertToShares
    );
    assert_eq!(conversion.control_conditions.len(), 1);
    assert_eq!(conversion.control_conditions[0].value, "totalSupply == 0");
    assert!(
        conversion
            .reads
            .iter()
            .any(|item| item.value.0 == "totalAssets")
    );

    let preview = records
        .iter()
        .find(|item| item.function.0 == "previewDeposit(uint256)")
        .unwrap();
    assert_eq!(preview.operation.value, EconomicOperation::PreviewDeposit);
    assert_eq!(preview.modifiers.len(), 1);
    assert_eq!(preview.modifiers[0].value, "active");
    assert!(
        preview
            .unresolved
            .iter()
            .any(|item| item.detail.contains("modifier body"))
    );
    assert_eq!(preview.related_functions.len(), 1);
    assert_eq!(
        preview.related_functions[0].value.0,
        "convertToShares(uint256)"
    );
    assert_eq!(
        source_text(
            &preview.related_functions[0],
            FLOW_SOURCE,
            "VaultFlowFixture.sol"
        ),
        "convertToShares(assets)"
    );
}

#[test]
fn rejects_compiler_errors_and_mismatched_source_ids() {
    let error = reconstruct_from_ast_json(
        r#"{"errors":[{"severity":"error","formattedMessage":"bad Solidity"}]}"#,
        "VaultWithOracle.sol",
        VAULT_SOURCE,
    )
    .unwrap_err();
    assert!(error.contains("bad Solidity"));

    let error = reconstruct_from_ast_json(VAULT_AST, "VaultWithOracle.sol", "unrelated source")
        .unwrap_err();
    assert!(error.contains("does not match AST offsets"));

    let mut output: serde_json::Value = serde_json::from_str(VAULT_AST).unwrap();
    output["sources"]["VaultWithOracle.sol"]["id"] = serde_json::json!(5);
    let error = reconstruct_from_ast_json(&output.to_string(), "VaultWithOracle.sol", VAULT_SOURCE)
        .unwrap_err();
    assert!(error.contains("source id does not match"));
}

#[test]
fn control_graph_separates_returning_if_branches() {
    let records = reconstruct_from_ast_json(FLOW_AST, "VaultFlowFixture.sol", FLOW_SOURCE)
        .expect("flow fixture AST must be accepted");
    let conversion = records
        .iter()
        .find(|item| item.function.0 == "convertToShares(uint256)")
        .unwrap();
    let edges = &conversion.control_edges;
    assert!(
        edges
            .iter()
            .all(|edge| !edge_text(edge, FLOW_SOURCE, "VaultFlowFixture.sol").is_empty())
    );

    let true_edge = edges
        .iter()
        .find(|edge| {
            edge.kind == ControlEdgeKind::True
                && edge_text(edge, FLOW_SOURCE, "VaultFlowFixture.sol") == "totalSupply == 0"
        })
        .unwrap();
    let false_edge = edges
        .iter()
        .find(|edge| edge.kind == ControlEdgeKind::False && edge.from == true_edge.from)
        .unwrap();
    assert_eq!(
        edge_text(false_edge, FLOW_SOURCE, "VaultFlowFixture.sol"),
        "totalSupply == 0"
    );
    assert_ne!(true_edge.to, false_edge.to);
    assert!(edges.iter().any(|edge| edge.from == true_edge.to
        && edge.to.0 == "exit"
        && edge.kind == ControlEdgeKind::Exit));
    assert!(edges.iter().any(|edge| edge.from == false_edge.to
        && edge.to.0 == "exit"
        && edge.kind == ControlEdgeKind::Exit));
    assert!(
        !edges
            .iter()
            .any(|edge| edge.from == true_edge.to && edge.to == false_edge.to)
    );
}

#[test]
fn control_graph_models_require_failure_and_ternary_paths() {
    let records = reconstruct_from_ast_json(VAULT_AST, "VaultWithOracle.sol", VAULT_SOURCE)
        .expect("vault fixture AST must be accepted");
    let deposit = records
        .iter()
        .find(|item| item.function.0 == "deposit(uint256)")
        .unwrap();
    let edges = &deposit.control_edges;
    assert!(
        edges
            .iter()
            .all(|edge| !edge_text(edge, VAULT_SOURCE, "VaultWithOracle.sol").is_empty())
    );

    let guard = edges
        .iter()
        .find(|edge| {
            edge.kind == ControlEdgeKind::False
                && edge.to.0 == "exit"
                && edge_text(edge, VAULT_SOURCE, "VaultWithOracle.sol") == "price > 0"
        })
        .unwrap();
    assert!(edges.iter().any(|edge| edge.from == guard.from
        && edge.kind == ControlEdgeKind::True
        && edge_text(edge, VAULT_SOURCE, "VaultWithOracle.sol") == "price > 0"));

    let choice = edges
        .iter()
        .find(|edge| {
            edge.kind == ControlEdgeKind::True
                && edge_text(edge, VAULT_SOURCE, "VaultWithOracle.sol") == "totalSupply == 0"
        })
        .unwrap();
    let alternative = edges
        .iter()
        .find(|edge| edge.from == choice.from && edge.kind == ControlEdgeKind::False)
        .unwrap();
    assert_ne!(choice.to, alternative.to);
    assert_eq!(
        edge_text(alternative, VAULT_SOURCE, "VaultWithOracle.sol"),
        "totalSupply == 0"
    );
    let merge = edges
        .iter()
        .find(|edge| edge.from == choice.to && edge.kind == ControlEdgeKind::Next)
        .unwrap();
    assert!(edges.iter().any(|edge| edge.from == alternative.to
        && edge.to == merge.to
        && edge.kind == ControlEdgeKind::Next));
}

#[test]
fn loop_graph_routes_break_continue_and_condition_exits() {
    let records = reconstruct_from_ast_json(LOOP_AST, "VaultLoopFixture.sol", LOOP_SOURCE)
        .expect("loop fixture AST must be accepted");
    assert_eq!(records.len(), 4);
    for (function, condition) in [
        ("sumUntil(uint256)", "i < limit"),
        ("countAtLeastOnce(uint256)", "count < limit"),
        ("countWhile(uint256)", "count < limit"),
    ] {
        let graph = records
            .iter()
            .find(|item| item.function.0 == function)
            .unwrap();
        assert!(
            graph.unresolved.is_empty(),
            "{function}: {:?}",
            graph.unresolved
        );
        let edges = &graph.control_edges;
        assert!(
            edges
                .iter()
                .all(|edge| !edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol").is_empty())
        );

        let loop_exit = edges
            .iter()
            .find(|edge| {
                edge.kind == ControlEdgeKind::False
                    && edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol") == condition
                    && edges.iter().any(|next| {
                        next.from == edge.to
                            && next.to.0 == "exit"
                            && next.kind == ControlEdgeKind::Exit
                    })
            })
            .expect("false condition must reach the return after the loop");
        let break_edge = edges
            .iter()
            .find(|edge| edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol").starts_with("break"))
            .expect("break must be represented");
        assert_eq!(
            break_edge.to, loop_exit.to,
            "break must skip remaining loop work"
        );
        assert_eq!(break_edge.kind, ControlEdgeKind::Next);

        let continue_edge = edges
            .iter()
            .find(|edge| {
                edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol").starts_with("continue")
            })
            .expect("continue must be represented");
        assert_eq!(continue_edge.kind, ControlEdgeKind::LoopBack);
        assert_ne!(continue_edge.to, loop_exit.to);
        if function == "sumUntil(uint256)" {
            assert!(
                edges.iter().any(|edge| edge.from == continue_edge.to
                    && edge.to == loop_exit.from
                    && edge.kind == ControlEdgeKind::LoopBack),
                "for continue must execute the update before rechecking the condition"
            );
        } else if function == "countAtLeastOnce(uint256)" {
            assert_eq!(
                continue_edge.to, loop_exit.from,
                "do while continue must reach the condition"
            );
            assert!(
                edges
                    .iter()
                    .any(|edge| edge.from == loop_exit.from && edge.kind == ControlEdgeKind::True),
                "do while condition true must revisit the body"
            );
        } else {
            assert_eq!(
                continue_edge.to, loop_exit.from,
                "while continue must recheck the condition"
            );
        }
    }
}

#[test]
fn nested_loop_break_stays_inside_outer_loop() {
    let records = reconstruct_from_ast_json(LOOP_AST, "VaultLoopFixture.sol", LOOP_SOURCE)
        .expect("loop fixture AST must be accepted");
    let graph = records
        .iter()
        .find(|item| item.function.0 == "nestedLoops(uint256,uint256)")
        .unwrap();
    assert!(graph.unresolved.is_empty());
    let edges = &graph.control_edges;
    let inner_exit = edges
        .iter()
        .find(|edge| {
            edge.kind == ControlEdgeKind::False
                && edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol") == "count < innerLimit"
        })
        .unwrap();
    let outer_exit = edges
        .iter()
        .find(|edge| {
            edge.kind == ControlEdgeKind::False
                && edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol") == "i < outerLimit"
        })
        .unwrap();
    let inner_break = edges
        .iter()
        .find(|edge| edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol").starts_with("break"))
        .unwrap();
    assert_eq!(inner_break.to, inner_exit.to);
    assert_ne!(inner_break.to, outer_exit.to);

    let outer_continue = edges
        .iter()
        .find(|edge| edge_text(edge, LOOP_SOURCE, "VaultLoopFixture.sol").starts_with("continue"))
        .unwrap();
    assert_eq!(outer_continue.kind, ControlEdgeKind::LoopBack);
    assert!(edges.iter().any(|edge| edge.from == outer_continue.to
        && edge.to == outer_exit.from
        && edge.kind == ControlEdgeKind::LoopBack));
    assert_ne!(outer_continue.to, inner_exit.from);
}
