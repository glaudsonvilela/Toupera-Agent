//! Source-level reachability across internal calls in the same contract.

use std::collections::HashMap;

use crate::{
    EconomicFunction, EconomicOperation, Evidence, FunctionId, ReachabilityPath,
    ReachabilityTerminal, Uncertainty, UncertaintyKind,
};

const MAX_HOPS: usize = 16;
const MAX_PATHS: usize = 4096;

pub(crate) fn populate_reachability(functions: &mut [EconomicFunction]) {
    let snapshot = functions.to_vec();
    let indices: HashMap<_, _> = snapshot
        .iter()
        .enumerate()
        .map(|(index, function)| {
            (
                (function.contract.0.clone(), function.function.0.clone()),
                index,
            )
        })
        .collect();
    for (root_index, root) in functions.iter_mut().enumerate() {
        let mut stack = vec![root_index];
        let mut calls = Vec::new();
        explore(
            &snapshot, &indices, root_index, &mut stack, &mut calls, root,
        );
    }
}

fn explore(
    snapshot: &[EconomicFunction],
    indices: &HashMap<(String, String), usize>,
    current: usize,
    stack: &mut Vec<usize>,
    calls: &mut Vec<Evidence<FunctionId>>,
    root: &mut EconomicFunction,
) {
    for call in &snapshot[current].related_functions {
        if root.reachability_paths.len() >= MAX_PATHS {
            root.unresolved.push(Uncertainty {
                kind: UncertaintyKind::AmbiguousSemantics,
                detail: "internal call path limit reached".into(),
                location: Some(call.location.clone()),
            });
            return;
        }
        let key = (snapshot[current].contract.0.clone(), call.value.0.clone());
        let Some(&target_index) = indices.get(&key) else {
            root.unresolved.push(Uncertainty {
                kind: UncertaintyKind::MissingInput,
                detail: format!(
                    "internal call target {} was not reconstructed",
                    call.value.0
                ),
                location: Some(call.location.clone()),
            });
            continue;
        };
        if stack.contains(&target_index) || calls.len() >= MAX_HOPS {
            root.unresolved.push(Uncertainty {
                kind: UncertaintyKind::AmbiguousSemantics,
                detail: "recursive or overlong internal call path was truncated".into(),
                location: Some(call.location.clone()),
            });
            continue;
        }
        calls.push(call.clone());
        stack.push(target_index);
        let target = &snapshot[target_index];
        let terminals = target
            .reads
            .iter()
            .cloned()
            .map(ReachabilityTerminal::StorageRead)
            .chain(
                target
                    .writes
                    .iter()
                    .cloned()
                    .map(ReachabilityTerminal::StorageWrite),
            )
            .chain(
                target
                    .oracle_dependencies
                    .iter()
                    .cloned()
                    .map(ReachabilityTerminal::OracleCall),
            );
        for terminal in terminals {
            if root.reachability_paths.len() >= MAX_PATHS {
                break;
            }
            root.reachability_paths.push(ReachabilityPath {
                calls: calls.clone(),
                terminal,
            });
        }
        if matches!(
            target.operation.value,
            EconomicOperation::ConvertToShares | EconomicOperation::ConvertToAssets
        ) && root.reachability_paths.len() < MAX_PATHS
        {
            root.reachability_paths.push(ReachabilityPath {
                calls: calls.clone(),
                terminal: ReachabilityTerminal::Conversion(target.operation.clone()),
            });
        }
        explore(snapshot, indices, target_index, stack, calls, root);
        stack.pop();
        calls.pop();
    }
}
