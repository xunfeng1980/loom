---
phase: 02-duckdb-extension-scaffold
plan: "01"
subsystem: duckdb-extension-cpp
tags: [duckdb, arrow, ffi, c++, extension]
dependency_graph:
  requires:
    - 01-scaffold-and-ffi-boundary/01-02-PLAN (loom_decode ABI + libloom_ffi.a)
  provides:
    - duckdb-ext/loom_extension.cpp (entry + loom_scan + OneShotStream + arrow_scan delegation)
    - duckdb-ext/vendor/duckdb-src/duckdb.hpp (DuckDB v1.5.3 amalgamated header)
    - duckdb-ext/vendor/append_metadata.cmake (official footer-stamping script)
    - crates/loom-ffi/tests/buffer_layout.rs (Wave-0 buffer layout assertion)
  affects:
    - 02-02-PLAN (CMake build + footer-stamp + smoke-test — consumes all artifacts above)
tech_stack:
  added:
    - DuckDB v1.5.3 amalgamation (libduckdb-src.zip → duckdb.hpp + duckdb.cpp)
    - DuckDB append_metadata.cmake (footer-stamping script for Plan 02-02)
  patterns:
    - OneShotStream (ArrowArrayStream C-struct wrapper for one decoded batch)
    - produce-callback factory (arrow_scan three-pointer delegation — D-01 stable surface)
    - LoomScanState RAII (GlobalTableFunctionState with exactly-once release)
key_files:
  created:
    - duckdb-ext/loom_extension.cpp
    - duckdb-ext/vendor/duckdb-src/duckdb.hpp
    - duckdb-ext/vendor/duckdb-src/duckdb.cpp
    - duckdb-ext/vendor/append_metadata.cmake
    - duckdb-ext/vendor/null.txt
    - duckdb-ext/.gitignore
    - crates/loom-ffi/tests/buffer_layout.rs
  modified: []
decisions:
  - "DUCKDB_CPP_EXTENSION_ENTRY macro IS present in the v1.5.3 amalgamation (2 occurrences) — macro path used, no manual fallback needed"
  - "D-01 honored: OneShotStream + produce-callback factory used for arrow_scan delegation; no direct DataChunk population"
  - "loom_decode i32 return code checked in LoomInit; IOException thrown on rc != 0"
  - "Arrow buffer layout confirmed by Wave-0 Rust test: n_buffers==2, buffers[0]=validity, buffers[1]=values"
metrics:
  duration_minutes: 15
  tasks_completed: 2
  tasks_total: 2
  files_created: 7
  files_modified: 0
  completed_date: "2026-06-07"
---

# Phase 2 Plan 01: DuckDB Extension Scaffold — Source Authoring Summary

One-liner: DuckDB v1.5.3 amalgamation vendored, Wave-0 buffer-layout test passing, and loom_extension.cpp authored with OneShotStream + produce-callback factory delegating to arrow_scan (D-01), loom_decode return-code check, and DUCK-03 teardown on every path.

## What Was Built

### Task 1: DuckDB v1.5.3 build inputs + Wave-0 checks

**Artifacts acquired:**
- `duckdb-ext/vendor/duckdb-src/duckdb.hpp` (1.9 MB, DuckDB v1.5.3 amalgamated C++ header)
- `duckdb-ext/vendor/duckdb-src/duckdb.cpp` (24 MB, amalgamated implementation for static fallback)
- `duckdb-ext/vendor/append_metadata.cmake` (official footer-stamping script, 3.1 KB)
- `duckdb-ext/vendor/null.txt` (exactly 1 byte — null — per RESEARCH Pitfall 3; verified by `wc -c`)
- `duckdb-ext/.gitignore` (excludes `build/` and the CLI binary; tracks the vendored amalgamation)

**WAVE-0 CHECK 1 — DUCKDB_CPP_EXTENSION_ENTRY macro:**
- Result: **PRESENT** (2 occurrences in duckdb.hpp)
- Consequence: `DUCKDB_CPP_EXTENSION_ENTRY(loom, loader)` macro used directly in loom_extension.cpp; the manual `extern "C" void loom_duckdb_cpp_init(...)` fallback is NOT needed.

**WAVE-0 CHECK 2 — Arrow buffer layout:**
- Test file: `crates/loom-ffi/tests/buffer_layout.rs`
- Asserts: `rc==0`, `length==4`, `null_count==1`, `n_buffers==2`, `buffers[0]` (validity) non-null, `buffers[1]` (values) non-null
- Result: `cargo test -p loom-ffi --release --test buffer_layout` → **1 passed**
- Confirmed layout: buffers[0]=validity bitmap (non-null because one null element), buffers[1]=int32 values. These indices are the load-bearing assumption for the arrow_scan path.

### Task 2: loom_extension.cpp

**Entry point:** `DUCKDB_CPP_EXTENSION_ENTRY(loom, loader)` expands to `extern "C" void loom_duckdb_cpp_init(duckdb::ExtensionLoader &loader)` — the only symbol DuckDB v1.5.3 dlsym's (`extension_load.cpp` line 634).

**loom_scan registration:** `TableFunction("loom_scan", {LogicalType::VARCHAR}, LoomScan, LoomBind, LoomInit)` registered via `loader.RegisterFunction(fn)`.

**LoomBind:** Accepts the VARCHAR argument, ignores it (D-04). Declares one nullable INTEGER column "value". Returns `TableFunctionData`.

**LoomInit:** Calls `loom_decode(nullptr, 0, &array, &schema)`. Checks `rc != 0` and throws `IOException("loom_decode failed with code %d", rc)` without reading output pointers (T-02-PANIC, PITFALLS P5).

**LoomScanState:** `GlobalTableFunctionState` subclass. Holds `ArrowArray arrow_array = {}` and `ArrowSchema arrow_schema = {}` (zero-initialized). Destructor checks `stream_handed_off` flag before releasing (prevents double-free when stream took ownership) and null-checks release pointers before calling them (DUCK-03, PITFALLS P1).

**OneShotStream:** Heap struct implementing the Arrow C Stream Interface:
- `get_schema`: shallow-copies owned schema into `*out` (schema stays owned by stream — PITFALLS P2)
- `get_next`: first call transfers array bitwise to `*out`, zeros `s->array.release` (consumer owns it); subsequent calls set `out->release = nullptr` (end-of-stream sentinel)
- `release`: frees remaining array+schema, deletes the struct, zeros `private_data` and `release` (DUCK-03, T-02-DF)

**produce-callback factory (D-01 — stable surface for Phase 3+):**
- `produce(uintptr_t factory_ptr, ArrowStreamParameters &)`: builds `ArrowArrayStreamWrapper` with a fully-wired `ArrowArrayStream` (OneShotStream callbacks + private_data). Called by DuckDB's arrow_scan once per scan.
- `arrow_scan_get_schema(ArrowArrayStream *, ArrowSchema &)`: delegates to `OneShotStream::get_schema`.
- Both are cast to `uintptr_t` and passed as `Value::POINTER` args to `arrow_scan`.

**LoomScan (D-01 delegation):**
- Moves array+schema from `LoomScanState` into `LoomStreamFactory` (stack context)
- Sets `state.stream_handed_off = true` and zeros the state's release pointers
- Creates `Connection(db)` from `DatabaseInstance::GetDatabase(ctx)`
- Calls `conn.TableFunction("arrow_scan", {Value::POINTER(factory_ptr), Value::POINTER(produce_fn), Value::POINTER(get_schema_fn)})` — DuckDB drives the stream and performs Arrow→DataChunk conversion internally
- Fetches the result chunk and moves it into the output DataChunk
- On end-of-stream: `output.SetCardinality(0)`

**Legacy symbols:** `loom_init` and `loom_version` are NOT exported (confirmed by negative grep).

## Verification Results

| Check | Result |
|-------|--------|
| `test -f duckdb-ext/vendor/duckdb-src/duckdb.hpp` | PASS |
| `test -f duckdb-ext/vendor/duckdb-src/duckdb.cpp` | PASS |
| `test -f duckdb-ext/vendor/append_metadata.cmake` | PASS |
| `wc -c < duckdb-ext/vendor/null.txt` prints `1` | PASS |
| `grep -c DUCKDB_CPP_EXTENSION_ENTRY duckdb.hpp` | PASS (count=2, macro present) |
| `cargo test -p loom-ffi --release --test buffer_layout` | PASS (1 test passed) |
| `grep -q loom_duckdb_cpp_init loom_extension.cpp` | PASS |
| `grep -q loom_decode loom_extension.cpp` | PASS |
| `grep -q arrow_scan loom_extension.cpp` | PASS (D-01 positive guard) |
| `grep -q produce loom_extension.cpp` | PASS (D-01 positive guard) |
| `grep -c LoomScanState loom_extension.cpp` ≥ 2 | PASS (count=10) |
| `grep -q 'rc != 0' loom_extension.cpp` | PASS (return-code guard) |
| `! grep -q FlatVector::GetData loom_extension.cpp` | PASS (D-01 negative guard) |
| `grep -q release loom_extension.cpp` | PASS |
| `! grep -qE '(loom_init\|loom_version)' loom_extension.cpp` | PASS |
| Combined `OK_ARROW_SCAN_NO_DIRECT_DATACHUNK` | PASS |

## Deviations from Plan

None - plan executed exactly as written.

The DUCKDB_CPP_EXTENSION_ENTRY macro was found present in the amalgamation (WAVE-0 CHECK 1 result: PRESENT), so no manual fallback was needed.

Network was available (github.com releases and raw.githubusercontent.com both accessible), so the downloads completed as specified. The release download initially failed on the first attempt but succeeded on retry with the same URL — no deviation logged.

## Known Stubs

| Stub | File | Reason |
|------|------|--------|
| `loom_decode(nullptr, 0, ...)` call ignores input bytes | `duckdb-ext/loom_extension.cpp:LoomInit` | Phase 2 stub — `loom_decode` returns hardcoded `[1,2,3,null]`; Phase 3+ will pass real encoded bytes |
| VARCHAR path argument accepted but ignored (D-04) | `duckdb-ext/loom_extension.cpp:LoomBind` | Intentional — Phase 2 proves the plumbing; the path is wired in Phase 3 |

These stubs are intentional per the plan objective ("prove the *plumbing*"). Phase 3 will supply real decode logic and wire the path argument.

## Threat Surface Scan

No new network endpoints, auth paths, or schema changes introduced. The plan's threat model covers all surfaces touched by this plan:

| Threat | Mitigation Status |
|--------|-------------------|
| T-02-PANIC: loom_decode panic across FFI | `rc != 0` check in LoomInit + IOException throw — implemented |
| T-02-DF: double-free in OneShotStream + LoomScanState | stream_handed_off flag + null-before-release pattern — implemented |
| T-02-LIFE: schema dangling read | Schema owned by OneShotStream until stream.release() — implemented |
| T-02-IDX: buffer index assumptions | Wave-0 test pins n_buffers==2, buffers[0]=validity, buffers[1]=values — passed |
| T-02-SC: supply chain (DuckDB downloads) | All three artifacts from official DuckDB GitHub release tag v1.5.3 — accepted |

## Self-Check: PASSED

All created files found on disk. Both commits (a383497, 07559ea) verified in git log.
