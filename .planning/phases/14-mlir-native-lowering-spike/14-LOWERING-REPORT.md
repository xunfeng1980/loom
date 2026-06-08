# Phase 14 MLIR/Native Lowering Report

**Status:** Complete lowering spike
**Date:** 2026-06-08
**Gate:** `scripts/native-lowering-test.sh`

## Summary

Phase 14 completed the first verifier-gated native-lowering spike. It proves a
small but important boundary:

```text
L2CoreProgram
  -> verify_l2_core accepted report with VerifiedArtifactFacts
  -> fail-closed lowering support predicate
  -> deterministic textual MLIR artifact for bounded Int32 copy
  -> typed primitive equivalence evidence
```

This is not production native compiler completion. It does not implement a
custom Loom MLIR dialect, LLVM lowering, JIT execution, vectorization, or a
compiler-correctness proof.

## What Is Complete

### verifier-gated support predicate

`crates/loom-core/src/native_lowering.rs` adds `check_lowering_support`. It
requires:

- an accepted `FullVerificationReport`,
- present `VerifiedArtifactFacts` from that report,
- the exact `l2core.copy.v0` feature,
- one input slice,
- one Int32 output builder,
- one constant-bounded `ForRange`,
- `ReadInput` followed by `AppendValue`,
- no unsupported loops, null emission, scratch capability, extra statements, or
  unsupported expression shapes.

Unsupported accepted programs are rejected before artifact emission with stable
diagnostics.

### textual MLIR artifact

`lower_to_textual_mlir` emits deterministic textual MLIR for the supported
bounded Int32 copy slice. The artifact records:

- backend: `textual-mlir`,
- entry symbol: `loom_l2core_copy_i32`,
- MLIR text,
- verifier/facts linkage metadata,
- row-count bound.

The emitted dialect stack is standard `func`, `arith`, `scf`, and `memref`.
LLVM lowering and a custom Loom dialect are deferred.

### supported subset

The supported subset remains intentionally tiny:

```text
for i in 0..N:
  value = read input0 at i + 0 width 4
  append value to out0
```

The current supported output is a typed primitive Int32 vector for regression
evidence. Rust still owns Arrow construction. Phase 14 does not generate code
that constructs Arrow arrays or mutates Arrow raw buffers.

### rejected shapes

Focused tests cover fail-closed rejection for:

- verifier-rejected programs,
- missing verifier facts,
- supported verifier acceptance but unsupported cursor loop,
- `AppendNull`,
- non-Int32 output,
- extra scratch capability,
- unsupported expression shape,
- unsupported optional feature before MLIR emission,
- short input for the supported typed reference copy.

### equivalence evidence

`execute_supported_copy_i32` provides typed primitive equivalence evidence for
the supported slice. It is not a general `L2Core` interpreter and not a formal
compiler-correctness proof.

### optional toolchain evidence

`scripts/native-lowering-test.sh` runs focused Rust tests and checks Phase 14
planning/docs. It also probes `mlir-opt` when available.

On this machine, `mlir-opt` was not installed, so optional textual MLIR
validation was skipped explicitly. This is expected and does not fail the gate.

## Deferred

Deferred beyond Phase 14:

- production MLIR pass pipeline,
- custom Loom MLIR dialect,
- `melior`/MLIR C API integration,
- LLVM lowering,
- JIT/native execution,
- vectorization,
- generated Arrow raw-buffer construction,
- FSST/ALP/dict/RLE/string/native kernel lowering,
- multi-column native lowering,
- compiler-correctness proof,
- real Vortex file/container ingress.

## Gate Evidence

Phase 14 final verification:

```bash
cargo test --workspace
bash scripts/native-lowering-test.sh
bash scripts/full-verifier-test.sh
bash scripts/safety-proof-test.sh
bash scripts/mvp0-verify.sh
git diff --check
```

`scripts/native-lowering-test.sh` reports optional `mlir-opt` validation as
skipped when local MLIR tooling is unavailable.

## Requirement Closure

- `LOWER-01`: Complete. Lowering contract and verifier-gated support predicate
  exist.
- `LOWER-02`: Complete. Deterministic textual MLIR emission exists for bounded
  Int32 copy without mandatory MLIR/LLVM dependencies.
- `LOWER-03`: Complete. Focused typed primitive equivalence and negative tests
  exist.
- `LOWER-04`: Complete. `scripts/native-lowering-test.sh` runs focused tests and
  optional `mlir-opt` probing.
- `LOWER-05`: Complete. Public/planning docs preserve the narrow Phase 14 spike
  scope.
