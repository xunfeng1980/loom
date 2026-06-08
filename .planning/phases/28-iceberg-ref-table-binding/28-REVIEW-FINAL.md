---
phase: 28-iceberg-ref-table-binding
reviewed: 2026-06-08T23:22:53Z
depth: standard
files_reviewed: 17
files_reviewed_list:
  - crates/loom-iceberg-binding/src/binding_contract.rs
  - crates/loom-iceberg-binding/tests/binding_handoff.rs
  - crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs
  - crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json
  - crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-metadata.json
  - crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-source-evidence.json
  - crates/loom-iceberg-binding/tests/fixtures/local/forged-oracle-evidence.json
  - crates/loom-iceberg-binding/tests/fixtures/local/manifest-only-sidecar.json
  - crates/loom-iceberg-binding/tests/fixtures/local/mismatch-schema-sidecar.json
  - crates/loom-iceberg-binding/tests/fixtures/local/mismatch-snapshot-sidecar.json
  - crates/loom-iceberg-binding/tests/fixtures/local/rejected-missing-identity.json
  - crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json
  - crates/loom-iceberg-binding/tests/fixtures/local/unsupported-remote-metadata.json
  - crates/loom-iceberg-binding/tests/fixtures/local/source/demo-events.parquet
  - scripts/iceberg-binding-test.sh
  - .planning/phases/28-iceberg-ref-table-binding/28-REVIEW.md
  - .planning/phases/28-iceberg-ref-table-binding/28-REVIEW-FIX.md
findings:
  critical: 1
  warning: 1
  info: 0
  total: 2
status: issues_found
---

# Phase 28: Final Code Review Report

**Reviewed:** 2026-06-08T23:22:53Z
**Depth:** standard
**Files Reviewed:** 17
**Status:** issues_found

## Summary

Reviewed the Phase 28 Iceberg binding implementation and the fixes for CR-01, CR-02, and WR-01. The core accepted-binding path now recomputes artifact SHA-256, runs the verifier, requires sidecar-relative source/oracle evidence, validates local source bytes by hash, and compares decoded Int32 value digests before constructing accepted evidence.

Two issues remain. The Phase 28 release gate currently fails after the WR-01 fix because its production-source regexes no longer match the implementation, and the stale-source negative fixture no longer exercises the source hash mismatch branch it is meant to cover.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: BLOCKER - Phase 28 release gate fails on stale production-source regexes

**File:** `scripts/iceberg-binding-test.sh:362`

**Issue:** `bash scripts/iceberg-binding-test.sh` exits with `[FAIL] decoded values digest computation missing production source pattern: decoded_values_sha256\(&artifact_bytes, in crates/loom-iceberg-binding/src/binding_contract.rs`. The implementation calls `decoded_values_sha256(artifact_bytes, payload_kind, registry)` at `crates/loom-iceberg-binding/src/binding_contract.rs:996`, without `&artifact_bytes`, so the hardened gate is now a false negative. The next check at `scripts/iceberg-binding-test.sh:367` also requires `loom-decoded-int32-v1`, which does not appear in `binding_contract.rs`, so correcting the first regex would expose another stale marker failure.

**Fix:** Align the gate with the current implementation or add the intended stable canonicalization marker to the production source. For example:

```bash
check_source_regex "decoded values digest computation" \
    "${binding_contract_src}" \
    'decoded_values_sha256\(artifact_bytes,'
check_source_regex "decoded Int32 row canonicalization" \
    "${binding_contract_src}" \
    'append_int32_array_digest_lines'
```

Alternatively, add a real `loom-decoded-int32-v1` version line to the canonical digest payload in `append_int32_array_digest_lines` and keep the script check pointed at that behavior.

## Warnings

### WR-01: WARNING - Stale source fixture bypasses the source hash mismatch branch

**File:** `crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json:7`

**Issue:** `SourceEvidenceStatus` requires `source.path` and `source.sha256` in `binding_contract.rs:516`, but `stale-source-evidence.json` still uses legacy top-level `source_path` / `source_sha256` fields and leaves `source` with only `accepted` and `status`. The `stale_source_and_forged_oracle_evidence_flags_return_no_bytes` test at `crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs:445` therefore passes by JSON deserialization failure, not by reaching the implemented `source evidence SHA-256 does not match local source bytes` check at `binding_contract.rs:966`.

**Fix:** Make the fixture conform to the current evidence schema while keeping the hash wrong, so it reaches the intended validation branch:

```json
"source": {
  "accepted": true,
  "status": "accepted",
  "path": "source/demo-events.parquet",
  "sha256": "bb6b66e15e903679917b57081ac69055e09609301a2b2492018ccae99d50fb97"
}
```

Then assert the diagnostic includes `source evidence SHA-256 does not match local source bytes` so future regressions cannot pass through an earlier parse error.

---

_Reviewed: 2026-06-08T23:22:53Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
