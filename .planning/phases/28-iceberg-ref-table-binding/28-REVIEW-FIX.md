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
**Status:** fixed: requires human verification
**Applied fix:** Extended source/oracle evidence with source path/hash and decoded values SHA-256, decoded verified LMT1/LMP1 non-null Int32 rows into a deterministic digest, compared source and values evidence before acceptance, and added forged same-row-count/same-artifact-hash regression coverage.

### CR-02: BLOCKER - Sidecar evidence paths bypass the local-only path policy

**Files modified:** `Cargo.lock`, `crates/loom-iceberg-binding/Cargo.toml`, `crates/loom-iceberg-binding/src/binding_contract.rs`, `crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs`, `crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json`, `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-schema-sidecar.json`, `crates/loom-iceberg-binding/tests/fixtures/local/mismatch-snapshot-sidecar.json`
**Commit:** `f5c5119`
**Status:** fixed: requires human verification
**Applied fix:** Replaced permissive evidence path resolution with sidecar-relative local-only resolution, rejected remote markers, absolute paths, and parent traversal before evidence reads, and added negative cases for `s3://`, `gs://`, `abfs://`, `warehouse`, `catalog`, `credential`, `token`, `secret`, `access_key`, absolute paths, and `..`.

### WR-01: WARNING - Release gate marker checks can pass on strings instead of behavior

**Files modified:** `scripts/iceberg-binding-test.sh`
**Commit:** `8c2ed0b`
**Status:** fixed
**Applied fix:** Removed broad required-marker scans over tests and fixtures, added targeted production-source regex checks for verifier invocation, artifact SHA helper behavior, decoded values digesting, and sidecar-local evidence path resolution, then used named cargo tests as behavior proofs.

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
