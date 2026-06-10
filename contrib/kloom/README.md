# kloom — K Framework Semantics for L2Core

Independent executable semantics for Loom's L2Core language, serving as a
**spec-oracle** in the differential validation architecture.

## Position in Loom Architecture

```
Production path (default): native (MLIR/LLVM/JIT) — fast, unchanged
                                │  emits builder-event trace via TracedOutputBuilder
           ┌─────────────────────┼─────────────────────┐  differential validation
           ▼                     ▼                     ▼
Spec oracle: kloom (K)         Impl oracle: native output
(krun executable)              (TracedOutputBuilder)
```

- **kloom** is the **specification baseline**. It defines L2Core semantics via K
  rewriting rules — the semantics *is* the interpreter.
- **Native** is the **system under test**. Its output is compared against kloom's
  reference trace per builder event (mirror reconciliation).
- Any divergence → fail-closed for that run, and the divergent shape's native
  route is disabled for the process lifetime (interpreter fallback).

## Scope (v4)

kloom v4 covers the full L2Core surface exercised by the verifier and the
native codegen path:

- **Types**: `int32`, `int64`, `float32`, `float64`, `bool`
- **Capabilities**: `input` columns, `output` builders (with nullable flag)
- **Statements**:
  - `appendValue(builder, scalarExpr)`
  - `appendNull(builder)`
  - `readInput(capability, offset, width, bind)`
  - `letScalar(name, expr)`
  - `forRange(index, start, end, body)`
  - `cursorLoop(cursor, limit, progress, body)`
  - `failClosed(code)`
- **Expressions**: constants, variables, `add`, `sub`, `mul`, `eq`, `lt`, `le`
- **Program**: `program <caps> body <stmts> maxRows <n>`
- **Output**: builder-event trace (`append-value:builder:type`, `append-null:builder:type`,
  `terminal:finished`)

## K-Oracle Outcome Taxonomy (Phase 48)

The Rust harness that invokes krun returns one of four typed outcomes:

| Outcome | Meaning | Route impact |
|---------|---------|--------------|
| `ProducedTrace` | krun ran and emitted a reference trace | Compared against native trace |
| `SkippedRefereeAbsent` | krun/kompile missing or timed out | Recorded skip; route proceeds |
| `UnsupportedProgram` | Program contains unmodelled constructs (Min/Max/Bytes) | Recorded skip; route proceeds |
| Hard error (garbled / non-zero exit) | K present but output unusable | Fail-closed |

- `SkippedRefereeAbsent` and `UnsupportedProgram` do **not** disable the native
  route; only a genuine trace divergence does.
- The `LOOM_ALLOW_K_ORACLE_SKIP=1` environment variable enables skip tolerance
  for local development without K installed. CI gates run strict (no skip).

## Per-Shape Native-Route Disable

On a native↔K trace divergence, the shape's `schema_fingerprint` is recorded in
an in-process registry. Subsequent route requests for that shape short-circuit
before JIT execution and before any krun invocation, falling back to the
interpreter (or fail-closed per policy). Disabled shapes are never admitted to
the runtime cache or replay evidence.

## Trust Model

kloom is **outside the production TCB**. It runs in CI / offline only.
Its value is **statistical independence** — bugs in kloom and bugs in the Rust
implementation have low correlation, making simultaneous failure unlikely.

## Directory Layout

```
kloom/
├── src/
│   └── kloom.k                     # Main K definition (syntax + semantics) v4
├── tests/
│   ├── syntax/                     # Parse-only tests
│   └── semantics/                  # krun execution tests with expected traces
├── scripts/
│   ├── kloom-diff.sh               # Differential gate: K vs native (strict)
│   └── kloom-llvm-feasibility.sh   # LLVM-backend kompile + trace parity (skip-aware)
├── docs/
│   ├── SEMANTICS.md                # Semantic design notes and Lean alignment
│   └── LLVM-BACKEND-FEASIBILITY.md # Findings doc for LLVM backend evidence
└── README.md                       # This file
```
