# Phase 14 Lowering Contract

**Status:** Plan 14-01 contract
**Date:** 2026-06-08
**Scope:** Verifier-gated native-lowering support predicate for the first
textual MLIR spike

## Scope

Phase 14 starts Loom's native-lowering path as a deliberately narrow spike. The
first deliverable is not a production compiler and not a JIT. It is a
fail-closed support predicate that proves Loom can decide whether a verified
`L2Core` artifact is eligible for the first textual MLIR lowering slice.

The only accepted shape in plan 14-01 is the Phase 13 bounded Int32 copy sample:

```text
for i in 0..N:
  value = read input0 at i + 0 width 4
  append value to out0
```

No MLIR text is emitted in plan 14-01. Emission is intentionally deferred until
the verifier-gated support boundary is executable and tested.

## Lowering Preconditions

Lowering is a post-verifier backend. A program is lowerable only when all of the
following are true:

- `verify_l2_core(program)` has produced a `FullVerificationReport`.
- `FullVerificationReport::is_ok()` is true.
- `FullVerificationReport::facts().is_some()` is true.
- The `VerifiedArtifactFacts` are consumed as part of that report.
- The program satisfies the Phase 14 supported subset.

Standalone copied `VerifiedArtifactFacts` are not sufficient. Facts are
verifier-tied evidence, not independent trust tokens.

## Supported Subset

The first supported subset is intentionally tiny:

- one required feature: `l2core.copy.v0`;
- no optional features;
- exactly one `InputSlice` capability;
- exactly one `OutputBuilder` capability;
- no `Scratch` capability;
- output Arrow type is `Int32`;
- one top-level finite constant-bounded `ForRange`;
- loop start is `0`;
- loop iteration count equals the verifier row-count bound;
- loop body is exactly `ReadInput` followed by `AppendValue`;
- `ReadInput.width` is exactly `4`;
- `ReadInput.offset` is exactly `index + 0`;
- `AppendValue.value` is exactly the value bound by `ReadInput`;
- no `CursorLoop`;
- no `AppendNull`;
- no `FailClosed` in the lowered happy path.

## Rejected Shapes

The support predicate must reject before artifact emission when it sees:

- verifier rejection;
- missing verifier facts;
- unknown or unsupported required/optional feature;
- scratch capabilities, multiple inputs, multiple builders, or missing
  supported capabilities;
- non-`Int32` output type;
- unsupported null emission;
- cursor loops or non-constant loop bounds;
- extra statements;
- unsupported scalar expressions;
- row-count, builder-event, or capability/fact mismatches.

Unsupported accepted programs fail closed with lowering diagnostics. They do not
produce partial textual MLIR or native artifacts.

## Arrow Boundary

Phase 14 plan 14-01 does not generate code that constructs Arrow arrays or
mutates Arrow buffers.

Later plans may compare typed primitive buffers for the supported Int32 copy
slice, but Rust remains responsible for Arrow construction/checking. Direct
generated Arrow raw-buffer construction is out of scope for the spike.

## Textual MLIR Artifact

Plan 14-02 emits deterministic textual MLIR after the support predicate accepts
the program. The emitted dialect stack is deliberately standard:

- `func` for the entry function;
- `arith` for loop constants;
- `scf` for the bounded row loop;
- `memref` for typed input/output memory references.

LLVM lowering, a custom Loom dialect, MLIR pass-pipeline integration,
vectorization, and JIT/native execution remain deferred. The textual artifact is
design evidence plus regression artifact; it is not proof of production native
execution or compiler correctness.

## Diagnostics

Lowering diagnostics are stable enough for tests and reviewer-visible reports.
The initial diagnostic categories are:

- `verifier-rejected`;
- `missing-verifier-facts`;
- `unsupported-feature`;
- `unsupported-statement`;
- `unsupported-type`;
- `unsupported-nullability`;
- `unsupported-loop-shape`;
- `unsupported-capability-shape`;
- `unsupported-expression-shape`.

Each diagnostic includes a path and message. The path should identify the
program or fact location that made lowering impossible.

## Optional Toolchain Evidence

MLIR/LLVM/JIT validation is optional evidence in Phase 14. Plan 14-01 introduces
no mandatory dependency on `melior`, LLVM, Cranelift, `mlir-opt`,
`mlir-translate`, or a JIT runtime.

If later plans find local MLIR tooling, they may run additional checks. Missing
tooling must be reported as skipped optional evidence rather than treated as a
normal release-gate failure.

## Non-Goals

- No textual MLIR emission in plan 14-01.
- No production MLIR pass pipeline.
- No custom Loom MLIR dialect.
- No mandatory `melior`, LLVM, Cranelift, or JIT dependency.
- No vectorization.
- No FSST/ALP/dict/RLE/string/native kernel lowering.
- No multi-column native lowering.
- No direct generated Arrow raw-buffer writes.
- No compiler-correctness proof.
