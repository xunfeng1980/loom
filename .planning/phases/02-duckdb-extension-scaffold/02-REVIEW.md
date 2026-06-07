---
phase: 02-duckdb-extension-scaffold
reviewed: 2026-06-07T00:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - duckdb-ext/loom_extension.cpp
  - duckdb-ext/CMakeLists.txt
  - scripts/duckdb-smoke-test.sh
  - scripts/check-core-invariants.sh
  - .github/workflows/ci.yml
  - crates/loom-ffi/tests/buffer_layout.rs
findings:
  critical: 2
  warning: 5
  info: 2
  total: 9
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-06-07
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Reviewed the Phase 2 DuckDB extension scaffold: the C++ table function, CMake build,
two shell scripts, CI workflow, and Rust buffer-layout test. The core Arrow-buffer read
logic (`validity_buf` bitmap math, `batch_emitted` EOS protocol) and the DUCK-03
destructor fix are sound. Two blockers were found: a missing null guard on `arr.buffers`
in `LoomScan` (UB when the pointer is null) and a silent-false-pass in the
`check-core-invariants.sh` CORE-01 D-02 check (a failed `cargo tree -p loom-core`
produces an empty result that is mistaken for "clean"). Five warnings cover the cmake
configure error swallowing, a wrong exit-code capture pattern, the macOS symbol-hiding
comment being incorrect, an unsafe `values_buf` dereference without null guard, and
the macOS CI job not running Clippy/tests. Two info items cover a diagnostic/message
quality issue and the `\b1\b` grep fragility in the SELECT * smoke check.

---

## Critical Issues

### CR-01: `arr.buffers` pointer never null-checked before indexing in `LoomScan`

**File:** `duckdb-ext/loom_extension.cpp:172`
**Issue:** `arr.buffers[0]` and `arr.buffers[1]` are read unconditionally. The Arrow C
Data Interface permits `buffers` to be a null pointer in certain degenerate cases (e.g.
zero-length arrays with no buffer allocation, or a malformed payload returned by a buggy
`loom_decode`). If `arr.buffers` is null, indexing it is undefined behaviour — a
dereference of a null `const void**`. The Phase 2 hardcoded array always provides a
valid `buffers` pointer, so this never fires today, but the pattern propagates
unsafely to Phase 3 when real-file decode is wired in. More immediately: there is
also no null check on `values_buf` (see WR-03), which is structurally the same bug
one level down.

**Fix:**
```cpp
// Guard arr.buffers before any indexing:
if (arr.buffers == nullptr) {
    throw IOException("loom_decode returned an Arrow array with null buffers pointer");
}
const auto *validity_buf = static_cast<const uint8_t *>(arr.buffers[0]);
const auto *values_buf   = static_cast<const int32_t *>(arr.buffers[1]);
if (values_buf == nullptr) {
    throw IOException("loom_decode returned an Arrow array with null values buffer (buffers[1])");
}
```
Add both guards before the loop at line 175. The `validity_buf` null check (buffers[0])
is already handled inside the loop body, but the `values_buf` and `arr.buffers` null
checks are missing.

---

### CR-02: CORE-01 D-02 check silently passes when `cargo tree` fails

**File:** `scripts/check-core-invariants.sh:115`
**Issue:** The `loom-core` vortex-dependency check discards `cargo tree` errors
with `2>/dev/null` and then uses `|| true` to absorb a nonzero pipeline exit:
```sh
loom_core_vortex=$(cargo tree -p loom-core 2>/dev/null | grep -v '^#' | grep 'vortex' || true)
if [ -z "$loom_core_vortex" ]; then
    pass "cargo tree -p loom-core | grep vortex → clean"
```
If `cargo tree -p loom-core` fails for any reason (package not found in workspace,
Cargo.lock stale, network issue), stderr is silenced, the pipeline emits no output,
`loom_core_vortex` is empty, and the check reports **PASS** — claiming D-02 isolation
is intact when it was never verified. This is a genuine silent-false-pass.

**Fix:**
```sh
# Capture exit code separately, do not discard stderr:
loom_core_tree=$(cargo tree -p loom-core 2>&1) || loom_core_tree_exit=$?
if [ "${loom_core_tree_exit:-0}" -ne 0 ]; then
    fail "cargo tree -p loom-core failed — cannot verify D-02 isolation (CORE-01 D-02):"
    echo "$loom_core_tree" | head -5 >&2
else
    loom_core_vortex=$(printf '%s\n' "$loom_core_tree" | grep -v '^#' | grep 'vortex' || true)
    if [ -z "$loom_core_vortex" ]; then
        pass "cargo tree -p loom-core | grep vortex → clean"
    else
        fail "loom-core has a vortex dependency — breaks D-02 isolation:"
        echo "$loom_core_vortex" >&2
    fi
fi
```
The pattern mirrors the more robust `arrow_dupes_raw` capture at line 69 (though that
one also has a milder diagnostic issue — see WR-02).

---

## Warnings

### WR-01: `cmake` configure failure silently swallowed in `duckdb-smoke-test.sh`

**File:** `scripts/duckdb-smoke-test.sh:65-68`
**Issue:** The cmake configure step is:
```sh
cmake -S "${REPO_ROOT}/duckdb-ext" \
      -B "${REPO_ROOT}/duckdb-ext/build" \
      -DCMAKE_BUILD_TYPE=Release \
      2>&1 | grep -v '^--' || true
```
Under `set -o pipefail`, the pipeline exit code is cmake's exit code if cmake fails
(grep exits 0 when it has output to filter). But `|| true` swallows that nonzero
exit, making cmake configure failure invisible. The subsequent `cmake --build` step
will then fail (or produce confusing output) — there is indirect detection, but no
explicit error. If cmake configure fails while still writing a partial build directory,
`cmake --build` may emit opaque messages pointing at the wrong root cause.

**Fix:**
```sh
# Capture configure separately and check exit code:
cmake -S "${REPO_ROOT}/duckdb-ext" \
      -B "${REPO_ROOT}/duckdb-ext/build" \
      -DCMAKE_BUILD_TYPE=Release \
      2>&1 | grep -v '^--'
if [ "${PIPESTATUS[0]}" -ne 0 ]; then
    fail "cmake configure failed"
fi
cmake --build "${REPO_ROOT}/duckdb-ext/build" 2>&1
```
Alternatively, run cmake configure without piping (accept the `--` noise in CI output)
so `set -e` catches it directly.

---

### WR-02: Exit-code capture for `cargo tree -d` is always 0 due to `||` short-circuit

**File:** `scripts/check-core-invariants.sh:69-71`
**Issue:**
```sh
arrow_dupes_raw=$(cargo tree -d 2>&1) || arrow_dupes_raw=""
arrow_tree_exit=$?
```
When `cargo tree -d` exits nonzero, the `||` branch (`arrow_dupes_raw=""`) executes
and exits 0. `$?` on line 70 then captures 0, not cargo's failure code. The explicit
error branch at line 71–74 is therefore dead code: it never fires. A `cargo tree`
failure is still eventually detected (the empty `arrow_dupes_raw` triggers "No arrow-*
crates found" at line 84), but with a misleading message ("query suspect") instead of
"cargo tree failed". Debugging CI failures becomes unnecessarily difficult.

**Fix:**
```sh
# Capture exit code BEFORE the || side-effect clears it:
arrow_dupes_raw=$(cargo tree -d 2>&1)
arrow_tree_exit=$?
if [ "$arrow_tree_exit" -ne 0 ]; then
    arrow_dupes_raw=""   # clear to prevent accidental use
    fail "cargo tree -d failed (exit $arrow_tree_exit) — cannot verify arrow version unification (CORE-01):"
    ...
fi
```
Or restructure using `if ! arrow_dupes_raw=$(cargo tree -d 2>&1); then` to avoid the
`$?` capture entirely.

---

### WR-03: `values_buf` dereferenced without null check

**File:** `duckdb-ext/loom_extension.cpp:184`
**Issue:** Inside the scan loop, `out_data[i] = values_buf[i]` executes for every
valid (non-null) element. `values_buf` is set from `arr.buffers[1]` (line 173) with no
null check. The Arrow C Data Interface requires `buffers[1]` to be non-null for a
primitive array with length > 0, but this contract is enforced only by the Rust side.
If a future `loom_decode` implementation has a bug, or if this pattern is copy-adapted
for Phase 3 before the guard is added, a null dereference causes a process crash inside
DuckDB.

**Fix:** Add the null guard shown in CR-01. The `values_buf` check is most naturally
placed immediately after the `arr.buffers` null check before the loop.

---

### WR-04: macOS `-exported_symbol` does not hide symbols — comment is incorrect

**File:** `duckdb-ext/CMakeLists.txt:65,75`
**Issue:** The comment at line 65 states:
> `-exported_symbol hides all non-API symbols exported from the Rust staticlib`

This is factually wrong. On macOS ld64, `-exported_symbol _name` **adds** a symbol to
the export set but does **not** suppress all other symbols. To achieve exclusive export
(hide everything else), the correct flag is `-exported_symbols_list <file>` with a
list file containing only `_loom_duckdb_cpp_init`. With the current flag, all public
symbols from both the C++ translation unit and the Rust staticlib that ld decides to
export remain visible in the dylib's export table.

The practical risk is mitigated by the System allocator (`#[global_allocator]`) meaning
there are no DEFINED malloc/free/realloc in `libloom_ffi.a`, and the ROADMAP criterion
4 check (`nm -g | grep malloc`) guards against rogue allocator symbols. However the
hiding is incomplete and the comment misleads future maintainers into thinking the
exports are controlled.

**Fix:**
```cmake
if(APPLE)
    # Write the symbols file at configure time:
    file(WRITE "${CMAKE_BINARY_DIR}/export_symbols.txt" "_loom_duckdb_cpp_init\n")
    target_link_options(loom_loadable_extension PRIVATE
        "-undefined" "dynamic_lookup"
        "-Wl,-exported_symbols_list,${CMAKE_BINARY_DIR}/export_symbols.txt"
    )
```
Update the comment to accurately describe `-exported_symbols_list` semantics.

---

### WR-05: macOS CI job skips Clippy and `cargo test` — Rust regressions undetected

**File:** `.github/workflows/ci.yml:128-170`
**Issue:** The `build-and-test-macos` job runs only:
- `cargo build --workspace --release`
- CMake C++ extension build
- DuckDB smoke-test

It omits the steps run on Linux: `cargo clippy`, `cargo test -p loom-ffi --release`,
and `bash scripts/check-core-invariants.sh`. This means:
1. Clippy warnings that only surface on macOS (e.g. platform-conditional code) never
   gate CI.
2. The `panic_does_not_abort` test and `buffer_layout` test do not run on macOS.
3. CORE invariants (arrow version unification, allocator checks) are only verified on
   `linux_amd64`.

If a macOS-specific regression is introduced, the macOS job will still pass as long as
the binary links.

**Fix:** Add at minimum Clippy and `cargo test -p loom-ffi --release` to the macOS job.
The CORE invariant script can be added as well with minimal cost since it runs cargo
commands that would already be cached.

---

## Info

### IN-01: `cargo tree -d` logic produces misleading "query suspect" message on cargo failure

**File:** `scripts/check-core-invariants.sh:84`
**Issue:** Due to the WR-02 exit-code capture bug, when `cargo tree -d` fails, the
failure surfaces as "No arrow-* crates found in dependency tree — expected at least one
(CORE-01 query suspect)" rather than the explicit "cargo tree -d failed" message. The
FAIL is still emitted (the check does not pass silently), but the diagnostic message
misleads the reader into investigating the dependency tree instead of the cargo invocation
itself.

**Fix:** Address via the WR-02 fix — the diagnostic message issue resolves automatically
when the exit-code capture is correct.

---

### IN-02: `\b1\b` grep in SELECT * smoke check can match DuckDB error messages

**File:** `scripts/duckdb-smoke-test.sh:162`
**Issue:** The assertion:
```sh
if ! echo "${ROWS_OUTPUT}" | grep -qE '\b1\b'; then
    fail "Value '1' not found in loom_scan output..."
fi
```
would pass if `ROWS_OUTPUT` contains any token matching `\b1\b`, including DuckDB error
messages (e.g. "Error at line 1:", "expected 1 argument"). If the extension fails to
load but DuckDB prints an error containing "1", "2", and "3" (unlikely but possible),
all three value checks pass while the extension never ran. The step 3 `count(*)` check
catches this failure via a separate DuckDB invocation, so the SELECT * check is not the
last line of defense and the overall smoke-test would still fail. This is a test
reliability issue, not a correctness hole in the shipped code.

**Fix:** Run DuckDB in CSV or tab-separated mode to make output parsing deterministic:
```sh
ROWS_OUTPUT=$("${DUCKDB_BIN}" -unsigned -csv -c \
    "LOAD '${EXT_PATH}'; SELECT * FROM loom_scan('test.bin');" 2>&1)
```
Then check for `^1$`, `^2$`, `^3$` lines and presence of a blank/empty line (null).

---

_Reviewed: 2026-06-07_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
