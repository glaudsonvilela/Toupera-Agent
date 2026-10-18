//! Optional evidence imported from independently run, local decompilers.
//! Neither format is treated as ground truth or merged into the native IR.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::bytecode::BytecodeAnalysis;
use crate::{BytecodePc, Confidence, Uncertainty, UncertaintyKind};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompanionAnalysis {
    pub native: BytecodeAnalysis,
    pub heimdall: HeimdallReport,
    pub gigahorse: GigahorseReport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HeimdallFunction {
    pub selector: String,
    pub signature: Option<String>,
    pub input_types: Vec<String>,
    pub state_mutability: Option<String>,
    pub native_entry_pc: Option<BytecodePc>,
    pub evmole_entry_pc: Option<BytecodePc>,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HeimdallReport {
    pub tool: String,
    pub input: String,
    pub functions: Vec<HeimdallFunction>,
    pub unresolved: Vec<Uncertainty>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GigahorseBlock {
    pub start_pc: BytecodePc,
    pub successor_pcs: Vec<BytecodePc>,
    pub native_block: bool,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GigahorseReport {
    pub tool: String,
    pub input: String,
    pub blocks: Vec<GigahorseBlock>,
    pub unresolved: Vec<Uncertainty>,
}

fn uncertainty(detail: impl Into<String>) -> Uncertainty {
    Uncertainty {
        kind: UncertaintyKind::AmbiguousSemantics,
        detail: detail.into(),
        location: None,
    }
}

fn pc(analysis: &BytecodeAnalysis, offset: usize) -> BytecodePc {
    BytecodePc {
        artifact: analysis.artifact.clone(),
        pc: offset,
    }
}

fn selector(value: &str) -> Option<String> {
    let hex = value.strip_prefix("0x").unwrap_or(value);
    (hex.len() == 8 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| format!("0x{}", hex.to_ascii_lowercase()))
}

/// Import Heimdall's `abi.json` created by `decompile --skip-resolving`.
/// The ABI is a hypothesis: selector agreement does not validate parameter
/// types, mutability, synthetic names, or path feasibility.
pub fn import_heimdall_abi(
    analysis: &BytecodeAnalysis,
    input_name: &str,
    abi_json: &str,
) -> Result<HeimdallReport, String> {
    let value: Value = serde_json::from_str(abi_json)
        .map_err(|error| format!("invalid Heimdall ABI JSON: {error}"))?;
    let entries = value
        .as_array()
        .ok_or("Heimdall ABI must be a JSON array")?;
    let mut report = HeimdallReport {
        tool: "heimdall-rs".into(),
        input: input_name.into(),
        functions: Vec::new(),
        unresolved: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    for entry in entries {
        if entry.get("type").and_then(Value::as_str) != Some("function") {
            continue;
        }
        let recovered_selector = entry.get("selector").and_then(Value::as_str).or_else(|| {
            entry
                .get("name")
                .and_then(Value::as_str)
                .and_then(|name| name.strip_prefix("Unresolved_"))
        });
        let Some(selector) = recovered_selector.and_then(selector) else {
            report.unresolved.push(uncertainty(
                "Heimdall function has no valid four-byte selector; ABI entry omitted",
            ));
            continue;
        };
        if !seen.insert(selector.clone()) {
            report.unresolved.push(uncertainty(format!(
                "Heimdall emitted duplicate selector {selector}"
            )));
            continue;
        }
        let native_entry_pc = analysis
            .selectors
            .iter()
            .find(|item| item.selector == selector)
            .map(|item| item.destination_pc.clone());
        let evmole_entry_pc = analysis
            .evmole
            .selectors
            .iter()
            .find(|item| item.selector == selector)
            .map(|item| item.destination_pc.clone());
        if native_entry_pc.is_none() && evmole_entry_pc.is_none() {
            report.unresolved.push(uncertainty(format!(
                "Heimdall selector {selector} has no independently recovered entry PC"
            )));
        } else if native_entry_pc.is_some()
            && evmole_entry_pc.is_some()
            && native_entry_pc != evmole_entry_pc
        {
            report.unresolved.push(uncertainty(format!(
                "native and EVMole disagree on entry PC for Heimdall selector {selector}"
            )));
        }
        report.functions.push(HeimdallFunction {
            selector,
            signature: entry
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
            input_types: entry
                .get("inputs")
                .and_then(Value::as_array)
                .map(|inputs| {
                    inputs
                        .iter()
                        .filter_map(|input| input.get("type").and_then(Value::as_str))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            state_mutability: entry
                .get("stateMutability")
                .and_then(Value::as_str)
                .map(str::to_owned),
            native_entry_pc,
            evmole_entry_pc,
            confidence: Confidence::Tentative,
        });
    }
    report.functions.sort_by(|a, b| a.selector.cmp(&b.selector));
    if report.functions.is_empty() {
        report.unresolved.push(uncertainty(
            "Heimdall ABI contains no functions with recoverable selectors",
        ));
    }
    Ok(report)
}

fn hex_pc(value: &str) -> Option<usize> {
    let digits = value.trim().strip_prefix("0x")?;
    (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| usize::from_str_radix(digits, 16).ok())
        .flatten()
}

fn successor_list(line: &str) -> Option<Vec<usize>> {
    let tail = line.split("succ=[").nth(1)?;
    let content = tail.split(']').next()?;
    if content.trim().is_empty() {
        return Some(Vec::new());
    }
    content.split(',').map(hex_pc).collect()
}

/// Import Gigahorse's `out/contract.tac` produced by `visualizeout.py`.
/// Only block labels and successor labels are read; TAC expressions remain
/// external evidence and are never silently added to native value flows.
pub fn import_gigahorse_tac(
    analysis: &BytecodeAnalysis,
    input_name: &str,
    tac: &str,
) -> Result<GigahorseReport, String> {
    if tac.len() > 16 * 1024 * 1024 {
        return Err("Gigahorse TAC exceeds the 16 MiB import limit".into());
    }
    let valid_pcs: BTreeSet<_> = analysis.instructions.iter().map(|item| item.pc).collect();
    let native_blocks: BTreeSet<_> = analysis.blocks.iter().map(|item| item.start_pc).collect();
    let mut blocks: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut current = None;
    let mut unresolved = Vec::new();
    for line in tac.lines() {
        let line = line.trim();
        if let Some(label) = line.strip_prefix("Begin block ") {
            current = hex_pc(label);
            if current.is_none() {
                unresolved.push(uncertainty("Gigahorse block has an invalid PC label"));
            }
            continue;
        }
        if let Some(start) = current {
            if line.contains("succ=[") {
                match successor_list(line) {
                    Some(successors) => blocks.entry(start).or_default().extend(successors),
                    None => unresolved.push(uncertainty(format!(
                        "Gigahorse block 0x{start:x} has an invalid successor list"
                    ))),
                }
                current = None;
            }
        }
    }
    if blocks.is_empty() {
        unresolved.push(uncertainty(
            "Gigahorse TAC contains no parsable block graph",
        ));
    }
    let mut report = GigahorseReport {
        tool: "gigahorse-toolchain".into(),
        input: input_name.into(),
        blocks: Vec::new(),
        unresolved,
    };
    for (start, successors) in blocks {
        if !valid_pcs.contains(&start) {
            report.unresolved.push(uncertainty(format!(
                "Gigahorse block 0x{start:x} is not a decoded runtime instruction PC"
            )));
            continue;
        }
        let mut accepted = Vec::new();
        for successor in successors {
            if valid_pcs.contains(&successor) {
                accepted.push(pc(analysis, successor));
            } else {
                report.unresolved.push(uncertainty(format!(
                    "Gigahorse successor 0x{successor:x} from block 0x{start:x} is outside decoded instruction PCs"
                )));
            }
        }
        report.blocks.push(GigahorseBlock {
            start_pc: pc(analysis, start),
            successor_pcs: accepted,
            native_block: native_blocks.contains(&start),
            confidence: Confidence::Tentative,
        });
    }
    Ok(report)
}
