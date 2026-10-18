//! Bounded abstract stack and word-memory propagation for legacy EVM runtime.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::bytecode::{
    BytecodeBlock, BytecodeInstruction, BytecodeValueFlow, BytecodeValueOrigin,
    BytecodeValueOriginKind, BytecodeValueSinkKind,
};
use crate::{
    BytecodePc, Confidence, ControlEdge, ControlEdgeKind, EvidenceLocation, Uncertainty,
    UncertaintyKind,
};

const MAX_EVALUATIONS: usize = 8192;
const MAX_ORIGINS: usize = 32;
const MAX_ARITHMETIC_PCS: usize = 32;
const MAX_FLOWS: usize = 10_000;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AbstractValue {
    constant: Option<usize>,
    origins: BTreeSet<BytecodeValueOrigin>,
    arithmetic_pcs: BTreeSet<usize>,
    uncertain: bool,
}

impl AbstractValue {
    fn unknown() -> Self {
        Self {
            uncertain: true,
            ..Self::default()
        }
    }

    fn literal(value: Option<usize>) -> Self {
        Self {
            constant: value,
            uncertain: value.is_none(),
            ..Self::default()
        }
    }

    fn origin(kind: BytecodeValueOriginKind, artifact: &str, pc: usize) -> Self {
        Self {
            origins: BTreeSet::from([BytecodeValueOrigin {
                kind,
                location: BytecodePc {
                    artifact: artifact.to_owned(),
                    pc,
                },
            }]),
            ..Self::default()
        }
    }

    fn combine(values: &[Self], arithmetic_pc: Option<usize>) -> Self {
        let mut result = Self::default();
        if values.len() == 1 {
            result.constant = values[0].constant;
        }
        for value in values {
            result.origins.extend(value.origins.iter().cloned());
            result.arithmetic_pcs.extend(&value.arithmetic_pcs);
            result.uncertain |= value.uncertain;
        }
        if let Some(pc) = arithmetic_pc {
            result.arithmetic_pcs.insert(pc);
        }
        result.cap();
        result
    }

    fn merge(&mut self, other: &Self) {
        if self.constant != other.constant {
            self.constant = None;
            self.uncertain = true;
        }
        if self.origins != other.origins {
            self.uncertain = true;
        }
        self.origins.extend(other.origins.iter().cloned());
        self.arithmetic_pcs.extend(&other.arithmetic_pcs);
        self.uncertain |= other.uncertain;
        self.cap();
    }

    fn cap(&mut self) {
        if self.origins.len() > MAX_ORIGINS || self.arithmetic_pcs.len() > MAX_ARITHMETIC_PCS {
            self.origins.clear();
            self.arithmetic_pcs.clear();
            self.constant = None;
            self.uncertain = true;
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AbstractState {
    stack: Vec<AbstractValue>,
    memory_words: BTreeMap<usize, AbstractValue>,
    last_external_call: Option<usize>,
    poisoned: bool,
}

impl AbstractState {
    fn pop(&mut self) -> Result<AbstractValue, ()> {
        self.stack.pop().ok_or(())
    }

    fn pops(&mut self, count: usize) -> Result<Vec<AbstractValue>, ()> {
        if self.stack.len() < count {
            return Err(());
        }
        (0..count).map(|_| self.pop()).collect()
    }

    fn push(&mut self, value: AbstractValue) -> Result<(), ()> {
        if self.stack.len() >= 1024 {
            return Err(());
        }
        self.stack.push(value);
        Ok(())
    }

    fn merge(&mut self, other: &Self) -> bool {
        let before = self.clone();
        if self.poisoned || other.poisoned || self.stack.len() != other.stack.len() {
            self.poisoned = true;
            self.stack.clear();
            self.memory_words.clear();
            self.last_external_call = None;
            return *self != before;
        }
        for (left, right) in self.stack.iter_mut().zip(&other.stack) {
            left.merge(right);
        }
        self.memory_words.retain(|offset, left| {
            if let Some(right) = other.memory_words.get(offset) {
                left.merge(right);
                true
            } else {
                false
            }
        });
        if self.last_external_call != other.last_external_call {
            self.last_external_call = None;
        }
        *self != before
    }

    fn invalidate_memory(&mut self, offset: Option<usize>, size: Option<usize>) {
        if let (Some(offset), Some(size)) = (offset, size) {
            let end = offset.saturating_add(size);
            self.memory_words
                .retain(|word, _| word.saturating_add(32) <= offset || *word >= end);
        } else {
            self.memory_words.clear();
        }
    }

    fn read_memory(&self, offset: Option<usize>, size: Option<usize>) -> AbstractValue {
        let (Some(offset), Some(size)) = (offset, size) else {
            return AbstractValue::unknown();
        };
        if size == 0 {
            return AbstractValue::default();
        }
        let end = offset.saturating_add(size);
        let words: Vec<_> = self
            .memory_words
            .iter()
            .filter(|(word, _)| word.saturating_add(32) > offset && **word < end)
            .map(|(_, value)| value.clone())
            .collect();
        if words.is_empty() {
            AbstractValue::unknown()
        } else {
            let mut value = AbstractValue::combine(&words, None);
            // Only a fully known single word can avoid the partial-memory caveat.
            value.uncertain |= size != 32 || !self.memory_words.contains_key(&offset);
            value
        }
    }
}

fn pc(artifact: &str, at: usize) -> BytecodePc {
    BytecodePc {
        artifact: artifact.to_owned(),
        pc: at,
    }
}

fn uncertainty(artifact: &str, at: usize, detail: &str) -> Uncertainty {
    Uncertainty {
        kind: UncertaintyKind::AmbiguousSemantics,
        detail: detail.to_owned(),
        location: Some(EvidenceLocation::Bytecode(pc(artifact, at))),
    }
}

fn record_flow(
    flows: &mut Vec<BytecodeValueFlow>,
    value: &AbstractValue,
    sink_kind: BytecodeValueSinkKind,
    artifact: &str,
    sink_pc: usize,
) {
    if flows.len() >= MAX_FLOWS {
        return;
    }
    for origin in &value.origins {
        if flows.len() >= MAX_FLOWS {
            break;
        }
        let flow = BytecodeValueFlow {
            origin: origin.clone(),
            sink_kind,
            sink_pc: pc(artifact, sink_pc),
            arithmetic_pcs: value
                .arithmetic_pcs
                .iter()
                .map(|at| pc(artifact, *at))
                .collect(),
            confidence: if value.uncertain {
                Confidence::Tentative
            } else {
                Confidence::Inferred
            },
        };
        if !flows.contains(&flow) {
            flows.push(flow);
        }
    }
}

fn fold_binary(opcode: u8, left: usize, right: usize) -> Option<usize> {
    match opcode {
        0x01 => left.checked_add(right),
        0x02 => left.checked_mul(right),
        0x03 => left.checked_sub(right),
        0x04 => (right != 0).then(|| left / right),
        0x06 => (right != 0).then(|| left % right),
        0x10 => Some(usize::from(left < right)),
        0x11 => Some(usize::from(left > right)),
        0x14 => Some(usize::from(left == right)),
        0x16 => Some(left & right),
        0x17 => Some(left | right),
        0x18 => Some(left ^ right),
        0x1b => right.checked_shl(u32::try_from(left).ok()?),
        0x1c => Some(right.checked_shr(u32::try_from(left).ok()?).unwrap_or(0)),
        _ => None,
    }
}

fn execute(
    instruction: &BytecodeInstruction,
    state: &mut AbstractState,
    artifact: &str,
    flows: &mut Vec<BytecodeValueFlow>,
    unresolved: &mut Vec<Uncertainty>,
) -> Result<Option<usize>, &'static str> {
    let op = instruction.opcode;
    let at = instruction.pc;
    if op == 0x5f || (0x60..=0x7f).contains(&op) {
        let literal = if op == 0x5f {
            Some(0)
        } else {
            instruction
                .immediate
                .as_deref()
                .and_then(|value| usize::from_str_radix(value, 16).ok())
        };
        state
            .push(AbstractValue::literal(literal))
            .map_err(|_| "stack overflow")?;
        return Ok(None);
    }
    if (0x80..=0x8f).contains(&op) {
        let depth = usize::from(op - 0x7f);
        let Some(value) = state
            .stack
            .get(state.stack.len().wrapping_sub(depth))
            .cloned()
        else {
            return Err("stack underflow at DUP");
        };
        state.push(value).map_err(|_| "stack overflow")?;
        return Ok(None);
    }
    if (0x90..=0x9f).contains(&op) {
        let depth = usize::from(op - 0x8f);
        if state.stack.len() <= depth {
            return Err("stack underflow at SWAP");
        }
        let top = state.stack.len() - 1;
        state.stack.swap(top, top - depth);
        return Ok(None);
    }
    let underflow = |_| "stack underflow or unsupported opcode semantics";
    match op {
        0x00 | 0x5b | 0xfe => {}
        0x01..=0x07 | 0x0a..=0x0b | 0x10..=0x14 | 0x16..=0x18 | 0x1a..=0x1d => {
            let operands = state.pops(2).map_err(underflow)?;
            let mut value = AbstractValue::combine(
                &operands,
                (op <= 0x0b || (0x1b..=0x1d).contains(&op)).then_some(at),
            );
            value.constant = operands[0]
                .constant
                .zip(operands[1].constant)
                .and_then(|(left, right)| fold_binary(op, left, right));
            state.push(value).map_err(|_| "stack overflow")?;
        }
        0x08..=0x09 => {
            let operands = state.pops(3).map_err(underflow)?;
            state
                .push(AbstractValue::combine(&operands, Some(at)))
                .map_err(|_| "stack overflow")?;
        }
        0x15 | 0x19 => {
            let operand = state.pop().map_err(underflow)?;
            let mut value = AbstractValue::combine(&[operand.clone()], None);
            value.constant = match op {
                0x15 => operand.constant.map(|number| usize::from(number == 0)),
                _ => None,
            };
            state.push(value).map_err(|_| "stack overflow")?;
        }
        0x20 => {
            let args = state.pops(2).map_err(underflow)?;
            let mut value = state.read_memory(args[0].constant, args[1].constant);
            if value.uncertain {
                unresolved.push(uncertainty(
                    artifact,
                    at,
                    "hash input memory is only partially reconstructed",
                ));
            }
            value.constant = None;
            value.uncertain = true;
            state.push(value).map_err(|_| "stack overflow")?;
        }
        0x30 | 0x32 | 0x33 | 0x38 | 0x3a | 0x41..=0x48 | 0x4a | 0x59 | 0x5a => {
            state
                .push(AbstractValue::unknown())
                .map_err(|_| "stack overflow")?;
        }
        0x31 | 0x3b | 0x3f | 0x40 | 0x49 => {
            state.pop().map_err(underflow)?;
            let value = if op == 0x31 {
                AbstractValue::origin(BytecodeValueOriginKind::Balance, artifact, at)
            } else {
                AbstractValue::unknown()
            };
            state.push(value).map_err(|_| "stack overflow")?;
        }
        0x34 => state
            .push(AbstractValue::origin(
                BytecodeValueOriginKind::CallValue,
                artifact,
                at,
            ))
            .map_err(|_| "stack overflow")?,
        0x35 => {
            state.pop().map_err(underflow)?;
            state
                .push(AbstractValue::origin(
                    BytecodeValueOriginKind::Calldata,
                    artifact,
                    at,
                ))
                .map_err(|_| "stack overflow")?;
        }
        0x36 => state
            .push(AbstractValue::origin(
                BytecodeValueOriginKind::Calldata,
                artifact,
                at,
            ))
            .map_err(|_| "stack overflow")?,
        0x37 | 0x39 | 0x3e => {
            let args = state.pops(3).map_err(underflow)?;
            state.invalidate_memory(args[0].constant, args[2].constant);
            if op == 0x37 {
                if let (Some(offset), Some(size)) = (args[0].constant, args[2].constant) {
                    if size >= 32 {
                        state.memory_words.insert(
                            offset,
                            AbstractValue::origin(BytecodeValueOriginKind::Calldata, artifact, at),
                        );
                    }
                }
            }
            if op == 0x3e {
                if let (Some(offset), Some(size), Some(call_pc)) =
                    (args[0].constant, args[2].constant, state.last_external_call)
                {
                    if size >= 32 {
                        let mut returned = AbstractValue::origin(
                            BytecodeValueOriginKind::ExternalReturn,
                            artifact,
                            call_pc,
                        );
                        returned.uncertain = true;
                        state.memory_words.insert(offset, returned);
                    }
                }
            }
        }
        0x3c => {
            let args = state.pops(4).map_err(underflow)?;
            state.invalidate_memory(args[1].constant, args[3].constant);
        }
        0x3d => {
            let value = state
                .last_external_call
                .map_or_else(AbstractValue::unknown, |call_pc| {
                    AbstractValue::origin(
                        BytecodeValueOriginKind::ExternalReturn,
                        artifact,
                        call_pc,
                    )
                });
            state.push(value).map_err(|_| "stack overflow")?;
        }
        0x50 => {
            state.pop().map_err(underflow)?;
        }
        0x51 => {
            let offset = state.pop().map_err(underflow)?;
            let value = state.read_memory(offset.constant, Some(32));
            if value.uncertain && value.origins.is_empty() {
                unresolved.push(uncertainty(
                    artifact,
                    at,
                    "memory word is not reconstructed",
                ));
            }
            state.push(value).map_err(|_| "stack overflow")?;
        }
        0x52 | 0x53 => {
            let args = state.pops(2).map_err(underflow)?;
            state.invalidate_memory(args[0].constant, Some(if op == 0x52 { 32 } else { 1 }));
            if op == 0x52 {
                if let Some(offset) = args[0].constant {
                    state.memory_words.insert(offset, args[1].clone());
                }
            }
        }
        0x54 | 0x5c => {
            let key = state.pop().map_err(underflow)?;
            if key.constant.is_none() {
                unresolved.push(uncertainty(
                    artifact,
                    at,
                    "storage key is not a resolved constant",
                ));
            }
            state
                .push(AbstractValue::origin(
                    if op == 0x54 {
                        BytecodeValueOriginKind::StorageRead
                    } else {
                        BytecodeValueOriginKind::TransientStorageRead
                    },
                    artifact,
                    at,
                ))
                .map_err(|_| "stack overflow")?;
        }
        0x55 | 0x5d => {
            let args = state.pops(2).map_err(underflow)?;
            record_flow(
                flows,
                &args[1],
                if op == 0x55 {
                    BytecodeValueSinkKind::StorageWrite
                } else {
                    BytecodeValueSinkKind::TransientStorageWrite
                },
                artifact,
                at,
            );
        }
        0x56 | 0x57 => {
            let dest = state.pop().map_err(underflow)?;
            if op == 0x57 {
                let condition = state.pop().map_err(underflow)?;
                record_flow(
                    flows,
                    &condition,
                    BytecodeValueSinkKind::BranchCondition,
                    artifact,
                    at,
                );
            }
            return Ok(dest.constant);
        }
        0x58 => state
            .push(AbstractValue::literal(Some(at)))
            .map_err(|_| "stack overflow")?,
        0x5e => {
            let args = state.pops(3).map_err(underflow)?;
            let copied = state.read_memory(args[1].constant, args[2].constant);
            state.invalidate_memory(args[0].constant, args[2].constant);
            if let (Some(dest), Some(size)) = (args[0].constant, args[2].constant) {
                if size == 32 {
                    state.memory_words.insert(dest, copied);
                }
            }
        }
        0xa0..=0xa4 => {
            state.pops(usize::from(op - 0xa0) + 2).map_err(underflow)?;
        }
        0xf0 | 0xf5 => {
            let args = state
                .pops(if op == 0xf0 { 3 } else { 4 })
                .map_err(underflow)?;
            record_flow(
                flows,
                &args[0],
                BytecodeValueSinkKind::ExternalCallValue,
                artifact,
                at,
            );
            let input = state.read_memory(args[1].constant, args[2].constant);
            if input.uncertain {
                unresolved.push(uncertainty(
                    artifact,
                    at,
                    "creation input memory is only partially reconstructed",
                ));
            }
            record_flow(
                flows,
                &input,
                BytecodeValueSinkKind::ExternalCallInput,
                artifact,
                at,
            );
            state
                .push(AbstractValue::unknown())
                .map_err(|_| "stack overflow")?;
        }
        0xf1 | 0xf2 | 0xf4 | 0xfa => {
            let args = state
                .pops(if matches!(op, 0xf1 | 0xf2) { 7 } else { 6 })
                .map_err(underflow)?;
            record_flow(
                flows,
                &args[1],
                BytecodeValueSinkKind::ExternalCallTarget,
                artifact,
                at,
            );
            let first_memory_arg = if matches!(op, 0xf1 | 0xf2) { 3 } else { 2 };
            if matches!(op, 0xf1 | 0xf2) {
                record_flow(
                    flows,
                    &args[2],
                    BytecodeValueSinkKind::ExternalCallValue,
                    artifact,
                    at,
                );
            }
            let input = state.read_memory(
                args[first_memory_arg].constant,
                args[first_memory_arg + 1].constant,
            );
            if input.uncertain {
                unresolved.push(uncertainty(
                    artifact,
                    at,
                    "external call input memory is only partially reconstructed",
                ));
            }
            record_flow(
                flows,
                &input,
                BytecodeValueSinkKind::ExternalCallInput,
                artifact,
                at,
            );
            let output_offset = args[first_memory_arg + 2].constant;
            let output_size = args[first_memory_arg + 3].constant;
            state.invalidate_memory(output_offset, output_size);
            if let (Some(offset), Some(size)) = (output_offset, output_size) {
                if size >= 32 {
                    let mut returned = AbstractValue::origin(
                        BytecodeValueOriginKind::ExternalReturn,
                        artifact,
                        at,
                    );
                    returned.uncertain = true;
                    state.memory_words.insert(offset, returned);
                }
            } else {
                unresolved.push(uncertainty(
                    artifact,
                    at,
                    "external call output memory offset or size is unknown",
                ));
            }
            state.last_external_call = Some(at);
            state
                .push(AbstractValue::origin(
                    BytecodeValueOriginKind::ExternalCallSuccess,
                    artifact,
                    at,
                ))
                .map_err(|_| "stack overflow")?;
        }
        0xf3 | 0xfd => {
            let args = state.pops(2).map_err(underflow)?;
            if op == 0xf3 {
                let returned = state.read_memory(args[0].constant, args[1].constant);
                if returned.uncertain {
                    unresolved.push(uncertainty(
                        artifact,
                        at,
                        "returned memory is only partially reconstructed",
                    ));
                }
                record_flow(
                    flows,
                    &returned,
                    BytecodeValueSinkKind::Return,
                    artifact,
                    at,
                );
            }
        }
        0xff => {
            let beneficiary = state.pop().map_err(underflow)?;
            record_flow(
                flows,
                &beneficiary,
                BytecodeValueSinkKind::ExternalCallTarget,
                artifact,
                at,
            );
        }
        _ => return Err("opcode semantics not modeled by abstract interpreter"),
    }
    Ok(None)
}

/// Adds edges only when a propagated constant points at a decoded JUMPDEST.
/// Dynamic/invalid jumps remain unresolved; values are dependency hints, not
/// feasibility proofs or semantic labels such as oracle and share price.
pub(super) fn analyze_value_flow(
    instructions: &[BytecodeInstruction],
    blocks: &[BytecodeBlock],
    edges: &mut Vec<ControlEdge>,
    artifact: &str,
    unresolved: &mut Vec<Uncertainty>,
) -> Vec<BytecodeValueFlow> {
    let by_pc: BTreeMap<_, _> = instructions.iter().map(|item| (item.pc, item)).collect();
    let by_start: BTreeMap<_, _> = blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.start_pc, i))
        .collect();
    let jumpdests: BTreeSet<_> = instructions
        .iter()
        .filter(|i| i.opcode == 0x5b)
        .map(|i| i.pc)
        .collect();
    let mut states = BTreeMap::from([(0usize, AbstractState::default())]);
    let mut queue = VecDeque::from([0usize]);
    let mut evaluations = 0;
    let mut flows = Vec::new();
    while let Some(start) = queue.pop_front() {
        evaluations += 1;
        if evaluations > MAX_EVALUATIONS {
            unresolved.push(uncertainty(
                artifact,
                start,
                "abstract propagation budget exhausted",
            ));
            break;
        }
        let Some(&block_index) = by_start.get(&start) else {
            continue;
        };
        let block = &blocks[block_index];
        let mut state = states[&start].clone();
        if state.poisoned {
            unresolved.push(uncertainty(
                artifact,
                start,
                "incompatible stack states at block entry",
            ));
            continue;
        }
        for at in &block.instruction_pcs {
            let instruction = by_pc[at];
            match execute(instruction, &mut state, artifact, &mut flows, unresolved) {
                Ok(target) if matches!(instruction.opcode, 0x56 | 0x57) => {
                    if let Some(target) = target.filter(|target| jumpdests.contains(target)) {
                        if let Some(&target_index) = by_start.get(&target) {
                            let candidate = ControlEdge {
                                from: block.id.clone(),
                                to: blocks[target_index].id.clone(),
                                kind: if instruction.opcode == 0x57 {
                                    ControlEdgeKind::True
                                } else {
                                    ControlEdgeKind::Next
                                },
                                location: EvidenceLocation::Bytecode(pc(artifact, instruction.pc)),
                            };
                            if !edges.contains(&candidate) {
                                edges.push(candidate);
                            }
                        }
                    }
                }
                Ok(_) => {}
                Err(detail) => {
                    unresolved.push(uncertainty(artifact, instruction.pc, detail));
                    state.poisoned = true;
                    break;
                }
            }
        }
        if state.poisoned {
            continue;
        }
        for edge in edges.iter().filter(|edge| edge.from == block.id) {
            let Some(next) = edge
                .to
                .0
                .strip_prefix("pc:")
                .and_then(|text| text.parse::<usize>().ok())
            else {
                continue;
            };
            let changed = if let Some(existing) = states.get_mut(&next) {
                existing.merge(&state)
            } else {
                states.insert(next, state.clone());
                true
            };
            if changed && !queue.contains(&next) {
                queue.push_back(next);
            }
        }
    }
    for instruction in instructions
        .iter()
        .filter(|item| matches!(item.opcode, 0x56 | 0x57))
    {
        let has_jump_edge = edges.iter().any(|edge| {
            matches!(&edge.location, EvidenceLocation::Bytecode(location) if location.pc == instruction.pc)
                && (instruction.opcode == 0x56 || edge.kind == ControlEdgeKind::True)
        });
        if !has_jump_edge {
            unresolved.push(uncertainty(
                artifact,
                instruction.pc,
                "jump destination unresolved or invalid",
            ));
        }
    }
    if flows.len() >= MAX_FLOWS {
        unresolved.push(uncertainty(
            artifact,
            0,
            "value-flow recording budget exhausted",
        ));
    }
    // Repeat evaluations may report the same observation. Keep one per PC/detail.
    let mut seen = BTreeSet::new();
    unresolved.retain(|item| {
        let key = (item.detail.clone(), format!("{:?}", item.location));
        seen.insert(key)
    });
    flows.sort_by_key(|flow| (flow.sink_pc.pc, flow.origin.location.pc, flow.origin.kind));
    flows
}
