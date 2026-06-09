# Phase 32 Research: MVP1 Architecture and Code Review

## Research Question

What must be reviewed to determine whether the current MVP1 implementation is a
truthful, maintainable, release-ready baseline?

## Key Findings

### 1. The Review Must Separate Proven Value from Supporting Scaffolding

The project now has a broad `mvp1-verify` gate. That gate is valuable, but it is
not a single semantic claim. It combines:

- historical MVP0/MVP1 phase gates through `scripts/mvp0-verify.sh`
- direct DuckDB SQL smoke over legacy `LMC1(LMP1/LMT1)` fixtures
- Parquet/Lance/Vortex source-backed single-column `LMA1` artifacts queried
  through DuckDB SQL
- native route/cache/fallback/toolchain gates that are bounded to primitive
  shapes and may use fallback or skip semantics
- Phase 31 Arrow semantic source equality tests outside broad DuckDB SQL support

The review should therefore produce a claim ledger and execution evidence matrix
rather than a pass/fail-only report.

### 2. `LMA1` Is the Current Source Semantic Contract; `LMC2` Is Still Future

Phase 31 intentionally moved full source compatibility to Arrow semantics:
source reader -> Arrow batches -> `LMA1` -> verifier -> decode equality.
Reports state `LMC2` remains the container direction but is not required for the
Phase 31 evidence.

The review should check docs and code for wording that implies `LMC2` is
implemented or that legacy `LMC1(LMP1/LMT1)` is the arbitrary-schema source
compatibility substrate.

### 3. DuckDB Source E2E Is Real but Narrow

`scripts/duckdb-source-e2e-test.sh` generates Parquet, Lance, and Vortex
fixtures, asserts `LMA1` magic, builds the extension, and executes DuckDB SQL:

- `SELECT value FROM loom_scan(path)`
- `SELECT COUNT(*), SUM(value), MIN(value), MAX(value) FROM loom_scan(path)`

This is real e2e evidence for source -> `LMA1` -> DuckDB SQL over a single
non-null Int32 column. It does not prove DuckDB can query every Arrow nested or
logical type from arbitrary `LMA1` artifacts.

### 4. Native Evidence Needs Explicit Labels

Phase 22-25 created runtime policy, backend identity, DuckDB route wiring,
bounded cache/equivalence, and fail-closed/fallback behavior. The positive native
evidence remains limited to verifier-gated non-null primitive raw/table shapes.
Unsupported strings, nullability, compressed layouts, arbitrary source semantic
artifacts, predicates, splits, and malformed/cancelled/mismatch routes are
fallback or fail-closed evidence.

The review should trace any public or planning claim that says "native execution"
or "MLIR ExecutionEngine" and classify it as:

- real supported primitive native/JIT evidence
- route/ABI/cache scaffolding
- toolchain-conditional validation
- fallback/interpreter evidence
- explicit deferred/non-goal

### 5. Phase 30 Must Stay Partial Unless Evidence Changes

Phase 30 has real DuckDB execution over Phase 29 accepted bytes, but StarRocks
runtime smoke, negative matrix expansion, main release-gate wiring, and final
dual-surface closeout remain pending/deferred. Phase 32 should not hide this by
calling MVP1 "dual query complete."

### 6. Review Artifacts Should Be Executable Enough to Re-run

The best fit for Phase 32 is not a large refactor. It should create durable
review artifacts and focused audit checks:

- claim ledger
- execution evidence matrix
- architecture boundary and ABI/FFI review
- code-quality review with narrow fix allowance
- MVP1 go/no-go matrix
- optional audit script that checks report markers and common claim/boundary
  regressions

## Recommended Plan Split

1. Claim ledger and documentation truth audit.
2. Execution evidence and release-gate audit.
3. Architecture, ABI/FFI, and dependency-boundary audit.
4. Code-quality review and narrow remediation.
5. MVP1 release readiness report, focused audit gate, roadmap/state closeout.

## Validation Strategy

The phase should verify:

- each review report exists and has required sections
- claim/evidence reports name exact commands and source files
- docs do not overclaim `LMA1`, `LMC2`, native execution, DuckDB arbitrary-schema
  SQL, or Phase 30 StarRocks completion
- dependency and public ABI checks still pass
- any narrow fixes run focused tests
- final `mvp1-verify` remains green or the report explicitly records why it was
  not rerun

## RESEARCH COMPLETE

