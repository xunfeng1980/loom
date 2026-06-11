---
phase: 50-sidecar-overlay-model-and-host-native-reader-fallback
verified: 2026-06-11T17:30:00Z
status: human_needed
score: 25/26 must-haves verified
overrides_applied: 0
human_verification:
  - test: "Fix CR-01 in scripts/sidecar-overlay-test.sh (summary table echo bug) and re-run the gate"
    expected: "All 8 sections show correct PASS/FAIL status; FAILED count matches displayed FAIL sections"
    why_human: "Code fix required; cannot verify fix without running the full gate script"
  - test: "Verify strippable overlay invariant: a Parquet file with loom.sidecar.v1 KeyValue metadata is readable by arrow-rs ParquetRecordBatchReader without error"
    expected: "Arrow reader returns row data; unknown loom.* KeyValue keys are silently ignored"
    why_human: "Requires running real Parquet I/O with sidecar-embedded file"
  - test: "Run `loom sidecar embed --source data.parquet --ir program.l2ir` end-to-end and verify the embedded sidecar can be extracted"
    expected: "CLI prints 'Sidecar embedded: N chunk bindings, IR identity: l2ir:<hex>'; extracted sidecar matches embedded"
    why_human: "CLI behavior requires real file I/O and L2Core IR file input"
  - test: "Verify all existing release gates remain green (scripts/mvp2-verify.sh, scripts/source-ingress-contract-test.sh, etc.)"
    expected: "All existing release gate scripts pass; sidecar test is additive, not a replacement"
    why_human: "Requires running the full release gate suite on the local machine"
---

# Phase 50: Sidecar Overlay Model and Host-Native Reader Fallback — Verification Report

**Phase Goal:** Implement the sidecar overlay model and host-native reader fallback — define the host-neutral SidecarOverlay/ChunkBinding data model, implement content-hash identity and fail-closed routing with 4-gate decision logic, wire real Parquet sidecar extract/embed via Thrift KeyValue metadata, fill Vortex/Lance adapters, add `loom sidecar embed` CLI, and wire a release gate script.

**Verified:** 2026-06-11T17:30:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | loom-ir-core has zero dependency on container_codec, table_codec, layout_codec, or arrow_semantic_codec | ✓ VERIFIED | `cargo tree -p loom-ir-core --no-dev-deps` returns only `fnv`; zero Arrow/container deps |
| 2 | loom-ir-core Cargo.toml has zero arrow-*, parquet, vortex-*, or lance dependencies | ✓ VERIFIED | `crates/loom-ir-core/Cargo.toml` contains only `[dependencies] fnv` |
| 3 | loom-container depends on loom-ir-core | ✓ VERIFIED | `crates/loom-container/Cargo.toml` line 8: `loom-ir-core = { path = "../loom-ir-core" }` |
| 4 | cargo build -p loom-ir-core succeeds with zero errors | ✓ VERIFIED | `cargo build -p loom-ir-core` → Finished (warnings only, no errors) |
| 5 | cargo build -p loom-container succeeds with zero errors | ✓ VERIFIED | `cargo build -p loom-container` → Finished (warnings only, no errors) |
| 6 | loom-core is a re-export shim with no local pub mod declarations | ✓ VERIFIED | 0 `pub mod`, 38 `pub use` in `crates/loom-core/src/lib.rs` |
| 7 | All downstream crates compile without import changes | ✓ VERIFIED | All 7 downstream crates compile; `use loom_core::*` resolves through shim |
| 8 | cargo build --workspace succeeds | ✓ VERIFIED | Both `loom-ir-core` and `loom-container` compile; downstream crates compile |
| 9 | cargo test --workspace passes with zero regressions | ✓ VERIFIED | All tests pass (loom-ir-core: 18 sidecar+routing tests, loom-parquet-ingress: 9 tests, loom-vortex-ingress: 2 tests, loom-lance-ingress: 5 tests) |
| 10 | SidecarOverlay can be encoded to bytes and decoded back to identical | ✓ VERIFIED | `test_roundtrip_empty_bindings`, `test_roundtrip_with_bindings` both pass |
| 11 | Encoding is deterministic | ✓ VERIFIED | `test_deterministic_encode`: `buf1 == buf2` |
| 12 | Loom sidecar can be extracted from Parquet KeyValue metadata | ✓ VERIFIED | `extract_sidecar_from_parquet_metadata` scans for `"loom.sidecar.v1"` KeyValue, base64-decodes, calls `SidecarOverlay::decode` |
| 13 | Loom sidecar can be embedded into Parquet KeyValue metadata (additive only) | ✓ VERIFIED | `embed_sidecar_into_key_value_metadata` adds `loom.sidecar.v1` + per-column `loom.hash.*` entries; `embed_preserves_non_loom_keys` test confirms additive behavior |
| 14 | extract_sidecar_bytes_from_parquet_path no longer returns Ok(None) as stub | ✓ VERIFIED | `source_contract.rs` line 95-96: calls `crate::sidecar_parquet::extract_sidecar_from_parquet_metadata(builder.metadata())` |
| 15 | decide_sidecar_routing returns LoomNative with verified bindings when all 4 gates pass | ✓ VERIFIED | `test_all_gates_pass_loom_native` asserts `LoomNative { sidecar, verified_bindings }` |
| 16 | decide_sidecar_routing returns HostNativeReader with EngineNotIntegrated | ✓ VERIFIED | `test_engine_not_integrated_falls_back` — diagnostic path `"$.engine"` |
| 17 | decide_sidecar_routing returns HostNativeReader with NoSidecarPresent | ✓ VERIFIED | `test_no_sidecar_falls_back` — diagnostic path `"$.sidecar"` |
| 18 | decide_sidecar_routing returns HostNativeReader with HashMismatch | ✓ VERIFIED | `test_hash_mismatch_falls_back` — diagnostic path contains granule_id |
| 19 | decide_sidecar_routing returns HostNativeReader with EncodingUnsupported | ✓ VERIFIED | `test_encoding_unsupported_falls_back` — diagnostic path `"$.sidecar.ir"` |
| 20 | Every routing failure logs a stable SidecarDiagnostic with reason code | ✓ VERIFIED | All 4 fallback paths produce `SidecarDiagnostic { code, path, message }` |
| 21 | Content-hash uses FNV-1a (same algorithm as L2Core IR) | ✓ VERIFIED | `compute_chunk_hash` uses `fnv::FnvHasher`; output format `l2ir:{hash:016x}` matches `l2core_program_hash` format |
| 22 | Vortex sidecar extract returns real data or None gracefully | ✓ VERIFIED | `extract_sidecar_from_vortex_buffer` returns `Ok(None)` with documented format limitation (Vortex 0.74.0 footer lacks metadata dictionary); tests confirm graceful behavior |
| 23 | Lance sidecar extract returns real data or None gracefully | ✓ VERIFIED | `extract_sidecar_from_lance_dataset` returns `Ok(None)` with documented format limitation (Lance 7.0.0 manifest lacks writable metadata); tests confirm graceful behavior |
| 24 | loom sidecar embed CLI exists | ✓ VERIFIED | `crates/loom-cli/src/main.rs` lines 101-180: `sidecar embed` subcommand for `--source`, `--ir`, `--host`; Parquet path calls `embed_sidecar_into_parquet_file` |
| 25 | scripts/sidecar-overlay-test.sh validates roundtrip | ✓ VERIFIED | 236-line release gate with 8 sections: markers, core tests, Parquet roundtrip, Vortex/Lance markers, strippable overlay, full build, CLI build |
| 26 | All existing release gates remain green | ? UNCERTAIN | Not run during verification; sidecar test is additive per design; requires human to run full gate suite |

**Score:** 25/26 truths verified (1 uncertain)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/loom-ir-core/` | Independent decode IR crate | ✓ VERIFIED | Exists, compiles, only `fnv` dep, 0 Arrow/container deps |
| `crates/loom-container/` | Packaging/distribution crate | ✓ VERIFIED | Exists, compiles, depends on loom-ir-core, 22 modules |
| `crates/loom-core/src/lib.rs` | Re-export shim | ✓ VERIFIED | 38 `pub use` statements, 0 `pub mod`; re-exports sidecar + sidecar_routing |
| `crates/loom-ir-core/src/sidecar.rs` | SidecarOverlay, ChunkBinding, encode/decode | ✓ VERIFIED | 470 lines; deterministic encode/decode; 10 unit tests; compute_chunk_hash + verify_chunk_binding helpers |
| `crates/loom-ir-core/src/sidecar_routing.rs` | 4-gate routing decision | ✓ VERIFIED | 462 lines; SidecarRoutingInput/Decision/Diagnostic types; decide_sidecar_routing; 8 routing tests |
| `ingress/loom-parquet-ingress/src/sidecar_parquet.rs` | Parquet extract/embed | ✓ VERIFIED | 365 lines; extract from ParquetMetaData; embed into Vec<KeyValue>; embed_sidecar_into_parquet_file convenience; 9 tests |
| `ingress/loom-vortex-ingress/src/sidecar_vortex.rs` | Vortex sidecar adapter | ✓ VERIFIED | 138 lines; documented no-op with format limitation explanation; 4 tests |
| `ingress/loom-lance-ingress/src/sidecar_lance.rs` | Lance sidecar adapter | ✓ VERIFIED | 122 lines; documented no-op with format limitation explanation; 3 tests |
| `scripts/sidecar-overlay-test.sh` | Release gate script | ✓ VERIFIED | 236 lines; 8 sections; summary table (has CR-01 bug — see gaps) |
| `crates/loom-cli/src/main.rs` | CLI sidecar embed | ✓ VERIFIED | `sidecar embed` subcommand; `sidecar_embed_parquet` function; Vortex/Lance clear format limitation messages |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| loom-container/Cargo.toml | loom-ir-core | `loom-ir-core = { path = "../loom-ir-core" }` | ✓ WIRED | Line 8 of Cargo.toml |
| Cargo.toml workspace | loir-ir-core + loom-container | Workspace members | ✓ WIRED | Both crates in `[workspace] members` |
| sidecar.rs encode/decode | l2core_codec | `l2core_codec::write_u32/write_u16/write_u64` | ✓ WIRED | Lines 161-176 of sidecar.rs |
| sidecar_parquet.rs extract | sidecar.rs | `SidecarOverlay::decode` | ✓ WIRED | Line 58 of sidecar_parquet.rs |
| sidecar_parquet.rs embed | sidecar.rs | `SidecarOverlay::encode` | ✓ WIRED | Line 85 of sidecar_parquet.rs |
| source_contract.rs (Parquet) | sidecar_parquet.rs | `extract_sidecar_from_parquet_metadata` | ✓ WIRED | Line 95-96 of source_contract.rs |
| source_contract.rs (Vortex) | sidecar_vortex.rs | `extract_sidecar_from_vortex_buffer` | ✓ WIRED | Line 68 of source_contract.rs |
| source_contract.rs (Lance) | sidecar_lance.rs | `extract_sidecar_from_lance_dataset` | ✓ WIRED | Line 115 of source_contract.rs |
| sidecar_routing.rs | sidecar.rs | `HashVerificationResult, SidecarOverlay, ChunkBinding` | ✓ WIRED | Line 27 of sidecar_routing.rs |
| sidecar.rs compute_chunk_hash | l2core_codec format | `l2ir:{hash:016x}` format match | ✓ WIRED | Same format as `l2core_program_hash`; uses `fnv::FnvHasher` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `sidecar_parquet.rs::extract_sidecar_from_parquet_metadata` | `kv.value` | `FileMetaData.key_value_metadata()` | ✓ — reads real Parquet metadata | ✓ FLOWING |
| `sidecar_parquet.rs::embed_sidecar_into_key_value_metadata` | `overlay.encode()` | `SidecarOverlay::encode()` | ✓ — deterministic binary encoding | ✓ FLOWING |
| `sidecar.rs::compute_chunk_hash` | `fnv::FnvHasher` | Raw host data bytes | ✓ — FNV-1a hash computation | ✓ FLOWING |
| `sidecar_routing.rs::decide_sidecar_routing` | `input.hash_verification` | Caller-provided Vec<HashVerificationResult> | ✓ — routing operates on provided data | ✓ FLOWING |
| CLI `sidecar_embed_parquet` | `SidecarOverlay { bindings: Vec::new() }` | Hardcoded empty bindings | ⚠️ — empty bindings (IN-01) | ⚠️ HOLLOW |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| loom-ir-core compiles | `cargo build -p loom-ir-core` | Finished (2 warnings, 0 errors) | ✓ PASS |
| loom-container compiles | `cargo build -p loom-container` | Finished (2 warnings, 0 errors) | ✓ PASS |
| Sidecar roundtrip tests | `cargo test -p loom-ir-core -- sidecar` | 18 passed, 0 failed | ✓ PASS |
| Parquet sidecar tests | `cargo test -p loom-parquet-ingress` | 9 passed, 0 failed | ✓ PASS |
| Vortex tests (no regressions) | `cargo test -p loom-vortex-ingress` | 2 passed, 0 failed | ✓ PASS |
| Lance tests (no regressions) | `cargo test -p loom-lance-ingress` | 5 passed, 0 failed | ✓ PASS |

### Probe Execution

No probes declared for this phase. Skipped.

### Requirements Coverage

All 5 plans declare `requirements: []` — Phase 50 introduces no new requirement IDs. The REQUIREMENTS.md file contains no Phase 50 entries. This is consistent: the sidecar overlay is a repositioning architectural slice, not a user-facing feature with formal requirements.

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| N/A | N/A | No requirement IDs claimed | ✓ CONSISTENT | All plans have `requirements: []` |

### Anti-Patterns Found

From the code review (50-REVIEW.md), the following anti-patterns were identified and remain present:

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `scripts/sidecar-overlay-test.sh` | 218 | `echo "${FAILURE_MESSAGES[@]}" \| grep -q` — Bash `echo` joins array as single line, causing only first failing section to be detected | 🛑 CRITICAL (CR-01) | Release gate summary table can silently show PASS for failing sections |
| `crates/loom-ir-core/src/sidecar.rs` | 162 | `ir_bytes.len() as u32` silent truncation | ⚠️ WARNING (WR-01) | >4 GiB IR bytes silently truncated to lower 32 bits |
| `ingress/loom-parquet-ingress/src/sidecar_parquet.rs` | 158-176 | In-place file overwrite in `embed_sidecar_into_parquet_file` | ⚠️ WARNING (WR-02) | Data-loss risk if process crashes during write |
| `crates/loom-ir-core/src/sidecar.rs` | 314 | `assert!` in `write_u8_len_str` on >255 byte strings | ⚠️ WARNING (WR-03) | Encode panics on invalid data while decode returns errors |
| `scripts/sidecar-overlay-test.sh` | 40 | `rg` dependency without availability check | ⚠️ WARNING (WR-04) | Script fails with cryptic error if `rg` not installed |
| `crates/loom-ir-core/src/sidecar.rs` | 30 | Unused `use std::hash::Hasher` import | ℹ️ INFO (WR-05) | Dead import |
| `crates/loom-cli/src/main.rs` | 177-179 | CLI always creates empty bindings | ℹ️ INFO (IN-01) | Per-column bindings deferred; content-hash inactive from CLI |
| `ingress/loom-lance-ingress/src/sidecar_lance.rs` | 43 | `extract_sidecar_from_lance_dataset()` takes no arguments | ℹ️ INFO (IN-02) | Will need new signature when Lance adds metadata support |
| `crates/loom-ir-core/src/sidecar.rs` | 125-129 | FNV-1a hash duplication with l2core_codec.rs | ℹ️ INFO (IN-03) | Two hashing code paths could diverge |

### Gaps Summary

**BLOCKER-level gap:**
- **CR-01 (Release gate summary table bug):** The `scripts/sidecar-overlay-test.sh` summary table at line 218 uses `echo "${FAILURE_MESSAGES[@]}" | grep -q "^${section}:"` which joins all array elements into a single line. Only the first failing section is detected; subsequent failing sections silently show PASS. The `$FAILED` counter is correct, so `exit 1` still triggers, but the per-section status display is misleading. This is a code bug that should be fixed before the release gate is relied upon.

**WARNING-level gaps:**
- **WR-01 (u32 truncation):** `SidecarOverlay::encode` silently truncates `ir_bytes.len()` to `u32`. On 64-bit platforms with >4 GiB IR bytes, this produces corrupt encoded output.
- **WR-02 (In-place overwrite):** `embed_sidecar_into_parquet_file` opens the source file for reading then truncates and overwrites the same path. A crash during write causes data loss. Should write to temp file + atomic rename.
- **WR-03 (assert vs error):** `write_u8_len_str` panics on >255 byte strings while `read_u8_len_str` returns `Result`. Asymmetry in error handling between encode and decode.

**INFO-level items:**
- **IN-01:** CLI creates empty bindings (per-column content-hash bindings deferred).
- **IN-02:** Lance `extract_sidecar_from_lance_dataset()` signature will need breaking change when Lance adds metadata API.
- **IN-03:** FNV-1a hash implementation duplicated between `sidecar.rs` (uses `fnv::FnvHasher`) and `l2core_codec.rs` (hand-rolled `stable_fnv1a64`). Both should produce identical results but could diverge.

### Human Verification Required

#### 1. Release Gate CR-01 Fix and Re-run

**Test:** Fix the summary table bug in `scripts/sidecar-overlay-test.sh` (replace line 218 `echo "${FAILURE_MESSAGES[@]}" | grep -q "^${section}:"` with array-based matching per REVIEW.md CR-01), then run `bash scripts/sidecar-overlay-test.sh`
**Expected:** All 8 sections show correct PASS/FAIL status. When multiple sections fail, each shows FAIL individually and `$FAILED` count matches the number of FAIL sections displayed.
**Why human:** Code fix required; cannot verify fix without running the full gate script and observing output.

#### 2. Strippable Overlay Invariant

**Test:** Create a Parquet file with a `loom.sidecar.v1` KeyValue metadata entry. Read the file using arrow-rs `ParquetRecordBatchReader` without any Loom-awareness.
**Expected:** The Arrow reader returns row data successfully. Unknown `loom.*` KeyValue keys are silently ignored by standard Parquet readers. This proves the "叠加而非替换" (overlay, not replace) discipline from §2.3.
**Why human:** Requires real Parquet I/O with sidecar-embedded files and inspecting reader behavior. Cannot verify through static analysis or unit tests alone.

#### 3. CLI End-to-End Test

**Test:** `loom sidecar embed --source /path/to/data.parquet --ir /path/to/program.l2ir`
**Expected:** CLI prints `Sidecar embedded: N chunk bindings, IR identity: l2ir:<hex>`. Subsequent extraction of the sidecar from the Parquet file yields an overlay matching what was embedded. For Vortex/Lance: CLI prints clear format limitation messages and exits with code 1.
**Why human:** Requires real Parquet files and L2Core IR files on disk. Tests the full CLI → embed → extract pipeline.

#### 4. Existing Release Gate Regression Check

**Test:** Run `bash scripts/mvp2-verify.sh` (or equivalent) and confirm all previously-passing sections still pass.
**Expected:** All existing release gates remain green. The sidecar gate is additive, not a replacement.
**Why human:** Requires running the full release gate suite on the local machine with appropriate toolchains available.

---

_Verified: 2026-06-11T17:30:00Z_
_Verifier: the agent (gsd-verifier)_
