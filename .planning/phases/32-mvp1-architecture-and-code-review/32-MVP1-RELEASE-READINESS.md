# Phase 32 MVP1 Release Readiness

## Decision

**Go/No-Go:** **GO for an MVP1 baseline with bounded claims.**

This is not a GO for broad production-native execution, arbitrary DuckDB
`LMA1` SQL, implemented `LMC2`, or live StarRocks runtime integration.
It is a GO for the reviewed MVP1 baseline defined below.

## Release Baseline Definition

The MVP1 baseline may be described as:

- verifier-backed Arrow semantic source compatibility for Parquet, Lance, and
  Vortex sources that materialize as Arrow and emit direct `LMA1` artifacts;
- DuckDB SQL execution through public `loom_scan(path)` for legacy Loom table
  payloads and the current Parquet/Lance/Vortex source-backed single-column
  `LMA1` e2e artifacts;
- bounded DuckDB native route/cache/fallback/fail-closed hardening over raw
  primitive helper fixtures;
- source SDK isolation from `loom-core` and `loom-ffi`;
- public `loom.h` remaining narrow while DuckDB route/native controls remain
  internal and non-frozen.

## Go/No-Go Matrix

| Capability / Claim | Decision | Evidence | Notes |
|---|---|---|---|
| MVP1 broad gate | GO | `scripts/mvp1-verify.sh` composes `mvp0-verify` then `duckdb-source-e2e-test.sh` | Must stay the public readiness entry point for this baseline. |
| Full Arrow semantic source compatibility | GO | Phase 31 report/tests and `full-arrow-semantic-compatibility-test.sh` | Claim is at source -> Arrow -> verifier-accepted `LMA1` -> decoded Arrow equality. |
| DuckDB SQL over source-backed `LMA1` | BOUNDED GO | `duckdb-source-e2e-test.sh` | Current proof is single-column Int32 `value` for Parquet/Lance/Vortex. |
| DuckDB SQL over legacy mixed-column Loom table payloads | GO | Existing DuckDB smoke and legacy table path | Keep separate from direct `LMA1` source semantics. |
| Phase 24/25 native route/cache/fail-closed hardening | BOUNDED GO | `duckdb-native-integration-test.sh`, `native-hardening-test.sh` | Positive native evidence is raw primitive helper/table only. |
| Native execution for `LMA1` Arrow semantic payloads | NO-GO | Claim ledger, boundary review, code path | Code marks `Arrow semantic payload` as not native-lowering ready and routes fallback/fail-closed. |
| DuckDB arbitrary nested/logical/multi-column `LMA1` SQL | NO-GO | FFI/DuckDB review | Direct FFI decode requires one batch and one column; DuckDB maps a small Arrow format set. |
| `LMC2` production wrapper | NO-GO / Deferred | Phase 31 and Phase 32 reviews | Direct `LMA1` is implemented; `LMC2` remains future wrapper direction. |
| Phase 30 bounded dual query-surface proof | BOUNDED GO | `dual-query-surface-test.sh`, `30-DUAL-QUERY-SURFACE-REPORT.md` | DuckDB is executable; StarRocks-compatible evidence is offline descriptor/query evidence unless optional runtime smoke is explicitly run. |
| Frozen host-neutral runtime ABI | NO-GO / Scaffold | Phase 22/23/32 reports | Public `loom.h` is narrow; `loom_runtime.h` sketch and internal DuckDB ABI are non-frozen. |

## Severity-Classified Findings

### BLOCKING

No BLOCKING finding prevents releasing the bounded MVP1 baseline as defined
above.

### HIGH

| ID | Finding | Required Handling |
|---|---|---|
| HIGH-32-01 | Native `LMA1` source semantic execution is unsupported. | Must remain a non-claim. Any future native source semantic claim needs a new implementation and positive equivalence gate. |
| HIGH-32-02 | DuckDB arbitrary nested/logical/multi-column `LMA1` SQL is unsupported. | Must remain a non-claim. Add negative tests or broaden adapter design before public wording changes. |
| HIGH-32-03 | Live StarRocks runtime integration remains a non-claim. | Cite Phase 30 only as bounded DuckDB executable plus StarRocks-compatible offline descriptor evidence unless supplemental runtime smoke is run and reported. |
| HIGH-32-04 | `LMC2` wrapper is not implemented. | Say direct `LMA1` today, future `LMC2` wrapper tomorrow. |

### MEDIUM

| ID | Finding | Required Handling |
|---|---|---|
| MED-32-01 | Direct `LMA1` FFI failures collapse to generic `DecodeFailed`. | Add structured diagnostics when broadening direct `LMA1` FFI. |
| MED-32-02 | Internal `loom_duckdb_internal.h` is hand-maintained. | Generate or signature-check before expanding internal ABI. |
| MED-32-03 | Native test facts are env-controlled for route coverage. | Keep labeled as test-assisted and internal; do not treat as public API. |
| MED-32-04 | `mvp0-verify` is historically named but now contains late MVP1 gates. | Continue recommending `mvp1-verify` for MVP1 readiness. |

### LOW / INFO

| ID | Finding | Handling |
|---|---|---|
| LOW-32-01 | Review audit gate is marker/report-based. | Correct by design; do not cite it as runtime proof. |
| LOW-32-02 | Vortex DuckDB fixture emitter has fixture-only `expect` diagnostics. | Optional cleanup. |
| INFO-32-01 | No high-severity production bug was found in the reviewed slice. | Continue with bounded release wording. |

## Required Remediation Before Broader Claims

These are not required for the bounded MVP1 baseline, but are required before
stronger claims:

1. Implement or explicitly retire `LMC2` wrapper semantics.
2. Add direct `LMA1` FFI negative tests for multi-column and multi-batch payloads.
3. Design DuckDB `LMA1` table/nested/logical support before claiming arbitrary
   source-backed SQL.
4. Implement true native execution for Arrow semantic payloads before citing
   native source execution.
5. Add a real StarRocks runtime integration phase before claiming live
   StarRocks execution beyond optional supplemental smoke.
6. Add stronger internal DuckDB FFI header drift checks before expanding the
   internal ABI.

## Recommended Next Work

| Priority | Item | Rationale |
|---|---|---|
| P0 | Keep Phase 30's offline-descriptor/runtime-smoke distinction visible. | Prevent accidental live StarRocks overclaim. |
| P1 | Add focused negative tests for unsupported DuckDB `LMA1` shapes. | Converts important non-claims into executable guards. |
| P1 | Decide whether `LMC2` is next or should remain future-only. | Current source path is direct `LMA1`; container story needs a clean decision. |
| P2 | Split native source semantic execution into its own design phase. | Avoids repeating the Phase 24/25 scaffold-vs-core confusion. |
| P2 | Generate or verify internal DuckDB FFI signatures. | Reduces internal ABI drift risk. |

## Final Readiness Rule

The bounded MVP1 baseline is ready only if both checks pass:

```bash
bash scripts/mvp1-review-audit-test.sh
RUSTC_WRAPPER= bash scripts/mvp1-verify.sh
```

If either check fails, this report's decision becomes **NO-GO until fixed**.

## Verification Result

Both readiness checks passed on 2026-06-09:

```bash
bash scripts/mvp1-review-audit-test.sh
RUSTC_WRAPPER= bash scripts/mvp1-verify.sh
```

The broad gate completed through `scripts/mvp0-verify.sh` and the MVP1 DuckDB
source e2e gate. The e2e gate generated Parquet, Lance, and Vortex
source-backed `LMA1` fixtures, loaded the DuckDB extension, and matched SQL
rows/aggregates for each source-backed artifact.

## Post-Review Phase 30 Update

Phase 30 was resumed after this review and completed as bounded dual
query-surface evidence. This updates the Phase 30 row from partial/deferred to
bounded GO for DuckDB executable plus StarRocks-compatible offline descriptor
evidence. It does not change the remaining non-claims: live StarRocks runtime
integration, broad native execution, arbitrary DuckDB `LMA1` SQL, and `LMC2`
remain outside the MVP1 baseline.
