//! Deterministic, source-backed economic risk candidates. These rules are
//! triage gates, not vulnerability verdicts or numeric invariant proofs.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::{
    Confidence, ContractId, EconomicFunction, EconomicOperation, EvidenceLocation, FindingKind,
    FunctionId, SourceSpan, Uncertainty, UncertaintyKind, ValueOrigin, ValueSink,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EconomicCandidate {
    pub contract: ContractId,
    pub function: FunctionId,
    pub kind: FindingKind,
    pub status: String,
    pub confidence: Confidence,
    pub rules: Vec<String>,
    pub reasons: Vec<String>,
    pub assumptions: Vec<String>,
    pub evidence: Vec<EvidenceLocation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SlitherSignal {
    pub check: String,
    pub impact: String,
    pub confidence: String,
    pub mapped_to_candidate: bool,
    pub location: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EconomicAnalysisMetrics {
    pub source_functions: usize,
    pub raw_rule_signals: usize,
    pub imported_slither_signals: usize,
    pub mapped_slither_signals: usize,
    pub candidates_after_deduplication: usize,
    pub signals_removed_by_deduplication: usize,
    pub false_positives_reviewed: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EconomicAnalysisReport {
    pub candidates: Vec<EconomicCandidate>,
    pub slither_signals: Vec<SlitherSignal>,
    pub metrics: EconomicAnalysisMetrics,
    pub unresolved: Vec<Uncertainty>,
}

struct Collector {
    candidates: BTreeMap<(String, String, String), EconomicCandidate>,
    raw: usize,
}

impl Collector {
    fn new() -> Self {
        Self {
            candidates: BTreeMap::new(),
            raw: 0,
        }
    }

    fn add(
        &mut self,
        function: &EconomicFunction,
        kind: FindingKind,
        rule: &str,
        reason: &str,
        assumption: &str,
        evidence: Vec<EvidenceLocation>,
    ) {
        if evidence.is_empty() {
            return;
        }
        self.raw += 1;
        let key = (
            function.contract.0.clone(),
            function.function.0.clone(),
            format!("{kind:?}"),
        );
        let candidate = self
            .candidates
            .entry(key)
            .or_insert_with(|| EconomicCandidate {
                contract: function.contract.clone(),
                function: function.function.clone(),
                kind,
                status: "UNVERIFIED".into(),
                confidence: Confidence::Tentative,
                rules: Vec::new(),
                reasons: Vec::new(),
                assumptions: Vec::new(),
                evidence: Vec::new(),
            });
        push_unique(&mut candidate.rules, rule.to_owned());
        push_unique(&mut candidate.reasons, reason.to_owned());
        push_unique(&mut candidate.assumptions, assumption.to_owned());
        for location in evidence {
            push_unique(&mut candidate.evidence, location);
        }
    }
}

fn push_unique<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if !items.contains(&item) {
        items.push(item);
    }
}

fn location(function: &EconomicFunction) -> EvidenceLocation {
    function.operation.location.clone()
}

fn name_contains(value: &str, needles: &[&str]) -> bool {
    let lower = value.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn read_named(function: &EconomicFunction, needles: &[&str]) -> Vec<EvidenceLocation> {
    function
        .reads
        .iter()
        .filter(|item| name_contains(&item.value.0, needles))
        .map(|item| item.location.clone())
        .collect()
}

fn write_named(function: &EconomicFunction, needles: &[&str]) -> Vec<EvidenceLocation> {
    function
        .writes
        .iter()
        .filter(|item| name_contains(&item.value.0, needles))
        .map(|item| item.location.clone())
        .collect()
}

fn arithmetic_named(function: &EconomicFunction, needles: &[&str]) -> Vec<EvidenceLocation> {
    function
        .arithmetic
        .iter()
        .filter(|item| name_contains(&item.value.0, needles))
        .map(|item| item.location.clone())
        .collect()
}

fn calls_named(function: &EconomicFunction, needles: &[&str]) -> Vec<EvidenceLocation> {
    function
        .external_calls
        .iter()
        .filter(|item| name_contains(&item.value.0, needles))
        .map(|item| item.location.clone())
        .collect()
}

fn related_named(function: &EconomicFunction, needles: &[&str]) -> bool {
    function
        .related_functions
        .iter()
        .any(|item| name_contains(&item.value.0, needles))
}

fn fee_evidence(function: &EconomicFunction) -> Vec<EvidenceLocation> {
    let mut evidence = read_named(function, &["fee"]);
    evidence.extend(arithmetic_named(function, &["fee"]));
    evidence.extend(calls_named(function, &["fee"]));
    evidence.extend(
        function
            .related_functions
            .iter()
            .filter(|item| name_contains(&item.value.0, &["fee"]))
            .map(|item| item.location.clone()),
    );
    evidence
}

fn rounding_hint(function: &EconomicFunction) -> Option<(&'static str, EvidenceLocation)> {
    for item in &function.arithmetic {
        let lower = item.value.0.to_ascii_lowercase();
        if ["rounding.up", "roundup", "ceildiv", "muldivup"]
            .iter()
            .any(|s| lower.contains(s))
        {
            return Some(("up", item.location.clone()));
        }
        if ["rounding.down", "rounddown", "muldivdown"]
            .iter()
            .any(|s| lower.contains(s))
        {
            return Some(("down", item.location.clone()));
        }
    }
    None
}

// Recognize only the literal ceil-division idiom `(x + d - 1) / d` and
// the matching floor division `x / d`. Other arithmetic needs a richer AST.
fn division_formula(expression: &str) -> Option<(String, String, bool)> {
    let compact: String = expression
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    let (left, denominator) = compact.split_once('/')?;
    if denominator.is_empty()
        || !denominator
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    let ceil_suffix = format!("+{denominator}-1");
    if let Some(numerator) = left
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .and_then(|inner| inner.strip_suffix(&ceil_suffix))
    {
        if !numerator.is_empty() {
            return Some((numerator.to_owned(), denominator.to_owned(), true));
        }
    }
    if !left.is_empty()
        && left
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Some((left.to_owned(), denominator.to_owned(), false));
    }
    None
}

fn formula_rounding_difference(
    execution: &EconomicFunction,
    preview: &EconomicFunction,
) -> Option<(EvidenceLocation, EvidenceLocation)> {
    for executed in &execution.arithmetic {
        let Some((executed_numerator, executed_denominator, executed_ceil)) =
            division_formula(&executed.value.0)
        else {
            continue;
        };
        for proposed in &preview.arithmetic {
            let Some((preview_numerator, preview_denominator, preview_ceil)) =
                division_formula(&proposed.value.0)
            else {
                continue;
            };
            if executed_numerator == preview_numerator
                && executed_denominator == preview_denominator
                && executed_ceil != preview_ceil
            {
                return Some((executed.location.clone(), proposed.location.clone()));
            }
        }
    }
    None
}

fn oracle_conversion(function: &EconomicFunction) -> Vec<EvidenceLocation> {
    function
        .value_flows
        .iter()
        .filter(|flow| {
            matches!(flow.origin.value, ValueOrigin::OracleCall(_))
                && matches!(
                    flow.sink.value,
                    ValueSink::ConversionArgument(_) | ValueSink::StorageWrite(_)
                )
        })
        .map(|flow| flow.sink.location.clone())
        .collect()
}

fn oracle_guard(function: &EconomicFunction) -> bool {
    function.value_flows.iter().any(|flow| {
        matches!(flow.origin.value, ValueOrigin::OracleCall(_))
            && matches!(flow.sink.value, ValueSink::Guard)
    })
}

fn oracle_targets(function: &EconomicFunction) -> BTreeSet<String> {
    function
        .oracle_dependencies
        .iter()
        .map(|item| item.value.0.clone())
        .collect()
}

fn matching<'a>(
    functions: &'a [EconomicFunction],
    source: &EconomicFunction,
    operation: EconomicOperation,
) -> Option<&'a EconomicFunction> {
    let mut matches = functions
        .iter()
        .filter(|item| item.contract == source.contract && item.operation.value == operation);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn counterpart(operation: EconomicOperation) -> Option<EconomicOperation> {
    match operation {
        EconomicOperation::Deposit => Some(EconomicOperation::PreviewDeposit),
        EconomicOperation::Mint => Some(EconomicOperation::PreviewMint),
        EconomicOperation::Withdraw => Some(EconomicOperation::PreviewWithdraw),
        EconomicOperation::Redeem => Some(EconomicOperation::PreviewRedeem),
        _ => None,
    }
}

fn opposite(operation: EconomicOperation) -> Option<EconomicOperation> {
    match operation {
        EconomicOperation::Deposit => Some(EconomicOperation::Redeem),
        EconomicOperation::Mint => Some(EconomicOperation::Withdraw),
        _ => None,
    }
}

fn accounting_writes(function: &EconomicFunction) -> BTreeSet<String> {
    function
        .writes
        .iter()
        .filter_map(|item| {
            name_contains(&item.value.0, &["asset", "share", "supply", "balance"])
                .then(|| item.value.0.to_ascii_lowercase())
        })
        .collect()
}

fn scale_exponents(function: &EconomicFunction) -> BTreeMap<u32, EvidenceLocation> {
    let mut found = BTreeMap::new();
    for item in &function.arithmetic {
        let compact: String = item
            .value
            .0
            .chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect();
        for marker in ["1e", "10**"] {
            for (offset, _) in compact.match_indices(marker) {
                let digits: String = compact[offset + marker.len()..]
                    .chars()
                    .take_while(|ch| ch.is_ascii_digit())
                    .collect();
                if let Ok(exponent) = digits.parse::<u32>() {
                    if (1..=36).contains(&exponent) {
                        found.insert(exponent, item.location.clone());
                    }
                }
            }
        }
    }
    found
}

fn deterministic_rules(functions: &[EconomicFunction], collector: &mut Collector) {
    for function in functions {
        let operation = function.operation.value;
        if let Some(preview_operation) = counterpart(operation) {
            if let Some(preview) = matching(functions, function, preview_operation) {
                let execution_fee = fee_evidence(function);
                let preview_fee = fee_evidence(preview);
                if execution_fee.is_empty() != preview_fee.is_empty() {
                    let mut evidence = vec![location(function), location(preview)];
                    evidence.extend(execution_fee);
                    evidence.extend(preview_fee);
                    collector.add(function, FindingKind::FeeBypass, "fee-preview-asymmetry",
                        "Fee-related evidence appears on only one side of a preview/execution pair",
                        "Fee handling may be delegated to an unmodeled internal call; compare actual amounts and fee policy",
                        evidence);
                }
                if let (Some((a, a_loc)), Some((b, b_loc))) =
                    (rounding_hint(function), rounding_hint(preview))
                {
                    if a != b {
                        collector.add(function, FindingKind::RoundingAsymmetry, "rounding-preview-asymmetry",
                            "Preview and execution use different explicit rounding directions",
                            "Different rounding may be intentional; verify the operation's exact ERC-4626 and fee semantics",
                            vec![a_loc, b_loc]);
                    }
                }
                if let Some((execution_loc, preview_loc)) =
                    formula_rounding_difference(function, preview)
                {
                    collector.add(function, FindingKind::RoundingAsymmetry, "rounding-preview-formula-difference",
                        "Preview and execution contain matching numerator/denominator divisions with different rounding formulas",
                        "The preview may have another branch with matching rounding; check which branch is reachable and compare actual amounts",
                        vec![execution_loc, preview_loc]);
                }
                let execution_oracle = oracle_targets(function);
                let preview_oracle = oracle_targets(preview);
                if !execution_oracle.is_empty()
                    && !preview_oracle.is_empty()
                    && execution_oracle != preview_oracle
                {
                    let mut evidence = vec![location(function), location(preview)];
                    evidence.extend(
                        function
                            .oracle_dependencies
                            .iter()
                            .map(|item| item.location.clone()),
                    );
                    evidence.extend(
                        preview
                            .oracle_dependencies
                            .iter()
                            .map(|item| item.location.clone()),
                    );
                    collector.add(
                        function,
                        FindingKind::OracleFallbackMismatch,
                        "oracle-preview-source-asymmetry",
                        "Preview and execution reference different oracle targets",
                        "The targets may be equivalent wrappers or deliberate fallback paths",
                        evidence,
                    );
                }
                let execution_conversion = oracle_conversion(function);
                let preview_conversion = oracle_conversion(preview);
                if execution_conversion.is_empty() != preview_conversion.is_empty() {
                    let mut evidence = vec![location(function), location(preview)];
                    evidence.extend(execution_conversion);
                    evidence.extend(preview_conversion);
                    collector.add(function, FindingKind::PreviewExecutionMismatch, "oracle-preview-value-asymmetry",
                        "Oracle value reaches a conversion or accounting sink on only one side of a preview/execution pair",
                        "Source value flow is conservative; check whether internal helpers make both sides equivalent",
                        evidence);
                }
            }
        }
        if let Some(opposite_operation) = opposite(operation) {
            if let Some(other) = matching(functions, function, opposite_operation) {
                let left = oracle_conversion(function);
                let right = oracle_conversion(other);
                if left.is_empty() != right.is_empty() {
                    let mut evidence = vec![location(function), location(other)];
                    evidence.extend(left);
                    evidence.extend(right);
                    collector.add(function, FindingKind::PriceSourceAsymmetry, "opposite-price-source-asymmetry",
                        "One side of an asset/share entry-exit pair depends on an oracle value while the other does not",
                        "The two operations may use different but equivalent valuation routes",
                        evidence);
                }
                let a = accounting_writes(function);
                let b = accounting_writes(other);
                if !a.is_empty() && !b.is_empty() && a != b {
                    let mut evidence = vec![location(function), location(other)];
                    evidence.extend(function.writes.iter().map(|item| item.location.clone()));
                    evidence.extend(other.writes.iter().map(|item| item.location.clone()));
                    collector.add(function, FindingKind::AccountingDrift, "entry-exit-accounting-asymmetry",
                        "Entry and exit operations write different asset/share accounting fields",
                        "Equivalent accounting may occur in inherited or internal functions outside this source slice",
                        evidence);
                }
            }
        }
        if matches!(
            operation,
            EconomicOperation::Deposit
                | EconomicOperation::Mint
                | EconomicOperation::Withdraw
                | EconomicOperation::Redeem
        ) {
            let transfers = calls_named(function, &["transferfrom", ".transfer", "safetransfer"]);
            let share_updates = write_named(function, &["share", "supply", "balance"]);
            let mint_or_burn = related_named(function, &["_mint", "_burn", "mint(", "burn("]);
            if !transfers.is_empty() && share_updates.is_empty() && !mint_or_burn {
                let mut evidence = vec![location(function)];
                evidence.extend(transfers);
                collector.add(function, FindingKind::AccountingDrift, "asset-share-conservation-gap",
                    "An asset transfer is observed without a corresponding modeled share/supply update",
                    "The update may occur in an inherited contract or unsupported call path; this is not a conservation proof",
                    evidence);
            }
        }
        let conversion = oracle_conversion(function);
        if !conversion.is_empty() && !oracle_guard(function) {
            let mut evidence = conversion;
            evidence.extend(
                function
                    .oracle_dependencies
                    .iter()
                    .map(|item| item.location.clone()),
            );
            collector.add(function, FindingKind::OracleFallbackMismatch, "oracle-value-without-modeled-guard",
                "Oracle-derived value reaches conversion/accounting without a modeled guard on that value",
                "Validation may be performed inside the oracle, in an inherited helper, or outside the modeled source path",
                evidence);
        }
    }

    for function in functions
        .iter()
        .filter(|item| item.operation.value == EconomicOperation::ConvertToShares)
    {
        let formula: Vec<_> = function
            .arithmetic
            .iter()
            .filter(|item| {
                let expression = item.value.0.to_ascii_lowercase();
                expression.contains("totalsupply")
                    && expression.contains("totalassets")
                    && !expression.contains("virtual")
                    && !expression.contains("+ 1")
                    && !expression.contains("+1")
            })
            .map(|item| item.location.clone())
            .collect();
        let zero_supply = function.control_conditions.iter().any(|item| {
            let compact: String = item
                .value
                .chars()
                .filter(|ch| !ch.is_ascii_whitespace())
                .collect();
            name_contains(&compact, &["totalsupply"])
                && (compact.contains("==0") || compact.contains("<=0"))
        });
        let balance = matching(functions, function, EconomicOperation::TotalAssets)
            .map(|total_assets| calls_named(total_assets, &["balanceof"]))
            .unwrap_or_default();
        if !formula.is_empty() && zero_supply && !balance.is_empty() {
            let mut evidence = formula;
            evidence.extend(balance);
            evidence.extend(
                function
                    .control_conditions
                    .iter()
                    .filter(|item| name_contains(&item.value, &["totalsupply"]))
                    .map(|item| item.location.clone()),
            );
            collector.add(function, FindingKind::ShareInflation, "first-share-donation-sensitivity",
                "Share conversion uses supply/assets with a zero-supply branch while totalAssets reads a token balance",
                "Donation must be possible and virtual offsets/minimum liquidity absent before this becomes an exploitable inflation issue",
                evidence);
        }
    }

    for function in functions.iter().filter(|item| {
        matches!(
            item.operation.value,
            EconomicOperation::ConvertToShares | EconomicOperation::ConvertToAssets
        )
    }) {
        let counterpart_operation =
            if function.operation.value == EconomicOperation::ConvertToShares {
                EconomicOperation::ConvertToAssets
            } else {
                continue;
            };
        if let Some(other) = matching(functions, function, counterpart_operation) {
            let a = scale_exponents(function);
            let b = scale_exponents(other);
            if !a.is_empty()
                && !b.is_empty()
                && a.keys().collect::<Vec<_>>() != b.keys().collect::<Vec<_>>()
            {
                let evidence = a.values().chain(b.values()).cloned().collect();
                collector.add(function, FindingKind::DecimalMismatch, "conversion-scale-asymmetry",
                    "Asset-to-share and share-to-asset conversions use different literal decimal scales",
                    "Different token decimals may require distinct scales; validate units and reciprocal conversion algebra",
                    evidence);
            }
        }
    }
}

fn slither_kind(check: &str) -> Option<FindingKind> {
    match check {
        "divide-before-multiply" => Some(FindingKind::RoundingAsymmetry),
        "unchecked-transfer" | "arbitrary-send-erc20" => Some(FindingKind::AccountingDrift),
        _ => None,
    }
}

fn slither_span(element: &Value, source_name: &str, source_len: usize) -> Option<SourceSpan> {
    let map = element.get("source_mapping")?;
    let file = map
        .get("filename_relative")
        .and_then(Value::as_str)
        .or_else(|| map.get("filename_short").and_then(Value::as_str))
        .or_else(|| map.get("filename_used").and_then(Value::as_str))?;
    if file.rsplit('/').next()? != source_name.rsplit('/').next()? {
        return None;
    }
    let start = usize::try_from(map.get("start")?.as_u64()?).ok()?;
    let length = usize::try_from(map.get("length")?.as_u64()?).ok()?;
    let end = start.checked_add(length)?;
    if end > source_len {
        return None;
    }
    SourceSpan::new(source_name, start, end)
}

fn function_parent(element: &Value) -> Option<(String, String)> {
    let function = if element.get("type").and_then(Value::as_str) == Some("function") {
        element
    } else {
        element.pointer("/type_specific_fields/parent")?
    };
    let name = function
        .get("name")?
        .as_str()?
        .split('(')
        .next()?
        .rsplit('.')
        .next()?
        .to_owned();
    let contract = function
        .pointer("/type_specific_fields/parent/name")?
        .as_str()?
        .to_owned();
    Some((contract, name))
}

fn map_slither(
    functions: &[EconomicFunction],
    source_name: &str,
    source_len: usize,
    json: &str,
    collector: &mut Collector,
) -> Result<(Vec<SlitherSignal>, usize), String> {
    let output: Value =
        serde_json::from_str(json).map_err(|e| format!("invalid Slither JSON: {e}"))?;
    if output.get("success").and_then(Value::as_bool) != Some(true) {
        return Err("Slither JSON does not report success=true".into());
    }
    let detectors = output
        .pointer("/results/detectors")
        .and_then(Value::as_array)
        .ok_or("Slither JSON has no results.detectors array")?;
    let mut signals = Vec::new();
    let mut mapped = 0;
    for detector in detectors {
        let check = detector.get("check").and_then(Value::as_str).unwrap_or("");
        let kind = slither_kind(check);
        let elements = detector.get("elements").and_then(Value::as_array);
        let primary = elements.and_then(|items| items.first());
        let span = primary.and_then(|item| slither_span(item, source_name, source_len));
        let parent = elements.and_then(|items| items.iter().find_map(function_parent));
        let target = parent.and_then(|(contract, name)| {
            let mut matching = functions.iter().filter(|item| {
                item.contract.0 == contract
                    && item.function.0.split('(').next() == Some(name.as_str())
            });
            let first = matching.next()?;
            matching.next().is_none().then_some(first)
        });
        let was_mapped = if let (Some(kind), Some(function), Some(span)) =
            (kind, target, span.as_ref())
        {
            collector.add(function, kind, &format!("slither:{check}"),
                "Slither emitted a source-mapped signal in this economic category",
                "Slither impact/confidence describes its detector output, not a confirmed economic exploit",
                vec![EvidenceLocation::Source(span.clone())]);
            true
        } else {
            false
        };
        if was_mapped {
            mapped += 1;
        }
        signals.push(SlitherSignal {
            check: check.to_owned(),
            impact: detector
                .get("impact")
                .and_then(Value::as_str)
                .unwrap_or("Unknown")
                .to_owned(),
            confidence: detector
                .get("confidence")
                .and_then(Value::as_str)
                .unwrap_or("Unknown")
                .to_owned(),
            mapped_to_candidate: was_mapped,
            location: span,
        });
    }
    Ok((signals, mapped))
}

/// Analyze source IR. Optional Slither JSON is imported only for detector IDs
/// with an explicit economic mapping and an unambiguous function + source span.
pub fn analyze_economic_functions(
    functions: &[EconomicFunction],
    source_name: &str,
    source: &str,
    slither_json: Option<&str>,
) -> Result<EconomicAnalysisReport, String> {
    let mut collector = Collector::new();
    deterministic_rules(functions, &mut collector);
    let raw_rule_signals = collector.raw;
    let (slither_signals, mapped) = match slither_json {
        Some(json) => map_slither(functions, source_name, source.len(), json, &mut collector)?,
        None => (Vec::new(), 0),
    };
    let imported_slither_signals = slither_signals.len();
    let candidates: Vec<_> = collector.candidates.into_values().collect();
    let count = candidates.len();
    let mut unresolved: Vec<_> = functions
        .iter()
        .flat_map(|function| function.unresolved.iter().cloned())
        .collect();
    if slither_json.is_none() {
        unresolved.push(Uncertainty {
            kind: UncertaintyKind::MissingInput,
            detail: "Slither JSON was not supplied; complementary signals were not imported".into(),
            location: None,
        });
    }
    let mut operations = BTreeMap::<(String, String), usize>::new();
    for function in functions {
        *operations
            .entry((
                function.contract.0.clone(),
                format!("{:?}", function.operation.value),
            ))
            .or_default() += 1;
    }
    for ((contract, operation), count) in operations {
        if count > 1 && operation != "Other" {
            unresolved.push(Uncertainty {
                kind: UncertaintyKind::AmbiguousSemantics,
                detail: format!("{contract} has {count} functions classified as {operation}; pairwise rules were omitted for this operation"),
                location: None,
            });
        }
    }
    Ok(EconomicAnalysisReport {
        candidates,
        slither_signals,
        metrics: EconomicAnalysisMetrics {
            source_functions: functions.len(),
            raw_rule_signals,
            imported_slither_signals,
            mapped_slither_signals: mapped,
            candidates_after_deduplication: count,
            signals_removed_by_deduplication: raw_rule_signals + mapped - count,
            false_positives_reviewed: None,
        },
        unresolved,
    })
}
