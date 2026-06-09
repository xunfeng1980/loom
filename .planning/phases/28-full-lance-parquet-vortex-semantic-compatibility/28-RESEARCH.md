# Phase 28: Full Lance + Parquet + Vortex Semantic Compatibility - Research

**Researched:** 2026-06-09
**Domain:** Vortex semantic compatibility over Loom reader/verifier/native/runtime evidence
**Confidence:** MEDIUM-HIGH

## Executive Recommendation

Phase 28 should not mean "blindly accept every Vortex file." It should mean a
reviewer-visible compatibility system where every Vortex shape has one explicit
state: accepted with oracle/verifier evidence, unsupported with facts and no
emission, rejected fail-closed, interpreter-only, or native-supported.

Recommended implementation path:

1. Create a Phase 28 compatibility matrix/report model in `loom-vortex-ingress`
   that consumes the existing `VortexEncodingCoverage` vocabulary and adds
   semantic dimensions missing from Phase 21: null semantics, chunk ordering,
   original-vs-emitted shape, oracle class, artifact verifier class, DuckDB
   visibility, native route evidence, and deferral reason.
2. Convert Phase 21's matrix into executable assertions so accepted rows,
   unsupported rows, and canonicalized rows cannot drift silently.
3. Close one or two high-value semantic gaps with real implementation evidence:
   recommended first targets are nullable primitive artifact emission and
   structured dictionary/run-end representation, because both already have
   Loom model primitives and oracle tests.
4. Add explicit negative and no-overclaim gates: no accepted row without oracle,
   no artifact emitted for unsupported valid shapes, no native-supported label
   without production-lowering/ExecutionEngine evidence, and no StarRocks/Phase
   29 evidence claim.
5. Wire a focused `scripts/vortex-semantic-compatibility-test.sh` into
   `scripts/mvp0-verify.sh` only after the focused gate passes.

## Current Evidence Base

### Phase 18 Reader Boundary

Phase 18 established:

- `loom-vortex-ingress` as the isolated Vortex dependency boundary.
- `VortexReaderFacts`, `VortexReaderLayoutFact`, `VortexReaderDTypeFact`,
  `VortexReaderSegmentFact`, and split facts.
- Accepted emission for non-null `i32/i64/f32/f64` single columns and non-null
  primitive struct/table arrays.
- Unsupported facts for valid string/unsupported table shapes.
- Rejected fail-closed behavior for malformed files.
- Vortex scan/execution as oracle evidence for emitted fixtures.

### Phase 21 Coverage Matrix

Phase 21 added the key vocabulary:

- support states: `accepted`, `unsupported`, `rejected`;
- emission kinds: `none`, `LMP1`, `LMT1`;
- emission dispositions: `none`, `canonical-raw`, `canonical-table`,
  `structured-layout`;
- lowering dispositions: `interpreter-only`,
  `production-lowering-supported`, `fail-closed/deferred`.

Implemented rows include:

- accepted non-null primitive and primitive table baseline;
- nullable primitive facts with no emission;
- chunked primitive canonical raw evidence;
- dictionary/run-end/bitpack/FOR canonical raw evidence;
- string and wider compression deferrals.

The core residual risk is overclaiming canonical raw evidence as structured
Vortex semantic support or native support.

### Native Execution Reality

The native path has been corrected after the Phase 24/25 audit:

- DuckDB native primitive path now feeds real artifact value buffers into Melior
  `ExecutionEngine::invoke_packed`.
- Shell gates require `native-execution-engine-output`.
- Fallback/toolchain skip is no longer acceptable as primitive native success.

Phase 28 must preserve this distinction. Compatibility may be interpreter-only,
but native-supported rows need actual production-lowering and ExecutionEngine
evidence.

## Recommended Phase Shape

### Plan 28-01: Compatibility Matrix Contract

Purpose: make compatibility claims machine-readable and impossible to overclaim.

Deliver:

- `VortexSemanticCompatibilityReport`
- `VortexSemanticCompatibilityRow`
- row fields for original shape, emitted shape, support state, oracle class,
  verifier class, runtime class, native class, and deferral reason
- tests that map existing Phase 21 coverage into semantic rows

### Plan 28-02: Matrix Fixture Assertions and Drift Gate

Purpose: convert the matrix into executable regression evidence.

Deliver:

- deterministic row assertions for all Phase 21 implemented rows;
- no-overclaim tests for canonical raw vs structured layout;
- unsupported valid shape checks for nullable/string/compression gaps;
- focused script seed.

### Plan 28-03: Nullable Primitive Artifact Semantics

Purpose: close a high-value semantic gap already identified in Phase 21.

Recommended target:

- support nullable primitive `i32/i64/f32/f64` emission only if validity is
  represented in Loom artifacts and verifier/oracle equality proves null
  positions and values.

Risk:

- existing real-ingress path may not expose enough Vortex validity detail
  without canonical scan materialization. If blocked, keep row unsupported and
  record exact API limitation.

### Plan 28-04: Structured Encoding Semantics

Purpose: distinguish true structured support from canonical raw support.

Recommended target:

- dictionary and run-end structured semantics first, because `LayoutNode` already
  has `Dictionary` and `RunEnd` and existing tests cover dictionary/RLE decoder
  behavior.
- bitpack/FOR structured Vortex facts next if APIs expose stable width/reference
  facts.

Risk:

- if real Vortex APIs only expose canonical scan rows, Phase 28 should not fake
  structured facts. It should preserve canonical raw and mark structured support
  deferred with a stable reason.

### Plan 28-05: Gate, Report, and Milestone Handoff

Purpose: close the phase with a reproducible command and explicit tradeoffs.

Deliver:

- `28-LANCE-PARQUET-VORTEX-SEMANTIC-COMPATIBILITY-REPORT.md`;
- `scripts/vortex-semantic-compatibility-test.sh`;
- `scripts/mvp0-verify.sh` wiring;
- final no-overclaim checks for Phase 29 skip/defer, public API creep, native
  fallback, and unsupported rows.

## Validation Architecture

### Independent Axes

Every accepted row should be validated across these axes:

1. Reader facts: real Vortex input produces stable Loom-owned facts.
2. Artifact emission: emitted bytes exist only for accepted rows.
3. Artifact verifier: emitted bytes pass `verify_artifact`.
4. Oracle equivalence: Loom decoded rows match Vortex scan rows, including null
   positions and row ordering.
5. Runtime/native disposition: native-supported rows require production-lowering
   and ExecutionEngine evidence; interpreter-only rows must say so.
6. Public surface: no StarRocks, public C ABI, CLI, or new SQL surface is added
   by accident.

### Negative Tests

Required negative classes:

- unsupported valid inputs emit no bytes;
- malformed inputs reject before partial output;
- canonical raw rows do not claim structured support;
- rows without oracle evidence cannot become accepted;
- rows without verifier evidence cannot become accepted;
- rows without ExecutionEngine evidence cannot become native-supported;
- Phase 29 skipped/deferred evidence is not cited as completed dual-query proof.

## Tradeoffs

- Matrix-first is less satisfying than claiming all Vortex support immediately,
  but it prevents false completion and gives reviewers a stable support map.
- Nullable primitive emission is a better first semantic gap than strings because
  Arrow/Loom validity semantics already exist locally; real string compression
  params require more Vortex-specific extraction.
- Structured dictionary/run-end support is valuable only if original Vortex
  shape facts are available. If APIs canonicalize too early, the honest result
  is canonical raw support plus deferred structured support.
- Phase 28 proceeds without Phase 29 dual-query evidence. This is acceptable
  only if the final report clearly states that second-host proof is missing.

## Sources Consulted

- `.planning/phases/18-complete-vortex-reader/18-READER-REPORT.md`
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md`
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-REPORT.md`
- `.planning/debug/native-query-zero-buffer.md`
- `crates/loom-vortex-ingress/src/lib.rs`
- `crates/loom-core/src/artifact_verifier.rs`
- `crates/loom-core/src/production_native_lowering.rs`
- `crates/loom-core/src/runtime_abi.rs`
