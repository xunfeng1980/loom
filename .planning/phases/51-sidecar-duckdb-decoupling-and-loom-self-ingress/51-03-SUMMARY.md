---
phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
plan: 03
subsystem: build
tags: [cmake, duckdb, sidecar, ffi, staticlib, decoupling, dependency-boundary]

# Dependency graph
requires:
  - phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
    plan: 01
    provides: "Lean loom-sidecar-ffi staticlib with C ABI for sidecar extract/verify/route"
  - phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
    plan: 02
    provides: "loom-self-ingress crate, feature-gated loom-cli lean compilation"
provides:
  - "LOOM_SIDECAR_ONLY CMake option for DuckDB extension build"
  - "Sidecar-aware loom_scan that extracts/routes sidecar overlays in lean mode"
  - "Verified dependency boundaries: zero container in sidecar/lean paths"
affects: [51-sidecar-duckdb-decoupling-and-loom-self-ingress]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CMake option LOOM_SIDECAR_ONLY switches between libloom_sidecar_ffi.a and libloom_ffi.a at build time"
    - "Preprocessor-gated C++ source with #ifdef LOOM_SIDECAR_ONLY for two compile modes in one file"

key-files:
  modified:
    - "contrib/duckdb-ext/CMakeLists.txt — option(LOOM_SIDECAR_ONLY), conditional cargo build, LLVM guard, sidecar include path"
    - "contrib/duckdb-ext/loom_extension.cpp — sidecar-aware scan path with SidecarBind/SidecarScan, full mode in #else"
    - "crates/loom-ffi/include/loom.h — documentation comment pointing to loom_sidecar.h"

key-decisions:
  - "Sidecar-only DuckDB extension returns VARCHAR diagnostic rows rather than attempting Arrow decode — preserves dependency boundary as the primary value proof"
  - "lvalue-as-right-value LLVM link: LOOM_SIDECAR_ONLY=ON reuses LIBLOOM_FFI variable name (reassigned to LIBLOOM_SIDECAR_FFI) so downstream target_link_libraries works unchanged"
  - "Full mode existing code wrapped in #else block rather than separate compilation unit — single source file, two compile modes"

requirements-completed: [SC-2, SC-5]

# Metrics
duration: 7 min
completed: 2026-06-11
status: complete
---

# Phase 51 Plan 03: DuckDB Extension Sidecar Build Path and Dependency Boundary Verification Summary

**Added a `LOOM_SIDECAR_ONLY` CMake option to the DuckDB extension build system, enabling it to link the lean `libloom_sidecar_ffi.a` instead of the full `libloom_ffi.a`. Updated the C++ extension source with a sidecar-aware scan path that extracts sidecar overlays, evaluates routing decisions, and returns diagnostic information — all without linking loom-container, codecs, or LLVM/MLIR. Verified all dependency boundaries with workspace build and `cargo tree` greps.**

## Performance

- **Duration:** 7 min
- **Started:** 2026-06-11T10:56:00Z
- **Completed:** 2026-06-11T11:03:37Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- `LOOM_SIDECAR_ONLY=ON` bypasses llvm-config requirement and links only `libloom_sidecar_ffi.a` — confirmed via successful `cmake -S contrib/duckdb-ext -B build/sidecar -DLOOM_SIDECAR_ONLY=ON` configure step
- Sidecar-aware `loom_scan` extracts sidecar overlays from Parquet files, evaluates 4-gate routing (LoomNative / HostNativeReader / NoSidecar), and returns diagnostic VARCHAR rows
- Full mode (default) unchanged: existing Arrow decode, native codegen, Arrow semantic paths all functional
- `cargo tree -p loom-sidecar-ffi | grep loom-container` → zero lines; `cargo tree -p loom-cli --no-default-features | grep loom-container` → zero lines; `cargo tree -p loom-ffi | grep loom-container` → found (existing path intact)
- `cargo test -p loom-ir-core -p loom-parquet-ingress -p loom-container` all pass
- CLI lean mode: `sidecar embed` works, `inspect` fails with "requires full feature"

## Task Commits

Each task was committed atomically:

1. **Task 1: Add LOOM_SIDECAR_ONLY CMake option and sidecar build path** — `e37d6d2` (feat)
2. **Task 2: Add sidecar-aware scan path to DuckDB extension C++ source** — `d7f6143` (feat)
3. **Task 3: Full workspace build, test, and dependency boundary verification** — `de05614` (feat)

## Files Modified

- `contrib/duckdb-ext/CMakeLists.txt` — Added `option(LOOM_SIDECAR_ONLY)`, sidecar staticlib path, conditional cargo build target, LLVM guard wrapping, sidecar include directory, `target_compile_definitions`
- `contrib/duckdb-ext/loom_extension.cpp` — Added `#ifdef LOOM_SIDECAR_ONLY` section with SidecarBind/SidecarInit/SidecarScan using `loom_sidecar_extract`/`loom_sidecar_route`/`loom_sidecar_free_bytes`; wrapped existing full-mode code in `#else`; added `#endif` at file end
- `crates/loom-ffi/include/loom.h` — Added documentation comment pointing sidecar-aware consumers to `loom_sidecar.h`

## Decisions Made

- **VARCHAR diagnostic vs. Parquet reader integration:** Sidecar-only `loom_scan` returns a single VARCHAR diagnostic row with routing information rather than attempting to chain into DuckDB's internal Parquet reader. The value proof is the dependency boundary, not full SQL results. Integrating with DuckDB's internal `ParquetScanFunction` would require accessing internal DuckDB APIs that may change across versions — deferred to a future plan if needed.
- **LIBLOOM_FFI variable reassignment:** When `LOOM_SIDECAR_ONLY=ON`, `LIBLOOM_FFI` is reassigned to `LIBLOOM_SIDECAR_FFI` path, so downstream `target_link_libraries` and include logic uses the same variable name regardless of mode — minimal CMake diff.
- **Single-source C++ file:** Both modes live in `loom_extension.cpp` with `#ifdef`/`#else`/`#endif` guards rather than separate compilation units. This avoids build system complexity and keeps the DuckDB extension registration in one place.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

- Pre-existing dead-code warning in `loom-lance-ingress` (`embed_sidecar_into_lance_dataset`) — unrelated to this plan
- Pre-existing unreachable-pattern warnings in `loom-ir-core/src/l2core_codec.rs` — unrelated to this plan

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Phase 51 is now complete with all 3 plans executed
- All success criteria verified:
  1. ✅ `cmake -S contrib/duckdb-ext -B build/sidecar -DLOOM_SIDECAR_ONLY=ON` configure succeeds without llvm-config requirement
  2. ✅ Default full build (`cmake -S contrib/duckdb-ext -B build/full`) continues to work
  3. ✅ `cargo build --workspace --release` passes cleanly
  4. ✅ `cargo tree -p loom-sidecar-ffi | grep loom-container` returns zero lines
  5. ✅ `cargo tree -p loom-cli --no-default-features | grep loom-container` returns zero lines
  6. ✅ `cargo test -p loom-ir-core -p loom-parquet-ingress -p loom-container` passes
- Ready for phase closeout / verification

## Known Stubs

- **Sidecar-only DuckDB `loom_scan` returns diagnostic VARCHAR only:** In sidecar mode, `loom_scan(path)` returns a single VARCHAR diagnostic row describing the routing decision. It does not produce Arrow arrays, populate DataChunks with data, or chain into DuckDB's internal Parquet reader. This is intentional — the value proof is the dependency boundary, and full SQL results require the full build (`LOOM_SIDECAR_ONLY=OFF`).

## Self-Check: PASSED

- All 3 modified files exist on disk
- All 3 commits (e37d6d2, d7f6143, de05614) found in git history
- All 6 plan success criteria verified
- CMake configure for both sidecar-only and full modes succeeds
- Dependency boundary greps confirmed clean

---
*Phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress*
*Completed: 2026-06-11*
