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

## Status

M2 and M3 are complete within documented slices. M2 reconstructs one Solidity file without imports through local solc AST, including modifier composition, source control/call flow, conservative value tracing, and source-evidenced fixed proxy relations. M3 decodes explicitly supplied legacy runtime bytecode, recovers direct selector dispatch and known CFG edges, traces bounded stack and word-memory dependencies to economic sinks, and compares a local compiler runtime when available. EVMole reports additional selector, ABI, storage, and CFG candidates separately. Optional Heimdall ABI and Gigahorse TAC imports add independent selector and block evidence with tentative confidence. Unknown jumps, partial memory, unknown ABI semantics, EOF, and creation code remain unresolved or outside the slice. Neither milestone confirms vulnerabilities. See [Source Reconstruction](docs/SOURCE-RECONSTRUCTION.md), [Bytecode Reconstruction](docs/BYTECODE-RECONSTRUCTION.md), [Economic IR](docs/ECONOMIC-IR.md), and [agente.md](agente.md).

## Development

```sh
cargo test -p security-engine
```
