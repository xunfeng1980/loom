# Phase 14 Research: MLIR/Native Lowering Spike

**Status:** Research report
**Date:** 2026-06-08
**Phase:** 14 — MLIR/Native Lowering Spike
**Depends on:** Phase 13 `L2Core` verifier foundation and `VerifiedArtifactFacts`

## Executive Summary

Phase 14 should be a narrow translation-validation spike, not a production
compiler. The right first target is:

```text
verified L2Core bounded-copy program
  -> verifier report with VerifiedArtifactFacts
  -> constrained lowering support check
  -> textual MLIR module or native plan artifact
  -> optional toolchain validation/JIT when local MLIR tools exist
  -> output equivalence check against the Rust interpreter path
```

Recommended direction:

- Treat native lowering as a **post-verifier backend**. It must require an
  accepted `verify_l2_core` report and present `VerifiedArtifactFacts`.
- Start with **textual MLIR emission** over standard dialects (`func`, `scf`,
  `arith`, `memref`) and defer a custom Loom dialect until repeated lowering
  patterns justify it.
- Lower only the Phase 13 sample shape first: finite `ForRange`, `ReadInput`,
  `AppendValue`, one `Int32` output builder, no `CursorLoop`, no null emission,
  no strings, no floats, no arbitrary Arrow raw-buffer writes.
- Prefer a **small Loom-owned runtime ABI** over direct generated Arrow memory
  mutation. For the spike, native code may fill a typed primitive output buffer
  whose length and capacity are derived from verifier facts; Rust then wraps or
  compares it.
- Keep MLIR/LLVM toolchain availability optional in normal verification. The
  release gate can validate textual lowering deterministically and run
  `mlir-opt`/JIT only when installed.

The main decision is not "MLIR vs Rust"; it is "do we preserve the Phase 13
verified semantics boundary when code becomes native?" The answer should be yes:
Phase 14 lowering is allowed only for a tiny supported subset and must fail
closed otherwise.

## Local Starting Point

Phase 13 delivered the exact handoff Phase 14 needs:

- `crates/loom-core/src/l2_core.rs` defines `L2CoreProgram`,
  capabilities, finite input slices, typed output builders, loop forms, and
  `VerifiedArtifactFacts`.
- `crates/loom-core/src/full_verifier.rs` defines `verify_l2_core`, typed
  diagnostics, proof-obligation traces, and facts only for accepted programs.
- `crates/loom-core/tests/full_verifier.rs` contains the canonical bounded
  copy program:

```text
for i in 0..4:
  value = read input0 at i + 0 width 4
  append value to out0
```

Phase 13 final report names the Phase 14 preconditions:

- successful `verify_l2_core` report,
- no unknown required features,
- finite input capabilities,
- finite loop bounds or discharged monotone progress obligations,
- arithmetic/range/resource constraints represented,
- typed Arrow builder events only,
- present `VerifiedArtifactFacts`,
- lifecycle invariant that lowering cannot occur before verifier acceptance.

## External Evidence

### MLIR Lowering Model

MLIR is designed around dialects and progressive lowering. The official Toy
tutorial frames dialects as a way to support language-specific constructs while
retaining a path to LLVM or other codegen infrastructure:
https://mlir.llvm.org/docs/Tutorials/Toy/

The Toy LLVM lowering chapter uses dialect conversion patterns and a full
conversion so only legal operations remain after lowering:
https://mlir.llvm.org/docs/Tutorials/Toy/Ch-6/

Implication for Loom:

- Phase 14 should not generate arbitrary LLVM IR directly as the first step.
- The spike should model a legal-operation boundary: only supported L2Core
  statements lower, and unsupported statements reject before emission.
- A future custom `loom` dialect is plausible, but a textual standard-dialect
  path is enough to validate the compiler boundary now.

### Standard Dialects For The First Slice

The `scf` dialect is a common structured-control-flow lowering stage and is
typically lowered to control-flow and then to final targets such as LLVM or
SPIR-V: https://mlir.llvm.org/docs/Dialects/SCFDialect/

The `arith` dialect covers integer/floating arithmetic, comparisons, casts, and
bitwise operations: https://mlir.llvm.org/docs/Dialects/ArithOps/

The `memref` dialect represents memory references, loads, stores, and views,
but its documentation warns that lowerings may produce LLVM GEP attributes whose
validity depends on in-bounds indices and no signed overflow:
https://mlir.llvm.org/docs/Dialects/MemRef/

The `llvm` dialect represents LLVM-level functions and LLVM IR-compatible
types/operations: https://mlir.llvm.org/docs/Dialects/LLVM/

Implication for Loom:

- `ForRange` maps naturally to `scf.for`.
- scalar constants/adds/comparisons map to `arith`.
- bounded input/output buffers can be modeled as `memref` values, but every
  index and capacity fact must come from `VerifiedArtifactFacts`.
- LLVM lowering is a later step in the same pipeline, not a Phase 14 requirement
  for every developer machine.

### Execution And JIT

MLIR has an `ExecutionEngine` that can create an execution engine for MLIR IR
and translate a module to LLVM IR internally:
https://mlir.llvm.org/doxygen/classmlir_1_1ExecutionEngine.html

LLVM ORC provides modular JIT APIs, including LLVM IR compilation, linking,
symbol lookup, eager/lazy compilation, and custom program representations:
https://llvm.org/docs/ORCv2.html

Implication for Loom:

- JIT execution is feasible, but it pulls in toolchain, symbol, linking, and
  host ABI complexity.
- Phase 14 should separate "lowering artifact is valid and constrained" from
  "local machine can JIT it."
- A JIT path can be optional evidence, not the only acceptance gate.

### Rust Integration Options

`melior` is a Rust wrapper around the MLIR C API and aims to provide a safe,
complete Rust-facing API:
https://docs.rs/crate/melior/latest

Cranelift is a Bytecode Alliance compiler backend written in Rust, used for JIT
and AOT compilation, and described as general-purpose code generation:
https://cranelift.dev/

Implication for Loom:

- `melior` is the natural Rust-native bridge if Phase 14 needs programmatic
  MLIR construction, but it inherits system MLIR version/toolchain friction.
- Direct Cranelift is attractive for a Rust-only JIT proof, but it bypasses the
  MLIR/native-lowering research question and should be a fallback/backend
  comparison rather than the primary Phase 14 path.
- Textual MLIR emission is the lowest-coupling way to learn the boundary before
  committing to bindings.

### Arrow Boundary

The Arrow C Data Interface is a small, stable C ABI for sharing Arrow memory and
allows integrations without linking Arrow C++:
https://arrow.apache.org/docs/format/CDataInterface.html

Local Loom currently relies on Rust `OutputBuilder` typed append APIs and direct
DuckDB DataChunk population, not generated raw Arrow writes.

Implication for Loom:

- Native lowering should not start by generating code that mutates Arrow
  buffers directly.
- The safer first ABI is a tiny typed runtime call or typed buffer fill, then
  Rust validates counts/schema and constructs Arrow output through existing
  paths.
- Arrow C Data remains the long-term interchange boundary, but Phase 14 should
  avoid making generated code responsible for all Arrow invariants.

## Option Analysis

| Option | Description | Benefits | Risks | Recommendation |
|---|---|---|---|---|
| Textual MLIR emitter | Rust emits `.mlir` text for a tiny L2Core subset. | Lowest coupling, no new build dependency, easy snapshots. | Not a real compiler integration yet. | **Use first.** |
| `melior` MLIR builder | Rust constructs MLIR through MLIR C API bindings. | Real MLIR API, future pass pipeline integration. | System MLIR version friction; larger dependency. | Defer until textual shape is stable. |
| MLIR ExecutionEngine/JIT | Compile lowered MLIR to native and run in-process. | Strong demo of native path. | Toolchain/linking/symbol complexity; host-specific. | Optional spike gate only. |
| Direct LLVM/ORC | Emit LLVM IR and JIT through ORC. | Mature JIT path. | Skips MLIR design value; larger unsafe ABI surface. | Defer. |
| Cranelift | Rust-native codegen/JIT backend. | Simpler Rust integration, lower toolchain friction. | Not MLIR; weaker fit for research phase. | Keep as fallback comparison. |

## Recommended Phase 14 Scope

### In Scope

- Define a lowering support predicate for `L2CoreProgram` plus
  `FullVerificationReport`.
- Accept exactly one initial shape:
  - artifact accepted by `verify_l2_core`,
  - one finite `ForRange` with constant start/end,
  - `ReadInput` from one `InputSlice`,
  - `AppendValue` into one `Int32` output builder,
  - output max events equals row bound,
  - no `CursorLoop`,
  - no `AppendNull`,
  - no `FailClosed` in the lowered happy path,
  - no strings/floats/booleans until later plans.
- Emit deterministic textual MLIR using `func`, `scf`, `arith`, and `memref`.
- Add tests that unsupported accepted programs fail closed at the lowering
  boundary.
- Add an output-equivalence test against a Rust reference execution for the
  bounded-copy sample.
- Add an optional script that runs `mlir-opt`/`mlir-translate` when present and
  skips with an explicit message when absent.

### Out Of Scope

- Production MLIR pass pipeline.
- Custom `loom` dialect.
- Vectorization.
- Direct generated Arrow raw-buffer mutation.
- Native lowering for FSST, ALP, dictionary, RLE, strings, null bitmaps, or
  multi-column tables.
- Replacing the interpreter or verifier.
- Proving compiler correctness in Lean/Rocq.
- Requiring system MLIR/LLVM in the normal Rust workspace build.

## Proposed Lowering Shape

For a bounded copy from `input0` to `out0`, the conceptual MLIR shape is:

```mlir
module {
  func.func @loom_l2core_copy_i32(
      %input: memref<?xi32>,
      %output: memref<?xi32>,
      %rows: index) {
    %c0 = arith.constant 0 : index
    %c1 = arith.constant 1 : index
    scf.for %i = %c0 to %rows step %c1 {
      %v = memref.load %input[%i] : memref<?xi32>
      memref.store %v, %output[%i] : memref<?xi32>
    }
    return
  }
}
```

This is intentionally not the final ABI. It is the minimal semantic skeleton:
bounded row loop, checked index domain, typed load/store, no nulls, no variable
width values.

## Safety Boundary

Phase 14 must maintain these invariants:

1. **Verify-before-lower:** lowering requires `FullVerificationReport::is_ok()`
   and `facts().is_some()`.
2. **Facts are tied to the report:** the lowering API should accept a report,
   not standalone copied facts.
3. **Subset rejection:** any unsupported statement/type/capability rejects with
   a typed lowering diagnostic.
4. **No raw Arrow writes:** generated/native code cannot construct Arrow arrays
   directly in the first spike.
5. **Capacity from facts:** input/output memory ranges and row count come from
   verifier facts and resource bounds.
6. **Equivalence oracle:** lowered output for the sample must match the existing
   safe Rust interpretation of the same L2Core program.
7. **Optional native execution:** local absence of MLIR tools should not
   silently pass native execution; it should be reported as skipped optional
   evidence.

## Recommended Implementation Architecture

Potential module names:

- `crates/loom-core/src/native_lowering.rs`
- `crates/loom-core/src/mlir_lowering.rs`

Potential Rust types:

```rust
pub struct LoweringRequest<'a> {
    pub program: &'a L2CoreProgram,
    pub report: &'a FullVerificationReport,
}

pub struct LoweringArtifact {
    pub backend: LoweringBackend,
    pub entry_symbol: String,
    pub mlir_text: String,
    pub facts_digest: String,
}

pub enum LoweringDiagnosticCode {
    VerifierRejected,
    MissingVerifierFacts,
    UnsupportedFeature,
    UnsupportedStatement,
    UnsupportedType,
    UnsupportedNullability,
    UnsupportedLoopShape,
    UnsupportedCapabilityShape,
}
```

This keeps Phase 14 aligned with Phase 13's diagnostic style and avoids
committing to `melior` or LLVM packages in `Cargo.toml`.

## Suggested Phase 14 Plan Breakdown

### 14-01: Lowering Contract And Support Predicate

- Add a native-lowering contract doc.
- Implement a support checker that requires accepted verifier report + facts.
- Emit stable lowering diagnostics for rejected/unsupported programs.
- Add tests for missing facts, verifier-rejected programs, unsupported loop,
  unsupported type, unsupported nulls.

### 14-02: Textual MLIR Emission

- Emit deterministic textual MLIR for the bounded Int32 copy sample.
- Snapshot-test emitted MLIR.
- Keep the emitter pure Rust with no system MLIR dependency.
- Record the dialect stack and ABI assumptions in docs.

### 14-03: Rust Reference Execution And Equivalence Gate

- Add a tiny reference executor for the supported L2Core subset or reuse the
  existing safe path if one is introduced.
- Compare lowered-plan semantics against expected `Int32` output.
- Add optional `mlir-opt` validation script when tool exists.

### 14-04: Optional Native/JIT Probe And Closeout

- If local MLIR tools or bindings are available, run a JIT/toolchain probe.
- If unavailable, record skipped evidence without failing the release gate.
- Update roadmap/state/docs with Phase 14 outcome and next boundary.

## Risks

| Risk | Impact | Mitigation |
|---|---:|---|
| MLIR system dependency destabilizes normal Rust builds | High | Start with textual MLIR and optional external validation. |
| Lowering bypasses verifier semantics | High | Require accepted report + facts through API and tests. |
| Generated code mutates Arrow buffers incorrectly | High | Use typed primitive buffers/runtime callbacks first; no raw Arrow arrays. |
| JIT/linking complexity consumes the phase | Medium | Make JIT optional and put contract/textual lowering first. |
| The spike overfits the sample | Medium | Add explicit unsupported-case diagnostics and document the accepted subset. |
| Equivalence tests become mistaken for proof | Medium | Call them regression evidence, not soundness proof. |

## Open Questions For Planning

1. Should Phase 14 expose lowering through `loom lower-l2core --sample`, or keep
   it internal until the textual MLIR path is stable?
2. Should the output artifact be stored only in memory, or should tests write a
   `.mlir` snapshot fixture under `fixtures/` or `target/`?
3. Should the first runtime ABI be pure typed-buffer fill or append-callback
   calls into Rust?
4. Should optional MLIR validation use `mlir-opt` only, or also
   `mlir-translate`/ExecutionEngine when available?

## Research Recommendation

Proceed with Phase 14 as a four-plan spike:

```text
14-01 lowering contract + support checker
14-02 deterministic textual MLIR emitter
14-03 equivalence/reference gate + optional mlir-opt validation
14-04 optional native/JIT probe + closeout docs
```

Do not add `melior`, LLVM, or Cranelift as mandatory dependencies in the first
plan. The project should learn the semantic and safety shape first, then choose
the binding/backend only after the textual artifact and verifier-precondition
tests are stable.
