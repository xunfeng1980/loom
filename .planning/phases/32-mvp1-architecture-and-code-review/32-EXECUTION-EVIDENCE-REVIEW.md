# Phase 32 Execution Evidence Matrix

## Scope

This review classifies the MVP1 and late MVP0 gates by what they prove, what
they do not prove, and where native/fallback/skip/scaffold semantics enter the
story. A passing gate proves only its concrete assertions.

## Gate Matrix

| Gate | Command | Inputs | Proves | Negative Assertions | Source Status | Native / Fallback / Skip Status | Does Not Prove |
|---|---|---|---|---|---|---|---|
| MVP1 release gate | `bash scripts/mvp1-verify.sh` | The full inherited MVP0 gate plus generated Parquet/Lance/Vortex source-backed `LMA1` fixtures | `mvp0-verify` runs before `duckdb-source-e2e`; the current DuckDB source e2e slice runs after inherited gates | None beyond child gate failures and ordering by script structure | Mixed: inherited synthetic/fixture gates plus real source-backed fixture generation | Delegates native/fallback semantics to child gates | It does not add broad native coverage, StarRocks, arbitrary DuckDB `LMA1` schemas, or `LMC2`. |
| Workspace and dependency baseline | `cargo test --workspace`; dependency greps in `mvp0-verify` | Workspace tests and Cargo metadata | Tests compile/run; `loom-core` and `loom-ffi` remain Vortex/FastLanes-free; direct `vortex-file` dependency remains isolated | Fails on dependency boundary violations | Mostly unit/integration fixtures | Not a native gate | It does not prove source reader semantics or DuckDB runtime behavior. |
| Phase 24 DuckDB native integration | `bash scripts/duckdb-native-integration-test.sh` | Generated `LMC1` native primitive, FSST, bitpack, malformed fixtures; real DuckDB extension/CLI | Native primitive SQL aggregate can route as `native-candidate` with `native-execution-engine-output`; projection order works; fallback/fail-closed/cancel diagnostics are visible; public SQL remains `loom_scan(path)` | Fails if primitive native route falls back or skips when forced; fails if strict fallback-disabled path succeeds | Synthetic Loom fixtures | Native proven only for bounded primitive helper table; strings/compressed shapes use fallback/fail-closed; helper tests may allow explicit native toolchain skip | It does not prove `LMA1` source artifacts execute natively or that all Arrow/Vortex encodings have native semantics. |
| Phase 25 native hardening | `bash scripts/native-hardening-test.sh` | Generated `LMC1` native primitive, FSST, bitpack, nullable, malformed fixtures; route report | Repeated native primitive aggregate equality; cache miss/insert/hit order; projection cache drift; fallback visibility; strict fail-closed; cancellation; helper mismatch/cache safety; no public API creep markers | Fails if forced primitive native route uses fallback/skip; fails on missing cache evidence unless explicit skip/failure condition is allowed for selected paths | Synthetic Loom fixtures | Strongest native evidence is still bounded primitive route. FSST, nullable, and bitpack assertions are interpreter fallback or fail-closed. Some helper paths run with `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`. | It does not prove native decoding of real Parquet/Lance/Vortex `LMA1`, nested Arrow types, dictionary/run-end/bitpack/FOR native semantics, or production performance. |
| Phase 31 full Arrow semantic source compatibility | `bash scripts/full-arrow-semantic-compatibility-test.sh` | Core `LMA1` tests; Parquet/Lance full schema tests; Vortex dtype semantic tests | Source readers that materialize Arrow can be encoded as verifier-accepted `LMA1` and decoded back to equal Arrow batches in focused tests | Fails on missing semantic markers or failed source equality tests | Real source-reader materialization through adapter crates | Not a native gate; semantic/interpreter artifact path | It does not prove DuckDB can query every Arrow nested/logical type, StarRocks integration, direct physical Parquet/Lance/Vortex decoding, or `LMC2`. |
| MVP1 DuckDB source e2e | `bash scripts/duckdb-source-e2e-test.sh` | Generated Parquet, Lance, and Vortex source-backed `LMA1` fixtures; real DuckDB extension/CLI | Each source family emits an `LMA1` artifact; DuckDB `loom_scan(path)` returns rows `7,-1,42`; aggregate is `3,48,-1,42` | Fails on missing fixtures, non-`LMA1` magic, build/load errors, row mismatch, aggregate mismatch | Real source-backed fixture generation; bounded value column | DuckDB execution is through current single-column `LMA1` adapter path and may use interpreter fallback; native `LMA1` lowering remains unsupported | It does not prove arbitrary `LMA1` schema SQL, multi-column source-backed `LMA1`, nested/logical DuckDB query support, or native execution. |
| Phase 29 Iceberg binding | `bash scripts/iceberg-binding-test.sh` | Local Iceberg metadata and sidecar-bound Loom artifacts | Accepted bindings require local artifact bytes, SHA-256, live `verify_artifact`, and evidence JSON | Fails on stale/mismatched/manifest-only evidence | Local fixture metadata and Loom artifacts | Not a native gate | It does not prove DuckDB/StarRocks SQL by itself. Phase 30 consumes a bounded accepted binding fixture for DuckDB. |
| DuckDB smoke | `bash scripts/duckdb-smoke-test.sh` | Generated MVP0/MVP1 fixture payloads and real DuckDB extension/CLI | Public `loom_scan(path)` can query current supported legacy `.loom` fixtures | Fails on build/load/query mismatch | Synthetic/generated Loom fixtures | Mostly interpreter/direct DataChunk path depending on fixture | It does not prove source-backed arbitrary `LMA1` schemas or StarRocks. |

## Dedicated DuckDB Source E2E Review

`scripts/duckdb-source-e2e-test.sh` is the most direct MVP1 user-value gate
added after Phase 31.

Proves:

- Parquet, Lance, and Vortex adapter crates can generate source-backed `.loom`
  files for the focused fixture.
- Each produced file begins with `LMA1`.
- The DuckDB extension builds, loads, and queries each artifact through public
  `loom_scan(path)`.
- The concrete SQL values are asserted: rows `7`, `-1`, `42`; aggregate
  `COUNT=3`, `SUM=48`, `MIN=-1`, `MAX=42`.

Does Not Prove:

- DuckDB can query arbitrary Arrow nested/logical `LMA1` schemas.
- DuckDB can query multi-column source-backed `LMA1` artifacts.
- `LMA1` executes through MLIR/native lowering.
- `LMC2` exists as the production source container.
- StarRocks can consume the same source-backed artifacts.

Residual claim label: **bounded executable DuckDB evidence**.

## Dedicated Native Gate Review

Phase 24/25 native gates are real and useful, but they must be cited precisely.

Proves:

- DuckDB route diagnostics can distinguish `native-candidate`,
  `interpreter-fallback`, `fail-closed`, `cancelled`, cache events, and mismatch
  helper failures.
- Forced native primitive fixtures fail if they unexpectedly fall back or skip.
- Fallback-disabled unsupported paths fail closed with stable diagnostics.
- Cache smoke evidence records miss/insert/hit ordering for the bounded route.
- Public SQL/API remains `loom_scan(path)` with no route-specific public SQL.

Does Not Prove:

- Native execution for `LMA1` Arrow semantic payloads.
- Native execution for Parquet/Lance/Vortex source semantics.
- Native decoding for nullable, string, compressed, dictionary, run-end,
  bitpack/FOR, nested, or logical Arrow families.
- Second-host engine independence.

Residual claim label: **bounded native primitive route and hardening evidence**,
with explicit **fallback** and **skip** semantics for unsupported/toolchain paths.

## Review Risks To Carry Forward

| Risk | Severity | Carry Forward |
|---|---|---|
| Phase names can imply broader completion than gate assertions prove. | High | Reports and docs must cite gate scope, not phase title alone. |
| `mvp0-verify` now contains late MVP1 gates despite its historical name. | Medium | Public docs should prefer `mvp1-verify` for MVP1 readiness. |
| Native route reports can be mistaken for native semantic execution. | High | Native reports must name fallback/skipped/scaffold status. |
| `LMA1` source semantic success can be mistaken for DuckDB arbitrary schema support. | High | Keep source semantic and query-engine support as separate rows. |
| Phase 30 DuckDB evidence can be mistaken for dual-engine completion. | High | Keep StarRocks/full dual-surface marked deferred until a dedicated gate exists. |

## Audit Gate Role

`scripts/mvp1-review-audit-test.sh` is a marker/report audit seed. It checks
that this review material exists and that key scripts contain the ordering and
assertion markers described above. It does not execute full runtime semantics
and must not be cited as proof of more than review artifact consistency.

