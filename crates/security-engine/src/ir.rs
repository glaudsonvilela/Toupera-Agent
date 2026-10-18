//! Compact, provenance-aware representation of a function's economic behavior.

use crate::{ContractId, EconomicOperation, FunctionId};
use serde::Serialize;

/// Identity of a source-level control block or a future EVM basic block.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct BasicBlockId(pub String);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum ControlEdgeKind {
    Next,
    True,
    False,
    LoopBack,
    Exit,
}

/// A directed edge in a function's source-level control graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ControlEdge {
    pub from: BasicBlockId,
    pub to: BasicBlockId,
    pub kind: ControlEdgeKind,
    pub location: EvidenceLocation,
}

/// Byte offsets into a source file, with an exclusive end.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct SourceSpan {
    file: String,
    start_byte: usize,
    end_byte: usize,
}

impl SourceSpan {
    /// Returns `None` for an empty file name or an empty/reversed span.
    pub fn new(file: impl Into<String>, start_byte: usize, end_byte: usize) -> Option<Self> {
        let file = file.into();
        if file.is_empty() || start_byte >= end_byte {
            return None;
        }
        Some(Self {
            file,
            start_byte,
            end_byte,
        })
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn start_byte(&self) -> usize {
        self.start_byte
    }

    pub fn end_byte(&self) -> usize {
        self.end_byte
    }

    /// Resolves this span against the matching source file, if in bounds and UTF-8 aligned.
    pub fn slice<'a>(&self, source: &'a str) -> Option<&'a str> {
        source.get(self.start_byte..self.end_byte)
    }
}

/// Location in an identified EVM bytecode artifact; bytecode ingestion comes later.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct BytecodePc {
    pub artifact: String,
    pub pc: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum EvidenceLocation {
    Source(SourceSpan),
    Bytecode(BytecodePc),
}

/// Confidence in the represented semantic fact, not in a vulnerability verdict.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum Confidence {
    Direct,
    Inferred,
    Tentative,
}

/// A value tied to its origin and the strength of its interpretation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Evidence<T> {
    pub value: T,
    pub location: EvidenceLocation,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct StorageId(pub String);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct CallTarget(pub String);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct ArithmeticExpression(pub String);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum UncertaintyKind {
    AmbiguousSemantics,
    MissingInput,
}

/// Explicitly records what the reconstruction could not establish.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Uncertainty {
    pub kind: UncertaintyKind,
    pub detail: String,
    pub location: Option<EvidenceLocation>,
}

/// One compiler-resolved modifier parameter and its invocation expression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModifierArgumentBinding {
    pub parameter: Evidence<String>,
    pub argument: Evidence<String>,
}

/// A dependency observed at the end of a source-level internal call path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ReachabilityTerminal {
    StorageRead(Evidence<StorageId>),
    StorageWrite(Evidence<StorageId>),
    OracleCall(Evidence<CallTarget>),
    Conversion(Evidence<EconomicOperation>),
}

/// Syntactic call reachability; this alone does not prove value propagation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReachabilityPath {
    pub calls: Vec<Evidence<FunctionId>>,
    pub terminal: ReachabilityTerminal,
}

/// Compiler-resolved internal call, including the declaring contract.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct ResolvedFunction {
    pub contract: ContractId,
    pub function: FunctionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ValueOrigin {
    Parameter(String),
    Storage(StorageId),
    OracleCall(CallTarget),
    ExternalCall(CallTarget),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ValueSink {
    Guard,
    Return,
    StorageWrite(StorageId),
    ConversionArgument(FunctionId),
}

/// Conservative value dependence with source and call-site provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValueFlow {
    pub origin: Evidence<ValueOrigin>,
    pub calls: Vec<Evidence<FunctionId>>,
    pub sink: Evidence<ValueSink>,
    pub confidence: Confidence,
}

/// Direct syntax observed in a locally resolved modifier applied to a function.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AppliedModifier {
    pub invocation: Evidence<String>,
    pub definition: Evidence<String>,
    pub bindings: Vec<ModifierArgumentBinding>,
    pub reads: Vec<Evidence<StorageId>>,
    pub writes: Vec<Evidence<StorageId>>,
    pub external_calls: Vec<Evidence<CallTarget>>,
    pub oracle_dependencies: Vec<Evidence<CallTarget>>,
    pub arithmetic: Vec<Evidence<ArithmeticExpression>>,
    pub control_conditions: Vec<Evidence<String>>,
    pub delegatecall_targets: Vec<Evidence<CallTarget>>,
    pub related_functions: Vec<Evidence<FunctionId>>,
    pub resolved_calls: Vec<Evidence<ResolvedFunction>>,
    pub unresolved: Vec<Uncertainty>,
}

/// Minimal per-function economic IR shared by future reconstruction and detectors.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EconomicFunction {
    pub contract: ContractId,
    pub function: FunctionId,
    pub operation: Evidence<EconomicOperation>,
    pub reads: Vec<Evidence<StorageId>>,
    pub writes: Vec<Evidence<StorageId>>,
    pub external_calls: Vec<Evidence<CallTarget>>,
    pub oracle_dependencies: Vec<Evidence<CallTarget>>,
    pub arithmetic: Vec<Evidence<ArithmeticExpression>>,
    pub control_conditions: Vec<Evidence<String>>,
    pub control_edges: Vec<ControlEdge>,
    pub delegatecall_targets: Vec<Evidence<CallTarget>>,
    pub modifiers: Vec<Evidence<String>>,
    pub applied_modifiers: Vec<AppliedModifier>,
    pub related_functions: Vec<Evidence<FunctionId>>,
    pub resolved_calls: Vec<Evidence<ResolvedFunction>>,
    pub reachability_paths: Vec<ReachabilityPath>,
    pub value_flows: Vec<ValueFlow>,
    pub unresolved: Vec<Uncertainty>,
}

impl EconomicFunction {
    pub fn new(
        contract: ContractId,
        function: FunctionId,
        operation: Evidence<EconomicOperation>,
    ) -> Self {
        Self {
            contract,
            function,
            operation,
            reads: Vec::new(),
            writes: Vec::new(),
            external_calls: Vec::new(),
            oracle_dependencies: Vec::new(),
            arithmetic: Vec::new(),
            control_conditions: Vec::new(),
            control_edges: Vec::new(),
            delegatecall_targets: Vec::new(),
            modifiers: Vec::new(),
            applied_modifiers: Vec::new(),
            related_functions: Vec::new(),
            resolved_calls: Vec::new(),
            reachability_paths: Vec::new(),
            value_flows: Vec::new(),
            unresolved: Vec::new(),
        }
    }
}
