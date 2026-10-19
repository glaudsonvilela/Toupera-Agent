//! Foundational types for focused Solidity/EVM economic security analysis.

mod adaptive_chip;
mod adaptive_cycle;
mod assurance;
mod bytecode;
mod bytecode_companions;
mod bytecode_evmole;
mod bytecode_flow;
mod cached_reassessment;
mod callgraph;
mod cfg;
mod context_package;
mod dataflow;
mod economic_analysis;
mod evidence_filter;
mod evidence_package;
mod forensic_plan;
mod forensic_probe;
mod foundry_judge;
mod ir;
mod llm_review;
mod local_coverage;
mod local_scenarios;
mod source;
mod trace_reconcile;
mod variable_lens;

use serde::Serialize;

pub use adaptive_cycle::plan_adaptive_step;
pub use assurance::guard_wcop_job;
pub use bytecode::{
    BytecodeAnalysis, BytecodeBlock, BytecodeEffect, BytecodeEffectKind, BytecodeEntrySummary,
    BytecodeImmutableValue, BytecodeInstruction, BytecodeMetadata, BytecodeMethodMatch,
    BytecodeSelector, BytecodeSourceComparison, BytecodeValueFlow, BytecodeValueOrigin,
    BytecodeValueOriginKind, BytecodeValueSinkKind, compare_with_solc_contract,
    normalized_runtime_hex, reconstruct_bytecode_hex,
};
pub use bytecode_companions::{
    CompanionAnalysis, GigahorseBlock, GigahorseReport, HeimdallFunction, HeimdallReport,
    import_gigahorse_tac, import_heimdall_abi,
};
pub use bytecode_evmole::{
    EvmoleControlEdgeEvidence, EvmoleReport, EvmoleSelectorEvidence, EvmoleStorageEvidence,
};
pub use cached_reassessment::reassess_cached_map;
pub use context_package::{
    ArtifactDigests, CachedContextPackage, ContextPackage, ContextSlice, EvidenceExcerpt,
    FunctionDigest, FunctionExcerpt, build_context_package, load_or_build_context_package,
    load_or_build_context_package_with_evm_version, load_or_build_context_package_with_forensic,
};
pub use economic_analysis::{
    EconomicAnalysisMetrics, EconomicAnalysisReport, EconomicCandidate, SlitherSignal,
    analyze_economic_functions,
};
pub use evidence_filter::{FilterReport, filter_evidence, gate_review_plan};
pub use evidence_package::{
    EvidenceFact, EvidenceManifest, EvidenceReady, HumanEvidenceReview, finalize_evidence_bundle,
    prepare_evidence_bundle, prepare_evidence_bundle_with_guard,
};
pub use forensic_plan::{ForensicPlan, SourceBenchRow, benchmark_sources, build_forensic_plan};
pub use forensic_probe::{ForensicInput, ForensicReport, run_forensic_probe};
pub use foundry_judge::{
    FoundryCase, FoundryCoverage, FoundryPlan, FoundryRejection, FoundryResult, FoundryRun,
    HarnessApproval, RejectionReview, judge_foundry_case, prepare_foundry_cases,
    reject_foundry_case,
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
    empty_review_report, prepare_review_plan, run_review,
};
pub use local_coverage::{LocalCoverageReport, analyze_recorded_scenarios};
pub use local_scenarios::{LocalScenarioReport, run_guided_scenarios, run_local_scenarios};
pub use source::{
    compile_source_ast_file, compile_source_ast_text, compile_source_ast_text_with_evm_version,
    reconstruct_from_ast_json, reconstruct_source_file,
};
pub use trace_reconcile::reconcile_trace;
pub use variable_lens::{VariableLensReport, build_variable_lens};

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
