---
phase: 27-lance-parquet-archival-readability-dataset-ingress
verified: 2026-06-08T21:48:17Z
status: passed
score: 10/10 must-haves verified
overrides_applied: 0
---

# Phase 27: Lance + Parquet Archival Readability / Dataset Ingress Verification Report

**Phase Goal:** Supported local Lance datasets and Parquet files produce source-neutral facts, verifier-backed Loom artifacts, oracle/equivalence evidence, and current plus legacy archival-readability proof for the narrow non-null primitive/table slice.
**Verified:** 2026-06-08T21:48:17Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | All five Phase 27 plans have summaries and committed implementation evidence. | VERIFIED | `27-01-SUMMARY.md` through `27-05-SUMMARY.md` exist. All summary and review-fix commit hashes checked with `git cat-file -e` resolved: `a6f4e15`, `153db65`, `4ab9415`, `a8db424`, `a6dfaa8`, `aff8cf0`, `8dcbea3`, `bc0af38`, `ca6c789`, `941540e`, `1803f41`, `3e112d5`, `546aedb`, `aef7865`, `aa49a35`, `99b6046`, `be3bcf8`, `1c8cb6e`, `b13ecf2`, `115bf45`, `3d1d910`, `ddfa704`, `e94543f`, `1ff19da`, `d8d895c`, `bb724a1`. |
| 2 | `loom-source-ingress` remains source-neutral while Lance/Parquet SDK dependencies stay adapter-local. | VERIFIED | Direct dependency scan found `lance` only in root workspace pins and `crates/loom-lance-ingress/Cargo.toml`; `parquet` only in root workspace pins and `crates/loom-parquet-ingress/Cargo.toml`. `cargo tree -p loom-core`, `loom-ffi`, `loom-source-ingress`, and `loom-cli` showed no Lance/Parquet/object-store dependency matches. |
| 3 | Parquet and Lance adapters extract source-neutral facts and classify accepted/unsupported/rejected sources. | VERIFIED | `crates/loom-parquet-ingress/src/source_contract.rs` maps schema, row groups, layout, splits, coverage, and diagnostics. `crates/loom-lance-ingress/src/source_contract.rs` maps schema, version, manifest, fragments, splits, coverage, and diagnostics. Focused gate ran and passed contract tests for both adapters. |
| 4 | Malformed Parquet/Lance paths reject fail-closed without trusted facts. | VERIFIED | Parquet malformed test asserts `Rejected`, `facts = None`, no artifact verification, and no oracle evidence. Lance non-dataset test asserts the same. `bash scripts/lance-parquet-ingress-test.sh` passed these tests during verification. |
| 5 | Accepted supported primitive/table paths emit verifier-accepted `LMC1(LMP1)` or `LMC1(LMT1)` artifacts. | VERIFIED | Parquet and Lance `emit_source_ingress_lmc1_from_*_path` call `verify_artifact` before `SourceIngressReport::accepted`; handoff tests decode `LMP1` and `LMT1` containers and assert artifact byte lengths match report summaries. |
| 6 | Oracle/equivalence evidence exists for accepted Parquet and Lance artifacts. | VERIFIED | Parquet reports use `SourceOracleStrategy::ArrowScan`; Lance reports use `SourceOracleStrategy::SourceNativeScan`. Handoff tests assert row count, null checks, schema, and exact Int32/Int64/Float32/Float64 row equality against decoded Loom output. |
| 7 | Actual older-version Parquet 57.0.0 and Lance 6.0.0 fixtures exist, are paired with Loom artifacts, and are current-reader readable/rewriteable. | VERIFIED | `file` identifies `legacy-v1.parquet` as Apache Parquet. Lance fixture contains `_transactions`, `_versions`, and `data/*.lance`. Manifests record generator versions `57.0.0` and `6.0.0` plus hashes. Legacy tests verify paired Loom artifacts, read actual older sources with current readers, re-emit matching Loom bytes, rewrite to current source outputs, and read rewritten outputs back. |
| 8 | Code review findings are resolved: Parquet extension metadata rejection, sanitized diagnostics, and table-form renamed dependency guard. | VERIFIED | `27-REVIEW-FINAL.md` is clean. Code contains `field_has_extension_metadata` in both adapters; Parquet `layout_from_batches` rejects extension fields before bytes. Parquet diagnostic detail uses `sanitized_detail`. `scripts/lance-parquet-ingress-test.sh` scans inline and table-form `package = "lance"|"parquet"` dependency declarations. |
| 9 | `scripts/lance-parquet-ingress-test.sh` passes and `scripts/mvp0-verify.sh` invokes it after Phase 26 and before DuckDB smoke. | VERIFIED | `bash scripts/lance-parquet-ingress-test.sh` passed in this verification run. `bash -n scripts/mvp0-verify.sh` passed. Order check returned positions `[4464, 4609, 4736]` for source-ingress, Lance/Parquet, DuckDB smoke. |
| 10 | Deferred scope did not leak into Phase 27 public surfaces. | VERIFIED | Focused gate public-surface checks passed. Direct scan of FFI headers, `duckdb-ext/loom_extension.cpp`, CLI, core/source-ingress/ffi source found no Phase 27 Lance/Parquet routes, footer/manifest embedding controls, object credential controls, split worker controls, or new native-kernel public markers. Existing DuckDB projection-pushdown code predates Phase 27 and is not source-specific. |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/loom-source-ingress/src/lib.rs` | Source-neutral ingress vocabulary and accepted artifact handoff | VERIFIED | Contains `SourceIngressAcceptedArtifact { bytes, report }`; no Lance/Parquet SDK types. |
| `crates/loom-parquet-ingress/src/source_contract.rs` | Parquet facts, classification, oracle, verifier-routed emission | VERIFIED | Uses local `ParquetRecordBatchReaderBuilder`, source-neutral facts, extension rejection, `verify_artifact`, `SourceIngressReport::accepted`. |
| `crates/loom-lance-ingress/src/source_contract.rs` | Lance async facts, classification, oracle, verifier-routed emission | VERIFIED | Uses local async `Dataset::open`/scan, source-neutral facts, extension rejection, `verify_artifact`, `SourceIngressReport::accepted`. |
| `crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.parquet` | Actual older Parquet fixture | VERIFIED | Present; `file` reports Apache Parquet; manifest hash matches `d45ed9f...`. |
| `crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.loom` | Paired verifier-accepted Loom artifact | VERIFIED | Present; manifest hash matches `bfd642...`; legacy test verifies and decodes it. |
| `crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.lance/` | Actual older Lance dataset | VERIFIED | Present with `_transactions`, `_versions`, and `data/*.lance`; legacy test verifies tree hash and reads with current Lance. |
| `crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.loom` | Paired verifier-accepted Loom artifact | VERIFIED | Present; manifest hash matches `bfd642...`; legacy test verifies and decodes it. |
| `scripts/lance-parquet-ingress-test.sh` | Focused Phase 27 release gate | VERIFIED | Passed in verification run; checks fixtures, tests, dependency boundaries, report markers, and public-surface scope. |
| `scripts/mvp0-verify.sh` | Main release gate wiring | VERIFIED | Syntax passes; invokes Phase 27 gate after Phase 26 and before DuckDB smoke. |
| `27-ARCHIVAL-READABILITY-REPORT.md` | Final evidence and scope report | VERIFIED | Contains required evidence sections and states manifest-only evidence is failing, not passing. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `loom-parquet-ingress` | `loom-source-ingress` | `SourceFacts`, `SourceCoverage`, `SourceIngressReport`, `SourceIngressAcceptedArtifact` imports | WIRED | Public helpers return source-neutral reports and accepted handoff. |
| `loom-lance-ingress` | `loom-source-ingress` | Same source-neutral types | WIRED | Async helpers return source-neutral reports and accepted handoff. |
| Parquet/Lance emit helpers | `loom-core::artifact_verifier::verify_artifact` | Verification before accepted report construction | WIRED | Accepted report is constructed only after `ArtifactVerificationStatus::Accepted`. |
| Adapter handoff tests | `loom-core` container/table/layout decode | `decode_layout_payload_maybe_container`, `decode_table_payload_maybe_container` | WIRED | Tests decode emitted bytes and assert rows. |
| `scripts/mvp0-verify.sh` | `scripts/lance-parquet-ingress-test.sh` | Shell invocation | WIRED | Invocation order verified by Python index check. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| Parquet accepted emission | `batches` -> `artifact_bytes` | `ParquetRecordBatchReaderBuilder::try_new(file).build()` then `reader.collect()` | Yes | FLOWING |
| Lance accepted emission | `batches` -> `artifact_bytes` | `Dataset::open`, `dataset.scan().try_into_stream()`, `try_collect` | Yes | FLOWING |
| Parquet legacy proof | `source_batches`, `accepted.bytes`, `rewritten_batches` | Actual `legacy-v1.parquet`, paired `.loom`, current `ArrowWriter` rewrite | Yes | FLOWING |
| Lance legacy proof | `source_batches`, `accepted.bytes`, `rewritten_batches` | Actual `legacy-v1.lance/`, paired `.loom`, current `Dataset::write` rewrite | Yes | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Focused Phase 27 gate passes | `bash scripts/lance-parquet-ingress-test.sh` | Passed; included source-ingress, Parquet/Lance dependency, contract, handoff, legacy, artifact verifier, boundary, and public surface checks | PASS |
| Main verifier syntax is valid | `bash -n scripts/mvp0-verify.sh` | Exit 0 | PASS |
| Main verifier order is correct | Python order check for source-ingress -> Lance/Parquet -> DuckDB smoke | Printed `[4464, 4609, 4736]`; assertion passed | PASS |
| Commit evidence exists | `git cat-file -e <hash>^{commit}` for summary/review-fix hashes | All listed hashes resolved | PASS |

### Probe Execution

| Probe | Command | Result | Status |
|---|---|---|---|
| Phase 27 focused gate | `bash scripts/lance-parquet-ingress-test.sh` | Exit 0; final line `Phase 27 Lance/Parquet ingress closeout gate PASSED` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| `PHASE-27` | `27-01` through `27-05` | Lance + Parquet archival readability / dataset ingress | SATISFIED | All five plans declare `PHASE-27`; roadmap Phase 27 goal is satisfied by source-neutral facts, verifier-backed artifacts, oracle/equivalence, current and legacy fixture proof, and release gate wiring. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| None | - | - | - | Stub/debt-marker scan over Phase 27 adapter files, gate scripts, and final report found no TODO/FIXME/XXX/TBD/placeholder markers or empty implementations. |

### Human Verification Required

None.

### Gaps Summary

No blocking gaps found. The Phase 27 goal is achieved for the bounded local-file, non-null primitive/table slice. Later roadmap phases explicitly own Iceberg binding, StarRocks/DuckDB dual surface, and full semantic compatibility; no Phase 27 failures were deferred.

---

_Verified: 2026-06-08T21:48:17Z_
_Verifier: the agent (gsd-verifier)_
