//! Conservative reconstruction of legacy EVM runtime bytecode.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::{
    BasicBlockId, BytecodePc, Confidence, ControlEdge, ControlEdgeKind, Evidence, EvidenceLocation,
    Uncertainty, UncertaintyKind,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeMetadata {
    pub start_pc: usize,
    pub cbor_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeInstruction {
    pub pc: usize,
    pub opcode: u8,
    pub mnemonic: String,
    pub immediate: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeBlock {
    pub id: BasicBlockId,
    pub start_pc: usize,
    pub end_pc: usize,
    pub instruction_pcs: Vec<usize>,
    pub reachable_from_entry: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeSelector {
    pub selector: String,
    pub dispatch_pc: BytecodePc,
    pub destination_pc: BytecodePc,
    pub confidence: Confidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum BytecodeEffectKind {
    StorageRead,
    StorageWrite,
    TransientStorageRead,
    TransientStorageWrite,
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
    Create,
    Create2,
    SelfDestruct,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeEffect {
    pub kind: BytecodeEffectKind,
    pub location: BytecodePc,
    /// Only a literal pushed immediately before a storage opcode is recorded.
    pub constant_slot: Option<Evidence<String>>,
    pub confidence: Confidence,
}

/// Opcode effects reachable from one recovered selector destination. This is
/// control-flow reachability, not value dependence or proof of feasibility.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeEntrySummary {
    pub selector: String,
    pub entry_pc: BytecodePc,
    pub effects: Vec<BytecodeEffect>,
    pub incomplete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeMethodMatch {
    pub signature: String,
    pub selector: String,
    pub destination_pc: BytecodePc,
    pub confidence: Confidence,
}

/// Comparison against one local solc Standard JSON contract output. A selector
/// match alone does not prove that the source produced the deployed runtime.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeSourceComparison {
    pub source_name: String,
    pub contract_name: String,
    pub normalized_runtime_match: Option<bool>,
    pub matched_methods: Vec<BytecodeMethodMatch>,
    pub unmatched_source_methods: Vec<String>,
    pub unmatched_bytecode_selectors: Vec<String>,
    pub unresolved: Vec<Uncertainty>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BytecodeAnalysis {
    pub artifact: String,
    pub raw_size: usize,
    pub code_size: usize,
    pub normalized_hex: String,
    pub metadata: Option<BytecodeMetadata>,
    pub instructions: Vec<BytecodeInstruction>,
    pub blocks: Vec<BytecodeBlock>,
    pub control_edges: Vec<ControlEdge>,
    pub selectors: Vec<BytecodeSelector>,
    pub effects: Vec<BytecodeEffect>,
    pub entry_summaries: Vec<BytecodeEntrySummary>,
    pub unresolved: Vec<Uncertainty>,
}

/// Analyzes explicitly supplied legacy EVM *runtime* bytecode encoded as hex.
/// Creation code, EOF containers, library placeholders, and symbolic execution
/// are outside this function's scope.
pub fn reconstruct_bytecode_hex(artifact: &str, input: &str) -> Result<BytecodeAnalysis, String> {
    if artifact.is_empty() {
        return Err("bytecode artifact identity cannot be empty".into());
    }
    let compact: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let hex = compact
        .strip_prefix("0x")
        .or_else(|| compact.strip_prefix("0X"))
        .unwrap_or(&compact);
    if hex.is_empty() || hex.len() % 2 != 0 {
        return Err("bytecode must contain a nonempty even number of hex digits".into());
    }
    let raw: Vec<u8> = hex
        .as_bytes()
        .chunks_exact(2)
        .enumerate()
        .map(|(index, pair)| {
            let text = std::str::from_utf8(pair).expect("ASCII hex has UTF-8 bytes");
            u8::from_str_radix(text, 16)
                .map_err(|_| format!("invalid hex byte at offset {}", index * 2))
        })
        .collect::<Result<_, _>>()?;
    if raw.starts_with(&[0xef, 0x00]) {
        return Err("EOF-formatted bytecode is not supported by the legacy decoder".into());
    }
    let metadata = solidity_metadata(&raw);
    let code_size = metadata.as_ref().map_or(raw.len(), |item| item.start_pc);
    if code_size == 0 {
        return Err("runtime code is empty after removing recognized metadata".into());
    }
    let code = &raw[..code_size];
    let mut unresolved = Vec::new();
    let instructions = disassemble(code, artifact, &mut unresolved);
    let mut blocks = basic_blocks(&instructions, code_size);
    let control_edges = control_edges(&instructions, &blocks, artifact, &mut unresolved);
    mark_reachable(&mut blocks, &control_edges);
    let selectors = selectors(&instructions, &blocks, artifact, &mut unresolved);
    let effects = effects(&instructions, &blocks, artifact);
    let entry_summaries =
        entry_summaries(&selectors, &blocks, &control_edges, &effects, &unresolved);
    Ok(BytecodeAnalysis {
        artifact: artifact.to_owned(),
        raw_size: raw.len(),
        code_size,
        normalized_hex: encode_hex(code),
        metadata,
        instructions,
        blocks,
        control_edges,
        selectors,
        effects,
        entry_summaries,
        unresolved,
    })
}

/// Compares recovered selectors and normalized runtime bytes with explicitly
/// supplied local compiler output. Compilation settings and library links must
/// already match the target for a byte-for-byte match to be meaningful.
pub fn compare_with_solc_contract(
    analysis: &BytecodeAnalysis,
    standard_json: &str,
    source_name: &str,
    contract_name: &str,
) -> Result<BytecodeSourceComparison, String> {
    let output: Value = serde_json::from_str(standard_json)
        .map_err(|error| format!("invalid solc JSON: {error}"))?;
    if output
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|errors| {
            errors
                .iter()
                .any(|item| item.get("severity").and_then(Value::as_str) == Some("error"))
        })
    {
        return Err("solc JSON contains compilation errors".into());
    }
    let contract = output
        .get("contracts")
        .and_then(|sources| sources.get(source_name))
        .and_then(|contracts| contracts.get(contract_name))
        .ok_or_else(|| format!("solc JSON has no contract {source_name}:{contract_name}"))?;
    let mut unresolved = Vec::new();
    let normalized_runtime_match = match contract
        .pointer("/evm/deployedBytecode/object")
        .and_then(Value::as_str)
    {
        Some(hex) => match reconstruct_bytecode_hex("compiler-runtime", hex) {
            Ok(compiled) => Some(compiled.normalized_hex == analysis.normalized_hex),
            Err(_) => {
                unresolved.push(Uncertainty {
                    kind: UncertaintyKind::MissingInput,
                    detail: "compiler runtime bytecode is empty, unlinked, or unsupported".into(),
                    location: None,
                });
                None
            }
        },
        None => {
            unresolved.push(Uncertainty {
                kind: UncertaintyKind::MissingInput,
                detail: "solc output lacks evm.deployedBytecode.object".into(),
                location: None,
            });
            None
        }
    };
    let mut matched_methods = Vec::new();
    let mut unmatched_source_methods = Vec::new();
    let mut matched_selector_values = BTreeSet::new();
    let methods = contract
        .pointer("/evm/methodIdentifiers")
        .and_then(Value::as_object);
    if let Some(methods) = methods {
        for (signature, value) in methods {
            let Some(selector) = value.as_str() else {
                unresolved.push(Uncertainty {
                    kind: UncertaintyKind::AmbiguousSemantics,
                    detail: format!("non-string selector for {signature}"),
                    location: None,
                });
                continue;
            };
            let selector = format!(
                "0x{}",
                selector.trim_start_matches("0x").to_ascii_lowercase()
            );
            if let Some(recovered) = analysis
                .selectors
                .iter()
                .find(|item| item.selector == selector)
            {
                matched_selector_values.insert(selector.clone());
                matched_methods.push(BytecodeMethodMatch {
                    signature: signature.clone(),
                    selector,
                    destination_pc: recovered.destination_pc.clone(),
                    confidence: if normalized_runtime_match == Some(true) {
                        recovered.confidence
                    } else {
                        Confidence::Tentative
                    },
                });
            } else {
                unmatched_source_methods.push(signature.clone());
            }
        }
    } else {
        unresolved.push(Uncertainty {
            kind: UncertaintyKind::MissingInput,
            detail: "solc output lacks evm.methodIdentifiers".into(),
            location: None,
        });
    }
    let unmatched_bytecode_selectors = if methods.is_some() {
        analysis
            .selectors
            .iter()
            .filter(|item| !matched_selector_values.contains(&item.selector))
            .map(|item| item.selector.clone())
            .collect()
    } else {
        Vec::new()
    };
    Ok(BytecodeSourceComparison {
        source_name: source_name.to_owned(),
        contract_name: contract_name.to_owned(),
        normalized_runtime_match,
        matched_methods,
        unmatched_source_methods,
        unmatched_bytecode_selectors,
        unresolved,
    })
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn solidity_metadata(raw: &[u8]) -> Option<BytecodeMetadata> {
    if raw.len() < 3 {
        return None;
    }
    let cbor_len = u16::from_be_bytes([raw[raw.len() - 2], raw[raw.len() - 1]]) as usize;
    let start_pc = raw.len().checked_sub(cbor_len + 2)?;
    if cbor_len == 0 || !recognized_cbor_map(&raw[start_pc..raw.len() - 2]) {
        return None;
    }
    Some(BytecodeMetadata { start_pc, cbor_len })
}

fn recognized_cbor_map(bytes: &[u8]) -> bool {
    let mut cursor = 0;
    let Some((major, entries)) = cbor_header(bytes, &mut cursor) else {
        return false;
    };
    if major != 5 || entries > 32 {
        return false;
    }
    let mut recognized = false;
    for _ in 0..entries {
        let Some((key_major, key_len)) = cbor_header(bytes, &mut cursor) else {
            return false;
        };
        if key_major != 3 {
            return false;
        }
        let Some(end) = cursor.checked_add(key_len as usize) else {
            return false;
        };
        let Some(key) = bytes.get(cursor..end) else {
            return false;
        };
        if matches!(key, b"solc" | b"ipfs" | b"bzzr0" | b"bzzr1") {
            recognized = true;
        }
        cursor = end;
        if !skip_cbor_item(bytes, &mut cursor, 0) {
            return false;
        }
    }
    recognized && cursor == bytes.len()
}

fn cbor_header(bytes: &[u8], cursor: &mut usize) -> Option<(u8, u64)> {
    let head = *bytes.get(*cursor)?;
    *cursor += 1;
    let major = head >> 5;
    let extra = head & 31;
    let width = match extra {
        0..=23 => return Some((major, extra as u64)),
        24 => 1,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => return None,
    };
    let end = cursor.checked_add(width)?;
    let value = bytes.get(*cursor..end)?;
    *cursor = end;
    Some((
        major,
        value
            .iter()
            .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte)),
    ))
}

fn skip_cbor_item(bytes: &[u8], cursor: &mut usize, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    let Some((major, len)) = cbor_header(bytes, cursor) else {
        return false;
    };
    match major {
        0 | 1 | 7 => true,
        2 | 3 => {
            let Ok(size) = usize::try_from(len) else {
                return false;
            };
            let Some(end) = cursor.checked_add(size) else {
                return false;
            };
            if end > bytes.len() {
                return false;
            }
            *cursor = end;
            true
        }
        4 | 5 => {
            let Some(count) = len.checked_mul(if major == 5 { 2 } else { 1 }) else {
                return false;
            };
            if count > bytes.len() as u64 {
                return false;
            }
            (0..count).all(|_| skip_cbor_item(bytes, cursor, depth + 1))
        }
        6 => skip_cbor_item(bytes, cursor, depth + 1),
        _ => false,
    }
}

fn disassemble(
    code: &[u8],
    artifact: &str,
    unresolved: &mut Vec<Uncertainty>,
) -> Vec<BytecodeInstruction> {
    let mut result = Vec::new();
    let mut pc = 0;
    while pc < code.len() {
        let opcode = code[pc];
        let width = if (0x60..=0x7f).contains(&opcode) {
            usize::from(opcode - 0x5f)
        } else {
            0
        };
        let end = (pc + 1 + width).min(code.len());
        let immediate = (width > 0).then(|| encode_hex(&code[pc + 1..end]));
        if pc + 1 + width > code.len() {
            unresolved.push(uncertainty(artifact, pc, "truncated PUSH immediate"));
        }
        let mnemonic = opcode_name(opcode);
        if mnemonic == "UNKNOWN" {
            unresolved.push(uncertainty(
                artifact,
                pc,
                "unrecognized opcode; semantics unknown",
            ));
        } else if matches!(opcode, 0x49 | 0x4a | 0x5c | 0x5d | 0x5e | 0x5f) {
            unresolved.push(uncertainty(
                artifact,
                pc,
                "opcode availability depends on the EVM fork, which was not supplied",
            ));
        }
        result.push(BytecodeInstruction {
            pc,
            opcode,
            mnemonic: mnemonic.to_owned(),
            immediate,
        });
        pc = end;
    }
    result
}

fn opcode_name(opcode: u8) -> &'static str {
    match opcode {
        0x00 => "STOP",
        0x01 => "ADD",
        0x02 => "MUL",
        0x03 => "SUB",
        0x04 => "DIV",
        0x05 => "SDIV",
        0x06 => "MOD",
        0x07 => "SMOD",
        0x08 => "ADDMOD",
        0x09 => "MULMOD",
        0x0a => "EXP",
        0x0b => "SIGNEXTEND",
        0x10 => "LT",
        0x11 => "GT",
        0x12 => "SLT",
        0x13 => "SGT",
        0x14 => "EQ",
        0x15 => "ISZERO",
        0x16 => "AND",
        0x17 => "OR",
        0x18 => "XOR",
        0x19 => "NOT",
        0x1a => "BYTE",
        0x1b => "SHL",
        0x1c => "SHR",
        0x1d => "SAR",
        0x20 => "KECCAK256",
        0x30 => "ADDRESS",
        0x31 => "BALANCE",
        0x32 => "ORIGIN",
        0x33 => "CALLER",
        0x34 => "CALLVALUE",
        0x35 => "CALLDATALOAD",
        0x36 => "CALLDATASIZE",
        0x37 => "CALLDATACOPY",
        0x38 => "CODESIZE",
        0x39 => "CODECOPY",
        0x3a => "GASPRICE",
        0x3b => "EXTCODESIZE",
        0x3c => "EXTCODECOPY",
        0x3d => "RETURNDATASIZE",
        0x3e => "RETURNDATACOPY",
        0x3f => "EXTCODEHASH",
        0x40 => "BLOCKHASH",
        0x41 => "COINBASE",
        0x42 => "TIMESTAMP",
        0x43 => "NUMBER",
        0x44 => "PREVRANDAO",
        0x45 => "GASLIMIT",
        0x46 => "CHAINID",
        0x47 => "SELFBALANCE",
        0x48 => "BASEFEE",
        0x49 => "BLOBHASH",
        0x4a => "BLOBBASEFEE",
        0x50 => "POP",
        0x51 => "MLOAD",
        0x52 => "MSTORE",
        0x53 => "MSTORE8",
        0x54 => "SLOAD",
        0x55 => "SSTORE",
        0x56 => "JUMP",
        0x57 => "JUMPI",
        0x58 => "PC",
        0x59 => "MSIZE",
        0x5a => "GAS",
        0x5b => "JUMPDEST",
        0x5c => "TLOAD",
        0x5d => "TSTORE",
        0x5e => "MCOPY",
        0x5f => "PUSH0",
        0x60..=0x7f => "PUSH",
        0x80..=0x8f => "DUP",
        0x90..=0x9f => "SWAP",
        0xa0..=0xa4 => "LOG",
        0xf0 => "CREATE",
        0xf1 => "CALL",
        0xf2 => "CALLCODE",
        0xf3 => "RETURN",
        0xf4 => "DELEGATECALL",
        0xf5 => "CREATE2",
        0xfa => "STATICCALL",
        0xfd => "REVERT",
        0xfe => "INVALID",
        0xff => "SELFDESTRUCT",
        _ => "UNKNOWN",
    }
}

fn basic_blocks(instructions: &[BytecodeInstruction], code_size: usize) -> Vec<BytecodeBlock> {
    let mut leaders = BTreeSet::from([0]);
    for (index, instruction) in instructions.iter().enumerate() {
        if instruction.opcode == 0x5b {
            leaders.insert(instruction.pc);
        }
        if is_block_end(instruction.opcode) {
            if let Some(next) = instructions.get(index + 1) {
                leaders.insert(next.pc);
            }
        }
    }
    let starts: Vec<_> = leaders.into_iter().collect();
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end_pc = starts.get(index + 1).copied().unwrap_or(code_size);
            BytecodeBlock {
                id: BasicBlockId(format!("pc:{start}")),
                start_pc: *start,
                end_pc,
                instruction_pcs: instructions
                    .iter()
                    .filter(|item| item.pc >= *start && item.pc < end_pc)
                    .map(|item| item.pc)
                    .collect(),
                reachable_from_entry: false,
            }
        })
        .collect()
}

fn mark_reachable(blocks: &mut [BytecodeBlock], edges: &[ControlEdge]) {
    let mut reached = BTreeSet::from(["pc:0".to_owned()]);
    loop {
        let mut changed = false;
        for edge in edges {
            if reached.contains(&edge.from.0) {
                changed |= reached.insert(edge.to.0.clone());
            }
        }
        if !changed {
            break;
        }
    }
    for block in blocks {
        block.reachable_from_entry = reached.contains(&block.id.0);
    }
}

fn is_block_end(opcode: u8) -> bool {
    matches!(opcode, 0x00 | 0x56 | 0x57 | 0xf3 | 0xfd | 0xfe | 0xff)
        || opcode_name(opcode) == "UNKNOWN"
}

fn push_value(instruction: &BytecodeInstruction) -> Option<usize> {
    if instruction.opcode == 0x5f {
        return Some(0);
    }
    if !(0x60..=0x7f).contains(&instruction.opcode) {
        return None;
    }
    let hex = instruction.immediate.as_deref()?;
    if hex.len() != usize::from(instruction.opcode - 0x5f) * 2 {
        return None;
    }
    usize::from_str_radix(hex, 16).ok()
}

fn control_edges(
    instructions: &[BytecodeInstruction],
    blocks: &[BytecodeBlock],
    artifact: &str,
    unresolved: &mut Vec<Uncertainty>,
) -> Vec<ControlEdge> {
    let by_pc: BTreeMap<_, _> = instructions.iter().map(|item| (item.pc, item)).collect();
    let jumpdests: BTreeSet<_> = instructions
        .iter()
        .filter(|item| item.opcode == 0x5b)
        .map(|item| item.pc)
        .collect();
    let mut edges = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let Some(last_pc) = block.instruction_pcs.last() else {
            continue;
        };
        let last = by_pc[last_pc];
        let mut add = |to: &BytecodeBlock, kind| {
            edges.push(ControlEdge {
                from: block.id.clone(),
                to: to.id.clone(),
                kind,
                location: EvidenceLocation::Bytecode(BytecodePc {
                    artifact: artifact.to_owned(),
                    pc: last.pc,
                }),
            });
        };
        if matches!(last.opcode, 0x56 | 0x57) {
            let destination = block
                .instruction_pcs
                .iter()
                .rev()
                .nth(1)
                .and_then(|pc| by_pc.get(pc))
                .and_then(|item| push_value(item));
            if let Some(target) = destination.filter(|pc| jumpdests.contains(pc)) {
                if let Some(to) = blocks.iter().find(|item| item.start_pc == target) {
                    add(
                        to,
                        if last.opcode == 0x57 {
                            ControlEdgeKind::True
                        } else {
                            ControlEdgeKind::Next
                        },
                    );
                }
            } else {
                unresolved.push(uncertainty(
                    artifact,
                    last.pc,
                    "jump destination is dynamic, invalid, or not a direct preceding PUSH",
                ));
            }
            if last.opcode == 0x57 {
                if let Some(next) = blocks.get(index + 1) {
                    add(next, ControlEdgeKind::False);
                }
            }
        } else if !is_block_end(last.opcode) {
            if let Some(next) = blocks.get(index + 1) {
                add(next, ControlEdgeKind::Next);
            }
        }
    }
    edges
}

fn selectors(
    instructions: &[BytecodeInstruction],
    blocks: &[BytecodeBlock],
    artifact: &str,
    unresolved: &mut Vec<Uncertainty>,
) -> Vec<BytecodeSelector> {
    let jumpdests: BTreeSet<_> = instructions
        .iter()
        .filter(|item| item.opcode == 0x5b)
        .map(|item| item.pc)
        .collect();
    let mut found = Vec::new();
    for (index, window) in instructions.windows(4).enumerate() {
        let [selector, eq, target, jump] = window else {
            continue;
        };
        if selector.opcode != 0x63 || eq.opcode != 0x14 || jump.opcode != 0x57 {
            continue;
        }
        let Some(destination) = push_value(target).filter(|pc| jumpdests.contains(pc)) else {
            unresolved.push(uncertainty(
                artifact,
                jump.pc,
                "selector comparison has no literal JUMPDEST",
            ));
            continue;
        };
        found.push(BytecodeSelector {
            selector: format!("0x{}", selector.immediate.as_deref().unwrap_or("")),
            dispatch_pc: BytecodePc {
                artifact: artifact.to_owned(),
                pc: selector.pc,
            },
            destination_pc: BytecodePc {
                artifact: artifact.to_owned(),
                pc: destination,
            },
            confidence: if block_reachable(blocks, selector.pc)
                && index > 0
                && instructions[index - 1].opcode == 0x80
            {
                Confidence::Inferred
            } else {
                Confidence::Tentative
            },
        });
    }
    found
}

fn block_reachable(blocks: &[BytecodeBlock], pc: usize) -> bool {
    blocks
        .iter()
        .any(|block| block.start_pc <= pc && pc < block.end_pc && block.reachable_from_entry)
}

fn effects(
    instructions: &[BytecodeInstruction],
    blocks: &[BytecodeBlock],
    artifact: &str,
) -> Vec<BytecodeEffect> {
    instructions
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            let kind = match instruction.opcode {
                0x54 => BytecodeEffectKind::StorageRead,
                0x55 => BytecodeEffectKind::StorageWrite,
                0x5c => BytecodeEffectKind::TransientStorageRead,
                0x5d => BytecodeEffectKind::TransientStorageWrite,
                0xf1 => BytecodeEffectKind::Call,
                0xf2 => BytecodeEffectKind::CallCode,
                0xf4 => BytecodeEffectKind::DelegateCall,
                0xfa => BytecodeEffectKind::StaticCall,
                0xf0 => BytecodeEffectKind::Create,
                0xf5 => BytecodeEffectKind::Create2,
                0xff => BytecodeEffectKind::SelfDestruct,
                _ => return None,
            };
            let constant_slot = if matches!(instruction.opcode, 0x54 | 0x55 | 0x5c | 0x5d) {
                index
                    .checked_sub(1)
                    .and_then(|previous| instructions.get(previous))
                    .filter(|previous| {
                        previous.opcode == 0x5f || (0x60..=0x7f).contains(&previous.opcode)
                    })
                    .map(|previous| Evidence {
                        value: previous.immediate.clone().unwrap_or_else(|| "00".into()),
                        location: EvidenceLocation::Bytecode(BytecodePc {
                            artifact: artifact.to_owned(),
                            pc: previous.pc,
                        }),
                        confidence: Confidence::Inferred,
                    })
            } else {
                None
            };
            Some(BytecodeEffect {
                kind,
                location: BytecodePc {
                    artifact: artifact.to_owned(),
                    pc: instruction.pc,
                },
                constant_slot,
                confidence: if block_reachable(blocks, instruction.pc)
                    && !matches!(instruction.opcode, 0x5c | 0x5d)
                {
                    Confidence::Direct
                } else {
                    Confidence::Tentative
                },
            })
        })
        .collect()
}

fn entry_summaries(
    selectors: &[BytecodeSelector],
    blocks: &[BytecodeBlock],
    edges: &[ControlEdge],
    effects: &[BytecodeEffect],
    unresolved: &[Uncertainty],
) -> Vec<BytecodeEntrySummary> {
    selectors
        .iter()
        .map(|selector| {
            let start = format!("pc:{}", selector.destination_pc.pc);
            let mut reached = BTreeSet::from([start]);
            loop {
                let mut changed = false;
                for edge in edges {
                    if reached.contains(&edge.from.0) {
                        changed |= reached.insert(edge.to.0.clone());
                    }
                }
                if !changed {
                    break;
                }
            }
            let contains = |pc| {
                blocks.iter().any(|block| {
                    reached.contains(&block.id.0) && block.start_pc <= pc && pc < block.end_pc
                })
            };
            let effects = effects
                .iter()
                .filter(|effect| contains(effect.location.pc))
                .cloned()
                .collect();
            let incomplete = selector.confidence == Confidence::Tentative || unresolved.iter().any(|item| {
                matches!(&item.location, Some(EvidenceLocation::Bytecode(pc)) if contains(pc.pc))
            });
            BytecodeEntrySummary {
                selector: selector.selector.clone(),
                entry_pc: selector.destination_pc.clone(),
                effects,
                incomplete,
            }
        })
        .collect()
}

fn uncertainty(artifact: &str, pc: usize, detail: &str) -> Uncertainty {
    Uncertainty {
        kind: UncertaintyKind::AmbiguousSemantics,
        detail: detail.to_owned(),
        location: Some(EvidenceLocation::Bytecode(BytecodePc {
            artifact: artifact.to_owned(),
            pc,
        })),
    }
}
