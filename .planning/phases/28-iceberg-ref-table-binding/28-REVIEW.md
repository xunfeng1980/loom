---
phase: 28-iceberg-ref-table-binding
reviewed: 2026-06-08T23:03:39Z
depth: standard
files_reviewed: 22
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - crates/loom-ffi/tests/duckdb_runtime_ffi.rs
  - crates/loom-iceberg-binding/Cargo.toml
  - crates/loom-iceberg-binding/src/binding_contract.rs
  - crates/loom-iceberg-binding/src/lib.rs
  - crates/loom-iceberg-binding/tests/binding_contract.rs
  - crates/loom-iceberg-binding/tests/binding_handoff.rs
  - crates/loom-iceberg-binding/tests/dependency_boundary.rs
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
  - crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs
  - scripts/iceberg-binding-test.sh
  - scripts/mvp0-verify.sh
findings:
  critical: 2
  warning: 1
  info: 0
  total: 3
status: issues_found
---

# Phase 28: Code Review Report

**Reviewed:** 2026-06-08T23:03:39Z
**Depth:** standard
**Files Reviewed:** 22
**Status:** issues_found

## Summary

Reviewed the Phase 28 Iceberg binding crate, fixtures, dependency guards, release gate wiring, and DuckDB runtime FFI test isolation change. The workspace manifest and lockfile did not show an official `iceberg` crate dependency or Arrow 57/58 drift. The DuckDB runtime FFI cache isolation change is locally scoped and does not appear to mask the cache assertions in this test binary.

The binding acceptance path still has two fail-closed defects: the source/oracle evidence contract can be forged with self-consistent metadata because it does not prove decoded values or source identity, and sidecar-referenced evidence paths are followed without the same local/remote policy enforcement applied elsewhere.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: BLOCKER - Accepted bindings can be produced from self-consistent but forged oracle evidence

**File:** `crates/loom-iceberg-binding/src/binding_contract.rs:485`
**Issue:** `SourceOracleEvidenceArtifact` only carries row count, table identifiers, artifact SHA, status booleans, and a decoded-row fixture identity string. `validate_source_oracle_evidence` checks those fields and row count against the verified artifact, then `bind_iceberg_ref_from_paths` constructs accepted oracle evidence from `evidence.row_count`. It never validates decoded row values, a source artifact hash/path, or any independent oracle output. A sidecar can therefore point at any verifier-accepted artifact and a separately forged JSON file whose table UUID/schema/snapshot/hash/row count match that artifact, and the binding can be accepted without evidence that the artifact represents the Iceberg/source data.

**Fix:** Extend the evidence artifact with independently produced source/oracle identity and content proof, then validate it before constructing `SourceOracleEvidence::accepted`. For this phase's narrow Int32/LMT1 slice, require either decoded row values or a decoded values digest plus source artifact hash/path, then decode the verified Loom artifact and compare:

```rust
struct SourceOracleEvidenceArtifact {
    row_count: u64,
    table_uuid: String,
    schema_id: i32,
    snapshot_id: i64,
    artifact_sha256: String,
    source_path: String,
    source_sha256: String,
    decoded_row_fixture: DecodedRowFixtureEvidence,
}

struct DecodedRowFixtureEvidence {
    identity: String,
    strategy: String,
    row_count: u64,
    values_sha256: String,
    accepted: bool,
    oracle_accepted: bool,
    status: Option<String>,
}
```

Compute the decoded artifact values digest from `artifact_bytes`, compare it to `decoded_row_fixture.values_sha256`, and reject if the source path/hash or decoded digest is missing or mismatched. Add a negative test where forged evidence has the correct row count and hash but wrong decoded values.

### CR-02: BLOCKER - Sidecar evidence paths bypass the local-only path policy

**File:** `crates/loom-iceberg-binding/src/binding_contract.rs:363`
**Issue:** The accepted binding path reads `source_oracle_evidence_path` from the sidecar, resolves it, and opens it, but `local_policy_marker_for_binding` does not inspect `source_oracle_evidence_path`. `resolve_sidecar_relative_path` also accepts absolute paths and falls back to process-relative paths when a sidecar-relative sibling does not exist. This leaves the most important sidecar-controlled evidence artifact path outside the remote/object-store/credential marker checks that are applied to metadata, artifact path, and sidecar evidence subfields. A sidecar can reference an absolute or process-relative evidence JSON outside the sidecar bundle, including paths with remote/catalog/credential markers if a matching local path exists, instead of failing closed at policy validation.

**Fix:** Validate every sidecar-controlled path before filesystem access, including `source_oracle_evidence_path`, and confine relative paths to the sidecar directory:

```rust
fn resolve_local_sidecar_path(sidecar_path: &Path, referenced_path: &str) -> Result<PathBuf, IcebergBindingReport> {
    if let Some(marker) = forbidden_local_marker(referenced_path) {
        return Err(IcebergBindingReport::unsupported(None, format!("unsupported evidence path: {marker}")));
    }
    let referenced = Path::new(referenced_path);
    if referenced.is_absolute() {
        return Err(IcebergBindingReport::unsupported(None, "absolute evidence paths are unsupported"));
    }
    let base = sidecar_path.parent().ok_or_else(|| {
        IcebergBindingReport::rejected("sidecar path must have a parent directory")
    })?;
    Ok(base.join(referenced))
}
```

Add negative tests for `source_oracle_evidence_path` values containing `s3://`, `warehouse`, `credential`, absolute paths, and `../` traversal.

## Warnings

### WR-01: WARNING - Release gate marker checks can pass on strings instead of behavior

**File:** `scripts/iceberg-binding-test.sh:64`
**Issue:** `check_required_code_patterns` removes line comments, but it still accepts matches from string literals, test names, and fixture text. Several checks later in the script mix production source, tests, and JSON fixtures in one marker search. That means the gate can pass because a marker appears in a test function name, fixture string, or report-oriented text, even if the production code path no longer performs the required behavior. This is especially risky for markers like `shasum`, `DecodedRowFixture`, `manifest-only`, and evidence fixture names.

**Fix:** Split the gate into behavior checks and source-surface checks. Keep `cargo test` as the behavior proof, and replace marker checks with targeted assertions such as `rg -n 'verify_artifact\\(' crates/loom-iceberg-binding/src/binding_contract.rs`, `rg -n 'sha256_bytes\\(' crates/loom-iceberg-binding/src/binding_contract.rs`, and named test invocations for the fail-closed cases. Avoid satisfying production requirements from tests, fixtures, or report prose.

---

_Reviewed: 2026-06-08T23:03:39Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
