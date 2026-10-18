//! Foundational types for focused Solidity/EVM economic security analysis.

mod bytecode;
mod bytecode_companions;
mod bytecode_evmole;
mod bytecode_flow;
mod callgraph;
mod cfg;
mod context_package;
mod dataflow;
mod economic_analysis;
mod evidence_package;
mod foundry_judge;
mod ir;
mod llm_review;
mod source;

use serde::Serialize;

pub use bytecode::{
    BytecodeAnalysis, BytecodeBlock, BytecodeEffect, BytecodeEffectKind, BytecodeEntrySummary,
    BytecodeInstruction, BytecodeMetadata, BytecodeMethodMatch, BytecodeSelector,
    BytecodeSourceComparison, BytecodeValueFlow, BytecodeValueOrigin, BytecodeValueOriginKind,
    BytecodeValueSinkKind, compare_with_solc_contract, normalized_runtime_hex,
    reconstruct_bytecode_hex,
};
pub use bytecode_companions::{
    CompanionAnalysis, GigahorseBlock, GigahorseReport, HeimdallFunction, HeimdallReport,
    import_gigahorse_tac, import_heimdall_abi,
};
pub use bytecode_evmole::{
    EvmoleControlEdgeEvidence, EvmoleReport, EvmoleSelectorEvidence, EvmoleStorageEvidence,
};
pub use context_package::{
    ArtifactDigests, CachedContextPackage, ContextPackage, ContextSlice, EvidenceExcerpt,
    FunctionDigest, FunctionExcerpt, build_context_package, load_or_build_context_package,
};
pub use economic_analysis::{
    EconomicAnalysisMetrics, EconomicAnalysisReport, EconomicCandidate, SlitherSignal,
    analyze_economic_functions,
};
pub use evidence_package::{
    EvidenceFact, EvidenceManifest, EvidenceReady, HumanEvidenceReview, finalize_evidence_bundle,
    prepare_evidence_bundle,
};
pub use foundry_judge::{
    FoundryCase, FoundryPlan, FoundryResult, FoundryRun, HarnessApproval, judge_foundry_case,
    prepare_foundry_cases,
};
pub use ir::{
    AppliedModifier, ArithmeticExpression, BasicBlockId, BytecodePc, CallTarget, Confidence,
    ControlEdge, ControlEdgeKind, EconomicFunction, Evidence, EvidenceLocation,
    ModifierArgumentBinding, ProxyRelation, ReachabilityPath, ReachabilityTerminal,
    ResolvedFunction, SourceSpan, StorageId, Uncertainty, UncertaintyKind, ValueFlow, ValueOrigin,
    ValueSink,
};
pub use llm_review::{
    CachingProvider, DeepSeekProvider, HypothesisProvider, ModelConclusion, PreparedRequest,
    ProviderReply, RateCard, ReviewItem, ReviewMetrics, ReviewPlan, ReviewReport, TokenUsage,
    prepare_review_plan, run_review,
};
pub use source::{
    compile_source_ast_file, compile_source_ast_text, reconstruct_from_ast_json,
    reconstruct_source_file,
};

/// Stable identity of a contract in an analysis target.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct ContractId(pub String);

/// Stable identity of a function in an analyzed contract.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct FunctionId(pub String);

/// Economic operation associated with a Solidity function.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
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
