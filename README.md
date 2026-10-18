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

The Rust engine is intended to reconstruct the contract's economic model, not only scan code: identify relevant entry points, control and call flows, storage dependencies, oracle paths, and share/asset accounting. It should support source-backed analysis first and bytecode-only reconstruction as a planned capability, recording uncertainty and evidence provenance. The LLM should receive only a small relevant slice of code and state context, never the full repository by default. Foundry is the judge: a hypothesis advances only after reproducible validation.

See the [build and audit checklist](docs/ROADMAP-CHECKLIST.md) for the path from repository foundation through evidence review and final report.
The [tooling strategy](docs/TOOLING-STRATEGY.md) compares static analyzers, fuzzers, and formal tools for this economic scope and explains when they can reduce LLM usage.

## Status

M2 — The single-file source reconstruction slice is complete. The Rust `security-engine` crate compiles one Solidity file locally with solc and extracts economic IR from its AST. It captures storage access, arithmetic, composed same-file modifiers, a partial source control graph, inherited calls, conservative value tracing, and source-evidenced proxy relations for fixed local implementations. Unknown proxy targets remain tentative. See [Source Reconstruction](docs/SOURCE-RECONSTRUCTION.md), [Economic IR](docs/ECONOMIC-IR.md), and [agente.md](agente.md) for coverage and limits.

## Development

```sh
cargo test -p security-engine
```
