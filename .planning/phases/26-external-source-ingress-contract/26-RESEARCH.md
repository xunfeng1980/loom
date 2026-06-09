# Phase 26: External Source Ingress Contract - Research

**Researched:** 2026-06-09
**Domain:** Rust source-ingress contract, artifact verification handoff, source adapter boundaries
**Confidence:** HIGH for repository facts and recommended contract shape; MEDIUM for general adapter best-practice analogies because they are advisory rather than implementation constraints.

## User Constraints (from CONTEXT.md)

- Phase 26 is a source-neutral ingress contract before Lance, Parquet, Iceberg, MCAP, Zarr, LeRobot, or other source-specific integrations. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Phase 26 must abstract the proven `loom-vortex-ingress` boundary into Loom-owned concepts for source facts, diagnostics, support classification, emission disposition, dependency isolation, verifier-routed `LMC1`/`LMT1` emission, oracle/equivalence evidence, and fail-closed unsupported/rejected behavior. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Phase 26 must not implement Lance/Parquet ingestion, Iceberg binding, host-engine integration, object-store credential handling, predicate pushdown, parallel split execution, or arbitrary Vortex semantic compatibility. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Source-specific readers may be represented only through examples, fixtures, mock adapters, or contract tests proving the generic shape. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- The accepted / unsupported / rejected triad is locked; unsupported valid sources may expose facts but must not emit partial `.loom` bytes. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Source SDK dependencies must stay in source-specific crates; generic contract work must not add Lance, Parquet, Iceberg, MCAP, Zarr, LeRobot, object-store, or Vortex dependencies to `loom-core`, `loom-ffi`, or DuckDB extension code. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Emission is limited to verifier-routed `LMC1` wrapping `LMP1`/`LMT1` payloads for supported shapes, and emitted artifacts must pass the existing artifact verifier before acceptance. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Lowering/native disposition is descriptive metadata in this phase and must not trigger new native kernels. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Deferred ideas are out of scope: Lance and Parquet implementation, Iceberg ref/table binding, StarRocks + DuckDB dual query surface, full Vortex semantic compatibility, object-store credentials, remote IO policy, dataset catalog semantics, source-specific indexing, archival rewrite behavior, public SQL/API changes, predicate pushdown, parallel split execution, and new native kernels. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]

## Summary

Phase 26 should create a source-neutral contract layer that preserves the proven Vortex reader pattern while removing Vortex vocabulary from the generic API. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; .planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md] The contract should model source identity, schema/data facts, layout/segment/split facts where available, diagnostics, support state, emission target, emission disposition, oracle strategy, equivalence evidence, and lowering disposition. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md]

The recommended home is a new workspace crate, `ingress/loom-source-ingress`, containing only Loom-owned contract types and adapter-test scaffolding. [ASSUMED] This is preferable to placing the generic model in `loom-core` because `loom-core` is currently a pure artifact/decode/verifier crate with explicit Vortex/FastLanes dependency guards, while source-ingress is an upstream provenance/admission concern. [VERIFIED: crates/loom-core/Cargo.toml; scripts/mvp0-verify.sh] It is preferable to keeping generic types inside `loom-vortex-ingress` because Phase 27 should target generic ingress, not a Vortex-named crate. [VERIFIED: .planning/ROADMAP.md; .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]

Primary recommendation: define `loom-source-ingress` as a dependency-light contract crate, map `VortexReaderFacts`/`VortexEncodingCoverage`/`VortexIngressReport` into it inside `loom-vortex-ingress`, add mock adapter contract tests for non-Vortex edge cases, and add a Phase 26 gate before Phase 27 starts. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; scripts/complete-vortex-reader-test.sh; scripts/vortex-encoding-coverage-test.sh]

## Constraints

- `loom-core` and `loom-ffi` must remain free of Vortex/FastLanes dependencies; the current release gate checks `cargo tree -p loom-core` and `cargo tree -p loom-ffi` for those dependency names. [VERIFIED: scripts/mvp0-verify.sh]
- `vortex-file` and `vortex-layout` direct dependencies are currently allowed only in `ingress/loom-vortex-ingress`. [VERIFIED: ingress/loom-vortex-ingress/Cargo.toml; scripts/check-core-invariants.sh; scripts/mvp0-verify.sh]
- `loom-vortex-ingress` currently depends on `loom-core` and Vortex crates, so dependency direction is source adapter -> core, not core -> source adapter. [VERIFIED: ingress/loom-vortex-ingress/Cargo.toml]
- Current artifact emission targets are `LMC1` containers containing `LMP1` layout payloads or `LMT1` table payloads. [VERIFIED: crates/loom-core/src/container_codec.rs; crates/loom-core/src/layout_codec.rs; crates/loom-core/src/table_codec.rs]
- `verify_artifact` accepts only supported `LMC1` containers with `LMP1` or `LMT1` payload sections and rejects malformed structural input before reporting accepted facts. [VERIFIED: crates/loom-core/src/artifact_verifier.rs]
- Nyquist validation is explicitly disabled in `.planning/config.json`, so this research does not include the standard Validation Architecture section. [VERIFIED: .planning/config.json]
- Security enforcement is enabled in `.planning/config.json`; Phase 26 should treat object-store credentials, remote IO, and public API expansion as explicit non-goals and should test for dependency/API creep. [VERIFIED: .planning/config.json; .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- AGENTS.md requires GSD workflow discipline before file-changing work and says not to make direct repo edits outside a GSD workflow unless explicitly bypassed; this task is itself a GSD phase-research task and is limited to planning artifact creation. [VERIFIED: AGENTS.md]

## Existing Evidence

### Vortex Ingress Facts

- `VortexIngressStatus` already models `Accepted`, `Unsupported`, and `Rejected` with stable string spellings. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; ingress/loom-vortex-ingress/tests/reader_facts_contract.rs]
- `VortexIngressDiagnostic` and `VortexReaderDiagnostic` already use stable `code`, `path`, and `message` fields. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs]
- `VortexReaderFacts` already records source kind, Vortex file version, row count, root dtype, root layout encoding, layout facts, dtype facts, segment facts, split facts, statistics presence, footer size, support, emission kind, coverage, and diagnostics. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs]
- `VortexReaderDTypeFact`, `VortexReaderLayoutFact`, `VortexReaderSegmentFact`, and `VortexReaderSplitFact` are Loom-owned summaries rather than raw Vortex Rust types. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; .planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md]
- Valid but unsupported UTF-8 Vortex files produce reader facts with `Unsupported` support and `None` emission, while malformed buffers return `Rejected` with no trusted facts. [VERIFIED: ingress/loom-vortex-ingress/tests/reader_facts_contract.rs; ingress/loom-vortex-ingress/tests/single_column_to_loom.rs]

### Support, Emission, Lowering Disposition

- Phase 21 tracks reader support, artifact emission, emission disposition, oracle evidence, and native lowering disposition separately per Vortex shape. [VERIFIED: .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md]
- Current emission kinds are `none`, `LMP1`, and `LMT1`; current emission dispositions are `none`, `canonical-raw`, `canonical-table`, and `structured-layout`. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md]
- Current lowering dispositions are `interpreter-only`, `production-lowering-supported`, and `fail-closed/deferred`. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; ingress/loom-vortex-ingress/tests/reader_facts_contract.rs]
- Canonicalized raw/table emission is documented as a semantic bridge backed by verifier/oracle evidence, not proof that Loom natively understands the original source encoding. [VERIFIED: .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md]

### Oracle And Equivalence Evidence

- `emit_supported_lmc1_from_vortex_buffer` emits a table payload first when a supported table scan succeeds, otherwise emits a supported single-column layout, and reports unsupported conversion on failure. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs]
- Supported single-column Vortex ingress currently covers non-null `Int32`, `Int64`, `Float32`, and `Float64`, emits `LMC1(LMP1)`, verifies the artifact, decodes through Loom, and compares row values to Vortex scan oracle helpers. [VERIFIED: ingress/loom-vortex-ingress/tests/single_column_to_loom.rs]
- Supported struct/table Vortex ingress currently covers non-null primitive fields, emits `LMC1(LMT1)`, verifies the artifact, decodes table arrays through Loom, and compares field rows to a Vortex scan oracle. [VERIFIED: ingress/loom-vortex-ingress/tests/table_to_loom.rs]
- Phase 25 hardened native execution uses interpreter/reference output as the native route oracle and explicitly does not upgrade Vortex-backed evidence into arbitrary Vortex semantics. [VERIFIED: .planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md]

### External Best-Practice Evidence

- The Arrow C Data Interface deliberately uses small ABI-stable C definitions, avoids forcing a dependency on Arrow implementations, and is scoped to same-process zero-copy sharing rather than persistence or a high-level API. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html]
- Arrow C Data consumers may support only a subset of Arrow types, but they should document unsupported types; this aligns with Loom's explicit support matrix instead of silent partial handling. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html]
- Arrow C Data and C Stream both use release callbacks and opaque producer-owned `private_data` for lifetime management, which supports keeping source SDK state private to adapters. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html; https://arrow.apache.org/docs/format/CStreamInterface.html]
- Arrow C Stream documents schema/data result lifetimes separately from stream lifetime and says stream sources are not assumed thread-safe, which supports Phase 26 keeping parallel split execution and stream ABI design out of scope. [CITED: https://arrow.apache.org/docs/format/CStreamInterface.html]
- DataFusion's custom table provider guide separates schema/capability planning from physical execution and warns that planning-time `scan()` should stay lightweight and not perform heavy IO. [CITED: https://datafusion.apache.org/library-user-guide/custom-table-providers.html]
- DataFusion source adapters expose projection/filter/limit as pushdown hints but require capability declarations; this supports Phase 26 recording source capabilities without implementing predicate pushdown. [CITED: https://datafusion.apache.org/library-user-guide/custom-table-providers.html; https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html]

## Recommended Contract Model

### Core Vocabulary

Define source-neutral types with stable string conversions and no source SDK types. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; ASSUMED]

| Generic Type | Required Fields | Vortex Mapping |
|---|---|---|
| `SourceIngressStatus` | `accepted`, `unsupported`, `rejected` | `VortexIngressStatus` / `VortexReaderSupport` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceIdentity` | `source_kind`, `format`, `format_version`, `fingerprint`, optional `path_display` | `source_kind`, `vortex_file_version`, buffer/path facts [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceDiagnostic` | `code`, `path`, `message`, `family`, optional `source_detail` | `VortexIngressDiagnostic`, `VortexReaderDiagnostic` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceSchemaFact` | path, logical kind, nullable, field count/names, Arrow-compatible summary | `VortexReaderDTypeFact` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceLayoutFact` | path, layout class, row count, child count, child name/type, physical refs, metadata byte length | `VortexReaderLayoutFact` with generic names [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceSegmentFact` | id/index, byte range, length, alignment, ordering/overlap flags | `VortexReaderSegmentFact` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceSplitFact` | index, start row, end row, row count | `VortexReaderSplitFact` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceCoverage` | schema family, nullability, layout class, array encoding, split/stats presence, support, emission, lowering, notes | `VortexEncodingCoverage` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceEmissionDisposition` | `none`, `canonical-raw`, `canonical-table`, `structured-layout` | `VortexEmissionDisposition` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceEmissionKind` | `none`, `LMP1`, `LMT1` | `VortexReaderEmissionKind` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceLoweringDisposition` | `interpreter-only`, `production-lowering-supported`, `fail-closed/deferred` | `VortexLoweringDisposition` [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `SourceOracleEvidence` | strategy, status, row_count_checked, nulls_checked, source_native_scan_used, notes | Vortex scan oracle tests and unsupported reasons [VERIFIED: ingress/loom-vortex-ingress/tests/single_column_to_loom.rs; ingress/loom-vortex-ingress/tests/table_to_loom.rs] |
| `SourceIngressReport` | status, identity, facts, diagnostics, emission, verifier status, oracle evidence | `VortexIngressReport` plus reader facts and artifact verifier handoff [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; crates/loom-core/src/artifact_verifier.rs] |

### Model Rules

- `accepted` means source facts are valid, artifact emission is complete for the supported shape, the emitted `LMC1` artifact passed `verify_artifact`, and required oracle evidence for that shape exists. [VERIFIED: .planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md; ingress/loom-vortex-ingress/tests/single_column_to_loom.rs]
- `unsupported` means the source is readable enough to expose facts, but no `.loom` bytes may be emitted for that source shape in this phase. [VERIFIED: .planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md; ingress/loom-vortex-ingress/tests/reader_facts_contract.rs]
- `rejected` means the source cannot be opened or parsed into trustworthy facts, and the report must not carry a trust token or artifact bytes. [VERIFIED: ingress/loom-vortex-ingress/tests/reader_facts_contract.rs]
- Emission disposition must remain separate from source support so the planner can distinguish fact-bearing unsupported input, canonical raw/table bridges, and future structured layouts. [VERIFIED: .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md]
- Lowering disposition must describe the emitted Loom artifact shape, not the original external source encoding, unless a future phase adds structured layout support and native kernels for that source-derived shape. [VERIFIED: .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md; .planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md]
- Oracle strategy should be declared per adapter as `source-native-scan`, `arrow-scan`, `decoded-row-fixture`, or `unsupported`, with evidence status separate from implementation success. [VERIFIED: ingress/loom-vortex-ingress/tests/single_column_to_loom.rs; ASSUMED]
- Do not introduce a generic plugin framework in Phase 26; a trait or conversion helper is enough if it reduces duplication between Vortex and mock adapters. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md; ASSUMED]

### Mapping Vortex Without Leaking Names

| Vortex-specific Concept | Generic Name | Reason |
|---|---|---|
| `VortexReaderFacts` | `SourceFacts` or `SourceIngressFacts` | Facts should represent source schema/layout/segments, not a Vortex-only reader. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| `vortex_file_version` | `format_version` | Future formats have their own version/fingerprint concepts. [ASSUMED] |
| `root_dtype` | `root_schema` or `root_type` | Lance/Parquet/Zarr may expose schema rather than dtype vocabulary. [ASSUMED] |
| `root_layout_encoding` | `root_layout_class` plus optional `encoding_summary` | Some sources expose row groups/fragments/chunks rather than Vortex layout encoding ids. [ASSUMED] |
| `segment_facts` | `physical_segments` | Parquet row groups, Lance fragments, MCAP chunks, and Zarr chunks are source-specific physical segments. [ASSUMED] |
| `split_facts` | `row_splits` | Keeps Phase 22/27 handoff useful without implementing parallel execution. [VERIFIED: .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md; ASSUMED] |
| `Vortex scan oracle` | `SourceOracleEvidence` | Vortex scan is one oracle strategy, not the generic mechanism. [VERIFIED: .planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md] |

## Dependency Boundary

### Recommendation

Create `ingress/loom-source-ingress` for generic contract types and tests, and make `loom-vortex-ingress` depend on it to expose a `to_source_ingress_report` mapping or equivalent wrapper. [ASSUMED] The new crate should have no Lance/Parquet/Iceberg/MCAP/Zarr/LeRobot/object-store dependencies and should not depend on Vortex crates. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]

The new crate may depend on `loom-core` only if it needs to embed artifact verification summaries or exact artifact status enums; otherwise it should duplicate only source-level statuses and accept verifier summaries as plain data. [ASSUMED] The important invariant is that `loom-core` must not depend on `loom-source-ingress`, because artifact verification should remain source-neutral and usable without source adapter crates. [VERIFIED: crates/loom-core/Cargo.toml; crates/loom-core/src/artifact_verifier.rs]

### Tradeoffs

| Option | Benefits | Costs | Recommendation |
|---|---|---|---|
| New `loom-source-ingress` crate | Clean home for Phase 27; no Vortex-named generic API; can test mock adapters without Vortex deps. [ASSUMED] | Adds one workspace crate and some mapping boilerplate. [ASSUMED] | Use this. [ASSUMED] |
| `loom-core::source_ingress` module | Avoids a new crate; artifact verifier types are nearby. [ASSUMED] | Puts source provenance/admission concepts into the core decode/verifier crate and increases risk of future source dependency pressure. [VERIFIED: crates/loom-core/Cargo.toml; ASSUMED] | Avoid for Phase 26. [ASSUMED] |
| Generic module inside `loom-vortex-ingress` | Minimal churn and easy Vortex mapping. [ASSUMED] | Makes Phase 27 depend on a Vortex-named crate or copy the model, which defeats the phase goal. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md; ASSUMED] | Avoid except as temporary internal wrapper. [ASSUMED] |
| Put source contract in `loom-ffi` or DuckDB extension | Existing host path can consume artifacts. [VERIFIED: .planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md] | Source ingress is upstream of host execution, and Phase 26 explicitly excludes host-engine integration. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md] | Do not use. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md] |

### Dependency Guard Inputs

- Extend existing dependency guards to reject `vortex-*`, `lance*`, `parquet`, `iceberg`, `mcap`, `zarr`, `object_store`, and credential SDK dependencies in `loom-core`, `loom-ffi`, and `loom-source-ingress` unless a future phase explicitly changes the boundary. [VERIFIED: scripts/mvp0-verify.sh; ASSUMED]
- Keep `loom-vortex-ingress` as the only crate with `vortex-file` / `vortex-layout` direct dependencies in Phase 26. [VERIFIED: ingress/loom-vortex-ingress/Cargo.toml; scripts/mvp0-verify.sh]
- Do not add external packages in Phase 26; no package legitimacy audit is required if the implementation only adds an internal workspace crate. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md; ASSUMED]

## Testing/Gates

### Required Contract Tests

- Add tests proving stable strings for generic support, emission, emission disposition, lowering disposition, oracle strategy, and diagnostic code families. [VERIFIED: ingress/loom-vortex-ingress/tests/reader_facts_contract.rs; ASSUMED]
- Add Vortex mapping tests proving supported non-null primitive files map from `VortexReaderFacts` and `VortexEncodingCoverage` into generic `SourceIngressReport` with `accepted`, `LMP1`, `canonical-raw`, `production-lowering-supported`, and source-native oracle evidence. [VERIFIED: ingress/loom-vortex-ingress/tests/single_column_to_loom.rs; ASSUMED]
- Add Vortex table mapping tests proving supported non-null primitive structs map into `accepted`, `LMT1`, `canonical-table`, artifact-verifier accepted status, and source-native oracle evidence. [VERIFIED: ingress/loom-vortex-ingress/tests/table_to_loom.rs; ASSUMED]
- Add unsupported valid source tests using existing UTF-8 Vortex fixtures and mock adapters to prove fact-bearing `unsupported` reports emit no artifact bytes. [VERIFIED: ingress/loom-vortex-ingress/tests/reader_facts_contract.rs; ASSUMED]
- Add rejected input tests proving malformed sources return `rejected`, stable diagnostics, no facts, no artifact bytes, and no verifier acceptance. [VERIFIED: ingress/loom-vortex-ingress/tests/reader_facts_contract.rs; ASSUMED]
- Add dependency creep tests or script checks proving source-specific dependencies do not enter `loom-core`, `loom-ffi`, DuckDB extension code, or generic source-ingress contract code. [VERIFIED: scripts/mvp0-verify.sh; scripts/check-core-invariants.sh; ASSUMED]

### Release Gate Shape

Recommended new gate: `scripts/source-ingress-contract-test.sh`. [ASSUMED]

The gate should run:

```bash
cargo test -p loom-source-ingress
cargo test -p loom-vortex-ingress --test reader_facts_contract
cargo test -p loom-vortex-ingress --test single_column_to_loom
cargo test -p loom-vortex-ingress --test table_to_loom
cargo test -p loom-core --test artifact_verifier
```

The gate should also grep for contract/report docs and dependency creep markers before being wired into `scripts/mvp0-verify.sh` after Phase 25 native hardening and before DuckDB SQL smoke. [VERIFIED: scripts/mvp0-verify.sh; ASSUMED]

### Documentation Artifacts

- Add a final `26-SOURCE-INGRESS-CONTRACT.md` with the generic model, Vortex mapping table, adapter obligations, non-goals, and Phase 27 handoff assumptions. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md]
- Add a final report plus the standard per-plan `26-05-SUMMARY.md` closeout evidence; Phase 18 and Phase 21 both used final docs and release-gate scripts. [VERIFIED: .planning/phases/18-complete-vortex-reader/18-SUMMARY.md; scripts/vortex-encoding-coverage-test.sh]

## Risks/Tradeoffs

| Risk / Tradeoff | Why It Matters | Mitigation |
|---|---|---|
| Overfitting to Vortex | The only real adapter today is Vortex, so generic names could still encode Vortex assumptions. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] | Use Vortex mapping plus mock adapter tests for sources with schema-only facts, row-group/chunk facts, and no layout tree. [ASSUMED] |
| New crate churn | A new internal crate adds workspace maintenance. [ASSUMED] | Keep it type-only, dependency-light, and release-gated; do not add source SDK packages. [ASSUMED] |
| Putting contract in `loom-core` | It is tempting because verifier and artifact types live there. [VERIFIED: crates/loom-core/src/artifact_verifier.rs] | Keep core source-neutral; map emitted artifacts to verifier reports rather than embedding source adapters in core. [ASSUMED] |
| Unsupported facts mistaken for trust | Valid external metadata can be useful but must not imply safe Loom artifact emission. [VERIFIED: .planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md] | Require status + emission + verifier status to agree before accepted artifact handoff. [ASSUMED] |
| Canonical emission overstated | Canonical raw/table emission proves a safe Loom artifact bridge, not native support for the original external encoding. [VERIFIED: .planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md] | Keep `emission_disposition` and `lowering_disposition` separate and include notes in reports. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs] |
| Premature pushdown/splits | Source adapter literature exposes projection/filter/split concepts, but Phase 26 excludes pushdown and parallel split execution. [CITED: https://datafusion.apache.org/library-user-guide/custom-table-providers.html; VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md] | Record split/capability facts only; do not execute split plans or accept predicates. [ASSUMED] |
| Public API creep | Host/API controls could freeze before source bindings are ready. [VERIFIED: .planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md] | Keep Phase 26 Rust-internal and planning/report oriented; no public SQL, C ABI, DuckDB, or object-store auth surface. [VERIFIED: .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md] |

## Proposed Plan Inputs

1. Plan 26-01: Add `loom-source-ingress` contract crate with source-neutral enums/structs, stable string tests, and no external source SDK dependencies. [ASSUMED]
2. Plan 26-02: Map existing `loom-vortex-ingress` reports/facts/coverage into the generic contract without removing or breaking current Vortex-specific APIs. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; ASSUMED]
3. Plan 26-03: Add artifact-verifier handoff and oracle evidence contract tests covering accepted single-column, accepted table, unsupported valid, and rejected malformed cases. [VERIFIED: ingress/loom-vortex-ingress/tests/single_column_to_loom.rs; ingress/loom-vortex-ingress/tests/table_to_loom.rs; ASSUMED]
4. Plan 26-04: Add dependency/API creep gate and reviewer docs (`26-SOURCE-INGRESS-CONTRACT.md`) with explicit Vortex mapping and non-goals. [VERIFIED: scripts/mvp0-verify.sh; .planning/phases/26-external-source-ingress-contract/26-CONTEXT.md; ASSUMED]
5. Plan 26-05: Wire `scripts/source-ingress-contract-test.sh` into the release gate and close with a Phase 27 handoff that says Lance/Parquet adapters must implement the generic contract, declare oracle strategy, emit only verifier-accepted `LMC1`, and keep source SDK dependencies outside `loom-core`/`loom-ffi`. [VERIFIED: .planning/ROADMAP.md; scripts/mvp0-verify.sh; ASSUMED]

## Sources

### Repository Sources

- `.planning/ROADMAP.md` - Phase 26 ordering, Phase 27 handoff, and non-goals. [VERIFIED: codebase read]
- `.planning/STATE.md` - current Phase 26 focus and Phase 25 handoff assumptions. [VERIFIED: codebase read]
- `.planning/PROJECT.md` - project constraints, core value, dependency boundary, and key decisions. [VERIFIED: codebase read]
- `.planning/phases/26-external-source-ingress-contract/26-CONTEXT.md` - locked Phase 26 scope and decisions. [VERIFIED: codebase read]
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md` - native hardening baseline and non-goals inherited by Phase 26. [VERIFIED: codebase read]
- `.planning/phases/18-complete-vortex-reader/18-CONTEXT.md`, `18-READER-CONTRACT.md`, and `18-SUMMARY.md` - current Vortex reader facts and release evidence. [VERIFIED: codebase read]
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-CONTEXT.md` and `21-COVERAGE-MATRIX.md` - disposition vocabulary and finite coverage matrix. [VERIFIED: codebase read]
- `ingress/loom-vortex-ingress/src/lib.rs` and required ingress tests - current facts, diagnostics, support, emission, coverage, oracle helpers, and verifier-routed emission. [VERIFIED: codebase read]
- `crates/loom-core/src/artifact_verifier.rs`, `container_codec.rs`, `layout_codec.rs`, and `table_codec.rs` - artifact verifier and `LMC1`/`LMP1`/`LMT1` target model. [VERIFIED: codebase read]
- `scripts/mvp0-verify.sh`, `scripts/check-core-invariants.sh`, `scripts/complete-vortex-reader-test.sh`, and `scripts/vortex-encoding-coverage-test.sh` - release-gate and dependency-boundary patterns. [VERIFIED: codebase read]

### External Sources

- Apache Arrow C Data Interface v24.0.0 - ABI-stable small definitions, zero-copy same-process scope, release callbacks, partial support documentation, and non-goals. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html]
- Apache Arrow C Stream Interface v24.0.0 - stream callback/lifetime/thread-safety guidance. [CITED: https://arrow.apache.org/docs/format/CStreamInterface.html]
- Apache DataFusion Custom Table Provider guide - source adapter layering, capability hints, lightweight planning, and pushdown boundaries. [CITED: https://datafusion.apache.org/library-user-guide/custom-table-providers.html]
- DataFusion `TableProvider` Rust API docs - projection/filter/limit scan interface and filter pushdown declaration. [CITED: https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html]

## Confidence

- Existing Loom/Vortex facts: HIGH, directly verified in code and phase artifacts. [VERIFIED: codebase read]
- Recommended generic contract vocabulary: HIGH, because it is a direct source-neutral rename of existing working Vortex concepts. [VERIFIED: ingress/loom-vortex-ingress/src/lib.rs; ASSUMED]
- Recommended crate home: MEDIUM, because the codebase supports the dependency-boundary reasoning but the exact workspace-crate decision is still an implementation choice for the planner. [VERIFIED: Cargo.toml; ASSUMED]
- External best-practice analogies: MEDIUM, because Arrow/DataFusion sources are authoritative but advisory for Loom rather than binding project requirements. [CITED: https://arrow.apache.org/docs/format/CDataInterface.html; https://datafusion.apache.org/library-user-guide/custom-table-providers.html]

## Open Questions (RESOLVED)

1. RESOLVED: `loom-source-ingress` should not depend on `loom-core` in Phase 26. Store verifier handoff as plain source-contract data so the artifact verifier remains source-neutral and `loom-core` does not learn about source-ingress adapters. [ASSUMED]
2. RESOLVED: `SourceIdentity` should include an optional/reserved fingerprint field in Phase 26, but adapters may leave it absent until Phase 27 has real Lance/Parquet identity requirements. The contract reserves the concept without requiring a persistent hash format. [ASSUMED]
3. RESOLVED: keep conversion helpers adapter-local in Phase 26. Expose generic data types and constructors from `loom-source-ingress`, but do not introduce a broad public adapter trait or plugin framework until a second real adapter proves the shared abstraction. [ASSUMED]
