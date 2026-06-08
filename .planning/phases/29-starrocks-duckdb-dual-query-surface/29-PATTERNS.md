# Phase 29 Pattern Map

## Closest Existing Patterns

| Phase 29 Need | Existing Pattern | How to Reuse |
|---|---|---|
| Shared table/ref trust anchor | `crates/loom-iceberg-binding/src/binding_contract.rs` | Consume accepted binding facts and verifier evidence instead of parsing metadata twice. |
| Focused release gate | `scripts/iceberg-binding-test.sh` | Create a `scripts/dual-query-surface-test.sh` with strict bash, artifact checks, dependency guards, report markers, focused tests, and optional runtime smoke handling. |
| DuckDB query evidence | `scripts/duckdb-smoke-test.sh` | Reuse public `loom_scan(path)` SQL and CSV result capture. Avoid new public SQL route names. |
| Host adapter boundary | `.planning/phases/24-duckdb-native-execution-integration-mvp/24-DUCKDB-NATIVE-REPORT.md` | Treat host-specific code as an adapter over existing runtime/artifact facts, not a second ABI. |
| Equivalence matrix | `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md` | Compare deterministic result records and fail closed on mismatch. |
| Source/table-format adapter isolation | Phase 27/28 adapter crates and dependency boundary tests | Keep StarRocks-specific code in an adapter/test boundary; core/FFI/source-ingress stay engine-neutral. |

## Recommended File Shape

| New/Modified File | Purpose | Pattern Source |
|---|---|---|
| `crates/loom-dual-query-surface/Cargo.toml` | Optional adapter/test crate if implementation needs Rust helpers for binding-to-query fixture generation. | `crates/loom-iceberg-binding/Cargo.toml` |
| `crates/loom-dual-query-surface/src/lib.rs` | Crate docs and exports for bounded dual-query proof; no production connector claim. | `crates/loom-iceberg-binding/src/lib.rs` |
| `crates/loom-dual-query-surface/src/query_surface.rs` | Loom-owned query evidence model, canonical result rows, StarRocks/DuckDB disposition, and mismatch reports. | `binding_contract.rs`, `runtime_abi.rs` report modeling |
| `crates/loom-dual-query-surface/tests/query_surface_contract.rs` | Contract tests for same binding identity, canonical result comparison, unsupported runtime, and no accepted evidence on mismatch. | `binding_contract.rs` tests |
| `scripts/dual-query-surface-test.sh` | Focused Phase 29 gate, dependency/API boundary checks, DuckDB query check, optional StarRocks runtime smoke. | `iceberg-binding-test.sh`, `duckdb-smoke-test.sh` |
| `.planning/phases/29-starrocks-duckdb-dual-query-surface/29-DUAL-QUERY-REPORT.md` | Final evidence report with engine comparison, current-phase tradeoffs, release gate evidence, Phase 30 handoff. | `28-ICEBERG-BINDING-REPORT.md` |

## Dependency Boundary Pattern

- Do not add StarRocks dependencies to `loom-core`, `loom-ffi`, `loom-source-ingress`, `loom-cli`, or public headers.
- If a helper crate is created, keep it adapter-local and prefer standard library/process fixtures over a StarRocks SDK.
- If Docker/StarRocks runtime is used, keep it in scripts/tests only. Do not make Docker or StarRocks an application dependency.
- Public SQL remains DuckDB `loom_scan(path)`; no `loom_scan_starrocks`, `loom_scan_iceberg`, or public StarRocks C ABI symbol.

## Gate Pattern

The focused gate should prove:

1. Phase 28 accepted binding evidence still passes.
2. Canonical query rows are derived from verifier-accepted Loom bytes.
3. DuckDB query result over `loom_scan(path)` matches canonical expected records.
4. StarRocks runtime smoke either passes with real query output or is explicitly skipped with a diagnostic that is not counted as accepted StarRocks evidence.
5. Report language does not claim skipped StarRocks runtime as a pass.
6. Main release gate order is Phase 28 Iceberg binding, Phase 29 dual query surface, then DuckDB smoke.

## Current-Phase Tradeoffs

- A Stream Load/table proof is lower integration than a StarRocks native scanner, but it is the fastest way to prove same-artifact query equivalence without freezing a production connector.
- Optional StarRocks runtime smoke protects local portability. It must be explicit and visible so the phase can distinguish contract proof from live StarRocks proof.
- Canonical CSV/result fixtures are acceptable only when derived from accepted binding/artifact bytes. They are not a new source of truth.
