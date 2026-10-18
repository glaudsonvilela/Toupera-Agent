//! EVMole is a second, heuristic reading of the same local runtime bytes.
//! Its claims stay separate from native PC-backed observations.

use std::collections::BTreeSet;

use evmole::control_flow_graph::BlockType;
use evmole::{ContractInfoArgs, contract_info};
use serde::Serialize;

use crate::bytecode::{
    BytecodeEffectKind, BytecodeEntrySummary, BytecodeInstruction, BytecodeSelector, encode_hex,
};
use crate::{
    BytecodePc, Confidence, ControlEdge, ControlEdgeKind, EvidenceLocation, Uncertainty,
    UncertaintyKind,
};

const EVMOLE_VERSION: &str = "0.9.4";
const MAX_COMPANION_CODE_BYTES: usize = 24_576;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvmoleSelectorEvidence {
    pub selector: String,
    pub destination_pc: BytecodePc,
    pub dispatch: String,
    pub argument_types: Option<Vec<String>>,
    pub state_mutability: Option<String>,
    pub matches_native: bool,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvmoleStorageEvidence {
    pub slot: String,
    pub offset: u8,
    pub inferred_type: String,
    pub transient: bool,
    pub reads: Vec<String>,
    pub writes: Vec<String>,
    /// PCs independently observed by the native decoder for this slot and selector.
    pub corroborating_access_pcs: Vec<BytecodePc>,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvmoleControlEdgeEvidence {
    pub from_pc: BytecodePc,
    pub to_pc: BytecodePc,
    pub kind: ControlEdgeKind,
    pub matches_native: bool,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EvmoleReport {
    pub tool: String,
    pub version: String,
    pub selectors: Vec<EvmoleSelectorEvidence>,
    pub storage: Vec<EvmoleStorageEvidence>,
    pub control_edges: Vec<EvmoleControlEdgeEvidence>,
    pub unresolved: Vec<Uncertainty>,
}

fn pc(artifact: &str, offset: usize) -> BytecodePc {
    BytecodePc {
        artifact: artifact.to_owned(),
        pc: offset,
    }
}

fn uncertain(artifact: &str, offset: Option<usize>, detail: String) -> Uncertainty {
    Uncertainty {
        kind: UncertaintyKind::AmbiguousSemantics,
        detail,
        location: offset.map(|offset| EvidenceLocation::Bytecode(pc(artifact, offset))),
    }
}

fn selector_string(selector: &[u8; 4]) -> String {
    format!("0x{}", encode_hex(selector))
}

fn canonical_slot(hex: &str) -> Option<String> {
    let hex = hex.trim_start_matches('0').to_ascii_lowercase();
    if hex.len() > 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{hex:0>64}"))
}

fn corroborating_accesses(
    slot: &str,
    transient: bool,
    reads: &[String],
    writes: &[String],
    summaries: &[BytecodeEntrySummary],
) -> Vec<BytecodePc> {
    let mut pcs = BTreeSet::new();
    for summary in summaries {
        let expects_read = reads.contains(&summary.selector);
        let expects_write = writes.contains(&summary.selector);
        if !expects_read && !expects_write {
            continue;
        }
        for effect in &summary.effects {
            let same_access = match effect.kind {
                BytecodeEffectKind::StorageRead => expects_read && !transient,
                BytecodeEffectKind::StorageWrite => expects_write && !transient,
                BytecodeEffectKind::TransientStorageRead => expects_read && transient,
                BytecodeEffectKind::TransientStorageWrite => expects_write && transient,
                _ => false,
            };
            if same_access
                && effect
                    .constant_slot
                    .as_ref()
                    .and_then(|item| canonical_slot(&item.value))
                    .as_deref()
                    == Some(slot)
            {
                pcs.insert(effect.location.clone());
            }
        }
    }
    pcs.into_iter().collect()
}

fn add_edge(
    report: &mut EvmoleReport,
    artifact: &str,
    from: usize,
    to: usize,
    kind: ControlEdgeKind,
    valid_pcs: &BTreeSet<usize>,
    native_edges: &[ControlEdge],
) {
    if !valid_pcs.contains(&from) || !valid_pcs.contains(&to) {
        report.unresolved.push(uncertain(
            artifact,
            Some(from),
            format!("EVMole proposed an edge to undecoded PC {to}"),
        ));
        return;
    }
    let matches_native = native_edges.iter().any(|edge| {
        edge.kind == kind
            && edge.to.0 == format!("pc:{to}")
            && matches!(&edge.location, EvidenceLocation::Bytecode(location) if location.pc == from)
    });
    let edge = EvmoleControlEdgeEvidence {
        from_pc: pc(artifact, from),
        to_pc: pc(artifact, to),
        kind,
        matches_native,
        confidence: Confidence::Tentative,
    };
    if !report.control_edges.contains(&edge) {
        report.control_edges.push(edge);
    }
}

/// Run an independent local analyzer on the normalized runtime. EVMole may
/// infer entry points, ABI details and storage layout, but does not provide a
/// dispatch PC or per-access PC for every claim. Such evidence is not promoted
/// into native facts merely because it appears in this report.
pub(crate) fn analyze_with_evmole(
    code: &[u8],
    artifact: &str,
    instructions: &[BytecodeInstruction],
    native_selectors: &[BytecodeSelector],
    native_edges: &[ControlEdge],
    native_summaries: &[BytecodeEntrySummary],
) -> EvmoleReport {
    let mut report = EvmoleReport {
        tool: "evmole".into(),
        version: EVMOLE_VERSION.into(),
        selectors: Vec::new(),
        storage: Vec::new(),
        control_edges: Vec::new(),
        unresolved: Vec::new(),
    };
    if code.len() > MAX_COMPANION_CODE_BYTES {
        report.unresolved.push(Uncertainty {
            kind: UncertaintyKind::MissingInput,
            detail: format!(
                "EVMole companion skipped runtime larger than {MAX_COMPANION_CODE_BYTES} bytes"
            ),
            location: None,
        });
        return report;
    }
    let args = ContractInfoArgs::new(code)
        .with_selectors()
        .with_arguments()
        .with_state_mutability()
        .with_storage()
        .with_control_flow_graph();
    let info = match std::panic::catch_unwind(|| contract_info(args)) {
        Ok(info) => info,
        Err(_) => {
            report.unresolved.push(uncertain(
                artifact,
                None,
                "EVMole panicked while analyzing this runtime; native results remain available"
                    .into(),
            ));
            return report;
        }
    };
    let valid_pcs: BTreeSet<_> = instructions.iter().map(|item| item.pc).collect();
    let jumpdests: BTreeSet<_> = instructions
        .iter()
        .filter(|item| item.opcode == 0x5b)
        .map(|item| item.pc)
        .collect();

    for function in info.functions.unwrap_or_default() {
        let selector = selector_string(&function.selector);
        if !valid_pcs.contains(&function.bytecode_offset) {
            report.unresolved.push(uncertain(
                artifact,
                Some(function.bytecode_offset),
                format!("EVMole selector {selector} points outside decoded instructions"),
            ));
            continue;
        }
        if !jumpdests.contains(&function.bytecode_offset) {
            report.unresolved.push(uncertain(
                artifact,
                Some(function.bytecode_offset),
                format!("EVMole selector {selector} does not point to a JUMPDEST"),
            ));
            continue;
        }
        let matches_native = native_selectors.iter().any(|native| {
            native.selector == selector && native.destination_pc.pc == function.bytecode_offset
        });
        if let Some(native) = native_selectors
            .iter()
            .find(|native| native.selector == selector && !matches_native)
        {
            report.unresolved.push(uncertain(
                artifact,
                Some(function.bytecode_offset),
                format!(
                    "selector {selector} destination differs: native PC {}, EVMole PC {}",
                    native.destination_pc.pc, function.bytecode_offset
                ),
            ));
        }
        report.selectors.push(EvmoleSelectorEvidence {
            selector,
            destination_pc: pc(artifact, function.bytecode_offset),
            dispatch: function.dispatch.as_str().into(),
            argument_types: function.arguments.map(|args| {
                args.into_iter()
                    .map(|arg| arg.sol_type_name().to_string())
                    .collect()
            }),
            state_mutability: function
                .state_mutability
                .map(|state| state.as_json_str().to_owned()),
            matches_native,
            confidence: Confidence::Tentative,
        });
    }
    for native in native_selectors {
        if !report
            .selectors
            .iter()
            .any(|candidate| candidate.selector == native.selector)
        {
            report.unresolved.push(uncertain(
                artifact,
                Some(native.destination_pc.pc),
                format!(
                    "native selector {} was not recovered by EVMole",
                    native.selector
                ),
            ));
        }
    }
    report
        .selectors
        .sort_by_key(|candidate| (candidate.selector.clone(), candidate.destination_pc.pc));

    for (transient, records) in [
        (false, info.storage.unwrap_or_default()),
        (true, info.transient_storage.unwrap_or_default()),
    ] {
        for record in records {
            let slot = format!("0x{}", encode_hex(&record.slot));
            let reads: Vec<_> = record.reads.iter().map(selector_string).collect();
            let writes: Vec<_> = record.writes.iter().map(selector_string).collect();
            let corroborating_access_pcs =
                corroborating_accesses(&slot, transient, &reads, &writes, native_summaries);
            report.storage.push(EvmoleStorageEvidence {
                slot,
                offset: record.offset,
                inferred_type: record.r#type,
                transient,
                reads,
                writes,
                corroborating_access_pcs,
                confidence: Confidence::Tentative,
            });
        }
    }
    report.storage.sort_by_key(|item| {
        (
            item.transient,
            item.slot.clone(),
            item.offset,
            item.inferred_type.clone(),
        )
    });

    if let Some(graph) = info.control_flow_graph {
        for block in graph.blocks.values() {
            match &block.btype {
                BlockType::Terminate { .. } => {}
                BlockType::Jump { to } => add_edge(
                    &mut report,
                    artifact,
                    block.end,
                    *to,
                    ControlEdgeKind::Next,
                    &valid_pcs,
                    native_edges,
                ),
                BlockType::Jumpi { true_to, false_to } => {
                    for (to, kind) in [
                        (*true_to, ControlEdgeKind::True),
                        (*false_to, ControlEdgeKind::False),
                    ] {
                        add_edge(
                            &mut report,
                            artifact,
                            block.end,
                            to,
                            kind,
                            &valid_pcs,
                            native_edges,
                        );
                    }
                }
                BlockType::DynamicJump { to } | BlockType::DynamicJumpi { true_to: to, .. } => {
                    let conditional = matches!(block.btype, BlockType::DynamicJumpi { .. });
                    for jump in to {
                        if let Some(destination) = jump.to {
                            add_edge(
                                &mut report,
                                artifact,
                                block.end,
                                destination,
                                if conditional {
                                    ControlEdgeKind::True
                                } else {
                                    ControlEdgeKind::Next
                                },
                                &valid_pcs,
                                native_edges,
                            );
                        } else {
                            report.unresolved.push(uncertain(
                                artifact,
                                Some(block.end),
                                "EVMole could not resolve a dynamic jump".into(),
                            ));
                        }
                    }
                    if let BlockType::DynamicJumpi { false_to, .. } = &block.btype {
                        add_edge(
                            &mut report,
                            artifact,
                            block.end,
                            *false_to,
                            ControlEdgeKind::False,
                            &valid_pcs,
                            native_edges,
                        );
                    }
                }
            }
        }
    }
    report
        .control_edges
        .sort_by_key(|edge| (edge.from_pc.pc, edge.to_pc.pc, format!("{:?}", edge.kind)));
    report
}
