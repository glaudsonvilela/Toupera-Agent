# Economic IR — M1

The Rust `EconomicFunction` record is a compact description of one vault or oracle function. It is the intended boundary between future source/bytecode reconstruction, deterministic detectors, context slicing, and local test generation.

## Record

- **Identity:** `ContractId` and `FunctionId`; `BasicBlockId` names source graph nodes now and can later identify EVM blocks.
- **Operation:** `EconomicOperation` (deposit, mint, withdraw, redeem, previews, conversions, totals, oracle price, or other).
- **Dependencies:** storage reads/writes, external calls, oracle dependencies, arithmetic expressions, modifiers, and related functions. Compiler-resolved calls also identify their declaring contract across same-file inheritance.
- **Evidence:** each observation has a `SourceSpan` or a `BytecodePc` and a confidence level.
- **Unresolved:** `Uncertainty` records ambiguous semantics or missing input, optionally tied to a location.
- **Control flow:** `ControlEdge` links source graph nodes with `Next`, `True`, `False`, `LoopBack`, or `Exit` and a source location.
- **Local modifiers:** `AppliedModifier` keeps the invocation and definition spans, parameter-to-argument bindings, and direct reads, writes, calls, arithmetic, and conditions observed in the modifier body. These observations are also included in the function-level dependency lists.
- **Internal call reachability:** `ReachabilityPath` records each call-site span and a terminal storage read/write, oracle call, or conversion. It is call reachability, not proof that a value propagates through the chain.
- **Value dependence:** `ValueFlow` links a parameter, storage read, oracle call, or external call to a guard, return, storage write, or conversion argument. It retains source spans, contributing call sites, and `Inferred` confidence. A path is a conservative static inference, not a vulnerability verdict.
- **Proxy relation:** `ProxyRelation` records a locally created fixed implementation, its constructor binding, and the delegatecall receiver. Unknown targets remain tentative in `delegatecall_targets`.

`SourceSpan` uses UTF-8 byte offsets with an exclusive end and a path relative to the analyzed source root. Its `slice` method checks bounds and character boundaries against the supplied source text. A bytecode location identifies an artifact and program counter; bytecode ingestion itself is not implemented.

`Confidence::Direct` means the represented syntax or event was directly observed. `Inferred` and `Tentative` describe progressively weaker semantic interpretations. These levels do not confirm a vulnerability. Unknown behavior belongs in `unresolved`, never in a fabricated observation.

## Milestone state

M1 established this data model with a manually assembled vault fixture. The completed M2 source slice fills it from solc AST for a single source file, adds a partial source control graph, preserves declaring contracts across inheritance, and attaches same-file modifier effects, including inherited definitions; see [Source Reconstruction](SOURCE-RECONSTRUCTION.md). Local modifiers with exactly one `_` are composed into the function graph. Others remain explicitly unresolved. It does not recover bytecode or detect vulnerabilities.
