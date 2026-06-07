---
phase: 02-duckdb-extension-scaffold
verified: 2026-06-07T00:00:00Z
status: passed
score: 10/10 must-haves verified
overrides_applied: 0
re_verification: false
---

# Phase 2: DuckDB Extension Scaffold Verification Report

**Phase Goal:** A stub DuckDB v1.5.3 C++ extension that links the Rust libloom_ffi.a staticlib builds, loads (unsigned) into the matching prebuilt duckdb 1.5.3 CLI, and registers a loom_scan(VARCHAR) table function returning the 4 hardcoded rows (1,2,3,NULL) via loom_decode — without crashing.
**Verified:** 2026-06-07T00:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                                     | Status     | Evidence                                                                                                               |
|----|-----------------------------------------------------------------------------------------------------------|------------|------------------------------------------------------------------------------------------------------------------------|
| 1  | cmake --build produces a loadable loom.duckdb_extension without errors                                    | VERIFIED   | `cmake --build duckdb-ext/build` exits 0; `duckdb-ext/build/loom.duckdb_extension` exists (4,185,038 bytes)          |
| 2  | LOAD + SELECT * FROM loom_scan('test.bin') returns 1,2,3,NULL without crash or ABI error                  | VERIFIED   | `bash scripts/duckdb-smoke-test.sh` exits 0; output confirms 4 rows: 1, 2, 3, NULL via duckdb -unsigned              |
| 3  | LoomScanState destructor releases array+schema unconditionally on every teardown path (no stream_handed_off gate) | VERIFIED | loom_extension.cpp:65-71: destructor calls release with null-guards only; no `stream_handed_off` flag in shipped code |
| 4  | No rogue allocator symbols in libloom_ffi.a                                                               | VERIFIED   | `bash scripts/check-core-invariants.sh` exits 0; nm -g guard passes (PHASE 2 / ROADMAP criterion 4)                  |
| 5  | D-01 REVISED: no struct OneShotStream scaffolding; direct DataChunk population via FlatVector::GetData    | VERIFIED   | `grep -c 'struct OneShotStream' loom_extension.cpp` = 0; `grep -c 'FlatVector::GetData' loom_extension.cpp` = 1     |
| 6  | loom_extension.cpp exports exactly loom_duckdb_cpp_init; no legacy loom_init/loom_version               | VERIFIED   | DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) macro at line 234; no loom_init/loom_version present (grep exits 1)        |
| 7  | loom_decode return code checked; IOException thrown on rc != 0; output pointers never read on error       | VERIFIED   | loom_extension.cpp:120: `if (rc != 0) { throw IOException(...); }`                                                  |
| 8  | CR-01/WR-03 null guards: arr.buffers and values_buf null-checked before any dereference                   | VERIFIED   | loom_extension.cpp:172 `if (arr.buffers == nullptr || arr.n_buffers < 2)` and line 187 `if (values_buf == nullptr)` |
| 9  | CR-02 fix applied: loom-core vortex-isolation check is fail-closed (cargo tree failure does not pass silently) | VERIFIED | check-core-invariants.sh:120-131: `loom_core_tree_exit=0; cmd || loom_core_tree_exit=$?; if [ $loom_core_tree_exit -ne 0 ]` |
| 10 | WR-02 fix applied: cargo tree -d exit-code capture pattern correct                                        | VERIFIED   | check-core-invariants.sh:72-76: `arrow_tree_exit=0; arrow_dupes_raw=$(cargo tree -d 2>&1) \|\| arrow_tree_exit=$?`  |

**Score:** 10/10 truths verified

---

### Required Artifacts

| Artifact                                          | Expected                                            | Status     | Details                                                               |
|---------------------------------------------------|-----------------------------------------------------|------------|-----------------------------------------------------------------------|
| `duckdb-ext/loom_extension.cpp`                   | C++ extension; entry point, LoomBind/LoomInit/LoomScan, direct DataChunk population (D-01 REVISED), DUCK-03 teardown | VERIFIED | 237 lines (min 120); contains loom_duckdb_cpp_init, loom_decode, LoomScanState (10 occurrences), FlatVector::GetData |
| `duckdb-ext/vendor/duckdb-src/duckdb.hpp`         | DuckDB v1.5.3 amalgamated C++ header                | VERIFIED   | Exists; 2,036,023 bytes                                               |
| `duckdb-ext/vendor/duckdb-src/duckdb.cpp`         | DuckDB v1.5.3 amalgamated implementation            | VERIFIED   | Exists; 25,576,388 bytes                                              |
| `duckdb-ext/vendor/append_metadata.cmake`         | Official footer-stamping script for POST_BUILD      | VERIFIED   | Exists; 3,215 bytes                                                   |
| `duckdb-ext/vendor/null.txt`                      | Exactly 1 null byte required by append_metadata.cmake | VERIFIED | `wc -c` = 1                                                          |
| `crates/loom-ffi/tests/buffer_layout.rs`          | Wave-0 Arrow buffer layout assertion test           | VERIFIED   | Exists; contains n_buffers, length==4, null_count==1 assertions; `cargo test -p loom-ffi --release` exits 0 (1 test passed) |
| `duckdb-ext/CMakeLists.txt`                       | Hand-rolled CMake: cargo-build trigger, staticlib link, dynamic-lookup, symbol hiding, footer-stamp POST_BUILD | VERIFIED | 153 lines (min 50); contains libloom_ffi, cargo build, append_metadata, v1.5.3; no libduckdb link |
| `scripts/duckdb-smoke-test.sh`                    | Build + download CLI + LOAD + loom_scan row-count assertion | VERIFIED | 187 lines (min 20); -unsigned flag present; asserts count=4 and rows 1,2,3,NULL |
| `.github/workflows/ci.yml`                        | CI job extended with C++ extension build + DuckDB load smoke-test | VERIFIED | Contains loom_scan (2 occurrences), duckdb-smoke-test.sh, cmake --build; two-platform coverage (linux_amd64 + osx_arm64) |
| `scripts/check-core-invariants.sh`                | Extended with allocator-symbol guard (Phase 2 criterion 4) | VERIFIED | grep libloom_ffi.a present; nm -g guard for malloc/free/realloc; script exits 0 |

---

### Key Link Verification

| From                             | To                             | Via                                               | Status  | Details                                                              |
|----------------------------------|--------------------------------|---------------------------------------------------|---------|----------------------------------------------------------------------|
| `duckdb-ext/CMakeLists.txt`      | `target/release/libloom_ffi.a` | `add_custom_command cargo build + target_link_libraries` | WIRED | `cargo build -p loom-ffi --release --manifest-path ../Cargo.toml`; `target_link_libraries(... PRIVATE ${LIBLOOM_FFI})` |
| `duckdb-ext/CMakeLists.txt`      | `duckdb-ext/vendor/append_metadata.cmake` | POST_BUILD custom command                | WIRED | `-P ${CMAKE_SOURCE_DIR}/vendor/append_metadata.cmake`; correct non-double-nested path |
| `duckdb-ext/loom_extension.cpp`  | `loom_decode`                  | FFI call in LoomInit via loom.h include           | WIRED   | `extern "C" { #include "../crates/loom-ffi/include/loom.h" }`; `loom_decode(nullptr, 0, ...)` at line 110 |
| `duckdb-ext/loom_extension.cpp`  | DuckDB DataChunk               | LoomScan reads arr.buffers[0]/[1] via FlatVector  | WIRED   | `FlatVector::GetData<int32_t>(vec)` at line 180; bitmap loop at lines 192-200 |
| `scripts/duckdb-smoke-test.sh`   | `loom.duckdb_extension`        | duckdb -unsigned LOAD                             | WIRED   | `"${DUCKDB_BIN}" -unsigned -c "LOAD '${EXT_PATH}'; SELECT ..."` at lines 134, 157 |

---

### Data-Flow Trace (Level 4)

| Artifact                          | Data Variable  | Source                                     | Produces Real Data               | Status   |
|-----------------------------------|----------------|--------------------------------------------|----------------------------------|----------|
| `duckdb-ext/loom_extension.cpp`   | arr (ArrowArray) | `loom_decode(nullptr, 0, &array, &schema)` | Yes — Rust returns [1,2,3,null] | FLOWING  |
| `duckdb-ext/loom_extension.cpp`   | out_data (DuckDB vector) | `FlatVector::GetData<int32_t>(vec)`  | Yes — populates from values_buf[i] | FLOWING |

Note: loom_decode returns the Phase 1 hardcoded Int32Array [1,2,3,null]. This is the intentional Phase 2 stub — Phase 3 will supply real encoded bytes. The data path from loom_decode output to DuckDB DataChunk is fully wired.

---

### Behavioral Spot-Checks

| Behavior                              | Command                                    | Result                                                      | Status |
|---------------------------------------|--------------------------------------------|-------------------------------------------------------------|--------|
| Extension builds without errors       | `cmake --build duckdb-ext/build`           | exit 0; loom_loadable_extension linked                     | PASS   |
| Smoke-test: 4 rows returned           | `bash scripts/duckdb-smoke-test.sh`        | exit 0; "SELECT count(*) = 4"; rows 1,2,3,NULL confirmed   | PASS   |
| Core invariants (incl. allocator guard) | `bash scripts/check-core-invariants.sh` | exit 0; all CORE + Phase 2 checks passed                   | PASS   |
| loom-ffi release tests                | `cargo test -p loom-ffi --release`         | exit 0; 6 tests passed (buffer_layout + roundtrip + unit)  | PASS   |
| No OneShotStream dead scaffolding      | `grep -c 'struct OneShotStream' loom_extension.cpp` | 0                                               | PASS   |

---

### Probe Execution

| Probe                              | Command                                 | Result  | Status |
|------------------------------------|-----------------------------------------|---------|--------|
| `scripts/check-core-invariants.sh` | `bash scripts/check-core-invariants.sh` | exit 0  | PASS   |
| `scripts/duckdb-smoke-test.sh`     | `bash scripts/duckdb-smoke-test.sh`     | exit 0  | PASS   |

---

### Requirements Coverage

| Requirement | Source Plan | Description                                                     | Status    | Evidence                                                              |
|-------------|-------------|-----------------------------------------------------------------|-----------|-----------------------------------------------------------------------|
| DUCK-01     | 02-01, 02-02 | C++ DuckDB extension pinned to DuckDB v1.5.3 builds and loads  | SATISFIED | cmake --build exits 0; duckdb -unsigned LOAD succeeds; footer stamped v1.5.3/osx_arm64/CPP |
| DUCK-02     | 02-01       | loom_scan invokes Rust decoder; Arrow array adopted zero-copy   | SATISFIED | loom_decode called in LoomInit; reinterpret_cast to FFI_ArrowArray*; buffers read in LoomScan |
| DUCK-03     | 02-01, 02-02 | Extension releases imported Arrow array on every teardown path  | SATISFIED | ~LoomScanState: unconditional release with null-guards; no stream_handed_off gate; smoke-test exits 0 (clean teardown) |

---

### Anti-Patterns Found

| File                                       | Line  | Pattern             | Severity | Impact                                                              |
|--------------------------------------------|-------|---------------------|----------|---------------------------------------------------------------------|
| `.github/workflows/ci.yml` (macOS job)     | 128+  | macOS job omits Clippy and `cargo test -p loom-ffi --release` | WARNING | Rust regressions on macOS-specific code paths undetected until Linux CI; WR-05 from REVIEW, accepted/deferred |

No `TBD`, `FIXME`, or `XXX` debt markers found in any Phase 2 modified files. No stub returns (return null, return []) in execution paths. The intentional stubs (`loom_decode(nullptr, 0, ...)` and the ignored VARCHAR path argument) are correctly bounded and documented as Phase 3 work.

The `produce` and `arrow_scan_get_schema` functions referenced in 02-01-SUMMARY.md are NOT present in the shipped loom_extension.cpp — the SUMMARY reflects the pre-correction D-01 path. The shipped file uses only direct DataChunk population (D-01 REVISED). The SUMMARY is a historical artifact of the original execution; the codebase is correct per the revision.

---

### Human Verification Required

None. All must-haves are machine-verifiable and confirmed via code inspection + probe execution.

---

## Post-Correction Confirmation

The three correction notes from the verification instructions are all confirmed in the codebase:

1. **D-01 REVISED (no OneShotStream, direct DataChunk):** `grep -c 'struct OneShotStream' loom_extension.cpp` = 0. `FlatVector::GetData` present at line 180. The SUMMARY for Plan 01 incorrectly documents the pre-correction D-01 path (OneShotStream, arrow_scan delegation) — it is a historical artifact of the initial (wrong) execution. The shipped `.cpp` file matches the revised D-01: direct DataChunk population, no dead scaffolding.

2. **DUCK-03 leak fixed — unconditional release:** The `stream_handed_off` flag that caused the original leak is absent from the shipped destructor. `~LoomScanState` (lines 62-73) releases both `arrow_array` and `arrow_schema` unconditionally, guarded only by null-checks on the release function pointers to prevent double-free. This is the correct implementation.

3. **Code review critical fixes applied:** CR-01 (`arr.buffers == nullptr` guard at line 172) and WR-03 (`values_buf == nullptr` guard at line 187) are confirmed present. CR-02 (fail-closed `cargo tree -p loom-core` check) is confirmed in check-core-invariants.sh lines 120-131. WR-02 (correct exit-code capture for `cargo tree -d`) is confirmed at lines 65-76.

---

## Gaps Summary

No gaps. All 10 must-haves verified. All three DUCK requirements satisfied. All four ROADMAP Phase 2 success criteria met (with the note that SC-1 names the artifact `loom_extension.duckdb_extension` while it is actually `loom.duckdb_extension` — the naming difference is intentional per plan: the file base `loom` maps to the DuckDB entry symbol `loom_duckdb_cpp_init`).

---

_Verified: 2026-06-07T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
