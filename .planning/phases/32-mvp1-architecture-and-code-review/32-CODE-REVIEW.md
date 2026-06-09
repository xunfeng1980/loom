# Phase 32 Code Review

## Review Scope

Reviewed the core Arrow semantic artifact path, artifact verifier/native
lowering path, FFI decode path, DuckDB adapter/native route path, source fixture
emitters, and MVP1/review scripts. Findings are ordered by severity and include
disposition.

## Findings

### High Severity

No high-severity production bugs were found in the reviewed slice.

The code now consistently blocks the largest known overclaim: `LMA1` Arrow
semantic artifacts are verifier-accepted but are not native-lowering ready.
`check_production_lowering_support` accepts only `LMP1 layout` and `LMT1 table`
payload kinds, while `verify_arrow_semantic_artifact` records
`arrow-semantic-lowering-deferred`.

### Medium Severity

| ID | Severity | File | Finding | Disposition |
|---|---|---|---|---|
| CR-32-04-01 | Medium | `crates/loom-ffi/src/ffi.rs:139` | Direct `LMA1` FFI decode is intentionally hard-limited to exactly one batch and one column, but failures collapse to generic `DecodeFailed`. DuckDB bind uses this path to infer one `"value"` column. This is correct for the current e2e slice but opaque for arbitrary `LMA1` users. | Deferred. Fixing this well requires a broader `LMA1` table/record-batch FFI surface or structured diagnostics, not a narrow Phase 32 patch. |
| CR-32-04-02 | Medium | `duckdb-ext/loom_extension.cpp:726` | DuckDB `LMA1` bind support maps every accepted direct semantic artifact to one column named `"value"` and only supports Arrow formats bool/i32/i64/utf8/f32/f64. It will reject or fail opaque for nested/logical/multi-column `LMA1`. | Deferred. This is the intended bounded query surface; document and test as non-claim until a real multi-column/nested DuckDB `LMA1` design exists. |
| CR-32-04-03 | Medium | `crates/loom-ffi/include/loom_duckdb_internal.h:1` | Internal DuckDB FFI header is hand-maintained while Rust exports the corresponding `extern "C"` symbols. `cbindgen.toml` excludes those symbols from public `loom.h`, so drift protection depends on tests and marker checks rather than generated bindings. | Deferred. Before expanding internal ABI, add stronger signature/layout drift checks or generate the internal header. |
| CR-32-04-04 | Medium | `crates/loom-ffi/src/duckdb_runtime.rs:772`, `duckdb-ext/loom_extension.cpp:287` | Native test facts are injected through `LOOM_DUCKDB_TEST_USE_NATIVE_FACTS`. This is useful for route coverage, but it is an environment-controlled path inside the extension process and can make route reports look stronger if cited without test context. | Deferred. Keep the env var test-prefixed and undocumented as public API; readiness docs must keep native evidence labeled as bounded/test-assisted. |

### Low Severity

| ID | Severity | File | Finding | Disposition |
|---|---|---|---|---|
| CR-32-04-05 | Low | `scripts/mvp1-review-audit-test.sh:112` | Dependency guards use `cargo tree` plus broad regex markers. This is good enough for the Phase 32 marker gate but could false-positive if an unrelated package name contains a marker substring. | Accepted for marker gate. The main release gate already has more focused dependency checks for key historical constraints. |
| CR-32-04-06 | Low | `crates/loom-vortex-ingress/src/bin/emit_duckdb_vortex_lma1_fixture.rs:55` | The Vortex DuckDB fixture helper uses `expect` inside `vortex_file_bytes`. It is a fixture binary, not library API, so failure still exits the gate, but diagnostics are less structured than the Parquet/Lance fixture emitters. | Deferred as fixture-only maintainability cleanup. |
| CR-32-04-07 | Low | `scripts/duckdb-source-e2e-test.sh:129` | DuckDB source e2e checks physical row order without `ORDER BY`. That is useful for detecting fixture order changes, but it means the gate is asserting this fixture's scan order rather than general SQL unordered semantics. | Accepted. The current fixture path is deterministic and order-sensitive by design. |

## Test Gaps

| Gap | Severity | Current Evidence | Recommended Follow-up |
|---|---|---|---|
| Direct `LMA1` FFI rejection for multi-column and multi-batch payloads is not separately named in tests. | Medium | Positive single-column `LMA1` FFI roundtrip exists in `crates/loom-ffi/tests/roundtrip.rs`; runtime fallback for `LMA1` exists in `duckdb_runtime.rs` tests. | Add focused negative tests that assert multi-column/multi-batch direct `loom_decode` returns `DecodeFailed` until a broader FFI surface exists. |
| DuckDB `LMA1` source e2e covers only one non-null Int32 column named `value`. | Medium | `scripts/duckdb-source-e2e-test.sh` asserts `7,-1,42` and aggregate `3,48,-1,42` for Parquet/Lance/Vortex. | Add explicit unsupported nested/multi-column DuckDB `LMA1` tests if docs continue to emphasize full Arrow semantic source compatibility. |
| Internal DuckDB header drift coverage is marker-based. | Medium | Rust tests verify public header leakage and runtime FFI behavior; Phase 32 audit script checks internal markers. | Generate internal header or add compile-time signature checks before changing `loom_duckdb_internal.h`. |
| Phase 30 StarRocks/full dual-surface negative matrix remains outside MVP1 gates. | High, but deferred by roadmap | ROADMAP/STATE and claim ledger mark it partial/deferred. | Keep out of readiness claims until Phase 30 resumes or a replacement phase explicitly closes it. |
| Native `LMA1` semantics have no positive test because code intentionally rejects the path. | High, but non-goal | `arrow_semantic_lma1_uses_interpreter_fallback` verifies fallback and empty native buffers. | Treat true native source semantic execution as future feature work, not Phase 32 remediation. |

## Ownership And Error Handling Review

- Arrow C Data ownership is correctly centered on release callbacks. DuckDB
  state releases arrays/schemas on teardown, including error paths.
- Internal DuckDB plan/prepared handles use RAII wrappers in C++.
- Native buffers are copied into owned `LoomDuckDbPrepared` storage before C++
  receives pointers, and C++ keeps the prepared holder alive for native scans.
- `loom_decode` checks non-null output pointers and catches panics.
- Error fidelity remains coarse at the public `loom_decode` boundary: all
  semantic shape errors map to `DecodeFailed`.

## Narrow Remediation Applied

No production code changes were applied in this plan.

The narrow Phase 32 audit gate was already extended in Plan 32-03 to check:

- source dependency guards for `loom-core` and `loom-ffi`,
- public/internal header separation,
- cbindgen internal symbol exclusions,
- direct `LMA1` and future `LMC2` wording markers.

No additional low-blast-radius defect was found that justified changing
artifact semantics, public ABI, or DuckDB execution behavior in Plan 32-04.

## Residual Risk

The primary residual risk is not a hidden memory/ownership defect in the
reviewed slice; it is claim drift. The code has many connected scaffolds and
bounded gates whose names sound broader than their assertions. Future readiness
documents should continue to cite `32-CLAIM-LEDGER.md`,
`32-EXECUTION-EVIDENCE-REVIEW.md`, and `32-ARCHITECTURE-BOUNDARY-REVIEW.md`
instead of phase titles alone.

## Verification

```bash
rg -q "Findings|Severity|File|Residual Risk|Test Gaps" \
  .planning/phases/32-mvp1-architecture-and-code-review/32-CODE-REVIEW.md
cargo fmt --check
bash scripts/mvp1-review-audit-test.sh
git diff --check
```

