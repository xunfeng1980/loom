---
phase: 50-sidecar-overlay-model-and-host-native-reader-fallback
fixed_at: 2026-06-11T12:10:00Z
review_path: .planning/phases/50-sidecar-overlay-model-and-host-native-reader-fallback/50-REVIEW.md
iteration: 1
findings_in_scope: 6
fixed: 5
skipped: 1
status: partial
---

# Phase 50: Code Review Fix Report

**Fixed at:** 2026-06-11T12:10:00Z
**Source review:** .planning/phases/50-sidecar-overlay-model-and-host-native-reader-fallback/50-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 6 (1 Critical + 5 Warning)
- Fixed: 5
- Skipped: 1

## Fixed Issues

### CR-01: Release gate script summary table silently hides failing sections

**Files modified:** `scripts/sidecar-overlay-test.sh`
**Commit:** ff52017
**Applied fix:** Replaced the `echo "${FAILURE_MESSAGES[@]}" | grep -q "^${section}:"` pattern with proper array iteration over `FAILURE_MESSAGES`. The previous approach joined all array elements onto a single line with `echo`, so only the first failing section's message matched the start-of-line prefix — all subsequent failures silently showed PASS. The fix iterates each failure message individually and checks the prefix with `[[ "$msg" == "${section}:"* ]]`.

### WR-01: Silent u32 truncation of ir_bytes.len() in SidecarOverlay::encode

**Files modified:** `crates/loom-ir-core/src/sidecar.rs`
**Commit:** 4df331a
**Applied fix:** Replaced `self.ir_bytes.len() as u32` with `u32::try_from(ir_len).unwrap_or_else(|_| panic!(...))`. The panic message clearly reports the byte count and states that encoding is not supported. This is fail-closed: IR bytes larger than u32::MAX (4 GiB) produce a loud diagnostic instead of a silently truncated corrupt length prefix.

### WR-02: Unsafe in-place file overwrite in embed_sidecar_into_parquet_file

**Files modified:** `ingress/loom-parquet-ingress/src/sidecar_parquet.rs`
**Commit:** 67a2eb7
**Applied fix:** Changed the writer to target a temporary file (`path.with_extension("tmp.loom-sidecar")`) instead of overwriting the source path directly. After the writer successfully closes, the temp file is atomically renamed to the original path via `std::fs::rename`. If the rename fails, the temp file is cleaned up. This prevents data loss if the process crashes between `File::create` truncation and writer completion.

### WR-03: encode uses assert! for string length enforcement while decode returns errors

**Files modified:** `crates/loom-ir-core/src/sidecar.rs`
**Commit:** 4df331a
**Applied fix:** Added `# Pre-conditions` section to the `SidecarOverlay::encode()` doc comment documenting the 255-byte string field limit (u8 length prefix structural constraint). The existing `assert!` in `write_u8_len_str` remains as a documented fail-fast invariant. Changing `encode()` to return `Result` was considered but rejected as too invasive for a warning-level fix — it would require updating all callers (CLI, tests, Parquet embed path) and is better suited for a follow-up PR.

### WR-04: Release gate script depends on rg without availability check

**Files modified:** `scripts/sidecar-overlay-test.sh`
**Commit:** 79249ea
**Applied fix:** Added an early availability check for the `rg` (ripgrep) command after color setup and before function definitions. If `rg` is not found, the script exits with code 2 and a clear error message, avoiding the cryptic "command not found" failure that would otherwise occur under `set -euo pipefail`.

## Skipped Issues

### WR-05: Unused std::hash::Hasher import in sidecar.rs

**File:** `crates/loom-ir-core/src/sidecar.rs:30`
**Reason:** The import is **not** actually unused. The `fnv::FnvHasher` type's `.write()` and `.finish()` methods are trait methods from `std::hash::Hasher` — the trait must be in scope for those methods to be callable. Removing the import causes compilation errors (`no method named 'write'/'finish' found for struct 'FnvHasher'`). Verified by attempting removal: `cargo check -p loom-ir-core` fails with `E0599`.
**Original issue:** `use std::hash::Hasher;` is imported but allegedly never used. The fnv::FnvHasher is used directly (calling .write() and .finish() as inherent methods), not through the Hasher trait. This is dead code that could mislead readers.

---

_Fixed: 2026-06-11T12:10:00Z_
_Fixer: gsd-code-fixer_
_Iteration: 1_
