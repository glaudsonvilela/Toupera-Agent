//! Small, deterministic feedback policy for bounded local scenario selection.
//! Its memory is carried by the private scenario report, never by an external model.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::local_scenarios::ScenarioCase;

const MEMORY_LIMIT: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChipObservation {
    pub operation: String,
    pub amount: String,
    pub behavior: String,
    pub root_outcomes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AdaptiveChipReport {
    pub schema: String,
    pub previous_report_sha256: Option<String>,
    pub loaded_observations: usize,
    pub observations: Vec<ChipObservation>,
    pub skipped_repeat_inputs: usize,
    pub skipped_flat_neighbors: usize,
    pub candidates_ranked: usize,
    pub new_behavior_classes: usize,
    pub new_target_outcomes: usize,
    pub feedback_plateau: usize,
    pub stop_reason: Option<String>,
    pub memory_limit_reached: bool,
}

pub struct AdaptiveChip {
    report: AdaptiveChipReport,
    seen: BTreeSet<(String, String)>,
    behaviors: BTreeSet<(String, String)>,
    known_outcomes: BTreeSet<String>,
}

fn root_outcomes(case: &ScenarioCase) -> Vec<String> {
    let mut outcomes = case
        .observed_branch_outcomes
        .iter()
        .filter_map(|item| item.strip_prefix("1:").map(str::to_owned))
        .collect::<Vec<_>>();
    outcomes.sort();
    outcomes.dedup();
    outcomes
}

fn direction(before: &str, after: &str) -> &'static str {
    match (before.parse::<U256>(), after.parse::<U256>()) {
        (Ok(a), Ok(b)) if a < b => "up",
        (Ok(a), Ok(b)) if a > b => "down",
        (Ok(_), Ok(_)) => "same",
        _ => "unknown",
    }
}

fn behavior(case: &ScenarioCase, outcomes: &[String]) -> String {
    let state = match (&case.before, &case.after) {
        (Some(before), Some(after)) => format!(
            "{}:{}:{}:{}:{}",
            direction(&before.total_assets, &after.total_assets),
            direction(&before.total_supply, &after.total_supply),
            direction(&before.vault_asset_balance, &after.vault_asset_balance),
            direction(&before.actor_asset_balance, &after.actor_asset_balance),
            direction(&before.actor_share_balance, &after.actor_share_balance),
        ),
        _ => "unavailable".into(),
    };
    format!(
        "{}|{}|{}|{}|{}",
        case.status,
        case.preflight,
        state,
        case.discrepancies.is_empty(),
        outcomes.join(",")
    )
}

fn distance(a: U256, b: U256) -> U256 {
    if a >= b { a - b } else { b - a }
}

impl AdaptiveChip {
    pub fn load(
        previous_report: Option<&Path>,
        prior_coverage_path: &Path,
        base_sha256: &str,
        runtime_sha256: &str,
        block_hash: &str,
        prior_outcomes: &BTreeSet<String>,
    ) -> Result<Self, String> {
        let mut chip = Self {
            report: AdaptiveChipReport {
                schema: "adaptive-chip-v1".into(),
                ..Default::default()
            },
            seen: BTreeSet::new(),
            behaviors: BTreeSet::new(),
            known_outcomes: prior_outcomes.clone(),
        };
        if let Some(path) = previous_report {
            let bytes = fs::read(path)
                .map_err(|error| format!("cannot read previous scenario report: {error}"))?;
            let report: Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("invalid previous scenario report: {error}"))?;
            if report["schema"] != "local-scenarios-v1"
                || report["identity"]["base_report_sha256"] != base_sha256
                || report["identity"]["runtime_sha256"] != runtime_sha256
                || report["identity"]["block_hash"] != block_hash
                || report["guidance"].is_null()
            {
                return Err("previous scenario report identity or guidance differs".into());
            }
            chip.report.previous_report_sha256 = Some(hex::encode(Sha256::digest(&bytes)));
            let coverage: Value = serde_json::from_slice(
                &fs::read(prior_coverage_path)
                    .map_err(|error| format!("cannot read prior coverage: {error}"))?,
            )
            .map_err(|error| format!("invalid prior coverage: {error}"))?;
            if coverage["source_report_sha256"]
                != chip.report.previous_report_sha256.as_deref().unwrap_or("")
            {
                return Err("previous scenario report does not produce prior coverage".into());
            }
            if let Some(items) = report["guidance"]["adaptive_chip"]["observations"].as_array() {
                for item in items {
                    let observation: ChipObservation = serde_json::from_value(item.clone())
                        .map_err(|error| format!("invalid chip memory: {error}"))?;
                    chip.insert_loaded(observation);
                }
            } else {
                for item in report["cases"]
                    .as_array()
                    .ok_or("previous report lacks cases")?
                {
                    let sequence = item["sequence"].as_str().unwrap_or("");
                    if !["guided_source:", "guided_preview:", "guided_feedback:"]
                        .iter()
                        .any(|p| sequence.starts_with(p))
                    {
                        continue;
                    }
                    let case: ScenarioCase = serde_json::from_value(item.clone())
                        .map_err(|error| format!("invalid previous scenario case: {error}"))?;
                    let outcomes = root_outcomes(&case);
                    chip.insert_loaded(ChipObservation {
                        operation: case.operation.clone(),
                        amount: case.amount.clone(),
                        behavior: behavior(&case, &outcomes),
                        root_outcomes: outcomes,
                    });
                }
            }
        }
        chip.report.loaded_observations = chip.report.observations.len();
        Ok(chip)
    }

    fn insert_loaded(&mut self, observation: ChipObservation) {
        if self.report.observations.len() >= MEMORY_LIMIT {
            self.report.memory_limit_reached = true;
            return;
        }
        if !self
            .seen
            .insert((observation.operation.clone(), observation.amount.clone()))
        {
            return;
        }
        self.behaviors
            .insert((observation.operation.clone(), observation.behavior.clone()));
        self.report.observations.push(observation);
    }

    pub fn seed_baseline(&mut self, cases: &[ScenarioCase]) {
        for case in cases
            .iter()
            .filter(|case| case.sequence.starts_with("isolated:"))
        {
            let outcomes = root_outcomes(case);
            self.insert_loaded(ChipObservation {
                operation: case.operation.clone(),
                amount: case.amount.clone(),
                behavior: behavior(case, &outcomes),
                root_outcomes: outcomes,
            });
        }
    }

    pub fn consider(&mut self, operation: &str, amount: U256, scale: U256) -> bool {
        if self.seen.contains(&(operation.into(), amount.to_string())) {
            self.report.skipped_repeat_inputs += 1;
            return false;
        }
        let near = (scale / U256::from(1000)).max(U256::from(1));
        let neighbors = self
            .report
            .observations
            .iter()
            .filter(|item| item.operation == operation)
            .filter_map(|item| Some((item.amount.parse::<U256>().ok()?, &item.behavior)))
            .filter(|(value, _)| distance(*value, amount) <= near)
            .collect::<Vec<_>>();
        if neighbors.len() >= 2 && neighbors.iter().all(|(_, b)| *b == neighbors[0].1) {
            self.report.skipped_flat_neighbors += 1;
            return false;
        }
        true
    }

    pub fn observe(&mut self, case: &ScenarioCase, target_pcs: &[usize]) {
        let outcomes = root_outcomes(case);
        let behavior = behavior(case, &outcomes);
        let new_behavior = self
            .behaviors
            .insert((case.operation.clone(), behavior.clone()));
        let new_target = outcomes
            .iter()
            .filter(|item| {
                let Some((pc, _)) = item.split_once(':') else {
                    return false;
                };
                !self.known_outcomes.contains(*item)
                    && (target_pcs.is_empty()
                        || pc.parse::<usize>().is_ok_and(|pc| target_pcs.contains(&pc)))
            })
            .count();
        self.known_outcomes.extend(outcomes.iter().cloned());
        if new_behavior {
            self.report.new_behavior_classes += 1;
        }
        self.report.new_target_outcomes += new_target;
        if new_behavior || new_target > 0 || !case.discrepancies.is_empty() {
            self.report.feedback_plateau = 0;
        } else {
            self.report.feedback_plateau += 1;
        }
        self.seen
            .insert((case.operation.clone(), case.amount.clone()));
        if self.report.observations.len() < MEMORY_LIMIT {
            self.report.observations.push(ChipObservation {
                operation: case.operation.clone(),
                amount: case.amount.clone(),
                behavior,
                root_outcomes: outcomes,
            });
        } else {
            self.report.memory_limit_reached = true;
        }
    }

    pub fn candidates(
        &mut self,
        asset_scale: U256,
        share_scale: U256,
        target_functions: &[String],
        budget: usize,
    ) -> Vec<(String, U256)> {
        let mut ranked = BTreeMap::<(String, String), (i32, U256)>::new();
        for operation in ["deposit", "mint", "withdraw", "redeem"] {
            let scale = if matches!(operation, "deposit" | "withdraw") {
                asset_scale
            } else {
                share_scale
            };
            let mut amounts = vec![
                U256::ZERO,
                U256::from(1),
                scale / U256::from(4),
                scale / U256::from(2),
            ];
            for factor in [1, 2, 4, 8, 16] {
                let pivot = scale.saturating_mul(U256::from(factor));
                amounts.extend([
                    pivot.saturating_sub(U256::from(1)),
                    pivot,
                    pivot.saturating_add(U256::from(1)),
                ]);
            }
            let mut history = self
                .report
                .observations
                .iter()
                .filter(|item| item.operation == operation)
                .filter_map(|item| Some((item.amount.parse::<U256>().ok()?, item.behavior.clone())))
                .collect::<Vec<_>>();
            history.sort_by_key(|(amount, _)| *amount);
            for pair in history.windows(2) {
                if pair[0].1 != pair[1].1 && pair[1].0 > pair[0].0 + U256::from(1) {
                    let middle = pair[0].0 + (pair[1].0 - pair[0].0) / U256::from(2);
                    amounts.extend([
                        middle.saturating_sub(U256::from(1)),
                        middle,
                        middle.saturating_add(U256::from(1)),
                    ]);
                }
            }
            amounts.sort();
            amounts.dedup();
            for amount in amounts {
                if !self.consider(operation, amount, scale) {
                    continue;
                }
                let near_transition = history
                    .windows(2)
                    .any(|pair| pair[0].1 != pair[1].1 && amount > pair[0].0 && amount < pair[1].0);
                let far_from_history = history.iter().all(|(old, _)| {
                    distance(*old, amount) > (scale / U256::from(1000)).max(U256::from(1))
                });
                let score = (if target_functions.iter().any(|f| f == operation) {
                    4
                } else {
                    0
                }) + (if near_transition { 8 } else { 0 })
                    + (if far_from_history { 2 } else { 0 })
                    + (if amount > scale { 1 } else { 0 });
                ranked.insert((operation.into(), amount.to_string()), (score, amount));
            }
        }
        self.report.candidates_ranked = ranked.len();
        let mut candidates = ranked
            .into_iter()
            .map(|((op, _), (score, amount))| (score, op, amount))
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        candidates
            .into_iter()
            .take(budget)
            .map(|(_, op, amount)| (op, amount))
            .collect()
    }

    pub fn plateau(&self) -> usize {
        self.report.feedback_plateau
    }
    pub fn finish(mut self, reason: &str) -> AdaptiveChipReport {
        self.report.stop_reason = Some(reason.into());
        self.report
    }
}
