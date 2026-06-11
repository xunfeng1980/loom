---
phase: 50-sidecar-overlay-model-and-host-native-reader-fallback
plan: "00"
subsystem: architecture
tags: [rust, cargo, crate-split, l2core-ir, container]

requires:
  - phase: 49-independent-l2core-decode-ir-codec-and-content-hash-identity
    provides: "Independent L2Core IR codec and content-hash identity"

provides:
  - "loom-ir-core crate: zero-Arrow decode IR layer with L2Core types, codec, verifier"
  - "loom-container crate: packaging/distribution layer depending on loom-ir-core"
  - "L2DataType local type replacing arrow_schema::DataType in IR layer"
  - "L2DataType ↔ DataType bridge functions in container crate"

affects:
  - "50-01 through 50-04 (sidecar overlay, re-export shim, downstream rewiring)"

tech-stack:
  added: [fnv 1.0.7]
  patterns:
    - "Crate boundary enforced via zero-Arrow dependency in IR layer"
    - "Local L2DataType enum replaces arrow_schema::DataType for IR independence"
    - "Bridge functions (l2_to_arrow, arrow_to_l2) at container layer for type conversion"

key-files:
  created:
    - crates/loom-ir-core/Cargo.toml
    - crates/loom-ir-core/src/lib.rs
    - crates/loom-ir-core/src/error.rs
    - crates/loom-ir-core/src/l2_core.rs
    - crates/loom-ir-core/src/l2_core/constraints.rs
    - crates/loom-ir-core/src/l2core_codec.rs
    - crates/loom-ir-core/src/full_verifier.rs
    - crates/loom-container/Cargo.toml
    - crates/loom-container/src/lib.rs
    - crates/loom-container/src/*.rs (22 modules + 1 submodule)
  modified:
    - Cargo.toml (workspace members + fnv dep)
    - Cargo.lock

key-decisions:
  - "verifier.rs kept in loom-container (not ir-core) — depends on container_codec, table_codec, l1_model, l2_kernel_registry, alp_params, fsst_params"
  - "runtime_abi.rs kept in loom-container (not ir-core) — imports ArtifactVerificationStatus from artifact_verifier"
  - "L2DataType local enum defined in l2_core.rs to satisfy zero-Arrow constraint; maps 1:1 with the narrow supported Arrow subset (Boolean, Int32, Int64, Float32, Float64, Utf8)"

patterns-established:
  - "Two-crate architecture: loom-ir-core (zero container/Arrow deps) + loom-container (depends on ir-core, owns all Arrow/packaging)"
  - "Type bridge pattern: L2DataType in IR layer, conversion to arrow_schema::DataType at container boundary"

requirements-completed: []

duration: 13min
completed: 2026-06-11
status: complete
---

# Phase 50 Plan 00: Create loom-ir-core and loom-container Crates Summary

**Two-crate architectural split: loom-ir-core (zero-Arrow decode IR) and loom-container (packaging/distribution layer depending on ir-core), each compiling cleanly with 141 combined tests passing and all dependency edges verified.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-06-11T07:43:03Z
- **Completed:** 2026-06-11T07:56:23Z
- **Tasks:** 2
- **Files modified:** 34

## Accomplishments

- Created `loom-ir-core` crate with zero Arrow, Parquet, Vortex, Lance, serde, ron, or fsst dependencies — only `fnv` for content-hash computation
- Created `loom-container` crate with full Arrow dependency stack, depending on `loom-ir-core` with clean single-direction edge
- Defined local `L2DataType` enum in `l2_core.rs` (Boolean/Int32/Int64/Float32/Float64/Utf8) replacing `arrow_schema::DataType` to satisfy the zero-Arrow constraint while preserving the codec wire format
- Added `l2_to_arrow` and `arrow_to_l2` bridge functions in loom-container for boundary type conversion
- Fixed cross-crate imports across 7 container-layer files: `crate::l2_core` → `loom_ir_core::l2_core`, etc.
- Left `verifier.rs` and `runtime_abi.rs` in loom-container (not ir-core) — correct decision as they depend on container-layer modules

## Task Commits

Each task was committed atomically:

1. **Task 1: Create crates/loom-ir-core — independent decode IR layer** — `2f8e143` (feat)
2. **Task 2: Create crates/loom-container — packaging/distribution layer** — `e0d0361` (feat)

## Files Created/Modified

- `crates/loom-ir-core/Cargo.toml` — Zero Arrow deps, only fnv
- `crates/loom-ir-core/src/lib.rs` — Declares error, l2_core, l2core_codec, full_verifier
- `crates/loom-ir-core/src/error.rs` — Typed decode errors (clean copy, no deps)
- `crates/loom-ir-core/src/l2_core.rs` — L2Core model with local L2DataType replacing arrow_schema::DataType
- `crates/loom-ir-core/src/l2_core/constraints.rs` — Constraint model (clean copy)
- `crates/loom-ir-core/src/l2core_codec.rs` — IR codec with L2DataType replacing DataType
- `crates/loom-ir-core/src/full_verifier.rs` — L2Core verifier with L2DataType replacing DataType
- `crates/loom-container/Cargo.toml` — Full Arrow stack + loom-ir-core dep
- `crates/loom-container/src/lib.rs` — Declares 22 modules + type bridge functions
- `crates/loom-container/src/*.rs` — 22 container-layer modules + l1_model/bitpack.rs submodule
- `crates/loom-container/src/verifier.rs` — MVP0 structural verifier (kept in container, not ir-core)
- `crates/loom-container/src/runtime_abi.rs` — Runtime ABI (kept in container, depends on artifact_verifier)

## Decisions Made

- **verifier.rs stays in container**: The MVP0 structural verifier imports `container_codec`, `table_codec`, `layout_codec`, `l1_model`, `l2_kernel_registry`, `alp_params`, and `fsst_params` — all container-layer modules. Moving it to ir-core would create a circular or incorrect dependency.
- **runtime_abi.rs stays in container**: Imports `ArtifactVerificationStatus` from `artifact_verifier`, which is container-layer. Moving it would require extracting verification status types into ir-core, which is deferred to future plans.
- **L2DataType local enum**: Defined with exactly 6 variants (Boolean/Int32/Int64/Float32/Float64/Utf8) matching the narrow Arrow subset that the L2Core codec supports. The wire format is unchanged — byte tags 0-5 map identically.
- **FNV version 1.0.7**: The plan specified `=2.1.0` which doesn't exist on crates.io. Corrected to the latest actual version `=1.0.7`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] L2DataType defined to replace arrow_schema::DataType**
- **Found during:** Task 1 (l2_core.rs copy)
- **Issue:** `l2_core.rs`, `l2core_codec.rs`, and `full_verifier.rs` all import `arrow_schema::DataType`, violating must-have constraint "zero arrow-* dependencies in loom-ir-core"
- **Fix:** Defined local `L2DataType` enum in `l2_core.rs` with 6 supported variants; replaced all `DataType` references in the ir-core copies; added `l2_to_arrow`/`arrow_to_l2` bridge functions in container crate for boundary conversion
- **Files modified:** `l2_core.rs`, `l2core_codec.rs`, `full_verifier.rs` (ir-core copies); `lib.rs`, `kloom_harness.rs`, `native_lowering.rs`, `production_native_lowering.rs`, `decode_dialect.rs`, `arrow_buffer_lowering.rs`, `native_arrow_semantic.rs` (container)
- **Committed in:** `2f8e143` (Task 1) and `e0d0361` (Task 2)

**2. [Rule 3 - Blocking] fnv version 2.1.0 does not exist on crates.io**
- **Found during:** Task 1 (first cargo build)
- **Issue:** Plan specified `fnv = { version = "=2.1.0" }` but crates.io only has versions up to 1.0.7
- **Fix:** Changed to `fnv = { version = "=1.0.7" }`
- **Files modified:** `Cargo.toml`
- **Committed in:** `2f8e143` (Task 1)

### Architectural Deviations

**1. [Rule 4 - Structural] verifier.rs and runtime_abi.rs assigned to loom-container, not loom-ir-core**
- **Found during:** Task 1 (module dependency analysis)
- **Issue:** Plan assigned `verifier.rs` and `runtime_abi.rs` to ir-core, but:
  - `verifier.rs` imports `container_codec`, `table_codec`, `alp_params`, `fsst_params`, `l1_model`, `l2_kernel_registry` — all container-layer
  - `runtime_abi.rs` imports `ArtifactVerificationStatus` from `artifact_verifier` — container-layer
- **Fix:** Kept both files in loom-container. No type changes. The ir-core crate remains clean with zero container or Arrow deps.
- **Files modified:** `loom-container/src/verifier.rs`, `loom-container/src/runtime_abi.rs` (copied but not modified); `loom-ir-core/src/lib.rs` (omitted these modules)
- **Impact:** Architectural boundary preserved correctly; these modules correctly belong in the container layer per their actual dependency graph.

**2. [Rule 4 - Structural] Type mismatch cascade across container modules due to L2DataType**
- **Found during:** Task 2 (cargo build)
- **Issue:** Container modules that reference IR types (e.g., `OutputBuilderCapability.arrow_type`, `OutputSchemaFact.arrow_type`, `ProductionColumnShape.arrow_type`) now receive `L2DataType` from loom-ir-core instead of `arrow_schema::DataType`
- **Fix:** Changed `ProductionColumnShape.arrow_type` to `L2DataType`; updated `is_supported_primitive` to accept `&L2DataType`; added `crate::l2_to_arrow()` conversions at Arrow builder boundary in `decode_dialect.rs`, `arrow_buffer_lowering.rs`, and `native_arrow_semantic.rs`
- **Files modified:** `production_native_lowering.rs`, `decode_dialect.rs`, `arrow_buffer_lowering.rs`, `native_arrow_semantic.rs`, `kloom_harness.rs`, `native_lowering.rs`
- **Impact:** Minimal — 6 files with targeted edits; no API surface widened beyond the container crate

---

**Total deviations:** 4 (2 auto-fixed bugs/blockers, 2 architectural)
**Impact on plan:** All auto-fixes and architectural adjustments were necessary for correctness. The ir-core/container boundary is now correctly enforced: zero Arrow deps in ir-core, clean dependency direction. No scope creep.

## Issues Encountered

- Multiple import path fix rounds were needed during Task 2 (`crate::` → `loom_ir_core::` for IR-layer modules). Batch sed across 22 files handled this efficiently.
- Type bridge between `L2DataType` and `arrow_schema::DataType` required several iterations to identify all affected call sites in the container layer.

## Next Phase Readiness

- Both new crates compile independently and pass all existing tests
- Original `loom-core` files untouched — ready for Plan 50-01 (re-export shim and downstream rewiring)
- `loom-ir-core` has verified zero Arrow/container deps via `cargo tree`
- `loom-container → loom-ir-core` dependency edge verified; no reverse dependency
- Sidecar files (`sidecar.rs`, `sidecar_routing.rs`) do not exist yet — they are part of Plan 50-02

---
*Phase: 50-sidecar-overlay-model-and-host-native-reader-fallback*
*Completed: 2026-06-11*
