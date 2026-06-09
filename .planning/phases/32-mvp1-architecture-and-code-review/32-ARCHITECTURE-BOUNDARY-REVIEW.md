# Phase 32 Architecture Boundary Review

## Scope

This review checks whether MVP1 architecture boundaries match the claim and
execution evidence from 32-01 and 32-02. The focus is architecture, ABI/FFI
ownership, dependency isolation, and `LMA1`/`LMC2`/native boundary accuracy.

## Boundary Matrix

| Boundary | Owner | Allowed Dependencies | Forbidden Dependencies | Public / Internal Surface | Evidence | Finding |
|---|---|---|---|---|---|---|
| `loom-core` | Loom artifact, verifier, semantic model, interpreter facts | Arrow crates, FSST, RON/serde | Vortex/FastLanes runtime crates, Parquet, Lance, Iceberg, DuckDB, FFI, native backend | Rust crate API only; no C ABI | `cargo tree -p loom-core` forbidden dependency count was `0`; `crates/loom-core/Cargo.toml` has no source SDK deps | Pass. Core remains source-SDK-free and owns `LMA1` semantic verification. |
| `loom-ffi` public ABI | C ABI and Arrow C Data export | `loom-core`, Arrow FFI | Source SDKs, DuckDB C++ types in `loom.h`, route/cache public API | Public `crates/loom-ffi/include/loom.h` exposes only `loom_decode` and Arrow C Data incomplete-type pointers | `loom.h`; `cbindgen.toml` export exclusions; existing header leakage tests | Pass with tradeoff. Public ABI is narrow; internal DuckDB route symbols are excluded. |
| `loom-ffi` internal DuckDB bridge | Rust-owned runtime policy and internal DuckDB adapter handles | `loom-core`, `loom-native-melior`, Arrow | Source SDKs in `loom-ffi`; public exposure of internal route/cache/native controls | Internal `loom_duckdb_internal.h`; not a frozen public ABI | `loom_duckdb_internal.h`; `duckdb_runtime.rs`; `cargo tree -p loom-ffi` source-SDK forbidden count was `0` | Pass with tradeoff. `loom-ffi` directly links `loom-native-melior` for the internal bridge; keep this non-public and review if a second host appears. |
| Source adapters | Format-specific source readers and Arrow materialization | Their source SDK, Arrow, `loom-core`, `loom-source-ingress` | Source SDKs leaking into core/ffi | Rust adapter crates and fixture binaries | Parquet/Lance/Vortex adapter manifests; Phase 31 report/tests | Pass. SDKs are isolated to adapter crates. |
| DuckDB C++ extension | Natural DuckDB table-function adapter | DuckDB C++ API, public `loom.h`, internal `loom_duckdb_internal.h` | Duplicating Rust runtime policy; public route-specific SQL | Public SQL `loom_scan(path)` only; internal test env flags for diagnostics | `duckdb-ext/loom_extension.cpp`; Phase 24/25 gates | Pass with bounded behavior. C++ maps bind/init/scan/projection to Rust-owned runtime handles and keeps public SQL stable. |
| Native backend | Optional/bounded MLIR/melior/ExecutionEngine path | `loom-core`, optional `melior` | Source SDKs, public ABI freeze through native internals | Rust crate; internal reports consumed by `loom-ffi` | `crates/loom-native-melior/Cargo.toml`; native gates | Bounded. Native evidence is limited to accepted primitive/raw shapes and must remain separated from `LMA1` semantic claims. |
| Scripts and release gates | Evidence orchestration | Shell gates invoking focused Rust/DuckDB tests | Treating marker gates as runtime semantic proof | `mvp1-verify`, `mvp0-verify`, focused Phase 32 marker gate | `32-EXECUTION-EVIDENCE-REVIEW.md`; `scripts/mvp1-review-audit-test.sh` | Pass. Review gate is intentionally marker/report-only. |
| Public/planning docs | User-facing claims and planning truth | Evidence reports, claim ledger, roadmap/state | Overclaiming `LMC2`, arbitrary `LMA1` DuckDB SQL, broad native execution, StarRocks completion | README, README-zh, ROADMAP, STATE | `32-CLAIM-LEDGER.md`; docs updates in 32-01 | Pass with ongoing risk. Future docs must cite bounded evidence explicitly. |

## ABI / FFI Findings

### Public `loom.h`

Actual status: **pass**.

- Exposes only `loom_decode(input_ptr, input_len, out_array, out_schema)`.
- Uses forward-declared `FFI_ArrowArray` / `FFI_ArrowSchema` pointer targets.
- Does not expose `loom_duckdb_*`, `LoomDuckDb*`, cache, native preparation, or
  route-specific scan symbols.
- `cbindgen.toml` explicitly excludes internal DuckDB route symbols from the
  generated public header.

Residual risk: public `loom_decode` can now decode direct `LMA1`, but only the
single-batch/single-column shape is usable through this C ABI today.

### Internal DuckDB Header

Actual status: **pass with tradeoff**.

- `loom_duckdb_internal.h` contains internal plan/prepare/diagnostic/native
  buffer handles.
- The header says it is non-public and not the frozen `loom_runtime.h` ABI.
- C++ RAII holders destroy plan/prepare handles and Arrow arrays/schemas.

Tradeoff: the internal header is hand-maintained rather than generated from the
Rust declarations. Existing tests and Phase 32 marker checks reduce drift risk,
but future ABI changes should add explicit layout/signature checks.

### Runtime ABI Sketch

Actual status: **scaffold / non-frozen**.

`loom_runtime.h` remains a host-neutral sketch and is not used as the public
DuckDB ABI. It should not be cited as a stable multi-host runtime ABI until a
second consumer validates it.

## Dependency Findings

### Core / FFI Dependency Guard

Evidence:

```bash
cargo tree -p loom-core | awk '/vortex|fastlanes|parquet|lance|iceberg/{c++} END{print c+0}'
cargo tree -p loom-ffi  | awk '/vortex|fastlanes|parquet|lance|iceberg/{c++} END{print c+0}'
```

Observed result: both counts were `0`.

Finding: **pass** for source SDK isolation. `loom-ffi` does depend on
`loom-native-melior`; that is a native-backend coupling, not a source SDK leak.
Keep it internal and non-public.

### Source Adapter Isolation

Finding: **pass**.

- `loom-parquet-ingress` carries `parquet`.
- `loom-lance-ingress` carries `lance`.
- `loom-vortex-ingress` carries Vortex crates.
- All use `loom-source-ingress` and `loom-core` as the source-neutral handoff
  and artifact emission boundary.

## `LMA1` / `LMC2` Boundary

Actual status: **direct `LMA1` implemented; `LMC2` future wrapper deferred**.

- `LMA1_MAGIC` is the implemented Arrow semantic payload magic.
- `encode_arrow_semantic_payload` writes direct `LMA1` bytes after semantic
  verification.
- `decode_arrow_semantic_payload` decodes and verifies direct `LMA1`.
- `LMC2_MAGIC` and `is_arrow_semantic_container` exist as documented direction,
  but there is no implemented `LMC2` wrapper codec in the current evidence path.

Required wording: say **direct `LMA1` payload** for current artifacts and
**future `LMC2` wrapper** for the container direction.

## Native / `LMA1` Boundary

Actual status: **pass for fail-closed separation**.

- `verify_arrow_semantic_artifact` marks `LMA1` as payload kind
  `Arrow semantic payload` and adds lowering diagnostic
  `arrow-semantic-lowering-deferred`.
- `check_production_lowering_support` accepts only payload kinds
  `LMP1 layout` and `LMT1 table` for production native lowering.
- `artifact_raw_value_buffers` expects `LMP1`/`LMT1` raw primitive layouts and
  rejects unsupported payloads/shapes/kernels.
- DuckDB native output accepts only Int32, Int64, Float32, and Float64 native
  buffers with exact byte lengths and matching DuckDB vector types.

Finding: `LMA1` source semantic success cannot accidentally become native
execution through the current production lowering gate. It routes as semantic
Arrow/interpreter fallback or fail-closed depending on policy.

## DuckDB Adapter Boundary

Actual status: **pass with bounded support**.

- Public SQL remains `loom_scan(path)`.
- Projection pushdown uses DuckDB `column_ids` and Rust internal projected
  runtime plans; C++ does not compute native eligibility itself.
- Direct DataChunk fill remains the adapter implementation.
- `LMA1` bind support discovers a single `"value"` column by decoding via
  `loom_decode`; supported Arrow formats are bool, Int32, Int64, Utf8, Float32,
  and Float64.
- Single-batch scan is explicit through `batch_emitted` and `MaxThreads() = 1`.

Residual risk: `LMA1` direct payload support is intentionally narrow at the
DuckDB surface. Arbitrary nested/logical/multi-column source `LMA1` remains a
non-claim.

## Findings Summary

| ID | Severity | Finding | Action |
|---|---|---|---|
| AB-32-03-01 | Low | Source SDKs remain isolated from `loom-core` and `loom-ffi`. | Keep dependency guards in the review audit gate. |
| AB-32-03-02 | Low | Public `loom.h` excludes internal DuckDB route/cache/native controls. | Keep public/internal header checks. |
| AB-32-03-03 | Medium | `loom-ffi` directly links `loom-native-melior` for internal DuckDB native bridge. | Treat as internal implementation tradeoff; revisit when adding a second host or freezing runtime ABI. |
| AB-32-03-04 | Medium | Internal `loom_duckdb_internal.h` is hand-maintained. | Add stronger signature drift checks before any API expansion. |
| AB-32-03-05 | High | `LMA1` is direct payload today; `LMC2` wrapper is not implemented. | Keep docs and reports using direct `LMA1` / future `LMC2` wording. |
| AB-32-03-06 | High | Native lowering rejects `Arrow semantic payload`; it is not a native source semantic path. | Preserve fallback/fail-closed labeling for `LMA1` native claims. |

## Verification

```bash
rg -q "loom-core|loom-ffi|DuckDB|ABI|dependency|LMA1|LMC2" \
  .planning/phases/32-mvp1-architecture-and-code-review/32-ARCHITECTURE-BOUNDARY-REVIEW.md
bash scripts/mvp1-review-audit-test.sh
git diff --check
```

