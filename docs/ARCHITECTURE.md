# Architecture

## Analysis pipeline

```text
Authorized target
  → Scope validation
  → Solidity source and/or EVM bytecode
  → Contract reconstruction
  → Source AST or bytecode CFG / Call Graph / State Graph / Economic IR
  → Rust deterministic analysis
  → Slither
  → Candidate reduction
  → Context slicing
  → LLM reasoning
  → Economic hypothesis
  → Foundry test generation
  → Local execution / fork
  → Invariant reproduced?
      No  → discard or store as unverified
      Yes → validate again
  → Evidence package
  → Human review
  → Bug bounty report
```

The LLM does not receive the whole repository by default. Deterministic processing reduces the candidate set and assembles only relevant function, call, modifier, storage, arithmetic, external-call, oracle, conversion, comment, and invariant context.

## Rust contract reconstruction engine

The Rust core is planned as both an economic analyzer and a focused reverse-engineering engine. Its job is to reconstruct how an in-scope contract works before ranking possible issues. This remains limited to Solidity/EVM vault, oracle, pricing, and accounting behavior; it is not a general-purpose decompiler or broad blockchain security scanner.

The engine should accept source-backed targets and, in a later stage, bytecode-only targets. It should derive a compact model containing:

- entry points and economic operation candidates;
- control-flow and internal/external call relationships;
- storage reads/writes and their data dependencies;
- oracle inputs, fallback branches, and price-to-share/asset paths;
- arithmetic, decimal scaling, fees, and rounding behavior;
- proxy/implementation relationships when evidence is available.

For source-backed targets, solc AST and source locations provide the initial path. For bytecode-only targets, a staged EVM pipeline can disassemble and normalize bytecode, recover basic blocks/control flow and selector dispatch, then infer storage and call relationships. Inferred semantics must carry confidence and provenance (source span or bytecode program counter); ambiguous behavior stays explicitly unresolved and may be checked with local execution. Do not present decompiler guesses as recovered facts.

The compact economic IR should be the shared boundary between reconstruction, deterministic detectors, context slicing, and later Foundry test generation. Build source-first IR and provenance before adding bytecode recovery; bytecode reconstruction is a planned milestone, not part of M0.

The current M2 source-backed extractor and its limits are described in [Source Reconstruction](SOURCE-RECONSTRUCTION.md).

## Stack plan

- **Core:** Rust; Alloy for EVM-facing components when needed.
- **Solidity:** solc AST.
- **Static analysis:** Slither.
- **Validation:** Foundry; Foundry fuzzing first, Echidna later if justified.
- **Orchestration:** small Python layer.
- **LLM:** provider-agnostic interface, initially prepared for DeepSeek API.
- **Storage:** simple local storage for MVP; PostgreSQL later if needed.
- **Sandbox:** Docker when needed.

No LangChain or heavyweight framework is planned without a demonstrated need.

## Token-efficient reduction

Target shape: 10,000 functions → AST/Rust/Slither → about 500 candidates → sliced context → LLM hypotheses → Foundry reproduction attempts. Never one LLM call per function.

## Intermediate representation

A compact per-function record describes contract and function identity, storage reads/writes, external calls, economic operation, evidence locations, confidence, and unresolved facts. The M1 Rust data model is specified in [Economic IR](ECONOMIC-IR.md). This conceptual JSON example omits provenance fields for brevity; no JSON serializer or source extractor exists yet:

```json
{
  "contract": "Vault",
  "function": "redeem",
  "reads": ["totalAssets", "totalSupply"],
  "writes": ["balances"],
  "external_calls": ["oracle.latestPrice"],
  "economic_operation": "shares_to_assets"
}
```

## Deduplication and clones

Plan source, bytecode, AST, and function hashes to avoid repeat work. Known dependencies (for example OpenZeppelin, Chainlink interfaces, ERC20 base code, and SafeERC20) should be recognized and not repeatedly sent to an LLM. Later, compare forks/clones against their origin and prioritize changed code. This is a future capability, not part of M0.

## Finding lifecycle

`UNVERIFIED → REJECTED | REPRODUCED → CONFIRMED → REPORT_READY`. An LLM hypothesis is not a vulnerability verdict. Foundry/local fork reproduction and human review gate progression.
