---
phase: 27-lance-parquet-archival-readability-dataset-ingress
reviewed: 2026-06-08T21:42:35Z
depth: standard
files_reviewed: 4
files_reviewed_list:
  - crates/loom-parquet-ingress/src/source_contract.rs
  - crates/loom-parquet-ingress/tests/source_ingress_contract.rs
  - crates/loom-parquet-ingress/tests/source_ingress_handoff.rs
  - scripts/lance-parquet-ingress-test.sh
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
status: issues_found
---

# Phase 27: Code Review Rerun Report

**Reviewed:** 2026-06-08T21:42:35Z
**Depth:** standard
**Files Reviewed:** 4
**Status:** issues_found

## Narrative Findings (AI reviewer)

## Summary

Targeted re-review of the three findings from `27-REVIEW.md`.

Verified fixed:

- CR-01: Parquet Arrow extension metadata is now classified as `extension`, excluded from accepted coverage, reported as unsupported schema, and blocked before `LMC1` emission.
- WR-01: Parquet rejected-path details now pass through `sanitized_detail`, redacting credential/secret/token/access_key/URI-looking lines and omitting `source_detail` when sanitized detail is empty.

Still incomplete:

- WR-02 is only partially fixed. The closeout gate now checks `loom-cli` and catches inline renamed dependencies, but the manifest regex still misses Cargo's table-form renamed dependency syntax.

Verification run:

- `cargo test -p loom-parquet-ingress --test source_ingress_contract`
- `cargo test -p loom-parquet-ingress --test source_ingress_handoff`
- `bash -n scripts/lance-parquet-ingress-test.sh`
- `bash scripts/lance-parquet-ingress-test.sh`

## Warnings

### WR-01 [WARNING]: Renamed Lance/Parquet dependencies can still bypass the direct manifest guard in table form

**File:** `scripts/lance-parquet-ingress-test.sh:121`

**Issue:** `check_direct_source_deps` catches inline renamed dependencies such as `source_sdk = { package = "lance", ... }`, but it does not match Cargo's equivalent table-form syntax:

```toml
[dependencies.source_sdk]
package = "lance"
```

That leaves the WR-02 direct manifest guard incomplete for package-renamed `lance`/`parquet` dependencies. The new `loom-cli` cargo-tree check helps for public crates that are checked and whose dependencies resolve, but the direct workspace manifest scan can still miss a renamed source SDK declaration outside those checked trees.

**Fix:** Extend the direct dependency regex to also catch standalone package rename lines, excluding only the approved workspace and adapter manifests as it already does.

```bash
refs="$(
    rg -n '^[[:space:]]*([A-Za-z0-9_-]+[[:space:]]*=.*package[[:space:]]*=[[:space:]]*"(lance|parquet)"|(lance|parquet)[[:space:]]*=|package[[:space:]]*=[[:space:]]*"(lance|parquet)")' \
        Cargo.toml crates/*/Cargo.toml || true
)"
```

---

_Reviewed: 2026-06-08T21:42:35Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
