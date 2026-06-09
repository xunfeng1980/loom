# Phase 32 Claim Ledger

## Purpose

This ledger separates MVP1 value that is executable today from bounded evidence,
fallback-only paths, scaffolded contracts, skipped gates, deferred scope, and
unsupported claims. It is intentionally evidence-first: future plans should cite
the row status rather than re-infer completion from phase names.

## Status Legend

| Status | Meaning |
|---|---|
| proven | Executable code or a focused gate proves the claim for the stated scope. |
| bounded | True only for a named slice, fixture family, or adapter path. |
| fallback | The path is connected, but accepted execution currently routes through interpreter/semantic fallback. |
| scaffold | Contracts, reports, or adapters exist, but the value-producing implementation is not proven. |
| skipped | Gate accepts an explicit skip/toolchain absence condition. |
| deferred | Intentionally incomplete and recorded for later work. |
| unsupported | Current code rejects or does not implement the capability. |
| incorrect | Wording would be false as written and must be corrected. |

## Claim Inventory

| ID | Claim | Source | Evidence | Actual Status | Risk | Required Action |
|---|---|---|---|---|---|---|
| C-32-01 | `scripts/mvp1-verify.sh` is the broad MVP1 gate. | README, STATE, quick task report | `scripts/mvp1-verify.sh` runs `scripts/mvp0-verify.sh` first and then `scripts/duckdb-source-e2e-test.sh`. | proven | Low | Keep `mvp1-verify` as the recommended external check. |
| C-32-02 | Parquet, Lance, and Vortex sources that materialize as Arrow can emit verifier-accepted `LMA1`. | README, ROADMAP, `31-FULL-COMPATIBILITY-REPORT.md` | Phase 31 report defines accepted source path as reader open -> Arrow RecordBatch -> `LMA1` encode -> `verify_artifact` accept -> decoded Arrow equality. | proven | Medium | Keep the claim at the source semantic layer; do not imply query-engine or native support for every Arrow type. |
| C-32-03 | DuckDB source e2e covers Parquet/Lance/Vortex `LMA1` artifacts. | README, README-zh, STATE, `scripts/duckdb-source-e2e-test.sh` | The script generates source fixtures and queries `loom_scan(path)` for rows and aggregate results. `loom_decode_inner` accepts direct `LMA1` only as one batch and one column. | bounded | Medium | Preserve "single-column `LMA1` e2e" wording everywhere this gate is summarized. |
| C-32-04 | DuckDB can query mixed-column legacy Loom table payloads. | README, MVP0 gates, DuckDB smoke gate | Existing `LMC1(LMT1)` table path and DuckDB smoke coverage exercise mixed columns through `loom_scan(path)`. | proven | Low | No correction required; keep this separate from `LMA1` source semantics. |
| C-32-05 | DuckDB can query arbitrary `LMA1` nested/logical schemas. | Potential inference from Phase 31/README source compatibility wording | README explicitly says Phase 31 is not a DuckDB SQL claim. FFI direct `LMA1` decode is currently single-batch/single-column and DuckDB maps only a limited Arrow format set. | unsupported | High | Block any wording that generalizes DuckDB `LMA1` SQL beyond the current focused e2e slice. |
| C-32-06 | `LMC2` is implemented as the production wrapper around `LMA1`. | ROADMAP, STATE, Phase 31 report | Phase 31 report says `LMC2` remains the documented container direction and is not required for current evidence. | deferred | Medium | Keep `LMC2` described as future/documented direction, not implemented storage. |
| C-32-07 | Native lowering supports `LMA1` Arrow semantic artifacts. | Potential inference from Phase 22-25 names | Phase 31 report lists native lowering as a non-goal; `duckdb_runtime` reports `Arrow semantic payload` as unsupported for native lowering and routes through fallback. | unsupported | High | Keep native claims limited to supported non-null primitive/raw helper slices. |
| C-32-08 | Phase 24/25 DuckDB native route, cache, fallback, and fail-closed plumbing is real. | STATE, Phase 25 report, native hardening gate | Internal route reports, cache diagnostics, cancellation/mismatch tests, and hardening scripts exist, but positive native evidence remains bounded to helper primitive/reference-byte cases. | bounded | High | Describe as connected runtime/adapter hardening, not proof of broad semantic native execution. |
| C-32-09 | Phase 30 dual-query surface is complete. | ROADMAP, STATE, `30-DUCKDB-EXECUTION-REPORT.md`, `30-DUAL-QUERY-SURFACE-REPORT.md` | Post-review update: Phase 30 is complete as bounded evidence. DuckDB execution is real through `loom_scan(path)`; StarRocks-compatible evidence is offline descriptor/query evidence by default, with optional env-gated runtime smoke. | bounded | High | Cite Phase 30 as bounded dual query-surface evidence only; never imply default live StarRocks runtime integration. |
| C-32-10 | Public ABI is stable/frozen for all runtime/native surfaces. | `loom.h`, `loom_runtime.h`, Phase 22/23 reports | Public `loom_decode` C ABI is stable enough for the current DuckDB path. Runtime ABI sketch and internal DuckDB route API are explicitly non-public/unfrozen. | bounded | Medium | Preserve the distinction between public `loom.h`, internal `loom_duckdb_internal.h`, and non-frozen runtime ABI sketches. |
| C-32-11 | Phase 22 host-neutral runtime ABI proves engine independence. | ROADMAP, Phase 22 report | Runtime planning model is host-neutral by design, but only DuckDB has consumed it; second-consumer proof remains absent. | scaffold | Medium | Treat engine independence as a design/contract claim pending a second real host. |
| C-32-12 | `verify_artifact` accepts `LMA1`. | Phase 31 report, core tests | Phase 31 added IPC-backed `LMA1` codec and verifier acceptance after decoded Arrow semantic validation. | proven | Low | No correction required; keep Arrow IPC described as carrier, not trust boundary. |
| C-32-13 | Source compatibility excludes malformed/unreadable files and reader limitations. | Phase 31 report, README | Phase 31 accepted definition requires the source reader to open and materialize Arrow batches. Malformed/unreadable sources remain rejected. | proven | Low | Keep this exclusion visible in readiness reporting. |
| C-32-14 | Native gates require real toolchain execution in all environments. | Phase 16/23/25 scripts and reports | Some gates are strict by default but allow explicit managed-toolchain skips; Phase 25 hardening reports accept fallback/skipped diagnostics in negative paths. | skipped | Medium | Report skip allowance explicitly when presenting native evidence. |

## Immediate Corrections From This Ledger

| Correction | Reason | Status |
|---|---|---|
| STATE core value now names the current source DuckDB value as source-backed single-column `LMA1` e2e artifacts. | Avoids implying arbitrary `LMA1` SQL support. | applied |
| README / README-zh diagram labels native lowering as optional bounded evidence rather than trusted execution. | Avoids turning bounded/scaffolded native evidence into a broad trust claim. | applied |
