---
phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
fixed: 2026-06-11T19:45:00Z
based_on: 51-REVIEW.md
findings_fixed:
  critical: 1
  warning: 3
  info: 0
  total: 4
---

# Phase 51: Review Fix Report

## Summary

4 of 8 review findings were auto-fixed. The remaining 4 informational items (IN-01 through IN-04) are non-blocking quality improvements deferred to follow-up.

---

## Fixed Findings

### CR-01: Missing `loom_sidecar_free_cstr` — Memory Leak and Undefined Behavior Risk ✅

**Files changed:**
- `crates/loom-sidecar-ffi/src/ffi.rs` — Added `loom_sidecar_free_cstr` function (46 lines: null-check + `catch_unwind` + `CString::from_raw`)
- `contrib/duckdb-ext/loom_extension.cpp` — Added `loom_sidecar_free_cstr(const_cast<char *>(decision_json))` call after reading the decision JSON in sidecar mode

**Verification:**
- `cargo build -p loom-sidecar-ffi --release` passes
- `nm target/release/libloom_sidecar_ffi.a | grep loom_sidecar_free_cstr` confirms symbol exported
- `grep loom_sidecar_free_cstr crates/loom-sidecar-ffi/include/loom_sidecar.h` returns 3 references (function declaration + 2 doc references)
- `cargo test -p loom-ir-core -p loom-parquet-ingress -p loom-container` all pass

---

### WR-01: Incomplete JSON String Escaping ✅

**File changed:** `crates/loom-sidecar-ffi/src/ffi.rs` — Replaced simple `.replace()` chain with a character-level match that handles `\b`, `\f`, and all control characters (`\u00XX`).

**Verification:**
- `cargo build -p loom-sidecar-ffi --release` passes with no new warnings

---

### WR-02: `SidecarDiagnostic.code` Field Not Included in Routing JSON ✅

**File changed:** `crates/loom-sidecar-ffi/src/ffi.rs` — Added `"code":...` field to the diagnostic JSON object in `routing_decision_to_json`.

**Verification:**
- `cargo build -p loom-sidecar-ffi --release` passes

---

### WR-03: Debug Format Used Instead of Display for `HostNativeReaderReason` ✅

**File changed:** `crates/loom-sidecar-ffi/src/ffi.rs` — Changed `format!("{reason:?}")` to `format!("{reason}")` (Display) and replaced misleading comment about "stripping zero-byte from CString".

**Verification:**
- `cargo build -p loom-sidecar-ffi --release` passes
- `HostNativeReaderReason` Display impl outputs stable identifiers identical to current Debug output

---

## Unfixed Information Items

These four items were classified as INFO — non-blocking quality improvements:

- **IN-01:** Temp file collision risk in `loom-self-ingress::write_loom_file` — deferred
- **IN-02:** Unnecessary string allocation in `loom_sidecar_extract` — deferred
- **IN-03:** Misleading comment (already fixed as part of WR-03)
- **IN-04:** Unused includes in sidecar-only compilation mode — deferred

---

## Post-Fix Verification

- ✅ `cargo build --workspace --release` passes (pre-existing warnings only)
- ✅ `cargo test -p loom-ir-core -p loom-parquet-ingress -p loom-container` passes
- ✅ `cargo build -p loom-cli --no-default-features` passes (lean mode)
- ✅ `cargo build -p loom-cli` passes (full mode)
- ✅ `cargo tree -p loom-sidecar-ffi | grep loom-container` returns zero lines
- ✅ `cargo tree -p loom-cli --no-default-features | grep loom-container` returns zero lines
- ✅ `cargo tree -p loom-parquet-ingress --no-default-features -e no-dev | grep loom-core` returns zero lines
- ✅ `cargo tree -p loom-ffi | grep loom-container` returns found (existing path intact)
- ✅ `nm target/release/libloom_sidecar_ffi.a | grep loom_sidecar` shows 5 symbols (extract, verify, route, free_bytes, free_cstr)

---

_Fixed: 2026-06-11T19:45:00Z_
