---
phase: 23-production-native-backend-implementation
plan: 02
subsystem: native-backend
tags: [mlir, ods, tablegen, loom-decode, manifest, drift-tests]

requires:
  - phase: 23-production-native-backend-implementation
    provides: 23-01 backend contract and runtime-plan bridge
provides:
  - ODS/TableGen source evidence for the `loom.decode` dialect
  - Rust decode dialect manifest and drift tests
  - Phase 23 production backend gate script with strict ODS validation
affects: [phase-23, phase-24, native-backend, llvm-toolchain]

tech-stack:
  added: []
  patterns: [default MLIR-free manifest tests, strict mlir-tblgen validation]

key-files:
  created:
    - crates/loom-native-melior/mlir/include/LoomDecode/LoomDecodeDialect.td
    - crates/loom-native-melior/mlir/include/LoomDecode/LoomDecodeOps.td
    - crates/loom-native-melior/src/decode_dialect_manifest.rs
    - crates/loom-native-melior/tests/decode_dialect_manifest.rs
    - scripts/production-backend-test.sh
  modified:
    - crates/loom-native-melior/src/lib.rs

key-decisions:
  - "ODS evidence lives in loom-native-melior and does not enter loom-core."
  - "Default manifest tests use CARGO_MANIFEST_DIR and do not require mlir-tblgen."
  - "Strict ODS validation uses mlir-tblgen when managed LLVM/MLIR tooling is available."

patterns-established:
  - "Decode dialect manifest maps textual op names to ODS records and backend dispositions."
  - "Production backend gate separates default Rust tests from strict toolchain checks."

requirements-completed: []

duration: 5min
completed: 2026-06-08
---

# Phase 23-02: Compiled `loom.decode` ODS Dialect Evidence Summary

**ODS/TableGen source evidence and MLIR-free drift tests for the Phase 20 textual `loom.decode` surface**

## Performance

- **Duration:** 5 min
- **Started:** 2026-06-08T14:58:03Z
- **Completed:** 2026-06-08T15:02:38Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `LoomDecodeDialect.td` and `LoomDecodeOps.td` for the `loom.decode`
  dialect and the Phase 20/21 primitive operation surface.
- Added `decode_dialect_manifest` with stable textual names, ODS records,
  attributes, ODS source paths, and dispositions for structural,
  native-supported, declared-guarded, interpreter-only, and deferred ops.
- Added tests that compare the manifest against `loom_core::decode_dialect`,
  verify ODS source records, and prove default tests do not require `mlir-tblgen`.
- Added `scripts/production-backend-test.sh`, which runs Phase 23 backend tests
  and performs strict `mlir-tblgen` validation when the managed LLVM/MLIR
  toolchain is present.

## Task Commits

1. **Task 1: Add ODS dialect and operation sources** - `a7a773e`
2. **Task 2: Add dialect manifest and drift tests** - `b4a577b`
3. **Task 3: Add optional strict TableGen validation** - `31d7070`
4. **Plan metadata:** pending summary commit

## Files Created/Modified

- `crates/loom-native-melior/mlir/include/LoomDecode/LoomDecodeDialect.td` - ODS dialect definition.
- `crates/loom-native-melior/mlir/include/LoomDecode/LoomDecodeOps.td` - ODS operation definitions.
- `crates/loom-native-melior/src/decode_dialect_manifest.rs` - Rust manifest for textual/ODS drift checks.
- `crates/loom-native-melior/tests/decode_dialect_manifest.rs` - Manifest and source evidence tests.
- `crates/loom-native-melior/src/lib.rs` - Exposes the manifest module.
- `scripts/production-backend-test.sh` - Phase 23 backend gate script.

## Decisions Made

- Kept ODS as source evidence under `loom-native-melior`, preserving default
  workspace builds without mandatory LLVM/MLIR.
- Used manifest-level dispositions to make supported vs guarded vs
  interpreter/deferred op status explicit.
- Made `mlir-tblgen` validation discoverable and executable through
  `scripts/production-backend-test.sh`.

## Deviations from Plan

The plan said "add or prepare" strict TableGen validation. The local managed
toolchain was available, so the gate now actually runs `mlir-tblgen`.

During strict validation, `SymbolTable` was not available as an ODS trait in the
used TableGen context. The module op was simplified to `IsolatedFromAbove`, which
keeps the source evidence focused and lets TableGen generate op declarations.

**Total deviations:** 1 validation-driven ODS simplification.  
**Impact on plan:** Positive; strict validation is stronger than the minimum
planned evidence and now catches ODS drift early.

## Issues Encountered

Initial manifest tests assumed repo-root cwd. Cargo integration tests resolved
paths differently, so ODS path resolution now uses `CARGO_MANIFEST_DIR`.

## User Setup Required

None. Strict ODS validation uses existing managed LLVM/MLIR tooling when present;
explicit skip remains available through `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`.

## Verification

- `rg -n "def LoomDecode_Dialect|loom.decode|Raw|Bitpack|FrameOfReference|Verifier" crates/loom-native-melior/mlir/include/LoomDecode`
- `cargo test -p loom-native-melior --test decode_dialect_manifest`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-native-melior --test decode_dialect_manifest`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 scripts/production-backend-test.sh`
- `git diff --check`

All verification passed. `scripts/production-backend-test.sh` also ran strict
`mlir-tblgen` validation on this machine because LLVM/MLIR tooling was available.

## Next Phase Readiness

Phase 23 can proceed to 23-03. The next plan should wire validated backend
requests into the production melior/LLVM lowering pipeline and include pipeline
identity in backend reports.

---
*Phase: 23-production-native-backend-implementation*
*Completed: 2026-06-08*
