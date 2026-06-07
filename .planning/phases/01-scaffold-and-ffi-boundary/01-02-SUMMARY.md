---
phase: 01-scaffold-and-ffi-boundary
plan: "02"
subsystem: ffi-boundary
tags: [arrow-ffi, cbindgen, catch_unwind, extern-c, ci, invariant-script]
dependency_graph:
  requires: [workspace-root, loom-core-crate, loom-ffi-crate, arrow-version-pin, system-allocator, panic-abort]
  provides: [loom_decode-extern-c, loom-h-header, arrow-roundtrip-test, panic-safety-test, core-invariant-script, github-actions-ci]
  affects: [phase-2-duckdb-extension, all-subsequent-plans]
tech_stack:
  added: [cbindgen=0.29.3 (build-dep)]
  patterns: [extern-c-catch_unwind, arrow-c-data-interface-to_ffi, ptr-write-ownership, thread-local-test-sentinel, capture-to-variable-pipefail-safe]
key_files:
  created:
    - crates/loom-ffi/src/ffi.rs
    - crates/loom-ffi/build.rs
    - crates/loom-ffi/cbindgen.toml
    - crates/loom-ffi/include/loom.h
    - crates/loom-ffi/tests/roundtrip.rs
    - scripts/check-core-invariants.sh
    - .github/workflows/ci.yml
  modified:
    - crates/loom-ffi/Cargo.toml
    - crates/loom-ffi/src/lib.rs
    - Cargo.lock
decisions:
  - "loom_decode signature: pub unsafe extern C fn loom_decode(input_ptr: *const u8, input_len: usize, out_array: *mut FFI_ArrowArray, out_schema: *mut FFI_ArrowSchema) -> i32 (locked per CONTEXT)"
  - "LoomError codes: NullPointer=1, DecodeFailed=2, Panicked=3 — distinct nonzero i32 codes"
  - "No loom_free exported: Arrow release callback (installed by to_ffi) owns buffer teardown; C++ side calls array.release and schema.release"
  - "cbindgen excludes FFI_ArrowArray and FFI_ArrowSchema from export: incomplete-type pointer declaration in loom.h, no struct body redefinition (PITFALLS integration gotcha, T-01-09)"
  - "Panic sentinel uses thread_local! Cell<bool> (not AtomicBool): avoids cross-test races when cargo test runs tests in parallel; thread-local gives each test thread its own instance"
  - "Script uses capture-to-variable pattern for cargo test check: avoids pipefail pipe-in-condition fragility seen with bash set -euo pipefail"
  - "cargo tree -d arrow dedup check uses sed+awk to strip Unicode box-drawing chars before grep: prevents false positives from tree structure characters"
metrics:
  duration: "~20 minutes"
  completed: "2026-06-07T10:35:00Z"
  tasks_completed: 3
  tasks_total: 3
  files_created: 7
  files_modified: 3
---

# Phase 1 Plan 2: FFI Boundary — extern "C" loom_decode + cbindgen loom.h + CORE Invariant CI Summary

Locked FFI contract with `extern "C" fn loom_decode` wrapped in `catch_unwind`, exporting a real `Int32Array [1,2,3,null]` via Arrow C Data Interface (`to_ffi` + two `ptr::write` calls), cbindgen generating `include/loom.h` (no Arrow struct redefinition), a release-path roundtrip test + panic-safety test passing outside DuckDB, and a committed invariant script run by GitHub Actions CI.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | extern "C" loom_decode + Arrow C Data Interface export | 2c869e0 | Cargo.toml, src/lib.rs, src/ffi.rs |
| 2 | cbindgen build.rs → loom.h + roundtrip tests | 735ec20 | build.rs, cbindgen.toml, include/loom.h, tests/roundtrip.rs, Cargo.toml, Cargo.lock |
| 3 | CORE invariant script + GitHub Actions CI | b358ea9 | scripts/check-core-invariants.sh, .github/workflows/ci.yml |

## Locked FFI Contract

### `loom_decode` Signature

```rust
#[no_mangle]
pub unsafe extern "C" fn loom_decode(
    input_ptr: *const u8,
    input_len: usize,
    out_array: *mut arrow::ffi::FFI_ArrowArray,
    out_schema: *mut arrow::ffi::FFI_ArrowSchema,
) -> i32
```

C declaration in `loom.h`:
```c
int32_t loom_decode(const uint8_t *input_ptr,
                    uintptr_t input_len,
                    FFI_ArrowArray *out_array,
                    FFI_ArrowSchema *out_schema);
```

### `LoomError` Codes

| Variant | Code | Meaning |
|---------|------|---------|
| `NullPointer` | 1 | `out_array`, `out_schema`, or non-zero-length `input_ptr` was null |
| `DecodeFailed` | 2 | Arrow error in `to_ffi` or decode logic |
| `Panicked` | 3 | `catch_unwind` caught a panic in the inner decode body |
| *(success)* | 0 | Normal return |

### No `loom_free`

Buffer teardown is owned by the Arrow release callback installed by `to_ffi`. The C++ side must call:
- `array.release(&array)` — releases Arrow array buffers
- `schema.release(&schema)` — releases schema allocations

These must each be called exactly once, after which the respective `release` pointer is set to `null` (Arrow C Data Interface specification). No `loom_free` is exported; this design decision is final for MVP0 (revisit only if a non-Arrow heap allocation ever crosses the boundary).

### cbindgen Arrow Struct Exclusion

`cbindgen.toml` excludes `FFI_ArrowArray` and `FFI_ArrowSchema` from the generated header via `[export] exclude = [...]`. The generated `loom.h` uses them as incomplete-type pointers (valid C — you can declare pointer-to-incomplete-type). The C++ consumer must include an Arrow C Data Interface header before `loom.h` so the types are fully defined when the `loom_decode` declaration is seen. This prevents ABI struct-body mismatch between cbindgen's inferred layout and Arrow's own definition (PITFALLS integration gotcha, T-01-09).

## Invariants Verified

| Check | Command | Result |
|-------|---------|--------|
| Arrow version unification | `cargo tree -d` — arrow-* all at 58.3.0 | PASS |
| vortex-file absent | `grep vortex-file Cargo.lock` | PASS: absent |
| loom-core clean | `cargo tree -p loom-core \| grep vortex` | PASS: clean |
| panic=abort | `grep 'panic = "abort"' Cargo.toml` | PASS |
| System allocator | `grep global_allocator crates/loom-ffi/src/lib.rs` | PASS |
| loom.h generated | `cargo build -p loom-ffi --release` + file check | PASS |
| loom_decode in loom.h | `grep loom_decode crates/loom-ffi/include/loom.h` | PASS |
| No FFI_ArrowArray body in loom.h | `grep -E 'FFI_ArrowArray.*{' loom.h` | PASS: absent |
| Symbol in staticlib | `nm target/release/libloom_ffi.a \| grep loom_decode` | PASS: `T _loom_decode` |
| Tests pass | `cargo test -p loom-ffi` (5 tests) | PASS |
| catch_unwind present | `grep catch_unwind crates/loom-ffi/src/ffi.rs` | PASS |
| Full invariant script | `bash scripts/check-core-invariants.sh` | PASS: exit 0 |

## Tests

| Test | File | Coverage |
|------|------|----------|
| `error_codes_are_nonzero_and_distinct` | src/ffi.rs unit | LoomError codes are unique and nonzero |
| `null_out_array_returns_null_pointer_code` | src/ffi.rs unit | T-01-08 null-pointer guard |
| `null_out_schema_returns_null_pointer_code` | src/ffi.rs unit | T-01-08 null-pointer guard |
| `release_path_roundtrip` | tests/roundtrip.rs | ARROW-03: to_ffi + ptr::write + from_ffi + release, [1,2,3,null] values + nulls asserted |
| `panic_does_not_abort` | tests/roundtrip.rs | DUCK-04: set_panic_sentinel() → catch_unwind → Panicked code returned; process alive |

`cargo test -p loom-ffi` → 5 tests passed, 0 failed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Unused import warnings (zero-warning requirement)**
- **Found during:** Task 1 first build
- **Issue:** Initial `ffi.rs` imported `Array`, `Int32Array`, `DataType`, `Field` from arrow but only some were used in the builder block. `into_data()` requires `Array` trait in scope; the others were leftover from planning sketches.
- **Fix:** Removed unused `Int32Array`, `DataType`, `Field` imports; kept `Array` trait import for `into_data()`.
- **Files modified:** `crates/loom-ffi/src/ffi.rs`
- **Commit:** 2c869e0

**2. [Rule 1 - Bug] Test parallel race via global AtomicBool sentinel**
- **Found during:** Task 2, first test run
- **Issue:** Initial panic sentinel used a global `AtomicBool`. Cargo runs integration tests in parallel by default. The `panic_does_not_abort` test set the flag, but `release_path_roundtrip` (running concurrently on another thread) consumed it first, causing the roundtrip to panic and the panic test to get code 0 instead of 3.
- **Fix:** Changed sentinel to `thread_local! { static PANIC_SENTINEL: Cell<bool> }`. Each test thread has its own copy — arming the sentinel on one thread cannot affect a sibling thread. Also replaced initial `#[cfg(test)]` approach (which doesn't propagate to integration tests) with an always-present `set_panic_sentinel()` pub function.
- **Files modified:** `crates/loom-ffi/src/ffi.rs`, `crates/loom-ffi/tests/roundtrip.rs`
- **Commit:** 735ec20

**3. [Rule 1 - Bug] Invariant script CORE-02 check stripped #[global_allocator] attribute**
- **Found during:** Task 3, first script run
- **Issue:** Filter `grep -v '^#'` was intended to remove shell `#` comments but also removed Rust `#[global_allocator]` attribute lines (which start with `#[`). Result: CORE-02 always failed even when the allocator was present.
- **Fix:** Changed filter to `grep -v '^\s*//'` which correctly targets only Rust `//` line comments, not attribute lines.
- **Files modified:** `scripts/check-core-invariants.sh`
- **Commit:** b358ea9

**4. [Rule 1 - Bug] Invariant script CORE-01 false positive from Unicode tree chars**
- **Found during:** Task 3, first script run
- **Issue:** `cargo tree -d` output uses Unicode box-drawing characters (`├──`, `│`) as tree structure. The script's `grep -v '^[[:space:]]'` filter (intended to find "root-level" entries) didn't remove these lines because `├` is not ASCII whitespace. Result: the grep matched all arrow references in the tree (at nested depth), falsely reporting version conflicts.
- **Fix:** Changed check to strip Unicode tree-drawing chars with `sed 's/^[^a-zA-Z]*//'`, then extract unique (name, version) pairs with `awk`, then verify each arrow name appears with exactly one version string. This correctly identifies true version conflicts vs. multiple-path references.
- **Files modified:** `scripts/check-core-invariants.sh`
- **Commit:** b358ea9

**5. [Rule 1 - Bug] Invariant script test check fragile with `set -euo pipefail`**
- **Found during:** Task 3, first script run
- **Issue:** `if cargo test -p loom-ffi 2>&1 | grep -q 'test result: ok'; then` — under `set -euo pipefail`, this pattern can behave unexpectedly when cargo test produces panic output to stderr (from the panic_does_not_abort test). The condition was reporting FAIL even though cargo test exited 0 and all tests passed.
- **Fix:** Captured `cargo test` output to a variable with `test_out=$(cargo test ... 2>&1) && test_exit=0 || test_exit=$?`, then checked `$test_exit` and grepped `$test_out`. This pattern is robust against pipefail quirks.
- **Files modified:** `scripts/check-core-invariants.sh`
- **Commit:** b358ea9

## Known Stubs

None — all artifacts are complete and functional. The `loom_decode` implementation uses a hardcoded `Int32Array [1,2,3,null]` as the "decoded output" (per plan spec: Phase 1 proves the boundary, not the decoder). This is intentional: real decode logic arrives in Phase 3. The stub does NOT prevent the plan goal from being achieved — the goal is to prove the FFI contract and release ownership path, both of which are fully exercised.

## Threat Flags

No new security-relevant surface beyond the plan's threat model. All T-01-xx mitigations from the threat register are implemented and unit-tested:

| Threat ID | Status |
|-----------|--------|
| T-01-05 (panic across FFI → abort) | MITIGATED: catch_unwind present; `panic_does_not_abort` test passes |
| T-01-06 (double-free via Arrow release) | MITIGATED: exactly one ptr::write per struct; release_path_roundtrip confirms release fires once |
| T-01-07 (schema freed before array) | MITIGATED: FFI_ArrowSchema moved via its own ptr::write; independent lifetime |
| T-01-08 (null pointer deref) | MITIGATED: out_array + out_schema + input_ptr null-checked before any deref |
| T-01-09 (cbindgen redefines Arrow FFI structs) | MITIGATED: cbindgen.toml excludes FFI_ArrowArray/FFI_ArrowSchema; confirmed absent from loom.h |
| T-01-SC (cbindgen supply chain) | MITIGATED: cbindgen 0.29.3 pinned (STACK.md vetted); Cargo.lock committed |

## Self-Check: PASSED

Files confirmed present on disk:
- `crates/loom-ffi/src/ffi.rs` — present
- `crates/loom-ffi/build.rs` — present
- `crates/loom-ffi/cbindgen.toml` — present
- `crates/loom-ffi/include/loom.h` — present (generated)
- `crates/loom-ffi/tests/roundtrip.rs` — present
- `scripts/check-core-invariants.sh` — present, executable
- `.github/workflows/ci.yml` — present

Commits confirmed in git history:
- `2c869e0` — feat(01-02): extern "C" loom_decode
- `735ec20` — feat(01-02): cbindgen + roundtrip tests
- `b358ea9` — chore(01-02): invariant script + CI

All 3 commits present and verified.
