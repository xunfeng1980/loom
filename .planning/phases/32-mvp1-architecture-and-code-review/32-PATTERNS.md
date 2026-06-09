# Phase 32 Patterns: MVP1 Architecture and Code Review

## Existing Patterns to Reuse

### Report-First Closeout

Recent phases close with explicit reports that separate evidence, non-goals,
tradeoffs, and residual risks:

- `31-FULL-COMPATIBILITY-REPORT.md`
- `30-DUCKDB-EXECUTION-REPORT.md`
- `29-ICEBERG-BINDING-REPORT.md`
- `25-NATIVE-HARDENING-REPORT.md`
- `22-RUNTIME-ABI-REPORT.md`

Phase 32 should follow this style but use review-specific reports.

### Focused Gate Before Broad Gate

Focused scripts prove a phase-specific claim before broad release wiring:

- `scripts/full-arrow-semantic-compatibility-test.sh`
- `scripts/native-hardening-test.sh`
- `scripts/duckdb-source-e2e-test.sh`
- `scripts/iceberg-binding-test.sh`

Phase 32 should add a focused audit gate only if it checks review artifacts and
stable no-overclaim/boundary markers without pretending to prove runtime
semantics.

### Dependency Boundary Guards

Existing gates use `cargo tree`, `rg`, and allowlists to ensure SDKs and host
vocabulary stay isolated. Phase 32 should reuse this pattern for review findings
instead of inventing a new policy framework.

### Public/Internal Surface Separation

Current public surfaces:

- DuckDB public SQL: `loom_scan(path)`
- public FFI header: `crates/loom-ffi/include/loom.h`
- internal DuckDB route header: `crates/loom-ffi/include/loom_duckdb_internal.h`

Phase 32 should review whether docs and code keep these boundaries clear.

## Review Artifacts to Create

- `32-CLAIM-LEDGER.md`
- `32-EXECUTION-EVIDENCE-REVIEW.md`
- `32-ARCHITECTURE-BOUNDARY-REVIEW.md`
- `32-CODE-REVIEW.md`
- `32-MVP1-RELEASE-READINESS.md`
- `scripts/mvp1-review-audit-test.sh` if the final plan chooses a focused audit
  script

## Review Finding Severity

Use:

- `BLOCKING` — release claim or gate is materially false or unsafe
- `HIGH` — likely to mislead implementation or users
- `MEDIUM` — maintainability, test, or documentation risk
- `LOW` — cleanup or clarity issue
- `INFO` — context, non-blocking observation, or future work

## Narrow Fix Policy

Allowed:

- correct inaccurate docs
- add missing review marker checks
- fix small script/report wording issues
- fix low-risk code defects found during review if tests are obvious

Deferred:

- StarRocks runtime completion
- broad DuckDB arbitrary-schema SQL support
- native `LMA1` semantic lowering
- `LMC2` implementation
- large refactors

