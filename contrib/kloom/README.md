# kloom — K Framework Semantics for L2Core

Independent executable semantics for Loom's L2Core language, serving as a
**spec-oracle** in the differential validation architecture.

## Position in Loom Architecture

```
Production path (default): native (MLIR/LLVM/JIT) — fast, unchanged
                                │  emits builder-event trace via TracedOutputBuilder
           ┌─────────────────────┼─────────────────────┐  differential validation
           ▼                     ▼                     ▼
Spec oracle: kloom (K)    Impl oracle: Rust ref    Impl oracle: native output
(krun executable)         (l2_core_reference_executor)  (TracedOutputBuilder)
```

- **kloom** is the **specification baseline**. It defines L2Core semantics via K
  rewriting rules — the semantics *is* the interpreter.
- **Rust ReferenceExecutor** and **native** are both **implementations under test**.
- Any divergence from kloom → fail-closed (native disabled for that shape).

## Scope (v0)

Cover the **pure-append subset** that Phase 2 `checkAppendTrace` already targets:

- Types: `int32`, `int64`, `float32`, `float64`, `bool`
- Capabilities: `input` columns, `output` builders (with nullable flag)
- Statements: `appendValue(builder, scalarConst)`, `appendNull(builder)`
- Program: `program <caps> body <stmts> maxRows <n>`
- Output: builder-event trace (`append-value:builder:type`, `append-null:builder:type`,
  `terminal:finished`)

## Future Extensions

| Phase | Extension |
|-------|-----------|
| v1 | `readInput`, `letScalar` |
| v2 | `forRange` loops |
| v3 | `cursorLoop` |
| v4 | Full expression language (arithmetic, comparisons, boolean ops) |
| v5 | `failClosed` |

## Trust Model

kloom is **outside the production TCB**. It runs in CI / offline only.
Its value is **statistical independence** — bugs in kloom and bugs in the Rust
implementation have low correlation, making simultaneous failure unlikely.

## Directory Layout

```
kloom/
├── src/
│   └── kloom.k              # Main K definition (syntax + semantics)
├── tests/
│   ├── syntax/              # Parse-only tests
│   └── semantics/           # krun execution tests with expected traces
├── scripts/
│   └── kloom-diff.sh        # Differential gate: K vs Rust vs native
├── docs/
│   └── SEMANTICS.md         # Semantic design notes and Lean alignment
└── README.md                # This file
```
