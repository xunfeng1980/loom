# Phase 20 Research: Production Decode Dialect and Native Kernel Expansion

**Date:** 2026-06-08  
**Phase:** 20 — Production Decode Dialect and Native Kernel Expansion  
**Depends on:** Phase 16, Phase 17, Phase 18, Phase 19

## Executive Summary

Phase 20 should turn the previous native-lowering spikes into a production-shaped
lowering surface, not into host-engine integration and not into a second
unverified execution path.

Recommended direction:

1. Define a Loom-owned MLIR `loom.decode` dialect contract and textual surface
   for verified decode programs.
2. Require accepted artifact verification plus solver-backed
   `ConstraintDischargeStatus::Discharged` before production native lowering.
3. Lower from the dialect contract to standard MLIR dialects (`scf`, `arith`,
   `memref`, `vector`, `func`) and then to LLVM-compatible dialects only after
   the verifier gate.
4. Expand beyond the bounded Int32 copy slice to primitive Arrow/raw-buffer
   builders and multi-column table batches first.
5. Keep the C++/ODS registered dialect and `melior`/LLVM/JIT implementation
   behind optional tooling gates. The default workspace must stay buildable
   without MLIR/LLVM.

This is the right bridge between the current verifier line and the later host
native runtime phases: Loom still proves and gates what may lower; MLIR only
takes over after the trust boundary.

## Local Starting Point

Completed prerequisites:

- Phase 16: optional `loom-native-melior` crate with verifier-gated programmatic
  MLIR/JIT evidence for the bounded Int32 copy slice.
- Phase 17: unified artifact verifier pipeline from `LMC1` through L1/L2/facts
  and lowering readiness.
- Phase 18: complete Vortex reader boundary with recursive reader facts and
  accepted emission matrix for non-null primitive single columns and primitive
  struct/table emission.
- Phase 19: Bitwuzla-backed solver discharge over the current artifact/L2Core
  slice, with `Discharged` facts required for production native-lowering trust.

Current native-lowering limitations:

- `loom_core::native_lowering` accepts only `l2core.copy.v0`.
- The emitted MLIR is standard textual `func`/`arith`/`scf`/`memref` for bounded
  Int32 copy.
- `loom-native-melior` can validate/build that narrow artifact, but it is not a
  production decode dialect, not an Arrow builder lowering path, and not a broad
  kernel expansion.
- Local toolchain is now compatible with the pinned Phase 16 line:
  `/opt/homebrew/opt/llvm/bin/llvm-config --version` reports `22.1.7`, and
  `/opt/homebrew/opt/llvm/bin/mlir-opt --version` reports Homebrew LLVM
  `22.1.7`.

## External Research Notes

### MLIR Dialect Definition

MLIR's official dialect documentation describes dialects as the extension
mechanism for defining operations, attributes, and types. The expected
production mechanism is declarative TableGen/ODS, which generates C++ boilerplate
and documentation. That matters because a real registered `loom.decode` dialect
is not just a Rust enum; it implies an MLIR dialect definition, op verification,
parsing/printing, and lowering patterns.

Sources:

- https://mlir.llvm.org/docs/DefiningDialects/
- https://mlir.llvm.org/docs/DefiningDialects/Operations/

Implication for Loom:

- Phase 20 should define the dialect contract now.
- The first implementation can emit dialect-shaped textual artifacts and lower
  to standard dialects for validation.
- A compiled C++/ODS dialect should be optional-gated until the op set and
  semantics stop moving.

### Standard Dialect Lowering Path

MLIR's LLVM target docs describe a two-stage flow: first convert to dialects that
can translate to LLVM IR, then translate to LLVM IR. It also notes that important
transformations should happen in MLIR before final translation. This supports the
Phase 20 shape: keep Loom decode semantics in a domain dialect, then lower
progressively through standard MLIR dialects.

Sources:

- https://mlir.llvm.org/docs/TargetLLVMIR/
- https://mlir.llvm.org/docs/Dialects/LLVM/
- https://mlir.llvm.org/docs/Dialects/MemRef/

Implication for Loom:

- Do not skip directly from verifier facts to handwritten LLVM IR.
- Do not encode all semantics as opaque runtime calls.
- Lower decode ops into structured loops, memrefs, arithmetic, and builder
  primitives where MLIR can still optimize and verify the shape.

### Vectorization

MLIR's vector dialect explicitly separates vector values from buffer/memref
concerns and provides retargetable vector abstractions. Loom should therefore
model lane/vector decisions as backend-owned after verification, not as physical
SIMD widths in the distribution artifact.

Source:

- https://mlir.llvm.org/docs/Dialects/Vector/

Implication for Loom:

- Phase 20 should add a vectorization policy surface, not hard-code AVX/NEON/SVE.
- Initial native kernels may remain scalar while producing vectorizable MLIR.
- Physical vector width remains an MLIR/backend decision.

### Arrow Raw Buffers and C Data Interface

Arrow's columnar format defines fixed-size primitive arrays as contiguous buffers
with a fixed slot width and optional validity bitmap. The Arrow C Data Interface
exports arrays through `ArrowArray`/`ArrowSchema`, including buffer counts,
children, dictionary pointers, and a release callback.

Sources:

- https://arrow.apache.org/docs/format/Columnar.html
- https://arrow.apache.org/docs/format/CDataInterface.html

Implication for Loom:

- Phase 20 should start with primitive Arrow/raw-buffer builders before variable
  binary, dictionary, nested, or run-end encoded output.
- Builder ops should make output legality explicit: length, null count,
  validity buffer, value buffer, child arrays, and release ownership must be part
  of the lowering contract.
- Multi-column output should use a struct/table batch builder shape before host
  engine integration begins.

### melior and Toolchain Boundary

`melior` is the Rust binding layer over the MLIR C API. Its README currently
requires LLVM/MLIR 22 and says both melior and the MLIR C API remain alpha and
unstable. This validates the current architecture: keep `melior` inside an
optional backend crate, not in `loom-core` or `loom-ffi`.

Sources:

- https://github.com/mlir-rs/melior
- https://docs.rs/melior/latest/melior/

Implication for Loom:

- Phase 20 can use `melior` to validate and build standard-dialect modules.
- Default verification, artifact parsing, and dialect contract tests should not
  require a local LLVM install.
- Strict MLIR/backend gates can require LLVM/MLIR 22 and fail closed.

## Recommended Phase 20 Scope

Phase 20 is the production native-lowering surface phase.

In scope:

- `loom.decode` dialect contract and op inventory.
- Verifier-gated production support predicate that requires artifact report
  acceptance and discharged constraints.
- Dialect-shaped textual emission for supported artifacts.
- Lowering from `loom.decode` contract to standard MLIR dialects.
- Primitive Arrow/raw-buffer builder lowering for fixed-size primitive output.
- Multi-column table/struct batch lowering for the Phase 18 accepted primitive
  table matrix.
- Native kernel expansion beyond bounded Int32 copy, initially focused on raw
  primitive copy, bitpacked primitive unpack, and frame-of-reference primitive
  decode where verifier facts provide bounds.
- Equivalence gates against Rust interpreter/oracle output for supported cases.

Out of scope:

- Host runtime ABI and execution policy. That is Phase 22.
- DuckDB native execution integration. That is Phase 23.
- Native cache/fallback hardening. That is Phase 24.
- Wider Vortex encoding/layout semantics. That is Phase 21.
- New solver backend work. That remains Phase 19+ follow-up, not Phase 20.
- Arbitrary `L2Core` artifact codec/parser stabilization.
- Checked proof objects or formal proof completion.

## Dialect Contract Recommendation

Use a small `loom.decode` surface with operations grouped by role:

| Role | Example op family | Purpose |
|------|-------------------|---------|
| Artifact entry | `loom.decode.module`, `loom.decode.kernel` | Bind artifact id, feature set, row bound, and verified facts fingerprint |
| Input | `loom.decode.input_slice`, `loom.decode.column` | Declare verified byte/value inputs and logical column facts |
| Output | `loom.decode.builder`, `loom.decode.finish` | Declare Arrow/raw-buffer builders and final batch materialization |
| Control | `loom.decode.for_rows` | Express finite row loops sourced from verified row bounds |
| Decode primitive | `loom.decode.raw_copy`, `loom.decode.bit_unpack`, `loom.decode.for_delta` | Domain decode ops before lowering to standard loops/vector ops |
| Nulls | `loom.decode.validity_copy`, `loom.decode.validity_all_valid` | Keep validity bitmap handling explicit |
| Diagnostics | `loom.decode.assume_verified` | Attach verifier/solver evidence without recomputing it inside MLIR |

Contract invariants:

- Every operation references a verifier-owned fact id or derived lowering fact.
- Every loop has a finite row bound from `VerifiedArtifactFacts` or
  `ArtifactVerificationFacts`.
- Every load/store target is derived from discharged offset/range/overflow
  obligations.
- Unsupported ops, missing facts, `CollectedOnly` constraints, unknown solver
  evidence, or feature mismatch reject before MLIR emission.

## Native Kernel Expansion Recommendation

Expand in this order:

1. Raw primitive copy: Int32, Int64, Float32, Float64, non-null.
2. Primitive validity: all-valid and copied validity bitmap.
3. Multi-column primitive table batch: struct output with independent child
   builders.
4. Bitpack primitive unpack for Int32/Int64 when bounds and bit width are
   discharged.
5. Frame-of-reference primitive decode using widened arithmetic where overflow
   obligations are discharged.

Defer:

- ALP native lowering until primitive builder and floating output contracts are
  stable.
- Dictionary/RLE/FSST/native string output until variable-size builders and
  dictionary ownership are defined.
- Arbitrary Vortex layouts until Phase 21 widens encoding coverage.

## Recommended Plan Split

### 20-01: Production Lowering Contract and Discharged-Facts Gate

Deliverables:

- New production support predicate separate from the Phase 14 bounded-copy
  predicate.
- Reject unless artifact verification is accepted and constraints are
  `Discharged` or explicitly `NotRequired`.
- Stable diagnostics for unsupported native lowering shapes.
- Tests proving `CollectedOnly`, failed, unknown, skipped, and missing solver
  evidence reject before MLIR emission.

### 20-02: `loom.decode` Dialect Contract and Textual Surface

Deliverables:

- Dialect contract doc/spec with op inventory and invariants.
- Dialect-shaped textual emission for the first supported primitive table slice.
- Golden tests for deterministic textual emission.
- Explicit note that compiled C++/ODS registration is optional/toolchain-gated in
  Phase 20 unless the op surface stabilizes early.

### 20-03: Arrow Raw-Buffer Builder Lowering

Deliverables:

- Internal builder model for primitive value buffer, optional validity bitmap,
  length, null count, and child arrays.
- Lowering from `loom.decode` builder ops to standard MLIR `memref`/`arith`/`scf`
  shape.
- Rust reference builder equivalence tests for primitive columns.

### 20-04: Native Kernel Expansion for Primitive Multi-column Slices

Deliverables:

- Raw primitive copy kernels for Int32, Int64, Float32, Float64.
- Bitpack and/or FOR primitive decode if existing facts can discharge bounds.
- Multi-column table lowering over the Phase 18 accepted matrix.
- Negative tests for unsupported nullability, variable-size output, dictionary,
  RLE, FSST, ALP, and non-discharged constraints.

### 20-05: MLIR Validation Gate, Report, and Closeout

Deliverables:

- Optional strict MLIR 22 validation path using `mlir-opt`/`melior`.
- Updated native-lowering report with supported matrix and deferred work.
- Release-gate wiring that is skip-aware in normal mode and fail-closed in
  strict native-lowering mode.
- ROADMAP/STATE/README updates that do not claim host execution or arbitrary
  Vortex support.

## Success Criteria

Phase 20 is complete when:

- Production native lowering consumes only accepted artifact reports with
  discharged solver facts.
- The `loom.decode` dialect contract exists and has deterministic textual
  coverage for supported slices.
- Primitive Arrow/raw-buffer builder lowering is implemented and tested.
- Multi-column primitive table output can lower through the production path.
- Supported native kernels are wider than bounded Int32 copy and have
  interpreter/oracle equivalence tests.
- Unsupported artifacts fail closed before MLIR/native artifact creation.
- Default workspace tests do not require MLIR/LLVM.
- Strict MLIR validation passes on compatible LLVM/MLIR 22 environments.

## Open Questions for Planning

1. Should Phase 20 implement a compiled C++/ODS dialect as a hard deliverable, or
   keep it as optional evidence while locking the textual/semantic dialect
   contract first?
2. Should the first native-expanded decode primitive be bitpack or
   frame-of-reference? Bitpack is closer to byte/bit arithmetic; FOR is closer to
   primitive arithmetic and overflow obligations.
3. Should Phase 20 introduce a stable native-lowering artifact format, or wait
   until Phase 22 defines the host runtime ABI/cache key?
4. Should MLIR validation use `mlir-opt` textual files first, `melior` module
   construction first, or both with one canonical text golden?

## Recommendation

Use Phase 20 to make native lowering production-shaped, but keep it one layer
above host execution:

```text
LMC1 / Vortex reader artifact
  -> unified artifact verifier
  -> Bitwuzla-backed discharged facts
  -> loom.decode dialect contract
  -> standard MLIR lowering
  -> optional MLIR 22 validation/JIT evidence
```

The most important guardrail is that Phase 20 must consume `Discharged` facts,
not merely accepted structure or collected obligations. That keeps native
lowering as a backend of the verifier, rather than a bypass around it.
