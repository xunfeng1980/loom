---
phase: 50-sidecar-overlay-model-and-host-native-reader-fallback
reviewed: 2026-06-11T12:00:00Z
depth: standard
files_reviewed: 15
files_reviewed_list:
  - crates/loom-ir-core/src/sidecar.rs
  - crates/loom-ir-core/src/sidecar_routing.rs
  - crates/loom-ir-core/src/lib.rs
  - crates/loom-core/src/lib.rs
  - ingress/loom-parquet-ingress/src/sidecar_parquet.rs
  - ingress/loom-parquet-ingress/src/source_contract.rs
  - ingress/loom-parquet-ingress/src/lib.rs
  - ingress/loom-vortex-ingress/src/sidecar_vortex.rs
  - ingress/loom-vortex-ingress/src/source_contract.rs
  - ingress/loom-vortex-ingress/src/lib.rs
  - ingress/loom-lance-ingress/src/sidecar_lance.rs
  - ingress/loom-lance-ingress/src/source_contract.rs
  - ingress/loom-lance-ingress/src/lib.rs
  - crates/loom-cli/src/main.rs
  - scripts/sidecar-overlay-test.sh
findings:
  critical: 1
  warning: 5
  info: 3
  total: 9
status: issues_found
---

# Phase 50: Code Review Report

**Reviewed:** 2026-06-11
**Depth:** standard (per-file analysis with cross-module import tracing)
**Files Reviewed:** 15
**Status:** issues_found

## Summary

Phase 50 delivers the sidecar overlay model: host-neutral `SidecarOverlay`/`ChunkBinding` types in `loom-ir-core`, deterministic binary encode/decode, 4-gate fail-closed routing logic, Parquet KeyValue metadata extract/embed, and documented graceful no-ops for Vortex/Lance due to format metadata API limitations. The architecture is sound — the crate boundary between `loom-ir-core` (zero Arrow deps) and `loom-container` is cleanly enforced, and the Parquet-first deployment model is well-documented.

However, the review found **1 critical issue** in the release gate script (broken summary table that can silently hide failing sections) plus **5 warnings** including a silent data-truncation bug in the IR-bytes encode path, unsafe file I/O in the Parquet embed convenience function, and a missing dependency in the release gate. The core Rust code is well-structured with comprehensive test coverage (10+ sidecar tests, 8 routing tests, 9 Parquet tests).

## Critical Issues

### CR-01: Release gate script summary table silently hides failing sections

**File:** `scripts/sidecar-overlay-test.sh:216-224`
**Issue:** The summary table logic uses `echo "${FAILURE_MESSAGES[@]}" | grep -q "^${section}:"` to determine which sections failed. Bash's `echo` joins all array elements on a single line with spaces, so only the **first** failing section's message appears at the start of the line. All subsequent failing sections' FAIL/PASS markers will **silently show PASS** in the summary, even though `$FAILED` correctly counts them. This means the gate script could report `FAILED=3` but only show `FAIL` for one section — a false impression of where the failures occurred.

**Fix:**
```bash
# Replace lines 216-224 with array-based check:
for section in MARKERS CORE_SIDECAR_TESTS PARQUET_SIDECAR_ROUNDTRIP VORTEX_SIDECAR_MARKER LANCE_SIDECAR_MARKER STRIPPABLE_OVERLAY FULL_BUILD CLI_BUILD; do
    local matched=false
    for msg in "${FAILURE_MESSAGES[@]}"; do
        if [[ "$msg" == "${section}:"* ]]; then
            matched=true
            break
        fi
    done
    if $matched; then
        echo "${RED}FAIL${RST} ${section}"
    else
        echo "${GRN}PASS${RST} ${section}"
    fi
done
```

## Warnings

### WR-01: Silent u32 truncation of ir_bytes.len() in SidecarOverlay::encode

**File:** `crates/loom-ir-core/src/sidecar.rs:162`
**Issue:** `l2core_codec::write_u32(&mut buf, self.ir_bytes.len() as u32)` silently truncates `usize` to `u32`. On 64-bit platforms, if `ir_bytes` exceeds 4 GiB, the length prefix is silently truncated to the lower 32 bits, producing a corrupt encoded output. The decoder will then read a truncated or wrong-length IR byte range, resulting in malformed data that may pass or fail unpredictably. While IR bytes >4 GiB is unlikely in practice, silent truncation rather than a returned error is a correctness hazard. The codec format's u32-length-prefixed design has a structural 4 GiB limit; exceeding it should fail loudly.

**Fix:**
```rust
let ir_len = self.ir_bytes.len();
let ir_len_u32 = u32::try_from(ir_len).unwrap_or_else(|_| {
    // Fail-closed: IR bytes larger than u32::MAX cannot be encoded
    // in this format. Return an empty or minimal valid placeholder
    // and log a diagnostic; or make encode() return Result.
    // For now, clamp with a loud assertion:
    panic!("sidecar IR bytes exceed u32::MAX ({} bytes); encoding not supported", ir_len)
});
// Or better: change encode() to Result<Vec<u8>, SidecarCodecError>
l2core_codec::write_u32(&mut buf, ir_len_u32);
```

### WR-02: unsafe in-place file overwrite in embed_sidecar_into_parquet_file

**File:** `ingress/loom-parquet-ingress/src/sidecar_parquet.rs:158-176`
**Issue:** The function opens the input file for reading (line 139), reads all batches, then **overwrites the same file** (line 159: `File::create(path)`) — the same path that was the source. If the process crashes or the writer fails after `File::create` truncates the file but before all batches are written, the original Parquet data is irrecoverably lost. This is a data-loss risk for a convenience function exposed to the CLI.

**Fix:** Write to a temporary file first, then atomically rename:
```rust
let tmp_path = path.with_extension("tmp.loom-sidecar");
let out_file = File::create(&tmp_path).map_err(...)?;
// ... write batches ...
writer.close().map_err(...)?;
std::fs::rename(&tmp_path, path).map_err(|e| {
    SidecarCodecError::Malformed(format!("rename temp to {}: {e}", path.display()))
})?;
```

### WR-03: encode uses assert! for string length enforcement while decode returns errors

**File:** `crates/loom-ir-core/src/sidecar.rs:314-317` (write_u8_len_str) vs. lines 293-306 (read_u8_len_str)
**Issue:** The `write_u8_len_str` helper uses `assert!(bytes.len() <= u8::MAX as usize)` which **panics** the process on strings >255 bytes. Meanwhile, the decode counterpart `read_u8_len_str` returns a proper `Result` with `SidecarCodecError::Truncated` when the u8 length prefix exceeds remaining bytes. This is an asymmetry: encode panics on invalid data, decode errors gracefully. In library code, panics should be reserved for truly unreachable states (invariant violations), not user-data validation. The plan documents this as intentional (T-50-04: bindings count is u16, IDs are max 255 bytes), but a panic from encoding a too-long granule_id is still a crash that could be avoided.

**Fix:** Either:
- Return `Result` from encode (preferred for consistency with decode), or
- Document the 255-byte limit as a pre-condition on the public API with `#[doc]` and accept the panic as fail-fast (acknowledged risk).

### WR-04: Release gate script depends on `rg` without availability check

**File:** `scripts/sidecar-overlay-test.sh:40`
**Issue:** The `check_marker` function uses `rg` (ripgrep) via the `rg` command. The script has `set -euo pipefail`, so if `rg` is not installed, the script fails with a cryptic "command not found" error before reaching any gate section. Since this script is intended as a portable release gate, it should either use `grep` (which is universally available) or check for `rg` and fall back gracefully.

**Fix:** Add a check at the top of the script:
```bash
if ! command -v rg >/dev/null 2>&1; then
    echo "ERROR: ripgrep (rg) is required but not installed" >&2
    exit 2
fi
```
Or replace `rg -q --fixed-strings "${pattern}" "${file}"` with `grep -qF "${pattern}" "${file}"`.

### WR-05: Unused `std::hash::Hasher` import in sidecar.rs

**File:** `crates/loom-ir-core/src/sidecar.rs:30`
**Issue:** `use std::hash::Hasher;` is imported but never used. The `fnv::FnvHasher` is used directly (calling `.write()` and `.finish()` as inherent methods), not through the `Hasher` trait. This is dead code that could mislead readers into thinking the `Hasher` trait methods are being called.

**Fix:**
```rust
// Remove line 30:
// use std::hash::Hasher;
```

## Info

### IN-01: CLI `sidecar embed` always creates empty bindings

**File:** `crates/loom-cli/src/main.rs:177-179`
**Issue:** The `sidecar_embed_parquet` function creates a `SidecarOverlay` with `bindings: Vec::new()`. The comment says "per-column bindings deferred", which is intentional per the plan. However, this means the CLI produces sidecars that have an IR identity but zero chunk bindings — so Gate 3 (hash match) trivially passes with zero verifications. The sidecar model is functional (Gate 2 pass → Gate 4 check) but the content-hash binding feature is effectively unused from the CLI. This is a deferred feature, not a bug, but worth documenting for testers and downstream consumers.

**Suggestion:** Add a `println!` diagnostic noting "0 chunk bindings — content-hash verification is inactive for this sidecar" so testers understand what the CLI produced.

### IN-02: `extract_sidecar_from_lance_dataset()` takes no dataset argument

**File:** `ingress/loom-lance-ingress/src/sidecar_lance.rs:43-55`
**Issue:** The function signature `pub fn extract_sidecar_from_lance_dataset() -> Result<Option<SidecarOverlay>, SidecarCodecError>` takes no arguments — it doesn't accept a dataset handle, path, or buffer. This is intentional because it's a documented no-op (Lance manifest doesn't support metadata), but when Lance eventually adds metadata support, this function will need a new signature (breaking API change). The function name implies it extracts from "a" Lance dataset, but currently extracts from nothing.

**Suggestion:** Either rename to `extract_sidecar_from_lance_dataset_not_yet_supported()` to signal the no-op nature, or change the signature now to accept a placeholder parameter (e.g., `_dataset: &Dataset`) so the API doesn't need to break later.

### IN-03: FNV-1a hash implementation duplication between l2core_codec.rs and sidecar.rs

**File:** `crates/loom-ir-core/src/sidecar.rs:125-129` vs. `crates/loom-ir-core/src/l2core_codec.rs:678-684`
**Issue:** `compute_chunk_hash` uses `fnv::FnvHasher` from the external `fnv` crate, while `l2core_program_hash` uses a hand-rolled `stable_fnv1a64` function. Both implement FNV-1a with the same constants and should produce identical results, but having two hashing code paths creates a maintenance risk: if one implementation changes (crate upgrade, algorithm mis-match), hashes could silently diverge. The plan explicitly says "same algorithm family" — both are FNV-1a 64-bit, but they're implemented differently.

**Suggestion:** Extract a single `pub(crate) fn stable_fnv1a64(data: &[u8]) -> u64` function shared between `l2core_codec.rs` and `sidecar.rs` to guarantee identical hash computation. Alternatively, add a cross-module test that verifies `compute_chunk_hash(b"test") == l2core_program_hash(program_with_bytes(b"test"))`.

---

_Reviewed: 2026-06-11T12:00:00Z_
_Reviewer: ai-code-reviewer (gsd-code-review)_
_Depth: standard_
