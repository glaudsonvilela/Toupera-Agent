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

Authorized target → scope validation → Solidity source → AST/call graph/state graph → deterministic Rust analysis → Slither → candidate reduction → context slicing → LLM reasoning → economic hypothesis → Foundry test generation → local execution/fork → reproduced invariant → evidence package → human review → report.

The LLM should receive only a small relevant slice of code and state context, never the full repository by default. Foundry is the judge: a hypothesis advances only after reproducible validation.

## Status

M0 — Repository/Foundation. The Rust `security-engine` crate currently provides foundational IDs, economic operation and finding types; analysis is not implemented yet. See [agente.md](agente.md) for operational state and next steps.

## Development

```sh
cargo test -p security-engine
```
