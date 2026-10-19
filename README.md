<p align="center">
  <img src="assets/toupera-agent.png" alt="Toupera Agent — autonomous EVM DeFi economic security auditor" width="100%">
</p>

# Toupera Agent

**Autonomous EVM DeFi Economic Security Auditor**

Toupera Agent is a focused system for identifying, validating, and documenting economic inconsistencies in Solidity/EVM contracts within owned environments, local forks, and explicitly authorized security programs. Version 1 deliberately does not attempt to audit everything.

## V1 focus

```text
Solidity + EVM
        ↓
Vaults + Oracles
        ↓
Economic invariants
        ↓
Local reproducible validation
```

Initial analysis centers on vault accounting, oracle pricing, shares and assets, ERC-4626-style deposit/mint/withdraw/redeem flows, fees, conversions, and rounding. See [Economic Scope](docs/ECONOMIC-SCOPE.md) and [Security Scope](docs/SECURITY-SCOPE.md).

> High signal. Low token usage. Deterministic filtering first. LLM only where reasoning adds value.

## Architecture

Authorized target → scope validation → source and/or EVM bytecode → contract reconstruction → deterministic Rust analysis + Slither → candidate reduction + available local invariants → context slicing → LLM reasoning for unresolved questions → economic hypothesis → targeted Foundry reproduction → evidence package → human review → report.

The Rust engine reconstructs a partial economic model from source and a bounded legacy EVM runtime slice: relevant entry points, control and call flows, storage dependencies, and value paths. Source analysis can label oracle and share/asset operations when the source supports them. Bytecode-only paths retain generic labels and explicit uncertainty. The LLM should receive only a small relevant slice of code and state context, never the full repository by default. Foundry is the judge: a hypothesis advances only after reproducible validation.

See the [build and audit checklist](docs/ROADMAP-CHECKLIST.md) for the path from repository foundation through evidence review and final report.
The [tooling strategy](docs/TOOLING-STRATEGY.md) compares static analyzers, fuzzers, and formal tools for this economic scope and explains when they can reduce LLM usage.
The [VPS setup](docs/VPS-SETUP.md) records the verified development server, installed tools, and current deployment limits.
The [forensic quarantine cycle](docs/FORENSIC-QUARANTINE.md) maps weak points, audits selected variables in a second pass, and rebuilds priorities before any candidate can reach M5.

## Status

M2 and M3 are complete within documented slices. M2 reconstructs one Solidity file without imports through local solc AST, including modifier composition, source control/call flow, conservative value tracing, and source-evidenced fixed proxy relations. M3 decodes explicitly supplied legacy runtime bytecode, recovers direct selector dispatch and known CFG edges, traces bounded stack and word-memory dependencies to economic sinks, and compares a local compiler runtime when available. EVMole reports additional selector, ABI, storage, and CFG candidates separately. Optional Heimdall ABI and Gigahorse TAC imports add independent selector and block evidence with tentative confidence. Unknown jumps, partial memory, unknown ABI semantics, EOF, and creation code remain unresolved or outside the slice. Neither milestone confirms vulnerabilities. See [Source Reconstruction](docs/SOURCE-RECONSTRUCTION.md), [Bytecode Reconstruction](docs/BYTECODE-RECONSTRUCTION.md), [Economic IR](docs/ECONOMIC-IR.md), and [agente.md](agente.md).

M4 now emits source-backed economic risk candidates for accounting, previews, rounding, fees, decimals, first-share donation sensitivity, and oracle paths. Local Slither JSON can add mapped signals; candidates remain `UNVERIFIED` and are deduplicated before later AI review. Fixture calibration is deferred until implementation is complete. See [Economic Analysis](docs/ECONOMIC-ANALYSIS.md).

M5 builds bounded, local context slices for those candidates. SHA-256 fingerprints of source, AST, functions, compiler, engine, and optional normalized runtime/Slither input drive a private content cache, so unchanged inputs skip repeated analysis. The CLI does not send code to an LLM. See [Context Package](docs/CONTEXT-PACKAGE.md).

M6 prepares one structured review request per eligible, deduplicated slice. A provider interface and an opt-in DeepSeek adapter require a JSON hypothesis with assumptions, relevant paths, and a suggested local test; the candidate remains `UNVERIFIED`. Prompt response caching and provider token usage make repeat calls and spend visible. No API key or live model call was used during implementation. See [LLM Review](docs/LLM-REVIEW.md).

M7 prepares private Foundry cases from M5/M6 output with exact source digests. Its local judge requires a target-specific, reviewed harness and two consistent runs before proposing `REPRODUCED_PENDING_REVIEW`. A separately reviewed negative reproduction can be recorded as `REJECTED` only under documented premises. The scaffold deliberately fails until completed. A repository-owned synthetic [M7 pilot](docs/M7-PILOT.md) exercised reproduction, rejection, fuzz, coverage and Halmos; no real target was audited. See [Foundry Judge](docs/FOUNDRY-JUDGE.md).

M8 snapshots a case into a private evidence bundle. A report can reach `REPORT_READY` only after a positive M7 result, artifact integrity checks and explicit human review of scope, assertions, impact and duplicates. No real report was generated during implementation. See [Evidence Delivery](docs/EVIDENCE-DELIVERY.md).

The M7 judge records observed fuzz runs and target LCOV coverage; optional invariant handler calls/reverts remain separate from verified economic calls to the target. The synthetic pilot measured these outputs. The M4 candidate selector did not flag that fixture, so the pilot cases were prepared manually and the full M4–M7 path remains unvalidated.

A [real Sepolia pilot](docs/REAL-EVM-PILOT.md) connected verified multi-file source and deployed runtime for an ERC-4626 vault through M1–M6. The compiler's two constructor immutables explain the runtime differences; M4 found no eligible candidate, so M7 correctly created no case and M8 had no evidence to package. This validates the empty path on a real EVM target, while the positive candidate path still needs calibration and an authorized case.

The Rust forensic cycle now runs automatically from one manifest. It verifies a fixed-block runtime, scores source/bytecode hotspots, scans vault and asset event history from the detected deployment block when the RPC permits, reconciles 256-bit ledger values with selected receipts and historical state, quarantines selected variables for deeper crosschecks and read-only boundary probes, then rebuilds the map. Coverage and missing RPC traces are explicit. Only observed anomalies tied to the same source and runtime may become `UNVERIFIED` M5 candidates. The Sepolia read-only run reconciled the available event history and found no observed anomaly in those checks; an autonomous VPS job then locally replayed three published transactions and generated traces without broadcasting transactions. This is not a security clearance. New operation sequences on a fork and path coverage remain pending.

## Development

```sh
cargo test -p security-engine
```
