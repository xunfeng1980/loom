---
phase: 27-lance-parquet-archival-readability-dataset-ingress
fixed_at: 2026-06-08T21:40:05Z
review_path: .planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-REVIEW.md
iteration: 1
findings_in_scope: 3
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 27: Code Review Fix Report

**Fixed at:** 2026-06-08T21:40:05Z
**Source review:** `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 3
- Fixed: 3
- Skipped: 0

## Fixed Issues

### CR-01: Parquet accepts Arrow extension fields as primitive Loom artifacts

**Files modified:** `ingress/loom-parquet-ingress/src/source_contract.rs`, `ingress/loom-parquet-ingress/tests/source_ingress_contract.rs`, `ingress/loom-parquet-ingress/tests/source_ingress_handoff.rs`
**Commit:** `1ff19da`
**Applied fix:** Added Parquet extension metadata detection for `ARROW:extension:name`, classified those fields as `extension`, excluded them from accepted coverage, returned unsupported schema diagnostics, and blocked layout emission before Loom artifact bytes can be produced. Added Parquet contract and handoff regression coverage for extension-field rejection.

### WR-01: Parquet rejected-path detail sanitizer can leak secret-bearing or remote-looking error text

**Files modified:** `ingress/loom-parquet-ingress/src/source_contract.rs`
**Commit:** `d8d895c`
**Applied fix:** Replaced direct source-detail attachment with a Lance-style helper that trims to the first line, redacts credential/secret/token/access_key/URI-looking details, and omits `source_detail` when sanitized detail is empty.

### WR-02: Phase gate misses renamed source SDK dependencies on public surfaces

**Files modified:** `scripts/lance-parquet-ingress-test.sh`
**Commit:** `bb724a1`
**Applied fix:** Expanded the direct dependency guard to catch renamed `package = "lance"` and `package = "parquet"` dependency declarations. Extended source-SDK-free dependency tree and manifest checks to `loom-cli` when that package is present.

## Verification

- `cargo test -p loom-parquet-ingress --test source_ingress_contract` passed
- `cargo test -p loom-parquet-ingress --test source_ingress_handoff` passed
- `bash scripts/lance-parquet-ingress-test.sh` passed

---

_Fixed: 2026-06-08T21:40:05Z_
_Fixer: the agent (gsd-code-fixer)_
_Iteration: 1_
