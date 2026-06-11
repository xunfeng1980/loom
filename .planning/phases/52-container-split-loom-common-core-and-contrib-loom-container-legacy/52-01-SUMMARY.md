---
phase: 52-container-split-loom-common-core-and-contrib-loom-container-legacy
plan: 01
subsystem: infra
tags: [rust, crate-split, refactoring, loom-common, production-core]

# Dependency graph
requires:
  - phase: 49
    provides: Independent L2Core IR codec and content-hash identity
  - phase: 50
    provides: Sidecar overlay model and host-native reader fallback
provides:
  - crates/loom-common with 17 production-core modules (zero legacy container deps)
affects: [52-02, loom-core, loom-container]

# Tech tracking
tech-stack:
  added: [loom-common crate]
  patterns: [crate split, module re-export pattern, extraction of type-only modules]

key-files:
  created:
    - crates/loom-common/Cargo.toml
    - crates/loom-common/src/lib.rs
    - crates/loom-common/src/artifact_types.rs
    - crates/loom-common/src/verify_layout_types.rs
    - crates/loom-common/src/arrow_semantic.rs
    - crates/loom-common/src/arrow_semantic_codec.rs
    - crates/loom-common/src/arrow_semantic_verifier.rs
    - crates/loom-common/src/native_arrow_semantic.rs
    - crates/loom-common/src/arrow_buffer_lowering.rs
    - crates/loom-common/src/native_lowering.rs
    - crates/loom-common/src/production_native_lowering.rs
    - crates/loom-common/src/decode_dialect.rs
    - crates/loom-common/src/runtime_abi.rs
    - crates/loom-common/src/l1_model.rs
    - crates/loom-common/src/l1_model/bitpack.rs
    - crates/loom-common/src/l2_kernel_registry.rs
    - crates/loom-common/src/fsst_params.rs
    - crates/loom-common/src/alp_params.rs
    - crates/loom-common/src/arrow_builder_output.rs
    - crates/loom-common/src/kloom_harness.rs
  modified:
    - Cargo.toml (workspace members)
    - Cargo.lock
    - crates/loom-container/Cargo.toml (added loom-common dep)
    - crates/loom-container/src/lib.rs (replaced 15 module declarations with loom_common re-exports)
    - crates/loom-container/src/artifact_verifier.rs (replaced type defs with pub use loom_common)
    - crates/loom-container/src/verifier.rs (replaced extracted content with pub use loom_common; kept verify_table/verify_container)
    - crates/loom-core/Cargo.toml (added loom-common dep)
    - crates/loom-core/src/lib.rs (split re-exports between loom-common and loom-container)

key-decisions:
  - "Split artifact_verifier.rs: types → loom-common, container-dependent functions stay in loom-container"
  - "Split verifier.rs: verify_layout + types → loom-common, verify_table/verify_container stay in loom-container"
  - "Duplicate verify_artifact in loom-common (Arrow-semantic paths only) because native_arrow_semantic.rs needs it; full LMC1 path stays in loom-container"
  - "Replaced module declarations with pub use re-exports in loom-container/lib.rs to avoid type conflicts between crate-local and loom_common copies"

# Metrics
duration: 10 min
completed: 2026-06-11
status: complete
---

# Phase 52 Plan 01: Create loom-common — Production-Core Crate Summary

**Created `crates/loom-common` with 17 production-core modules (15 copied, 2 type-extracted) — zero dependency on legacy container packaging layer**

## Performance

- **Duration:** 10 min
- **Started:** 2026-06-11T12:03:22Z
- **Completed:** 2026-06-11T12:13:01Z
- **Tasks:** 2
- **Files modified/created:** 28

## Accomplishments
- Created `crates/loom-common` workspace member with full Cargo.toml depending on loom-ir-core and arrow deps
- Copied 15 clean production modules from loom-container to loom-common (arrow_semantic*, native*, runtime_abi, l1_model, l2_kernel_registry, etc.)
- Extracted `artifact_types.rs` (250 lines of pure type definitions) from artifact_verifier.rs
- Extracted `verify_layout_types.rs` (verify_layout + VerificationReport/Code/Diagnostic + 12 helper functions) from verifier.rs
- All 5 isolation gates pass: zero container_codec/layout_codec/table_codec references in loom-common source
- Full workspace `cargo check` passes with zero errors; all 55 loom-container tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Create loom-common crate, move 15 clean modules, extract 2 type modules** - `af3aabd` (feat)
2. **Task 2: Verify downstream crate compilation and gate loom-common isolation** - No code changes (verification-only task, all gates pass)

## Isolation Gate Results

| Gate | Description | Result |
|------|-------------|--------|
| G1 | container_codec/layout_codec/table_codec refs in loom-common/src (excl verify_layout_types) | 0 lines |
| G2 | container imports in artifact_types.rs | 0 matches |
| G3 | container imports in verify_layout_types.rs | 0 matches |
| G4 | contrib/loom-container in loom-common dep tree | 0 matches |
| G5 | loom-common module count (minimum 16) | 17 modules |
| G6 | contrib in loom-common inverted dep tree | 0 matches |

## Files Created/Modified
- `crates/loom-common/Cargo.toml` - New crate manifest with loom-ir-core and arrow workspace deps
- `crates/loom-common/src/lib.rs` - Module declarations for 17 modules + l2_to_arrow/arrow_to_l2 utilities
- `crates/loom-common/src/artifact_types.rs` - ArtifactVerificationStatus/Report/Facts/Options types extracted from artifact_verifier.rs
- `crates/loom-common/src/verify_layout_types.rs` - verify_layout function + VerificationReport/Code/Diagnostic + internal helpers
- `crates/loom-common/src/{arrow_semantic,arrow_semantic_codec,arrow_semantic_verifier,native_arrow_semantic,arrow_buffer_lowering,native_lowering,production_native_lowering,decode_dialect,runtime_abi,l1_model,l2_kernel_registry,fsst_params,alp_params,arrow_builder_output,kloom_harness}.rs` - Clean module copies from loom-container
- `Cargo.toml` - Added "crates/loom-common" to workspace members
- `crates/loom-container/Cargo.toml` - Added loom-common dependency
- `crates/loom-container/src/lib.rs` - Replaced 15 pub mod declarations with pub use re-exports from loom_common
- `crates/loom-container/src/artifact_verifier.rs` - Replaced type definitions with pub use loom_common::artifact_types::*
- `crates/loom-container/src/verifier.rs` - Replaced extracted content with pub use loom_common::verify_layout_types::*; kept verify_table/verify_container
- `crates/loom-core/Cargo.toml` - Added loom-common dependency
- `crates/loom-core/src/lib.rs` - Split re-exports: 17 modules from loom_common, 6 from loom-container

## Decisions Made
- **Type-only vs function split in artifact_verifier.rs:** ArtifactVerificationStage/Status/Diagnostic/Report/Facts/Options types → loom-common. verify_artifact (LMC1 path), verify_artifact_with_l2_core → stay in loom-container.
- **verify_layout extraction:** verify_layout + all 12 internal helpers → loom-common. verify_table (needs TableDescription) and verify_container (needs container_codec) → stay in loom-container.
- **Duplicate verify_artifact in loom-common:** Because native_arrow_semantic.rs needs verify_artifact and it can't depend on loom-container (circular), the Arrow-semantic paths (LMC2/LMA1 detection) were duplicated in loom-common's artifact_types.rs. The full LMC1 path stays in loom-container.
- **Module re-export approach:** Instead of keeping both crate-local module copies AND loom_common types (which caused type conflicts), loom-container/src/lib.rs now re-exports all 17 modules from loom_common. The copied files remain on disk for plan 52-02 cleanup.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Type conflicts between loom-container local copies and loom-common types**
- **Found during:** Task 1 (cargo check -p loom-core)
- **Issue:** When loom-container had both local module declarations (pub mod l1_model) AND re-exports from loom_common (pub use loom_common::verify_layout_types::*), the types were incompatible (e.g., l1_model::LayoutDescription vs loom_common::l1_model::LayoutDescription). This caused 3 E0308 errors.
- **Fix:** Replaced all 15 moved module declarations in loom-container/src/lib.rs with pub use re-exports from loom_common. The original files remain on disk (for plan 52-02 removal) but are no longer compiled as separate modules.
- **Files modified:** crates/loom-container/src/lib.rs
- **Verification:** cargo check --workspace passes with zero errors
- **Committed in:** af3aabd (Task 1 commit)

**2. [Rule 3 - Blocking] verify_artifact function not available in loom-common**
- **Found during:** Task 1 (import resolution in native_arrow_semantic.rs)
- **Issue:** native_arrow_semantic.rs calls verify_artifact() which depends on container_codec for the LMC1 path. The plan said verify_artifact stays in loom-container but native_arrow_semantic is now in loom-common.
- **Fix:** Added verify_artifact (Arrow-semantic paths only) to artifact_types.rs in loom-common. The LMC1 path returns Unsupported. The full LMC1-capable verify_artifact in loom-container shadows this version via pub use loom_common::artifact_types::* plus a local override.
- **Files modified:** crates/loom-common/src/artifact_types.rs
- **Verification:** cargo check -p loom-common passes; native_arrow_semantic.rs compiles
- **Committed in:** af3aabd (Task 1 commit)

**3. [Rule 2 - Missing Critical] Missing test imports after verifier.rs refactor**
- **Found during:** Task 1 (cargo test -p loom-container)
- **Issue:** The verifier.rs test module used `use super::*;` which previously imported DataType, LayoutDescription, LayoutNode from the local module definitions. After replacing extracted content with pub use loom_common::*, these types were no longer in scope.
- **Fix:** Added explicit imports: use arrow_schema::DataType; use crate::l1_model::{LayoutDescription, LayoutNode};
- **Files modified:** crates/loom-container/src/verifier.rs
- **Verification:** cargo test -p loom-container passes (55/55)
- **Committed in:** af3aabd (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking/missing-critical)
**Impact on plan:** All auto-fixes necessary for compilation correctness. Module re-export approach is a cleaner architecture than the original plan's dual-copy strategy. No scope creep.

## Issues Encountered
- The plan's "copy with cp, keep module declarations" strategy would have caused unresolvable type conflicts between loom-container and loom-common types. The re-export approach (Rule 3 fix #1) resolves this elegantly.
- verify_artifact function duplication was necessary because loom-common can't depend on loom-container. Plan 52-02 should clean this up by either moving the Arrow-semantic paths permanently or providing a callback mechanism.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- loom-common is fully isolated (zero legacy container deps) — ready for plan 52-02
- All 17 modules compile independently
- All 5 isolation gates documented and passing
- Plan 52-02 should: remove copied files from loom-container/src/, create contrib/loom-container, and complete the split

---

*Phase: 52-container-split-loom-common-core-and-contrib-loom-container-legacy*
*Completed: 2026-06-11*
