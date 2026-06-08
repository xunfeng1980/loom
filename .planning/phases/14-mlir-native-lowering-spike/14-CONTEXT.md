# Phase 14 Context: MLIR/Native Lowering Spike

**Status:** Context captured
**Date:** 2026-06-08
**Inputs:** `14-RESEARCH.md`, Phase 13 verifier report/spec, local `L2Core`
model and full verifier

## Phase Intent

Phase 14 begins the native-code path for Loom, but only as a constrained spike.
The phase should prove that an accepted `L2Core` artifact can be lowered into a
native-oriented representation without weakening the verifier boundary.

The goal is not to replace the interpreter and not to build a production MLIR
compiler. The goal is a small, reviewable, fail-closed lowering chain from the
Phase 13 bounded-copy sample to deterministic textual MLIR and optional local
toolchain validation.

## Locked Research Decisions

### D-14-01: Lowering is post-verifier only

Every lowering entry point must require an accepted `verify_l2_core` report and
present `VerifiedArtifactFacts`. Standalone facts are not enough because facts
are verifier-tied evidence, not independent trust tokens.

### D-14-02: Start with textual MLIR

The first implementation should emit deterministic textual MLIR using standard
dialects. This avoids making system MLIR, LLVM, `melior`, or Cranelift mandatory
workspace dependencies before the lowering contract is proven.

### D-14-03: Initial supported subset is tiny

The first lowerable program shape is:

- one finite constant-bounded `ForRange`,
- one input slice,
- one `Int32` output builder,
- `ReadInput` plus `AppendValue`,
- no `CursorLoop`,
- no `AppendNull`,
- no strings/floats/booleans,
- no direct Arrow buffer construction.

Unsupported accepted programs must reject with stable lowering diagnostics.

### D-14-04: Preserve Arrow builder semantics

Generated/native code must not directly construct Arrow arrays in the spike.
It should target a typed primitive buffer or a tiny runtime ABI, with Rust
remaining responsible for Arrow construction/checking.

### D-14-05: Native/JIT execution is optional evidence

`mlir-opt`, `mlir-translate`, MLIR ExecutionEngine, LLVM ORC, or Rust bindings
may be used as optional probes if present. Their absence should be reported as
skipped optional evidence, not as a normal release-gate failure.

## Required Phase Outputs

- A lowering contract and support predicate for `L2Core`.
- Stable lowering diagnostics for verifier rejection, missing facts, unsupported
  statements, unsupported types, unsupported nullability, unsupported loop
  shape, and unsupported capability shape.
- A deterministic textual MLIR artifact for the bounded Int32 copy sample.
- Tests that lowering requires verifier acceptance and rejects unsupported
  accepted programs.
- A reference/equivalence check for the supported sample.
- Optional MLIR/toolchain validation script or closeout evidence.

## Non-Goals

- No production MLIR pass pipeline.
- No custom Loom MLIR dialect.
- No required `melior`, LLVM, or Cranelift dependency.
- No direct generated Arrow raw-buffer mutation.
- No vectorization.
- No FSST/ALP/dict/RLE/native kernel lowering.
- No multi-column native lowering.
- No formal compiler correctness proof in this phase.

## Planning Recommendation

Plan Phase 14 as four sequential plans:

1. `14-01` Lowering contract and support predicate.
2. `14-02` Textual MLIR emission for bounded Int32 copy.
3. `14-03` Reference/equivalence gate plus optional `mlir-opt` validation.
4. `14-04` Final lowering report, public/planning docs, release-gate wiring,
   and closeout evidence.

The first two plans are load-bearing. Optional native/JIT execution is deferred
beyond Phase 14; Phase 14's optional toolchain evidence is textual MLIR
validation through `mlir-opt` when available.
