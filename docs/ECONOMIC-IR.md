# Economic IR — M1

The Rust `EconomicFunction` record is a compact description of one vault or oracle function. It is the intended boundary between future source/bytecode reconstruction, deterministic detectors, context slicing, and local test generation.

## Record

- **Identity:** `ContractId` and `FunctionId`; `BasicBlockId` names source graph nodes now and can later identify EVM blocks.
- **Operation:** `EconomicOperation` (deposit, mint, withdraw, redeem, previews, conversions, totals, oracle price, or other).
- **Dependencies:** storage reads/writes, external calls, oracle dependencies, arithmetic expressions, modifiers, and related functions.
- **Evidence:** each observation has a `SourceSpan` or a `BytecodePc` and a confidence level.
- **Unresolved:** `Uncertainty` records ambiguous semantics or missing input, optionally tied to a location.
- **Control flow:** `ControlEdge` links source graph nodes with `Next`, `True`, `False`, `LoopBack`, or `Exit` and a source location.
- **Local modifiers:** `AppliedModifier` keeps the invocation and definition spans plus direct reads, writes, calls, arithmetic, and conditions observed in the modifier body. These observations are also included in the function-level dependency lists.

`SourceSpan` uses UTF-8 byte offsets with an exclusive end and a path relative to the analyzed source root. Its `slice` method checks bounds and character boundaries against the supplied source text. A bytecode location identifies an artifact and program counter; bytecode ingestion itself is not implemented.

`Confidence::Direct` means the represented syntax or event was directly observed. `Inferred` and `Tentative` describe progressively weaker semantic interpretations. These levels do not confirm a vulnerability. Unknown behavior belongs in `unresolved`, never in a fabricated observation.

## Milestone state

M1 established this data model with a manually assembled vault fixture. The M2 extractor now fills it from solc AST for a single source file, adds a partial source control graph, and attaches locally resolved modifier effects; see [Source Reconstruction](SOURCE-RECONSTRUCTION.md). Modifier effects describe observed syntax, while function execution order remains unresolved until modifier placeholders are spliced into the graph. It does not recover bytecode or detect vulnerabilities.
