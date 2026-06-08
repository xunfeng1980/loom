---
phase: 28-iceberg-ref-table-binding
fixed_at: 2026-06-08T23:17:22Z
review_path: .planning/phases/28-iceberg-ref-table-binding/28-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 28: Code Review Fix Report

**Fixed at:** 2026-06-08T23:17:22Z
**Source review:** `.planning/phases/28-iceberg-ref-table-binding/28-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### CR-01: BLOCKER - Accepted bindings can be produced from self-consistent but forged oracle evidence

**Files modified:** `Cargo.lock`, `crates/loom-iceberg-binding/Cargo.toml`, `crates/loom-iceberg-binding/src/binding_contract.rs`, `crates/loom-iceberg-binding/tests/binding_handoff.rs`, `crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs`, `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json`, `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-source-evidence.json`, `crates/loom-iceberg-binding/tests/fixtures/local/forged-oracle-evidence.json`, `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-schema-sidecar.json`, `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-snapshot-sidecar.json`, `crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json`
**Commit:** `f5c5119`
**Status:** fixed: verified
**Applied fix:** Extended source/oracle evidence with source path/hash and decoded values SHA-256, decoded verified LMT1/LMP1 non-null Int32 rows into a deterministic digest, compared source and values evidence before acceptance, and added forged same-row-count/same-artifact-hash regression coverage.

### CR-02: BLOCKER - Sidecar evidence paths bypass the local-only path policy

**Files modified:** `Cargo.lock`, `crates/loom-iceberg-binding/Cargo.toml`, `crates/loom-iceberg-binding/src/binding_contract.rs`, `crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs`, `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json`, `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-schema-sidecar.json`, `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-snapshot-sidecar.json`
**Commit:** `f5c5119`
**Status:** fixed: verified
**Applied fix:** Replaced permissive evidence path resolution with sidecar-relative local-only resolution, rejected remote markers, absolute paths, and parent traversal before evidence reads, and added negative cases for `s3://`, `gs://`, `abfs://`, `warehouse`, `catalog`, `credential`, `token`, `secret`, `access_key`, absolute paths, and `..`.

### WR-01: WARNING - Release gate marker checks can pass on strings instead of behavior

**Files modified:** `scripts/iceberg-binding-test.sh`
**Commit:** `8c2ed0b`
**Status:** fixed: verified
**Applied fix:** Removed broad required-marker scans over tests and fixtures, added targeted production-source regex checks for verifier invocation, artifact SHA helper behavior, decoded values digesting, and sidecar-local evidence path resolution, then used focused cargo tests as behavior proofs.

## Tradeoffs

- The decoded value proof remains narrow by design: Phase 28 validates the current non-null Int32 table slice rather than a general Arrow canonicalization contract.
- Source proof is a local-file fixture hash, not a production Iceberg scan. This preserves the Phase 28 non-goal of avoiding catalogs, object stores, and official Iceberg SDK coupling.
- The adapter uses `arrow-array` and `arrow-data` to canonicalize decoded values, but still does not add the official `iceberg` crate or Arrow/Parquet version skew.

## Verification

- `cargo test -p loom-iceberg-binding --test binding_handoff` passed
- `cargo test -p loom-iceberg-binding --test mismatch_fail_closed` passed
- `cargo test -p loom-iceberg-binding --test binding_contract` passed
- `cargo check -p loom-iceberg-binding` passed
- `bash -n scripts/iceberg-binding-test.sh` passed
- `bash scripts/iceberg-binding-test.sh` passed

## Skipped Issues

None.

---

_Fixed: 2026-06-08T23:17:22Z_
_Fixer: the agent (gsd-code-fixer)_
_Iteration: 1_

## Final Review Follow-up

**Source review:** `.planning/phases/28-iceberg-ref-table-binding/28-REVIEW-FINAL.md`
**Status:** fixed: verified

### CR-01: BLOCKER - Phase 28 release gate fails on stale production-source regexes

**Files modified:** `scripts/iceberg-binding-test.sh`
**Applied fix:** Replaced stale regex checks for the previous decoded-value marker with checks aligned to the implemented `decoded_values_sha256(...)` validation, `append_int32_array_digest_lines(...)` canonicalization, sidecar/evidence path confinement, local source byte hashing, and explicit source-hash mismatch diagnostics. Removed reliance on stale named-test invocations from the gate.

### WR-01: WARNING - Stale source fixture bypasses the source hash mismatch branch

**Files modified:** `crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json`, `crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs`
**Applied fix:** Updated the stale-source fixture to the current nested `source.path`/`source.sha256` schema while keeping an intentionally stale source hash. The regression test now copies the bad evidence into the dynamic temp bundle, rewrites only the artifact hash to match the generated artifact, and asserts the exact source-hash or decoded-values diagnostic for stale and forged evidence respectively.

### Follow-up Verification

- `cargo test -p loom-iceberg-binding --test mismatch_fail_closed stale_source_and_forged_oracle_evidence_flags_return_no_bytes` passed
- `bash scripts/iceberg-binding-test.sh` passed
