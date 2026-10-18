use security_engine::{
    Confidence, EconomicOperation, Evidence, EvidenceLocation, reconstruct_from_ast_json,
};

const VAULT_SOURCE: &str = include_str!("../../../tests/fixtures/VaultWithOracle.sol");
const VAULT_AST: &str = include_str!("../../../tests/fixtures/VaultWithOracle.ast.json");
const PROXY_SOURCE: &str = include_str!("../../../tests/fixtures/VaultProxyFixture.sol");
const PROXY_AST: &str = include_str!("../../../tests/fixtures/VaultProxyFixture.ast.json");
const FLOW_SOURCE: &str = include_str!("../../../tests/fixtures/VaultFlowFixture.sol");
const FLOW_AST: &str = include_str!("../../../tests/fixtures/VaultFlowFixture.ast.json");

fn source_text<'a, T>(evidence: &Evidence<T>, source: &'a str, file: &str) -> &'a str {
    let EvidenceLocation::Source(span) = &evidence.location else {
        panic!("source reconstruction must have source provenance");
    };
    assert_eq!(span.file(), file);
    span.slice(source).expect("evidence must resolve to source")
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
