---
phase: 01-scaffold-and-ffi-boundary
reviewed: 2026-06-07T00:00:00Z
depth: standard
files_reviewed: 16
files_reviewed_list:
  - crates/loom-ffi/src/ffi.rs
  - crates/loom-ffi/src/lib.rs
  - crates/loom-ffi/build.rs
  - crates/loom-ffi/cbindgen.toml
  - crates/loom-ffi/include/loom.h
  - crates/loom-ffi/tests/roundtrip.rs
  - crates/loom-ffi/Cargo.toml
  - crates/loom-core/src/lib.rs
  - crates/loom-core/Cargo.toml
  - crates/loom-fixtures/src/lib.rs
  - crates/loom-fixtures/Cargo.toml
  - Cargo.toml
  - rust-toolchain.toml
  - scripts/check-core-invariants.sh
  - .github/workflows/ci.yml
  - .gitignore
findings:
  critical: 1
  warning: 2
  info: 3
  total: 6
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-06-07
**Depth:** standard
**Files Reviewed:** 16
**Status:** issues_found

## Summary

The scaffold is structurally sound in its Rust correctness: the `ptr::write` pairing is exact (one write per struct, no clone), null-pointer guards are correctly placed before the inner call, `AssertUnwindSafe` usage is justified, and the `from_ffi` / schema-release handling in the roundtrip test is memory-safe given Arrow's C Data Interface release-sets-null guarantee. `loom.h` correctly excludes `FFI_ArrowArray` and `FFI_ArrowSchema` struct bodies, and `forbid(unsafe_code)` in `loom-core` is properly enforced.

However, there is one blocker-class defect: **`panic = "abort"` in `[profile.release]` renders the `catch_unwind` wrapper a no-op in the deployed staticlib.** The entire DUCK-04 / PITFALLS-P3 panic-safety guarantee does not hold in the artifact that will be linked into DuckDB. The `panic_does_not_abort` test passes only because `cargo test` uses the dev/test profile (panic = unwind by default), creating a false sense of assurance. Two further warnings are present in the generated header and the invariant script.

---

## Critical Issues

### CR-01: `panic = "abort"` makes `catch_unwind` a no-op in the released staticlib

**File:** `Cargo.toml:39` and `crates/loom-ffi/src/ffi.rs:222`

**Issue:** `[profile.release]` sets `panic = "abort"`. When this strategy is active, the Rust runtime calls `abort()` directly on a panic without setting up any unwind frames. `std::panic::catch_unwind` cannot intercept such aborts — it only works when the panic strategy is `unwind`. As a result, **in the compiled release staticlib**, any panic inside `loom_decode_inner` (or anything it calls) immediately terminates the process, exactly as if `catch_unwind` were not there.

The `panic_does_not_abort` test in `tests/roundtrip.rs` passes because `cargo test` (without `--release`) compiles under the test/dev profile, where `panic` defaults to `"unwind"`. The test is exercising a code path that does not represent the behavior of the shipped artifact. The comment at `ffi.rs:216–217` ("Any caught panic maps to `LoomError::Panicked`") is factually wrong for release builds, and `Cargo.toml:40` contains an internal contradiction: it justifies `panic = "abort"` as the UB-prevention mitigation and then adds `catch_unwind` as if the two are complementary — they are mutually exclusive for recovery purposes.

The correct fix is to choose one strategy and apply it consistently:

**Option A — Keep `panic = "abort"` (abort is acceptable for MVP0):** Remove the `catch_unwind` wrapper and the `Panicked` variant, or document clearly that the catch_unwind is only meaningful in debug/test builds. Add `[profile.test]` with `panic = "abort"` if the test should reflect production behavior (though then `panic_does_not_abort` cannot pass — so the test must be removed or restructured).

**Option B — Switch to `panic = "unwind"` in release so `catch_unwind` actually works:** Replace `panic = "abort"` with `panic = "unwind"` in `[profile.release]`. The `catch_unwind` wrapper then provides genuine protection at the FFI boundary by mapping panics to `LoomError::Panicked`. Note: this requires a compatible unwinder in the final link. For a DuckDB staticlib target on Linux/macOS this is generally fine.

Recommended choice for a DuckDB in-process extension is Option B: a panic that kills the database process is far more disruptive than returning an error code. Option A is only acceptable if the entire DuckDB extension is designed to be crash-only.

```toml
# Cargo.toml — Option B: enable real catch_unwind protection
[profile.release]
panic = "unwind"   # changed from "abort"; allows catch_unwind to catch panics at FFI boundary
```

If Option A is chosen, remove the dead wrapper code:

```rust
// ffi.rs — Option A: remove catch_unwind (it has no effect with panic=abort)
// Replace lines 222-230 with:
match loom_decode_inner(input, out_array, out_schema) {
    Ok(()) => 0,
    Err(e) => e.code(),
}
```

---

## Warnings

### WR-01: `loom.h` uses `FFI_ArrowArray *` / `FFI_ArrowSchema *` without any declaration in scope

**File:** `crates/loom-ffi/include/loom.h:37–40`

**Issue:** The generated header uses `FFI_ArrowArray` and `FFI_ArrowSchema` as parameter types in the `loom_decode` declaration, but the header neither `#include`s a file that defines them nor emits forward declarations for them. Any C or C++ translation unit that `#include "loom.h"` without having previously included the Arrow C Data Interface header will receive a hard compile error ("unknown type name `FFI_ArrowArray`"). The cbindgen.toml comment (lines 12–15) documents the requirement, but that comment lives in a build-time configuration file, not in the generated header itself.

This is not a latent issue — it will surface the first time a consumer includes the header in isolation (e.g., in a test harness, a documentation example, or a CI step that validates the header compiles). The CI workflow has no step that tries to compile loom.h.

**Fix:** Add C forward declarations for the two incomplete struct types directly above the `loom_decode` declaration in cbindgen.toml's `header` or via a `[defines]` block, OR add a comment in the generated file that names the required prerequisite include. Forward declarations are preferable because they make the header self-describing and allow the compiler to enforce type safety even without the full Arrow header:

```toml
# cbindgen.toml — add to the header string so the generated loom.h is self-contained
header = """/* Generated by cbindgen — do not edit by hand. */
/* Loom FFI surface — Phase 1, Plan 02 (CORE-03) */

/* Forward declarations for Arrow C Data Interface types.
   Include the Arrow C Data Interface header (e.g. abi/arrow.h) before
   this file, or ensure these types are already declared. */
#ifndef ARROW_C_DATA_INTERFACE
typedef struct ArrowSchema FFI_ArrowSchema;
typedef struct ArrowArray  FFI_ArrowArray;
#endif
"""
```

Alternatively (simpler), add a note directly in the generated header comment so the consumer knows what to include first.

---

### WR-02: `check-core-invariants.sh` CORE-01 silently passes when `cargo tree` fails

**File:** `scripts/check-core-invariants.sh:67–80`

**Issue:** Line 67 runs `cargo tree -d 2>/dev/null` and pipes its output through `sed | grep | awk | sort`. Two compounding problems:

1. `2>/dev/null` discards stderr, so if `cargo tree` fails (no `Cargo.lock`, wrong toolchain, network error), the pipeline produces empty output rather than an error.
2. Line 75 treats an empty `arrow_versions` string as a passing condition (`[ -z "$arrow_versions" ]` → PASS) rather than flagging it as suspicious. An empty result could mean "no arrow crates found" or "cargo failed silently" — both are conditions that should fail the invariant, not pass it.

The `grep '^arrow'` stage within the pipeline also exits 1 when it finds no matches. Under `set -euo pipefail`, a command substitution from a failing pipeline _should_ propagate failure, but Bash's well-known exception is that `var=$(failing_cmd)` does not trigger `set -e` when the failing command is in a substitution on the left side of an assignment. Combined with the `-z` guard, the check will report PASS even if the dependency graph is completely invisible to the script.

**Fix:**

```bash
# scripts/check-core-invariants.sh — replace the CORE-01 block (lines 67–80)
arrow_versions=$(cargo tree -d 2>&1 \
    | sed 's/^[^a-zA-Z]*//' \
    | grep '^arrow' \
    | awk '{print $1, $2}' \
    | sort -u) || true   # normalize exit; check emptiness explicitly below

if [ -z "$arrow_versions" ]; then
    fail "cargo tree -d produced no arrow-* output — cargo may have failed or workspace has no arrow dependency"
else
    arrow_name_count=$(echo "$arrow_versions" | awk '{print $1}' | sort -u | wc -l | tr -d ' ')
    arrow_pair_count=$(echo "$arrow_versions" | wc -l | tr -d ' ')
    if [ "$arrow_pair_count" -eq "$arrow_name_count" ]; then
        pass "cargo tree -d: all arrow-* crates resolve to a single version (PITFALLS P9)"
    else
        fail "Duplicate arrow-* version conflict detected (PITFALLS P9):"
        echo "$arrow_versions" >&2
    fi
fi
```

---

## Info

### IN-01: `loom-core` is an unused dependency of `loom-ffi` in Phase 1

**File:** `crates/loom-ffi/Cargo.toml:14`

**Issue:** `loom-core` is listed as a dependency of `loom-ffi`, but neither `crates/loom-ffi/src/lib.rs` nor `crates/loom-ffi/src/ffi.rs` imports or uses anything from it. Rust will emit an `unused-extern-crates` lint (or Clippy's `unused_crate_dependencies`) once the lint is enabled. This is intentional for Phase 1 scaffolding (the actual decode calls arrive in Phase 3), but it is worth noting: if Clippy's `unused_crate_dependencies` lint is added to the `-D warnings` gate in CI before Phase 3, the build will fail.

**Fix:** No action required for Phase 1. Document the forward-reference intent with an inline comment so the reviewer context is preserved:

```toml
[dependencies]
# loom-core will supply the decode logic in Phase 3; the dependency is
# declared here now to lock the version and verify the crate builds together.
loom-core = { path = "../loom-core" }
```

---

### IN-02: CI Clippy runs without `--release`, masking release-profile-specific issues

**File:** `.github/workflows/ci.yml:65`

**Issue:** The Clippy step runs `cargo clippy --workspace -- -D warnings` without `--release`. This means Clippy analyses the code under the dev profile, where `panic = "unwind"` (the default). Any issue that only manifests under `panic = "abort"` (such as the CR-01 finding above) is invisible to this static analysis pass. The same applies to any future `#[cfg(not(debug_assertions))]` branches.

**Fix:** Add a parallel Clippy invocation targeting the release profile:

```yaml
- name: Clippy (release profile)
  run: cargo clippy --workspace --release -- -D warnings
```

---

### IN-03: Schema release idiom in `release_path_roundtrip` is correct but fragile and non-obvious

**File:** `crates/loom-ffi/tests/roundtrip.rs:108–110`

**Issue:** The idiom `unsafe { release_fn(&mut { ffi_schema } as *mut _) }` is memory-safe: `{ ffi_schema }` moves `ffi_schema` into a block expression (consuming the original binding), `&mut` borrows the resulting temporary, the release callback sets `temp.release = null`, and the temporary is then dropped with `release = None` (Drop is a no-op). However, this pattern is not idiomatic and relies on:

- Arrow's release callback faithfully setting `release = null` on the struct (Arrow C Data Interface spec, §Release callbacks);
- The reader understanding that `{ ffi_schema }` is a move expression, not a copy.

The code comment at line 103–112 explains the _why_ but not the _how_ of the idiom. A reader unfamiliar with Rust temporary lifetime rules may incorrectly believe this creates a dangling pointer or double-free risk.

**Fix:** Replace the move-into-block pattern with explicit `ManuallyDrop` to make the intent unambiguous:

```rust
// tests/roundtrip.rs — replace lines 108-110
use std::mem::ManuallyDrop;
let mut schema_for_release = ManuallyDrop::new(ffi_schema);
if let Some(release_fn) = schema_for_release.release {
    unsafe { release_fn(&mut *schema_for_release as *mut _) };
}
// ManuallyDrop::new prevents the Drop impl from running; release_fn took ownership.
```

---

_Reviewed: 2026-06-07_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
