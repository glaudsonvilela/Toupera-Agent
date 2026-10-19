//! Offline observation map derived from already recorded local scenario traces.
//! A scenario's root EVM frame is the vault; child frames may belong to tokens.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct RecordedScenarios {
    schema: String,
    identity: Value,
    cases: Vec<RecordedCase>,
    vault_runtime_jump_sites: usize,
    vault_jump_sites_observed: usize,
    vault_branch_outcomes_observed: usize,
}

#[derive(Deserialize)]
struct RecordedCase {
    sequence: String,
    step: usize,
    operation: String,
    amount: String,
    status: String,
    trace_steps: Option<usize>,
    observed_jump_sites: Vec<String>,
    observed_branch_outcomes: Vec<String>,
}

#[derive(Serialize)]
pub struct OperationObservation {
    pub operation: String,
    pub cases: usize,
    pub completed: usize,
    pub reverted: usize,
    pub trace_missing: usize,
    pub observed_jump_sites: usize,
    pub observed_branch_outcomes: usize,
    pub sites_seen_both_ways: usize,
    pub outcomes_seen_only_in_this_operation: usize,
}

#[derive(Serialize)]
pub struct BranchObservation {
    pub pc: usize,
    pub outcomes: Vec<String>,
    pub operations: Vec<String>,
    pub cases: usize,
}

#[derive(Serialize)]
pub struct WitnessCase {
    pub sequence: String,
    pub step: usize,
    pub operation: String,
    pub status: String,
    pub newly_accounted_outcomes: usize,
}

#[derive(Serialize)]
pub struct InputGroupObservation {
    pub group: String,
    pub cases: usize,
    pub observed_branch_outcomes: usize,
    pub outcomes_not_seen_with_zero_inputs: usize,
    pub outcomes_unique_to_this_group: usize,
}

#[derive(Serialize)]
pub struct LocalCoverageReport {
    pub schema: String,
    pub source_report_sha256: String,
    pub identity: Value,
    pub case_count: usize,
    pub vault_runtime_jump_sites: usize,
    pub vault_jump_sites_observed: usize,
    pub vault_branch_outcomes_observed: usize,
    pub runtime_sites_not_observed: usize,
    pub observed_sites_with_one_outcome: usize,
    pub operation_observations: Vec<OperationObservation>,
    pub input_group_observations: Vec<InputGroupObservation>,
    pub next_scenario_focus: String,
    pub branch_observations: Vec<BranchObservation>,
    pub representative_cases: Vec<WitnessCase>,
    pub representative_case_outcomes: usize,
    pub limitations: Vec<String>,
}

#[derive(Default)]
struct OperationCounts {
    cases: usize,
    completed: usize,
    reverted: usize,
    trace_missing: usize,
    sites: BTreeSet<usize>,
    outcomes: BTreeSet<(usize, String)>,
}

fn root_site(value: &str) -> Option<usize> {
    let (depth, pc) = value.split_once(':')?;
    (depth == "1").then(|| pc.parse::<usize>().ok()).flatten()
}

fn root_outcome(value: &str) -> Option<(usize, String)> {
    let mut parts = value.split(':');
    let depth = parts.next()?;
    let pc = parts.next()?.parse::<usize>().ok()?;
    let outcome = parts.next()?;
    if depth != "1" || parts.next().is_some() || !matches!(outcome, "taken" | "not_taken") {
        return None;
    }
    Some((pc, outcome.to_owned()))
}

pub fn analyze_recorded_scenarios(path: &Path) -> Result<LocalCoverageReport, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let recorded: RecordedScenarios = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid local scenario report: {error}"))?;
    if recorded.schema != "local-scenarios-v1" || recorded.cases.is_empty() {
        return Err("expected a nonempty local-scenarios-v1 report".into());
    }
    let mut operations: BTreeMap<String, OperationCounts> = BTreeMap::new();
    let mut all_sites = BTreeSet::new();
    let mut all_outcomes = BTreeSet::new();
    let mut case_outcomes = Vec::with_capacity(recorded.cases.len());
    let mut site_operations: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
    let mut site_cases: BTreeMap<usize, usize> = BTreeMap::new();
    let mut input_groups: BTreeMap<String, (usize, BTreeSet<(usize, String)>)> = BTreeMap::new();
    for case in &recorded.cases {
        let entry = operations.entry(case.operation.clone()).or_default();
        entry.cases += 1;
        entry.completed += usize::from(case.status == "completed");
        entry.reverted += usize::from(case.status == "reverted");
        entry.trace_missing += usize::from(case.trace_steps.is_none());
        let sites: BTreeSet<usize> = case
            .observed_jump_sites
            .iter()
            .filter_map(|site| root_site(site))
            .collect();
        let outcomes: BTreeSet<(usize, String)> = case
            .observed_branch_outcomes
            .iter()
            .filter_map(|outcome| root_outcome(outcome))
            .collect();
        if outcomes.iter().any(|(pc, _)| !sites.contains(pc)) {
            return Err(format!(
                "case {} has an outcome without its JUMPI site",
                case.sequence
            ));
        }
        for pc in &sites {
            site_operations
                .entry(*pc)
                .or_default()
                .insert(case.operation.clone());
            *site_cases.entry(*pc).or_default() += 1;
        }
        entry.sites.extend(sites.iter().copied());
        entry.outcomes.extend(outcomes.iter().cloned());
        let group = if case.sequence.starts_with("guided_limit:") {
            "guided_limit"
        } else if case.sequence.starts_with("guided_actor:") {
            "guided_actor"
        } else if case.sequence.starts_with("guided_asset_state:") {
            "guided_asset_state"
        } else if case.sequence.starts_with("above_limit:") {
            "above_limit"
        } else if case.sequence == "deposit_then_redeem" {
            "ordered_sequence"
        } else if case.amount == "0" {
            "zero_amount"
        } else {
            "ordinary_positive"
        };
        let group_entry = input_groups.entry(group.into()).or_default();
        group_entry.0 += 1;
        group_entry.1.extend(outcomes.iter().cloned());
        all_sites.extend(sites);
        all_outcomes.extend(outcomes.iter().cloned());
        case_outcomes.push(outcomes);
    }
    if all_sites.len() != recorded.vault_jump_sites_observed
        || all_outcomes.len() != recorded.vault_branch_outcomes_observed
        || all_sites.len() > recorded.vault_runtime_jump_sites
    {
        return Err(
            "root-frame counts do not match the verified vault counts in the report".into(),
        );
    }
    let mut outcome_operations: BTreeMap<(usize, String), usize> = BTreeMap::new();
    for counts in operations.values() {
        for outcome in &counts.outcomes {
            *outcome_operations.entry(outcome.clone()).or_default() += 1;
        }
    }
    let operation_observations = operations
        .into_iter()
        .map(|(operation, counts)| {
            let sites_seen_both_ways = counts
                .sites
                .iter()
                .filter(|pc| {
                    counts.outcomes.contains(&(**pc, "taken".into()))
                        && counts.outcomes.contains(&(**pc, "not_taken".into()))
                })
                .count();
            let outcomes_seen_only_in_this_operation = counts
                .outcomes
                .iter()
                .filter(|outcome| outcome_operations.get(*outcome) == Some(&1))
                .count();
            OperationObservation {
                operation,
                cases: counts.cases,
                completed: counts.completed,
                reverted: counts.reverted,
                trace_missing: counts.trace_missing,
                observed_jump_sites: counts.sites.len(),
                observed_branch_outcomes: counts.outcomes.len(),
                sites_seen_both_ways,
                outcomes_seen_only_in_this_operation,
            }
        })
        .collect();
    let branch_observations = all_sites
        .iter()
        .map(|pc| BranchObservation {
            pc: *pc,
            outcomes: ["taken", "not_taken"]
                .into_iter()
                .filter(|outcome| all_outcomes.contains(&(*pc, (*outcome).to_owned())))
                .map(str::to_owned)
                .collect(),
            operations: site_operations
                .get(pc)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            cases: *site_cases.get(pc).unwrap_or(&0),
        })
        .collect::<Vec<_>>();
    let observed_sites_with_one_outcome = branch_observations
        .iter()
        .filter(|site| site.outcomes.len() == 1)
        .count();
    let zero_outcomes = input_groups
        .get("zero_amount")
        .map(|(_, outcomes)| outcomes.clone())
        .unwrap_or_default();
    let positive_additions = input_groups
        .get("ordinary_positive")
        .map(|(_, outcomes)| outcomes.difference(&zero_outcomes).count())
        .unwrap_or(0);
    let next_scenario_focus = if !zero_outcomes.is_empty() && positive_additions == 0 {
        "Ordinary positive amounts did not add a root-frame outcome beyond zero inputs in this snapshot; prioritize source-derived thresholds, different actors, state transitions, and call order".into()
    } else {
        "Inspect which input group added root-frame outcomes, then choose source-derived thresholds and state transitions for the remaining paths".into()
    };
    let input_group_observations = input_groups
        .iter()
        .map(|(group, (case_count, outcomes))| {
            let other_outcomes: BTreeSet<_> = input_groups
                .iter()
                .filter(|(other, _)| *other != group)
                .flat_map(|(_, (_, outcomes))| outcomes.iter().cloned())
                .collect();
            InputGroupObservation {
                group: group.clone(),
                cases: *case_count,
                observed_branch_outcomes: outcomes.len(),
                outcomes_not_seen_with_zero_inputs: outcomes.difference(&zero_outcomes).count(),
                outcomes_unique_to_this_group: outcomes.difference(&other_outcomes).count(),
            }
        })
        .collect();
    // Greedy set cover provides a small, reproducible witness set for the
    // outcomes already observed. It does not prove that other paths are absent.
    let mut uncovered = all_outcomes.clone();
    let mut selected = BTreeSet::new();
    let mut representative_cases = Vec::new();
    while !uncovered.is_empty() {
        let next = case_outcomes
            .iter()
            .enumerate()
            .filter(|(index, _)| !selected.contains(index))
            .map(|(index, outcomes)| (index, outcomes.intersection(&uncovered).count()))
            .max_by_key(|(index, count)| (*count, usize::MAX - index));
        let Some((index, gain)) = next else { break };
        if gain == 0 {
            break;
        }
        selected.insert(index);
        let case = &recorded.cases[index];
        representative_cases.push(WitnessCase {
            sequence: case.sequence.clone(),
            step: case.step,
            operation: case.operation.clone(),
            status: case.status.clone(),
            newly_accounted_outcomes: gain,
        });
        uncovered.retain(|outcome| !case_outcomes[index].contains(outcome));
    }
    if !uncovered.is_empty() {
        return Err("could not account for every observed root-frame outcome".into());
    }
    Ok(LocalCoverageReport {
        schema: "local-coverage-v1".into(),
        source_report_sha256: format!("{:x}", Sha256::digest(&bytes)),
        identity: recorded.identity,
        case_count: recorded.cases.len(),
        vault_runtime_jump_sites: recorded.vault_runtime_jump_sites,
        vault_jump_sites_observed: all_sites.len(),
        vault_branch_outcomes_observed: all_outcomes.len(),
        runtime_sites_not_observed: recorded.vault_runtime_jump_sites - all_sites.len(),
        observed_sites_with_one_outcome,
        operation_observations,
        input_group_observations,
        next_scenario_focus,
        branch_observations,
        representative_case_outcomes: all_outcomes.len(),
        representative_cases,
        limitations: vec![
            "Operation labels come from top-level scenario calls; nested calls can share the same root PC".into(),
            "Runtime JUMPI count is static and can include unreachable code; this is not source-function or reachable-branch coverage".into(),
            "Representative cases retain already observed outcomes only; they do not replace full scenario or state checks".into(),
        ],
    })
}
