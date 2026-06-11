---
phase: 50-sidecar-overlay-model-and-host-native-reader-fallback
plan: "03"
subsystem: sidecar-routing
tags: [sidecar, routing, content-hash, fnv1a, fail-closed, loom-ir-core]

# Dependency graph
requires:
  - phase: 50-01
    provides: SidecarOverlay, ChunkBinding types in sidecar.rs
  - phase: 49
    provides: l2core_codec FNV-1a hash pattern, l2core_program_hash format
provides:
  - decide_sidecar_routing implementing 4-gate fail-closed routing logic
  - compute_chunk_hash and verify_chunk_binding content-hash helpers
  - HashVerificationResult type for per-granule hash verification
  - SidecarDiagnostic with stable code/path/message
affects: [50-04, sidecar-routing, host-native-reader-fallback]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "4-gate exhaustive routing: engine → sidecar → hash → encoding → decision"
    - "FNV-1a content-hash over raw host data bytes matching l2core_program_hash algorithm family"
    - "HashVerificationResult co-located with hash computation in sidecar.rs"
    - "SidecarDiagnostic with typed code + JSONPath-style location + message"

key-files:
  created:
    - crates/loom-ir-core/src/sidecar_routing.rs - 4-gate routing decision logic + 8 tests
  modified:
    - crates/loom-ir-core/src/sidecar.rs - compute_chunk_hash, verify_chunk_binding, HashVerificationResult
    - crates/loom-ir-core/src/lib.rs - pub mod sidecar_routing
    - crates/loom-core/src/lib.rs - re-export sidecar_routing

key-decisions:
  - "HashVerificationResult defined in sidecar.rs alongside hash computation, re-imported by sidecar_routing.rs"
  - "FNV-1a via fnv::FnvHasher crate for content-hash — matches l2core_program_hash algorithm family"
  - "Routing is exhaustive: every code path returns LoomNative or HostNativeReader with typed reason"
  - "encoding_supported is a bool input — actual encoding support checking is the caller's responsibility"
  - "No DuckDB/Parquet/Vortex/Lance imports in routing module — stays host-neutral"

patterns-established:
  - "4-gate fail-closed routing: engine_integrated → sidecar presence → hash match → encoding support"
  - "Content-hash uses l2ir:<hex> format matching existing L2Core IR identity convention"
  - "Diagnostics use JSONPath-style paths ($.engine, $.sidecar, $.hash.<granule_id>)"

requirements-completed: []

# Metrics
duration: 7min
completed: 2026-06-11
status: complete
---

# Phase 50 Plan 03: Fail-Closed Sidecar Routing Decision and Content-Hash Verification Summary

**4-gate fail-closed sidecar routing logic with FNV-1a content-hash verification over host data byte ranges**

## Performance

- **Duration:** 7 min
- **Started:** 2026-06-11T08:48:12Z
- **Completed:** 2026-06-11T08:55:23Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- `decide_sidecar_routing` implements exhaustive 4-gate logic: engine_integrated → sidecar presence → hash match → encoding support, returning either `LoomNative` or `HostNativeReader` with a specific typed reason
- `compute_chunk_hash` and `verify_chunk_binding` provide FNV-1a content-hash computation over raw host data bytes in `l2ir:<hex>` format
- 8 routing decision tests cover all gate paths: 4 positive (all-gates-pass, empty-bindings, multi-binding, no-mismatches) and 4 negative (engine-not-integrated, no-sidecar, hash-mismatch, encoding-unsupported)
- Full workspace builds clean; no test regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Create sidecar_routing.rs — 4-gate routing decision logic** - `ee3c20c` (feat)
2. **Task 2: Add content-hash verification helpers to sidecar.rs** - `2ff77b3` (feat)
3. **Task 3: Wire sidecar_routing into lib.rs and add tests** - `46ad17a` (test RED) + `6f49979` (feat GREEN)

_Task 3 followed TDD cycle: RED (failing tests) → GREEN (module wiring)_

## Files Created/Modified

- `crates/loom-ir-core/src/sidecar_routing.rs` - SidecarRoutingInput, SidecarRoutingDecision, HostNativeReaderReason, SidecarDiagnostic, SidecarDiagnosticCode, decide_sidecar_routing, 8 tests
- `crates/loom-ir-core/src/sidecar.rs` - HashVerificationResult, compute_chunk_hash, verify_chunk_binding
- `crates/loom-ir-core/src/lib.rs` - Added `pub mod sidecar_routing`
- `crates/loom-core/src/lib.rs` - Uncommented `pub use loom_ir_core::sidecar_routing`

## Decisions Made

- `HashVerificationResult` defined in `sidecar.rs` alongside hash computation helpers — cleaner module boundaries with hash-related types co-located
- Used `fnv::FnvHasher` from the existing workspace `fnv` dependency for FNV-1a hashing — matches the `l2core_program_hash` algorithm family
- Routing decision is exhaustive — every code path returns exactly one `SidecarRoutingDecision` variant; no `Option` or partial states
- `encoding_supported` is a `bool` input to `decide_sidecar_routing` — the caller (host adapter or runtime ABI) performs the actual IR decode and encoding support verification
- No DuckDB, Parquet, Vortex, or Lance imports in the routing module — stays host-neutral per repositioning design

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed duplicate HashVerificationResult import in sidecar_routing.rs**
- **Found during:** Task 3 (GREEN phase — module wiring)
- **Issue:** Both `use crate::sidecar::HashVerificationResult` (line 27) and `pub use crate::sidecar::HashVerificationResult` (line 50) were present, causing E0252 "name defined multiple times" compilation error
- **Fix:** Removed the redundant `pub use` re-export; kept the `use` import for local scope
- **Files modified:** `crates/loom-ir-core/src/sidecar_routing.rs`
- **Verification:** `cargo test -p loom-ir-core -- sidecar_routing` passes all 8 tests
- **Committed in:** `6f49979` (Task 3 GREEN commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Trivial; fixed during module wiring with no scope change.

## Issues Encountered

None — plan executed as designed with one trivial typo-level fix.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Sidecar routing decision logic is complete and tested — ready for Plan 50-04 (adapters consume routing decisions)
- `compute_chunk_hash` and `verify_chunk_binding` helpers are available — host adapters can now verify content-hash bindings against actual host data
- Routing module is re-exported from `loom-core` — downstream consumers have stable access

---
*Phase: 50-sidecar-overlay-model-and-host-native-reader-fallback*
*Completed: 2026-06-11*
