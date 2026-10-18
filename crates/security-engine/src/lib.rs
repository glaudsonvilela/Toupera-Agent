//! Foundational types for focused Solidity/EVM economic security analysis.

/// Stable identity of a contract in an analysis target.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContractId(pub String);

/// Stable identity of a function in an analyzed contract.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FunctionId(pub String);

/// Economic operation associated with a Solidity function.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EconomicOperation {
    Deposit,
    Mint,
    Withdraw,
    Redeem,
    PreviewDeposit,
    PreviewMint,
    PreviewWithdraw,
    PreviewRedeem,
    ConvertToShares,
    ConvertToAssets,
    TotalAssets,
    TotalSupply,
    OraclePrice,
    Other,
}

/// Initial finding categories in the M0 scope.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FindingKind {
    PriceSourceAsymmetry,
    DecimalMismatch,
    RoundingAsymmetry,
    PreviewExecutionMismatch,
    ShareInflation,
    FeeBypass,
    OracleFallbackMismatch,
    AccountingDrift,
}

/// A candidate economic issue. Findings require local reproduction before confirmation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub contract: ContractId,
    pub function: FunctionId,
    pub kind: FindingKind,
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_finding_with_vault_operation_types() {
        let finding = Finding {
            contract: ContractId("Vault".into()),
            function: FunctionId("redeem".into()),
            kind: FindingKind::RoundingAsymmetry,
            summary: "Preview and execution round in incompatible directions".into(),
        };

        assert_eq!(finding.contract.0, "Vault");
        assert_eq!(finding.function.0, "redeem");
        assert_eq!(finding.kind, FindingKind::RoundingAsymmetry);
        assert_eq!(EconomicOperation::Redeem, EconomicOperation::Redeem);
    }

    #[test]
    fn finding_kinds_are_hashable_for_deduplication() {
        let kinds = [FindingKind::DecimalMismatch, FindingKind::DecimalMismatch];
        let unique: std::collections::HashSet<_> = kinds.into_iter().collect();
        assert_eq!(unique.len(), 1);
    }
}
