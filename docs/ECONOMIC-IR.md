# Economic IR — M1

The Rust `EconomicFunction` record is a compact description of one vault or oracle function. It is the intended boundary between future source/bytecode reconstruction, deterministic detectors, context slicing, and local test generation.

## Record

- **Identity:** `ContractId` and `FunctionId`; `BasicBlockId` is reserved for future control-flow work.
- **Operation:** `EconomicOperation` (deposit, mint, withdraw, redeem, previews, conversions, totals, oracle price, or other).
- **Dependencies:** storage reads/writes, external calls, oracle dependencies, arithmetic expressions, modifiers, and related functions.
- **Evidence:** each observation has a `SourceSpan` or a `BytecodePc` and a confidence level.
- **Unresolved:** `Uncertainty` records ambiguous semantics or missing input, optionally tied to a location.

`SourceSpan` uses UTF-8 byte offsets with an exclusive end and a path relative to the analyzed source root. Its `slice` method checks bounds and character boundaries against the supplied source text. A bytecode location identifies an artifact and program counter; bytecode ingestion itself is not implemented.

`Confidence::Direct` means the represented syntax or event was directly observed. `Inferred` and `Tentative` describe progressively weaker semantic interpretations. These levels do not confirm a vulnerability. Unknown behavior belongs in `unresolved`, never in a fabricated observation.

## M1 boundary

The fixture at `tests/fixtures/VaultWithOracle.sol` is a small accounting example. The Rust integration test assembles its IR manually and verifies that source locations resolve to the claimed code. M1 does not parse solc AST, recover bytecode, classify functions automatically, or detect vulnerabilities. Those capabilities start in later milestones.
