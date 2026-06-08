---
phase: 28-iceberg-ref-table-binding
verified: 2026-06-09T00:00:00Z
status: passed
score: 9/9 must-haves verified
overrides_applied: 0
---

# Phase 28: Iceberg Ref/Table Binding Verification Report

**Phase Goal:** Local Iceberg table/ref metadata can be bound to verifier-backed Loom artifacts through sidecar/reference evidence, preserving schema/snapshot identity, source/oracle evidence, and fail-closed verifier facts without adding query surfaces or a second source-ingress framework.

**Status:** passed

## Goal Achievement

| # | Must-have | Status | Evidence |
|---|---|---|---|
| 1 | Adapter-local Iceberg binding crate exists and no official `iceberg` SDK is added by default | VERIFIED | `crates/loom-iceberg-binding` is a workspace member; dependency guard passes; no direct official `iceberg` crate is present. |
| 2 | Core, FFI, source-ingress, CLI, DuckDB, and public headers remain free of Iceberg query/catalog/credential and StarRocks route creep | VERIFIED | `scripts/iceberg-binding-test.sh` public-surface and manifest guards passed. |
| 3 | Local Iceberg metadata plus sidecar JSON produce bounded table/ref facts | VERIFIED | `cargo test -p loom-iceberg-binding --test binding_contract` passed inside the focused gate. |
| 4 | Valid unsupported metadata stays byte-free and malformed/missing identity is rejected with diagnostics only | VERIFIED | `binding_contract` tests passed and assert unsupported/rejected report shapes. |
| 5 | Accepted binding requires local artifact bytes, recomputed SHA-256, live verifier acceptance, sidecar-relative source/oracle evidence, source file SHA-256, and decoded values SHA-256 | VERIFIED | `binding_handoff` passed; production source contains `verify_artifact`, `sha256_bytes`, `resolve_local_sidecar_path`, `resolve_local_evidence_path`, source byte hashing, and `decoded_values_sha256`. |
| 6 | Schema/snapshot/table/hash/verifier/source/oracle/stale/forged/manifest-only mismatch cases fail closed with no accepted bytes | VERIFIED | `mismatch_fail_closed` passed, including exact diagnostics for stale source hash and forged decoded-values evidence. |
| 7 | Final report records binding schema, accepted/unsupported/rejected semantics, mismatch matrix, evidence, non-goals, tradeoffs, and Phase 29 handoff | VERIFIED | `28-ICEBERG-BINDING-REPORT.md` contains required report sections and release-gate evidence. |
| 8 | Main verifier invokes Phase 28 after Phase 27 and before DuckDB smoke | VERIFIED | `scripts/mvp0-verify.sh` order is guarded by `dependency_boundary` and the focused gate. |
| 9 | Focused Phase 28 gate passes from the repository root | VERIFIED | `bash scripts/iceberg-binding-test.sh` passed on 2026-06-09 after final gate/test fixture alignment. |

## Verification Commands

| Command | Result |
|---|---|
| `cargo test -p loom-iceberg-binding --test mismatch_fail_closed stale_source_and_forged_oracle_evidence_flags_return_no_bytes -- --nocapture` | passed |
| `bash scripts/iceberg-binding-test.sh` | passed |

## Review-Fix Closure

The earlier verifier run found one blocker: `scripts/iceberg-binding-test.sh` had stale production-source checks and one stale evidence fixture did not exercise the intended source-hash mismatch branch. The final follow-up fixed:

- gate checks for decoded value digest/source hashing/path confinement;
- stale-source fixture shape under the current nested `source.path` / `source.sha256` schema;
- mismatch regression assertions for exact stale-source and forged-values diagnostics.

## Residual Risks

- The accepted decoded-values proof remains intentionally narrow: current evidence covers the Phase 28 non-null Int32 table slice, not general Arrow value canonicalization.
- Source proof remains a local fixture hash rather than a production Iceberg catalog/source scan.
- Phase 29 dual-query work is explicitly deferred by user instruction; Phase 28 itself remains complete because query surfaces were out of scope.

---

_Verified: 2026-06-09_
_Verifier: Codex (follow-up after gsd-verifier gap report)_
