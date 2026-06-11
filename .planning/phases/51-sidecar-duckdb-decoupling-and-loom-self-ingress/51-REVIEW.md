---
phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
reviewed: 2026-06-11T19:30:00Z
depth: standard
files_reviewed: 11
files_reviewed_list:
  - crates/loom-sidecar-ffi/src/lib.rs
  - crates/loom-sidecar-ffi/src/ffi.rs
  - crates/loom-sidecar-ffi/Cargo.toml
  - crates/loom-sidecar-ffi/build.rs
  - crates/loom-sidecar-ffi/cbindgen.toml
  - crates/loom-self-ingress/src/lib.rs
  - crates/loom-self-ingress/Cargo.toml
  - crates/loom-cli/src/main.rs
  - crates/loom-cli/Cargo.toml
  - contrib/duckdb-ext/CMakeLists.txt
  - contrib/duckdb-ext/loom_extension.cpp
  - ingress/loom-parquet-ingress/Cargo.toml
  - crates/loom-ffi/include/loom.h
findings:
  critical: 1
  warning: 3
  info: 4
  total: 8
status: issues_found
---

# Phase 51: Code Review Report

**Reviewed:** 2026-06-11T19:30:00Z
**Depth:** standard
**Files Reviewed:** 13
**Status:** issues_found

## Summary

Phase 51 implements a lean sidecar FFI crate (`loom-sidecar-ffi`), a `.loom` file IO boundary crate (`loom-self-ingress`), feature-gated CLI compilation, and a DuckDB extension CMake option for sidecar-only builds. The architecture is sound — dependency boundaries are correctly enforced (`loom-container` stays out of the sidecar path, `loom-core` moved to dev-deps in `loom-parquet-ingress`). 

However, the review found one **critical blocker**: the `loom_sidecar_free_cstr` function referenced in documentation and C header comments was never implemented. This means every call to `loom_sidecar_verify` and `loom_sidecar_route` leaks a C string allocation. Additionally, if a consumer mistakenly uses `loom_sidecar_free_bytes` to free a C string returned by verify/route, the behavior is undefined (the allocation layout of `CString` differs from `Vec<u8>`).

Three warnings and four informational items were also identified covering JSON escaping, missing diagnostic fields, and code clarity.

## Critical Issues

### CR-01: Missing `loom_sidecar_free_cstr` — Memory Leak and Undefined Behavior Risk

**File:** `crates/loom-sidecar-ffi/src/ffi.rs:131-175`, `crates/loom-sidecar-ffi/src/ffi.rs:185-267`, `crates/loom-sidecar-ffi/include/loom_sidecar.h:37,56`

**Issue:** `loom_sidecar_verify` and `loom_sidecar_route` both allocate `CString` values via `CString::into_raw()` and return the raw pointer through output parameters. The documentation on both functions and the generated C header instruct callers to free these strings via `loom_sidecar_free_cstr` — but that function was never implemented.

```rust
// ffi.rs:163-165 — loom_sidecar_verify allocates a CString
let cstr = CString::new(hash).map_err(|_| LoomSidecarError::DecodeFailed)?;
let ptr = cstr.into_raw();
std::ptr::write(out_hash, ptr);
```

The only free function available is `loom_sidecar_free_bytes` (lines 285-309), which reconstructs a `Vec<u8>` via `Vec::from_raw_parts(ptr, len, len)`. This is **unsound for CString allocations** because:

1. `CString::into_raw()` produces a pointer to a null-terminated allocation whose layout (capacity, alignment) is determined by Rust's standard library allocator for `CString`, not by `Vec<u8>`.
2. `Vec::from_raw_parts(ptr, len, len)` assumes the allocation has exactly the same length and capacity. A `CString`'s internal capacity may differ from its length (the allocator may have rounded up).
3. The 51-01-SUMMARY.md acknowledged this gap: *"No separate CString free function — verify/route outputs use CString::into_raw(); freeing deferred to Phase 51-02/03"* — but it was never added in subsequent phases.

**Fix:**
Add `loom_sidecar_free_cstr` to `ffi.rs` and declare it in the C header:

```rust
/// Free a C string previously returned by [`loom_sidecar_verify`]
/// or [`loom_sidecar_route`].
///
/// The caller must ensure `ptr` came from a prior call to
/// `loom_sidecar_verify` or `loom_sidecar_route` and that this
/// function is called at most once per allocation.
///
/// # Returns
///
/// * `0` — String freed.
/// * `1` — `ptr` is null.
#[no_mangle]
pub unsafe extern "C" fn loom_sidecar_free_cstr(ptr: *mut c_char) -> i32 {
    if ptr.is_null() {
        return LoomSidecarError::NullPointer.code();
    }

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        // Safety: ptr must describe a valid CString allocation from the
        // global allocator (guaranteed by the contract — caller must pass
        // values obtained from loom_sidecar_verify or loom_sidecar_route).
        unsafe {
            let _ = CString::from_raw(ptr);
        }
        Ok(0)
    }));

    match result {
        Ok(Ok(0)) => LoomSidecarError::Success.code(),
        Ok(Err(e)) => e.code(),
        Ok(_) => LoomSidecarError::DecodeFailed.code(),
        Err(_) => LoomSidecarError::Panicked.code(),
    }
}
```

Also update the DuckDB extension in `loom_extension.cpp` (sidecar mode, around line 111) to call `loom_sidecar_free_cstr` on the `decision_json` pointer before returning from `SidecarBind`, similar to how `loom_sidecar_free_bytes` is called at line 137.

---

## Warnings

### WR-01: Incomplete JSON String Escaping

**File:** `crates/loom-sidecar-ffi/src/ffi.rs:372-379`

**Issue:** The `json_string` helper escapes `\`, `"`, `\n`, `\r`, and `\t` but does **not** escape other control characters (`\u0000`–`\u001F` excluding already-handled ones), backspace (`\b`), or form feed (`\f`). If any diagnostic message, granule ID, or content hash string contains an unescaped control character, the generated JSON would be invalid.

While Loom-internal strings are currently well-behaved, diagnostic messages are constructed from generic `message: String` fields (see `SidecarDiagnostic` in `sidecar_routing.rs`). A future change could introduce control characters without this code catching it.

**Fix:** Add escaping for all control characters, or use a proper JSON serialization library (e.g., `serde_json`) instead of manual formatting:

```rust
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            c if c < ' ' => {
                // Escape as \u00XX for other control chars
                write!(&mut out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
```

---

### WR-02: `SidecarDiagnostic.code` Field Not Included in Routing JSON

**File:** `crates/loom-sidecar-ffi/src/ffi.rs:358-363`

**Issue:** When serializing `HostNativeReader` diagnostics, only `path` and `message` are included in the JSON. The `code` field (`SidecarDiagnosticCode` enum: `EngineNotIntegrated`, `NoSidecarPresent`, `HashMismatch`, `EncodingUnsupported`) is omitted, making it impossible for the consumer to programmatically distinguish diagnostic types from the JSON alone.

```rust
write!(
    &mut buf,
    "{{\"path\":{},\"message\":{}}}",   // ← 'code' missing
    json_string(&d.path),
    json_string(&d.message)
)?;
```

**Fix:** Include the `code` field:

```rust
write!(
    &mut buf,
    "{{\"code\":{},\"path\":{},\"message\":{}}}",
    json_string(&d.code.to_string()),
    json_string(&d.path),
    json_string(&d.message)
)?;
```

(Note: `SidecarDiagnosticCode` already implements `Display`.)

---

### WR-03: Debug Format Used Instead of Display for `HostNativeReaderReason`

**File:** `crates/loom-sidecar-ffi/src/ffi.rs:350`

**Issue:** The routing decision JSON uses `{reason:?}` (Debug format) to serialize `HostNativeReaderReason`:

```rust
let reason_str = format!("{reason:?}"); // Debug-repr of the enum variant
```

`HostNativeReaderReason` has a `Display` impl that writes precisely `"HashMismatch"`, `"EngineNotIntegrated"`, etc. While Debug currently produces identical output for these simple unit variants, relying on Debug is fragile: if any variant gains associated data in the future, the Debug output would change format (e.g., `HashMismatch("extra")`) without this code being updated, silently producing invalid JSON.

**Fix:** Use the Display trait which provides a stable, explicitly controlled format:

```rust
let reason_str = format!("{reason}"); // Display impl (stable, explicit)
```

---

## Info

### IN-01: Temp File Name in `write_loom_file` Could Collide Under Concurrent Writes

**File:** `crates/loom-self-ingress/src/lib.rs:101`

**Issue:** `write_loom_file` uses a fixed temp file name via `path.with_extension("tmp.loom-ingress")`. Two concurrent writers to the same path would race on the temp file. While the CLI is single-user, if `loom-self-ingress` is ever used in a server context, this could cause data corruption.

**Fix:** Use a unique temp file name, e.g.:

```rust
let tmp_path = path.with_extension(format!("tmp.{}.loom-ingress", std::process::id()));
```

Or use a random suffix to eliminate collisions entirely.

---

### IN-02: Unnecessary String Allocation in `loom_sidecar_extract`

**File:** `crates/loom-sidecar-ffi/src/ffi.rs:89-90`

**Issue:** `CStr::from_ptr(file_path).to_string_lossy()` allocates a `Cow<str>` (possibly owned `String`), but the result is only used to construct a `&Path`. This could avoid allocation:

```rust
// Current:
let path = CStr::from_ptr(file_path).to_string_lossy();
let file = std::fs::File::open(Path::new(path.as_ref()))

// Could be:
let cstr = CStr::from_ptr(file_path);
let file = std::fs::File::open(Path::new(
    std::ffi::OsStr::from_bytes(cstr.to_bytes())
))
```

---

### IN-03: Misleading Comment in `routing_decision_to_json`

**File:** `crates/loom-sidecar-ffi/src/ffi.rs:349`

**Issue:** The comment reads:
```rust
// Strip the trailing zero-byte from CString to get the JSON string length
let reason_str = format!("{reason:?}"); // Debug-repr of the enum variant
```

The comment about "strip trailing zero-byte from CString" is a copy-paste artifact — the code below never strips anything from a CString; it formats a Rust enum variant. The comment is misleading and should be removed.

**Fix:** Replace with:
```rust
// Serialize the reason as a stable identifier string.
let reason_str = format!("{reason}"); // uses Display impl
```

---

### IN-04: Unused Includes in Sidecar-Only Compilation Mode

**File:** `contrib/duckdb-ext/loom_extension.cpp:34-35`

**Issue:** In `LOOM_SIDECAR_ONLY` mode, the `<fstream>` and `<limits>` headers are included unconditionally (lines 34-35) but are only used in the full-mode code path. This is not harmful (just dead includes) but adds unnecessary compilation overhead in sidecar mode.

**Fix:** Move `<fstream>` and `<limits>` inside the `#else` (full-mode) block, or gate them with `#ifndef LOOM_SIDECAR_ONLY`.

---

_Reviewed: 2026-06-11T19:30:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
