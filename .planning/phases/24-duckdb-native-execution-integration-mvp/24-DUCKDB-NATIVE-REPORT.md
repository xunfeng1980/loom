# Phase 24 DuckDB Native Execution Report

## Status

Phase 24 is complete as a DuckDB adapter MVP over the Phase 22 runtime ABI
model and Phase 23 production backend seed. The public SQL surface remains:

```sql
SELECT * FROM loom_scan(path)
```

Native execution is an internal route selected by verifier/runtime/backend
facts. Tests observe the route through `LOOM_DUCKDB_TEST_*` controls and a
temporary route report; these controls are not documented as public API.

## Delivered Surface

- DuckDB `Bind` reads the artifact, derives schema, and creates the all-column
  internal runtime plan.
- DuckDB global init finalizes projected source columns from DuckDB column ids,
  prepares native candidates, or falls back through `loom_decode`.
- DuckDB scan remains single-worker and single-batch with direct `DataChunk`
  output.
- Native output copies fixed-width primitive value buffers into the same logical
  DuckDB result shape as interpreter output.
- Route diagnostics are visible in the Phase 24 gate for native candidate,
  interpreter fallback, strict fail-closed, cancellation, malformed artifacts,
  toolchain skip/failure alternatives, and helper-level native mismatch.

## Decision Closure

| Decision | Satisfied By |
|---|---|
| D-01 | Runtime planning happens in DuckDB `Bind`; `LoomBind` creates the internal plan handle and stores route decision, cache key, and diagnostics. |
| D-02 | Backend prepare happens in global init; `LoomInit` calls the internal prepare handle only for native-candidate plans. |
| D-03 | Single-worker execution is enforced by `LoomScanState::MaxThreads() == 1` and checked by the Phase 24 gate. |
| D-04 | Single-batch behavior is preserved by `batch_emitted`; repeated SQL scans remain stable and each scan state emits one batch. |
| D-05 | Output delivery is direct `DataChunk` population; no stream or record-batch ABI was added. |
| D-06 | Native primitive buffers adapt into the same DuckDB vector fill path, guarded by kind, DuckDB type, Arrow type string, pointer, and exact byte length checks. |
| D-07 | Runtime policy controls fallback; unsupported string/native routes use interpreter fallback by default, and strict mode fails closed with stable diagnostic code/path output. |
| D-08 | Native output mismatch remains fail-closed; helper tests assert `native-output-mismatch` produces no native buffers and no interpreter downgrade. |
| D-09 | Cancellation maps to a backend cancellation route and emits no rows; helper tests and the SQL gate exercise the cancellation diagnostic path. |
| D-10 | Projection is proven by SQL selecting `f64_col, i32_col` from `loom_scan(path)` and verifying output order; predicates remain absent from the SQL path. |
| D-11 | Runtime modeling remains full-scan/single-worker; there is no real split execution in DuckDB. |
| D-12 | DuckDB calls native only for `native-candidate` routes with prepared buffers; unsupported native claims fall back or fail closed according to policy. |
| D-13 | Public SQL remains `loom_scan(path)`; no public native/interpreter functions or SQL mode parameters were added. |
| D-14 | Test controls are internal and prefixed `LOOM_DUCKDB_TEST_`, including route reporting, strict fallback, native facts, and cancellation hooks. |

## Route Evidence

`scripts/duckdb-native-integration-test.sh` now gates:

- `native-primitives-table.loom`, an `LMC1` wrapped `LMT1` table containing
  non-null Int32, Int64, Float32, and Float64 raw primitive columns.
- Native candidate or explicit toolchain skip/failure visibility for the
  native-eligible primitive table.
- Interpreter fallback over an unsupported string payload while preserving SQL
  row/aggregate results.
- Strict fail-closed behavior with diagnostic code/path text.
- Projection output order through public `loom_scan(path)` SQL only.
- Single-worker and single-batch adapter guards.
- Malformed artifact failure followed by a successful scan, proving the process
  and Arrow release ownership survive error paths.
- Helper-level `native-output-mismatch` and `cancelled` coverage where host
  cancellation/mismatch are not naturally observable through SQL alone.

## Release Gate

The main release gate now runs Phase 24 after Phase 23 backend evidence and
before the existing DuckDB SQL smoke gate:

```bash
LOOM_ALLOW_NATIVE_TOOL_SKIP=1 bash scripts/mvp0-verify.sh
```

This sequence proves runtime ABI, production backend, DuckDB native integration,
and existing DuckDB smoke behavior together.

## Non-Goals

- `loom_runtime.h remains unfrozen`.
- `no ArrowArrayStream`.
- `no predicate pushdown`.
- `no parallel split execution`.
- `no persistent native cache hardening`.
- `no arbitrary Vortex/native expansion`.

Phase 24 also does not add public SQL route controls, nullable native fixtures,
string native execution, bitpack/FOR native expansion, persistent cache reuse,
or broad native equivalence matrices. Those remain Phase 25+ hardening or later
compatibility work.
