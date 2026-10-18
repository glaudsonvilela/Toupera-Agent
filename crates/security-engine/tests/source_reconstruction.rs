use security_engine::{
    Confidence, ControlEdge, ControlEdgeKind, EconomicOperation, Evidence, EvidenceLocation,
    ReachabilityTerminal, ValueOrigin, ValueSink, reconstruct_from_ast_json,
};

const VAULT_SOURCE: &str = include_str!("../../../tests/fixtures/VaultWithOracle.sol");
const VAULT_AST: &str = include_str!("../../../tests/fixtures/VaultWithOracle.ast.json");
const PROXY_SOURCE: &str = include_str!("../../../tests/fixtures/VaultProxyFixture.sol");
const PROXY_AST: &str = include_str!("../../../tests/fixtures/VaultProxyFixture.ast.json");
const FLOW_SOURCE: &str = include_str!("../../../tests/fixtures/VaultFlowFixture.sol");
const FLOW_AST: &str = include_str!("../../../tests/fixtures/VaultFlowFixture.ast.json");
const LOOP_SOURCE: &str = include_str!("../../../tests/fixtures/VaultLoopFixture.sol");
const LOOP_AST: &str = include_str!("../../../tests/fixtures/VaultLoopFixture.ast.json");
const MODIFIER_SOURCE: &str = include_str!("../../../tests/fixtures/ModifierFlowFixture.sol");
const MODIFIER_AST: &str = include_str!("../../../tests/fixtures/ModifierFlowFixture.ast.json");
const CALL_SOURCE: &str = include_str!("../../../tests/fixtures/VaultCallFlowFixture.sol");
const CALL_AST: &str = include_str!("../../../tests/fixtures/VaultCallFlowFixture.ast.json");
const INHERITED_SOURCE: &str = include_str!("../../../tests/fixtures/InheritedVaultFixture.sol");
const INHERITED_AST: &str = include_str!("../../../tests/fixtures/InheritedVaultFixture.ast.json");

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
    assert_eq!(preview.applied_modifiers.len(), 1);
    assert_eq!(preview.applied_modifiers[0].definition.value, "active");
    assert!(
        preview.applied_modifiers[0]
            .reads
            .iter()
            .any(|item| item.value.0 == "totalAssets")
    );
    assert!(
        preview
            .reads
            .iter()
            .any(|item| item.value.0 == "totalAssets")
    );
    assert!(
        preview
            .control_conditions
            .iter()
            .any(|item| item.value == "totalAssets > 0")
    );
    assert!(preview.unresolved.is_empty());
    assert!(
        preview
            .control_edges
            .iter()
            .any(|edge| edge.kind == ControlEdgeKind::False
                && edge.to.0 == "exit"
                && edge_text(edge, FLOW_SOURCE, "VaultFlowFixture.sol") == "totalAssets > 0")
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
fn local_modifier_effects_keep_their_own_provenance_and_uncertainty() {
    let records =
        reconstruct_from_ast_json(MODIFIER_AST, "ModifierFlowFixture.sol", MODIFIER_SOURCE)
            .expect("modifier fixture AST must be accepted");
    let deposit = records
        .iter()
        .find(|item| item.function.0 == "deposit(uint256)")
        .unwrap();
    assert_eq!(deposit.applied_modifiers.len(), 1);
    let effect = &deposit.applied_modifiers[0];
    assert_eq!(
        source_text(
            &effect.invocation,
            MODIFIER_SOURCE,
            "ModifierFlowFixture.sol"
        ),
        "checkedAndAccounted(assets)"
    );
    assert_eq!(
        source_text(
            &effect.definition,
            MODIFIER_SOURCE,
            "ModifierFlowFixture.sol"
        ),
        "checkedAndAccounted"
    );
    assert!(
        effect
            .reads
            .iter()
            .any(|item| item.value.0 == "totalAssets")
    );
    assert!(
        effect
            .reads
            .iter()
            .any(|item| item.value.0 == "totalSupply")
    );
    assert!(
        effect
            .writes
            .iter()
            .any(|item| item.value.0 == "totalSupply")
    );
    assert!(
        effect
            .control_conditions
            .iter()
            .any(|item| item.value == "totalAssets >= minAssets")
    );
    assert!(
        effect
            .control_conditions
            .iter()
            .any(|item| item.value == "totalSupply < 1000")
    );
    assert!(effect.unresolved.is_empty());
    assert!(deposit.unresolved.is_empty());
    assert_eq!(effect.bindings.len(), 1);
    assert_eq!(effect.bindings[0].parameter.value, "minAssets");
    assert_eq!(effect.bindings[0].argument.value, "assets");
    assert_eq!(
        source_text(
            &effect.bindings[0].parameter,
            MODIFIER_SOURCE,
            "ModifierFlowFixture.sol"
        ),
        "uint256 minAssets"
    );
    assert_eq!(
        source_text(
            &effect.bindings[0].argument,
            MODIFIER_SOURCE,
            "ModifierFlowFixture.sol"
        ),
        "assets"
    );
    assert!(
        !deposit
            .unresolved
            .iter()
            .any(|item| item.detail.contains("not spliced"))
    );
    let return_to_postlude = deposit
        .control_edges
        .iter()
        .find(|edge| {
            edge.kind == ControlEdgeKind::Next
                && edge_text(edge, MODIFIER_SOURCE, "ModifierFlowFixture.sol")
                    .starts_with("return assets")
        })
        .expect("function return must enter modifier postlude");
    assert_ne!(return_to_postlude.to.0, "exit");
    assert!(
        deposit
            .control_edges
            .iter()
            .any(|edge| edge.from == return_to_postlude.to && edge.to.0 == "exit")
    );
    assert!(
        deposit
            .writes
            .iter()
            .any(|item| item.value.0 == "totalSupply" && effect.writes.contains(item))
    );
    assert!(
        deposit
            .writes
            .iter()
            .any(|item| item.value.0 == "totalAssets" && !effect.writes.contains(item))
    );
}

#[test]
fn multiple_modifiers_wrap_function_in_source_order() {
    let records =
        reconstruct_from_ast_json(MODIFIER_AST, "ModifierFlowFixture.sol", MODIFIER_SOURCE)
            .unwrap();
    let mint = records
        .iter()
        .find(|item| item.function.0 == "mint(uint256)")
        .unwrap();
    assert_eq!(
        mint.applied_modifiers
            .iter()
            .map(|item| item.definition.value.as_str())
            .collect::<Vec<_>>(),
        vec!["charged", "checkedAndAccounted"]
    );
    assert!(mint.unresolved.is_empty());
    let edges = &mint.control_edges;
    let returned = edges
        .iter()
        .find(|edge| edge_text(edge, MODIFIER_SOURCE, "ModifierFlowFixture.sol") == "return assets")
        .unwrap();
    assert_eq!(returned.kind, ControlEdgeKind::Next);
    let inner_postlude = edges.iter().find(|edge| edge.from == returned.to).unwrap();
    assert_eq!(
        edge_text(inner_postlude, MODIFIER_SOURCE, "ModifierFlowFixture.sol"),
        "totalSupply += 1"
    );
    let outer_postlude = edges
        .iter()
        .find(|edge| edge.from == inner_postlude.to)
        .unwrap();
    assert_eq!(
        edge_text(outer_postlude, MODIFIER_SOURCE, "ModifierFlowFixture.sol"),
        "totalSupply += 3"
    );
    assert_eq!(outer_postlude.to.0, "exit");
}

#[test]
fn internal_call_paths_reach_oracle_conversion_and_storage() {
    let records =
        reconstruct_from_ast_json(CALL_AST, "VaultCallFlowFixture.sol", CALL_SOURCE).unwrap();
    let deposit = records
        .iter()
        .find(|item| item.function.0 == "deposit(uint256)")
        .unwrap();
    assert!(deposit.unresolved.is_empty());
    assert!(deposit.reachability_paths.iter().any(|path| {
        path.calls.len() == 2 && path.calls[0].value.0 == "readPrice()" && path.calls[1].value.0 == "rawPrice()"
            && source_text(&path.calls[0], CALL_SOURCE, "VaultCallFlowFixture.sol") == "readPrice()"
            && matches!(&path.terminal, ReachabilityTerminal::OracleCall(call)
                if call.value.0 == "oracle.latestPrice"
                    && source_text(call, CALL_SOURCE, "VaultCallFlowFixture.sol") == "oracle.latestPrice()")
    }));
    assert!(deposit.reachability_paths.iter().any(|path| {
        path.calls.len() == 1
            && path.calls[0].value.0 == "convertToShares(uint256)"
            && matches!(&path.terminal, ReachabilityTerminal::Conversion(operation)
                if operation.value == EconomicOperation::ConvertToShares)
    }));
    assert!(deposit.reachability_paths.iter().any(|path| {
        path.calls.len() == 1
            && path.calls[0].value.0 == "convertToShares(uint256)"
            && matches!(&path.terminal, ReachabilityTerminal::StorageRead(read)
                if read.value.0 == "totalAssets")
    }));
}

#[test]
fn oracle_value_flow_distinguishes_guard_only_from_share_pricing() {
    let guarded =
        reconstruct_from_ast_json(VAULT_AST, "VaultWithOracle.sol", VAULT_SOURCE).unwrap();
    let guarded_deposit = guarded
        .iter()
        .find(|item| item.function.0 == "deposit(uint256)")
        .unwrap();
    let oracle_flows: Vec<_> = guarded_deposit
        .value_flows
        .iter()
        .filter(|flow| matches!(flow.origin.value, ValueOrigin::OracleCall(_)))
        .collect();
    assert!(!oracle_flows.is_empty());
    assert!(
        oracle_flows
            .iter()
            .all(|flow| flow.sink.value == ValueSink::Guard)
    );

    let priced =
        reconstruct_from_ast_json(CALL_AST, "VaultCallFlowFixture.sol", CALL_SOURCE).unwrap();
    let priced_deposit = priced
        .iter()
        .find(|item| item.function.0 == "deposit(uint256)")
        .unwrap();
    let to_conversion = priced_deposit.value_flows.iter().find(|flow| {
        matches!(&flow.origin.value, ValueOrigin::OracleCall(target) if target.0 == "oracle.latestPrice")
            && matches!(&flow.sink.value, ValueSink::ConversionArgument(function) if function.0 == "convertToShares(uint256)")
    }).expect("oracle value must reach conversion input");
    assert_eq!(
        to_conversion
            .calls
            .iter()
            .map(|call| call.value.0.as_str())
            .collect::<Vec<_>>(),
        vec!["rawPrice()", "readPrice()"]
    );
    assert_eq!(
        source_text(
            &to_conversion.origin,
            CALL_SOURCE,
            "VaultCallFlowFixture.sol"
        ),
        "oracle.latestPrice()"
    );
    assert_eq!(
        source_text(&to_conversion.sink, CALL_SOURCE, "VaultCallFlowFixture.sol"),
        "assets * price"
    );

    let to_supply = priced_deposit.value_flows.iter().find(|flow| {
        matches!(flow.origin.value, ValueOrigin::OracleCall(_))
            && matches!(&flow.sink.value, ValueSink::StorageWrite(storage) if storage.0 == "totalSupply")
    }).expect("oracle value must reach share supply update");
    assert_eq!(
        to_supply
            .calls
            .iter()
            .map(|call| call.value.0.as_str())
            .collect::<Vec<_>>(),
        vec!["rawPrice()", "readPrice()", "convertToShares(uint256)"]
    );
}

#[test]
fn inherited_members_keep_declaring_contract_and_economic_paths() {
    let records =
        reconstruct_from_ast_json(INHERITED_AST, "InheritedVaultFixture.sol", INHERITED_SOURCE)
            .unwrap();
    let deposit = records
        .iter()
        .find(|item| {
            item.contract.0 == "InheritedVaultFixture" && item.function.0 == "deposit(uint256)"
        })
        .unwrap();
    assert!(deposit.unresolved.is_empty(), "{:?}", deposit.unresolved);
    assert!(
        deposit
            .reads
            .iter()
            .any(|read| read.value.0 == "totalAssets")
    );
    assert!(
        deposit
            .writes
            .iter()
            .any(|write| write.value.0 == "totalSupply")
    );
    assert_eq!(deposit.applied_modifiers[0].definition.value, "active");
    assert!(deposit.control_edges.iter().any(|edge| {
        edge_text(edge, INHERITED_SOURCE, "InheritedVaultFixture.sol").contains("totalAssets > 0")
    }));
    assert!(
        deposit
            .resolved_calls
            .iter()
            .any(|call| call.value.contract.0 == "BaseVault"
                && call.value.function.0 == "readPrice()")
    );
    assert!(
        deposit
            .resolved_calls
            .iter()
            .any(|call| call.value.contract.0 == "BaseVault"
                && call.value.function.0 == "convertToShares(uint256)")
    );
    assert!(deposit.reachability_paths.iter().any(|path| {
        path.calls.iter().any(|call| call.value.0 == "readPrice()")
            && matches!(&path.terminal, ReachabilityTerminal::OracleCall(call) if call.value.0 == "oracle.latestPrice")
    }));
    assert!(deposit.value_flows.iter().any(|flow| {
        matches!(&flow.origin.value, ValueOrigin::OracleCall(call) if call.0 == "oracle.latestPrice")
            && matches!(&flow.sink.value, ValueSink::ConversionArgument(function) if function.0 == "convertToShares(uint256)")
    }));
    assert!(deposit.value_flows.iter().any(|flow| {
        matches!(&flow.origin.value, ValueOrigin::OracleCall(_))
            && matches!(&flow.sink.value, ValueSink::StorageWrite(storage) if storage.0 == "totalSupply")
    }));
}

#[test]
fn modifier_argument_and_guard_value_flows_are_composed() {
    let records =
        reconstruct_from_ast_json(MODIFIER_AST, "ModifierFlowFixture.sol", MODIFIER_SOURCE)
            .unwrap();
    let mint = records
        .iter()
        .find(|item| item.function.0 == "mint(uint256)")
        .unwrap();
    assert!(mint.unresolved.is_empty());
    assert!(mint.value_flows.iter().any(|flow| {
        matches!(&flow.origin.value, ValueOrigin::Parameter(name) if name == "assets")
            && flow.sink.value == ValueSink::Guard
            && source_text(&flow.sink, MODIFIER_SOURCE, "ModifierFlowFixture.sol")
                == "totalAssets >= minAssets"
    }));
    assert!(mint.value_flows.iter().any(|flow| {
        matches!(&flow.origin.value, ValueOrigin::Storage(storage) if storage.0 == "totalSupply")
            && matches!(&flow.sink.value, ValueSink::StorageWrite(storage) if storage.0 == "totalSupply")
            && source_text(&flow.sink, MODIFIER_SOURCE, "ModifierFlowFixture.sol") == "totalSupply"
    }));
}

#[test]
fn unresolved_modifier_links_and_missing_placeholders_stay_explicit() {
    let mut missing_definition: serde_json::Value = serde_json::from_str(FLOW_AST).unwrap();
    let contract = missing_definition["sources"]["VaultFlowFixture.sol"]["ast"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["nodeType"] == "ContractDefinition")
        .unwrap();
    let preview = contract["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["name"] == "previewDeposit")
        .unwrap();
    preview["modifiers"][0]["modifierName"]["referencedDeclaration"] = serde_json::json!(999999);
    let records = reconstruct_from_ast_json(
        &missing_definition.to_string(),
        "VaultFlowFixture.sol",
        FLOW_SOURCE,
    )
    .unwrap();
    let preview = records
        .iter()
        .find(|item| item.function.0 == "previewDeposit(uint256)")
        .unwrap();
    assert!(preview.applied_modifiers.is_empty());
    assert!(
        preview
            .unresolved
            .iter()
            .any(|item| item.detail.contains("not available"))
    );

    let mut missing_placeholder: serde_json::Value = serde_json::from_str(FLOW_AST).unwrap();
    let contract = missing_placeholder["sources"]["VaultFlowFixture.sol"]["ast"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["nodeType"] == "ContractDefinition")
        .unwrap();
    let modifier = contract["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["nodeType"] == "ModifierDefinition")
        .unwrap();
    modifier["body"]["statements"]
        .as_array_mut()
        .unwrap()
        .retain(|statement| statement["nodeType"] != "PlaceholderStatement");
    let records = reconstruct_from_ast_json(
        &missing_placeholder.to_string(),
        "VaultFlowFixture.sol",
        FLOW_SOURCE,
    )
    .unwrap();
    let preview = records
        .iter()
        .find(|item| item.function.0 == "previewDeposit(uint256)")
        .unwrap();
    assert!(
        preview.applied_modifiers[0]
            .unresolved
            .iter()
            .any(|item| item.detail.contains("exactly one"))
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
            graph.unresolved.iter().all(|item| item
                .detail
                .contains("loop value flow uses a one-iteration approximation")),
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
    assert!(graph.unresolved.iter().all(|item| {
        item.detail
            .contains("loop value flow uses a one-iteration approximation")
    }));
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
