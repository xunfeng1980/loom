---
phase: 04-l1-dict-rle-and-l2-escape-infrastructure
plan: "01"
subsystem: core
tags: [rust, arrow, dictionary, run-end, l2-kernel-registry]
requires:
  - phase: 03-l1-bitpack-for-and-arrow-builders
    provides: bitpack/FOR read-loop and Arrow OutputBuilder foundation
provides:
  - Boolean OutputBuilder support
  - Dictionary and RunEnd L1 decode support
  - L2KernelRegistry with FSST id 0 stub
  - registry-backed KernelEscape ArrayData routing
  - FOR-over-non-BitPack reference handling
affects: [loom-core, loom-fixtures, phase-04-wave-2]
tech-stack:
  added: []
  patterns: [typed Arrow child materialization, registry-dispatched kernel-owned ArrayData]
key-files:
  created:
    - crates/loom-core/src/l2_kernel_registry.rs
  modified:
    - crates/loom-core/src/arrow_builder_output.rs
    - crates/loom-core/src/error.rs
    - crates/loom-core/src/l1_model.rs
    - crates/loom-core/src/lib.rs
key-decisions:
  - "KernelEscape routing is exposed through decode_layout_to_array_data because L2 kernels own their Arrow ArrayData."
  - "OutputBuilder supports Boolean but intentionally does not support Utf8/String in Phase 4."
  - "Dictionary and RunEnd children are decoded recursively to temporary Arrow arrays, then appended with null propagation through OutputBuilder."
patterns-established:
  - "DecodedArray adapter: recursive child LayoutNode output is materialized to typed Arrow arrays before parent encodings read values/nulls."
  - "L2KernelRegistry id lookup returns Option and unknown ids become LoomDecodeError::UnknownKernel."
requirements-completed: [L1-05, L1-06, L2-01]
duration: 14min
completed: 2026-06-07
---

# Phase 04-01: L1 Dict/RLE and L2 Registry Summary

**loom-core now decodes dictionary and RunEnd layouts, routes KernelEscape id 0 through an empty Utf8 FSST stub, and preserves Arrow nulls across recursive L1 children**

## Performance

- **Duration:** 14 min
- **Started:** 2026-06-07T15:48:45Z
- **Completed:** 2026-06-07T16:02:41Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `OutputBuilder::Boolean`, `append_bool`, builder type introspection, and Phase-4 typed decode errors.
- Replaced Dictionary and RunEnd unimplemented arms with recursive child decode, bounds validation, null propagation, integer lookup, and boolean RunEnd expansion.
- Added `L2Kernel`, `L2KernelRegistry::default_for_mvp0`, FSST id 0 empty Utf8 stub, unknown-kernel errors, and `decode_layout_to_array_data`.
- Fixed FOR over non-BitPack children by materializing the child array, applying the reference, and preserving child nulls.

## Task Commits

1. **Task 1-3: Phase-4 loom-core behavior** - `5410d63` (feat)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/loom-core/src/l2_kernel_registry.rs` - L2 kernel trait, registry, FSST id 0 stub, and registry tests.
- `crates/loom-core/src/arrow_builder_output.rs` - Boolean builder support and output type reporting.
- `crates/loom-core/src/error.rs` - typed dictionary, RunEnd, builder, and kernel errors.
- `crates/loom-core/src/l1_model.rs` - Dictionary, RunEnd, KernelEscape helper, and FOR-over-child decode behavior with tests.
- `crates/loom-core/src/lib.rs` - exposes the new `l2_kernel_registry` module file.

## Decisions Made

- Top-level KernelEscape uses `decode_layout_to_array_data`; direct `synthesized_read_loop` calls still return a typed `UnimplementedEncoding("KernelEscape")` because builder-backed output cannot append Utf8 in Phase 4.
- FSST remains a stub that returns zero-length Utf8 `ArrayData`; real decompression is intentionally deferred.
- Unit tests use raw integer code layouts for nonzero dictionary lookup and bitpack validity for null-routing fixtures, keeping each behavior explicit.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] RunEnd validation initially stopped after reaching count**
- **Found during:** Task 2 acceptance tests
- **Issue:** A malformed extra non-monotonic run end was not inspected once the declared output count had already been reached.
- **Fix:** Validate every supplied run end and return typed errors for non-monotonic or out-of-bounds trailing entries.
- **Files modified:** `crates/loom-core/src/l1_model.rs`
- **Verification:** `RUSTC_WRAPPER= cargo test -p loom-core run_end`
- **Committed in:** `5410d63`

**2. [Rule 3 - Blocking] Unit fixtures conflated bitpack packing with dictionary lookup**
- **Found during:** Task 2 acceptance tests
- **Issue:** New dictionary/RLE tests used nonzero values through the local bitpack test helper, making failures ambiguous between bitpack fixture packing and dictionary behavior.
- **Fix:** Split coverage: raw integer code tests prove nonzero dictionary lookup, and bitpack-validity tests prove null propagation.
- **Files modified:** `crates/loom-core/src/l1_model.rs`
- **Verification:** `RUSTC_WRAPPER= cargo test -p loom-core dictionary`
- **Committed in:** `5410d63`

---

**Total deviations:** 2 auto-fixed (blocking correctness/verification issues).
**Impact on plan:** Both fixes strengthened the planned behavior without changing the Phase-4 API scope.

## Issues Encountered

- Cargo initially failed under the sandbox because `sccache` could not spawn. Verification succeeded by clearing the wrapper: `RUSTC_WRAPPER= cargo test ...`.

## Verification

- `RUSTC_WRAPPER= cargo test -p loom-core` - PASS, 40 unit tests plus doc tests.
- `RUSTC_WRAPPER= cargo tree -p loom-core | grep -c -E 'vortex|fastlanes'` - PASS, printed `0`.
- `rg -n "todo!|unimplemented!" crates/loom-core/src/l1_model.rs crates/loom-core/src/l2_kernel_registry.rs` - PASS, no production stubs introduced.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Wave 2 can now add `loom-fixtures` bridges and oracle tests against the public `decode_layout_to_array_data`, Dictionary, RunEnd, and L2 registry behavior. No DuckDB or FFI rewiring was required for this plan.

---
*Phase: 04-l1-dict-rle-and-l2-escape-infrastructure*
*Completed: 2026-06-07*
