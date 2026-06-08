---
phase: 05-fsst-l2-kernel-and-full-verification
plan: "04"
subsystem: duckdb
tags: [duckdb, sql, ffi, fixtures, acceptance]
requires:
  - phase: 05-fsst-l2-kernel-and-full-verification
    plan: "01"
    provides: real FSST L2 kernel
  - phase: 05-fsst-l2-kernel-and-full-verification
    plan: "02"
    provides: Vortex oracle fixture coverage
  - phase: 05-fsst-l2-kernel-and-full-verification
    plan: "03"
    provides: layout payload codec and non-empty FFI decode
provides:
  - deterministic DuckDB payload emitter
  - payload-aware loom_scan binding/init
  - direct DuckDB vector population for Int32, Int64, Boolean, and Utf8
  - MVP0 SQL acceptance gate across all supported encodings
affects: [loom-fixtures, duckdb-ext, scripts, requirements, roadmap]
tech-stack:
  added: []
  patterns: [payload file SQL gate, DuckDB-owned string copy, exact SQL row/aggregate checks]
key-files:
  created:
    - crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs
  modified:
    - crates/loom-fixtures/Cargo.toml
    - duckdb-ext/loom_extension.cpp
    - scripts/duckdb-smoke-test.sh
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
key-decisions:
  - "loom_scan now treats its VARCHAR argument as a payload path and never falls back to the legacy stub for non-empty payload files."
  - "The smoke script forces a release build of loom-ffi before relinking the DuckDB extension so Rust staticlib changes cannot be skipped by CMake's existing output rule."
  - "Utf8 values are copied into DuckDB-owned storage with StringVector::AddString before Arrow buffers are released."
patterns-established:
  - "DuckDB SQL gate uses generated .loom files plus exact row and aggregate checks."
  - "LMP1 dtype tag is read in C++ bind only to declare the single output column type."
requirements-completed: [VERIFY-01, VERIFY-02, VERIFY-03]
duration: 9min
completed: 2026-06-08
---

# Phase 05-04: DuckDB SQL Acceptance Gate Summary

**The MVP0 end-to-end DuckDB SQL gate now passes for bitpack, FOR, dict, RLE, FSST, and dict-over-FSST payloads.**

## Performance

- **Duration:** 9 min
- **Completed:** 2026-06-08T00:14:44Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments

- Added `emit_duckdb_payloads`, which writes deterministic `.loom` payloads and a manifest under `target/loom-duckdb-fixtures/`.
- Updated `loom_scan(VARCHAR)` so bind reads the payload file, infers the `LMP1` dtype tag, declares the right DuckDB column type, and init passes real bytes to `loom_decode`.
- Extended direct DuckDB `DataChunk` population for Int32, Int64, Boolean, and Utf8; Utf8 strings are copied via `StringVector::AddString`.
- Replaced the old stub smoke test with a real SQL gate that checks rows and aggregates for all supported MVP0 encodings.
- Marked `L2-02`, `L2-03`, `VERIFY-01`, `VERIFY-02`, and `VERIFY-03` complete after final verification passed.

## Task Commits

1. **Task 1: DuckDB payload emitter** - `6bcdf90` (feat)
2. **Task 2: Payload-aware loom_scan** - `2599111` (feat)
3. **Task 3: MVP0 SQL smoke gate** - `bd4a2ed` (test)
4. **Task 4: Close verification requirements** - `d503940` (chore)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/loom-fixtures/src/bin/emit_duckdb_payloads.rs` - generated `.loom` payload files and manifest.
- `crates/loom-fixtures/Cargo.toml` - binary target and normal `arrow-schema` dependency.
- `duckdb-ext/loom_extension.cpp` - payload path bind, payload byte decode, typed vector population.
- `scripts/duckdb-smoke-test.sh` - final SQL acceptance gate.
- `.planning/REQUIREMENTS.md` - Phase 5 requirements marked complete.
- `.planning/ROADMAP.md` - Phase 5 marked complete with plan checklist.

## Decisions Made

- Bind stores payload bytes in `LoomBindData`, so init does not repeat file IO.
- The C++ side reads only the minimal `LMP1` header needed for schema binding; Rust remains the full payload parser.
- The smoke script uses DuckDB `COPY (...) TO temp.csv` for stable exact comparisons instead of parsing box-table CLI output.

## Deviations from Plan

None.

## Issues Encountered

- CMake's existing Rust staticlib custom command does not rerun when `libloom_ffi.a` already exists, so the smoke script now explicitly runs `cargo build -p loom-ffi --release` and removes the old extension before relinking.

## Verification

- `cargo run -p loom-fixtures --bin emit_duckdb_payloads` - PASS.
- `cmake -S duckdb-ext -B duckdb-ext/build -DCMAKE_BUILD_TYPE=Release` - PASS.
- `cmake --build duckdb-ext/build` - PASS.
- `bash scripts/duckdb-smoke-test.sh` - PASS, all six payloads matched rows and aggregates.
- `cargo test --workspace` - PASS.
- `cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}'` - PASS, printed `0`.
- `rg -n 'vortex_file|vortex-file|\\.vortex|VortexFile|from_path|read_file' crates/loom-fixtures` - PASS, no matches.

## User Setup Required

None.

## Next Phase Readiness

MVP0 is complete. A follow-up milestone can focus on packaging, richer descriptors, broader kernels, or replacing direct DataChunk population with an ArrowArrayStream path if/when multi-column record batches are introduced.

---
*Phase: 05-fsst-l2-kernel-and-full-verification*
*Completed: 2026-06-08*
