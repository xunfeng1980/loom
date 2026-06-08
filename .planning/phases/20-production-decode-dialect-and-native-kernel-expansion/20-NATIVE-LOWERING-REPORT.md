# Phase 20 Native Lowering Report

## Scope

Phase 20 establishes the first production-shaped native-lowering surface after
the unified artifact verifier and Bitwuzla-backed solver discharge. It does not
claim host-engine native execution, arbitrary Vortex coverage, checked proof
objects, or a frozen complete MLIR dialect.

## Trust Boundary

Production native lowering now starts from `ArtifactVerificationReport`.
Standalone `L2Core` facts, accepted structure without facts, and collected-only
constraints cannot emit dialect or MLIR artifacts. The support gate requires
accepted artifact facts, row-count bounds, associated L2/native facts, supported
payload shape, and `ConstraintDischargeStatus::Discharged` or `NotRequired`.

## Dialect Contract

Phase 20 adds a Loom-owned `loom.decode` dialect contract and deterministic
textual surface. The hard deliverable is the textual/semantic contract. Compiled
C++/ODS dialect registration remains optional and toolchain-gated until the op
surface is stable.

## Supported Matrix

Supported production lowering seed:

| Area | Supported |
|------|-----------|
| Payload facts | `LMP1 layout`, `LMT1 table` |
| Constraint status | `Discharged`, `NotRequired` |
| Output types | non-null Int32, Int64, Float32, Float64 |
| Output shape | single primitive column and primitive multi-column table |
| Native kernel | raw primitive copy |
| MLIR text | standard `func`/`arith`/`scf`/`memref` builder-buffer text |

Deferred/fail-closed:

- nullable output;
- variable-size strings;
- dictionary/RLE/FSST/ALP/native string output;
- nested output;
- bitpack and frame-of-reference native lowering until Phase 21 pairs encoding
  coverage with discharged bit-offset/range/overflow lowering facts;
- unsupported payload kinds;
- `CollectedOnly`, failed, unknown, skipped, or missing constraint evidence.

## Arrow Builder Lowering

`loom_core::arrow_buffer_lowering` models primitive Arrow/raw-buffer output as
engine-independent buffer plans. Each column records primitive type, row count,
value-buffer byte length, validity policy, and null-count policy. The initial
validity policy is all-valid only.

## MLIR Validation

`loom-native-melior` now validates Phase 20 standard-MLIR text through
`validate_production_standard_mlir`. Default tests are skip-aware when compatible
MLIR tooling is unavailable. Strict gate evidence passed locally with LLVM/MLIR
22.1.7.

## Encoding/Lowering Coupling

Phase 20 and Phase 21 are coupled axes, not a one-way sequence. Phase 20 defines
the first lowering seed over a narrow primitive matrix. Phase 21 must classify
each newly accepted Vortex encoding/layout as one of:

- interpreter-only for now;
- production-lowering-supported with a dialect/native delta;
- fail-closed/deferred with stable diagnostics.

## Commands Run

- `cargo test -p loom-core --test production_native_lowering`
- `cargo test -p loom-core --test decode_dialect`
- `cargo test -p loom-core --test arrow_buffer_lowering`
- `cargo test -p loom-core --test production_native_kernels`
- `cargo test -p loom-vortex-ingress table_to_loom`
- `cargo test -p loom-native-melior --test production_pipeline`
- `bash scripts/production-native-lowering-test.sh`
- `LOOM_REQUIRE_PRODUCTION_NATIVE=1 bash scripts/production-native-lowering-test.sh`

Final release-gate commands are recorded in `20-SUMMARY.md`.

## Deferred Work

- Actual compiled C++/ODS `loom.decode` dialect registration.
- Bitpack/FOR production native lowering with discharged offset/range/overflow
  facts.
- Dictionary/RLE/FSST/ALP/string/native variable-size builders.
- Host runtime ABI, cache, fallback, and native callable contract.
- DuckDB native execution integration.

## Phase 21 Handoff

Phase 21 owns expanded Vortex encoding/layout/storage coverage. It must pair
each new encoding with interpreter/lowering/deferred status and must not assume
the Phase 20 lowering surface is permanently complete.

## Phase 22 Handoff

Phase 22 owns host runtime ABI/execution policy. It must decide predicate and
projection pushdown shape, concurrency/reentrancy/thread ownership, cache key,
memory ownership, interpreter fallback, and which assumptions remain
DuckDB-shaped until Phase 26 validates a second engine.
