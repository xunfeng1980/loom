# Phase 27: Lance + Parquet Archival Readability / Dataset Ingress - Research

**Researched:** 2026-06-09 [VERIFIED: system date]
**Domain:** Rust source adapters for Lance and Parquet archival readability through the Phase 26 source-ingress contract [VERIFIED: .planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-CONTEXT.md]
**Confidence:** HIGH for Parquet and internal contract mapping; MEDIUM for Lance API stability because Lance is current at 7.0.0 and exposes a broad async dataset surface. [VERIFIED: docs.rs/parquet; VERIFIED: docs.rs/lance]

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

## Implementation Decisions

### Source Coverage
- Recommended: include both Lance and Parquet in the phase, but keep each to a
  minimal local-file adapter slice. Tradeoff: this proves the source-ingress
  contract generalizes across two Arrow-adjacent formats without turning the
  phase into a full source framework.
- Recommended: accepted emission should start with non-null primitive
  Int32/Int64/Float32/Float64 single-column and simple table shapes already
  representable by `LMP1`/`LMT1`. Tradeoff: this chooses durable Loom artifact
  evidence over broad nested/null/extension-type coverage.
- Recommended: facts may include Lance fragment/schema/version-style summaries
  and Parquet row-group/schema/page-adjacent summaries where available, but
  those facts are descriptive and source-neutral. Tradeoff: reviewers get
  archival evidence without freezing source SDK internals into Loom APIs.

### Adapter Boundary
- Recommended: create source-specific adapter crates or modules that own Lance
  and Parquet SDK dependencies and map into `loom-source-ingress`. Tradeoff:
  extra adapter boilerplate preserves the Phase 26 dependency boundary.
- Recommended: keep `loom-core`, `loom-ffi`, `loom-source-ingress`, DuckDB
  extension code, and public headers free of Lance/Parquet/source SDK deps.
  Tradeoff: host/query integration remains deferred, but the artifact contract
  stays portable.
- Recommended: do not expose Lance/Parquet SDK objects, dataset handles,
  readers, credentials, Arrow stream ownership handles, or object-store state in
  generic public types. Tradeoff: some useful metadata must be summarized as
  strings/facts rather than passed through directly.

### Archival Readability Proof
- Recommended: require two value proofs for each source family where feasible:
  current-version read/write/verify and legacy-file-with-Loom readability. The
  legacy proof may use checked-in or generated fixtures if research identifies a
  stable, license-safe fixture path. Tradeoff: this proves durability intent
  without designing a full archival container format.
- Recommended: emitted Loom artifacts should be paired with source facts and
  verifier/oracle evidence rather than embedded into Lance manifests or Parquet
  footers in this phase. Tradeoff: pairing is less integrated but avoids
  source-format writer internals and long-term compatibility traps.
- Recommended: if old-version source writer tooling is brittle or unavailable,
  prefer a small fixture compatibility matrix and explicitly record the gap in
  the report instead of widening dependencies. Tradeoff: avoids spending the
  phase on historical build archaeology while still capturing archival risk.

### Oracle And Equivalence
- Recommended: Parquet oracle should use an Arrow/Parquet scan path or decoded
  row fixture selected during research; Lance oracle should use Lance-native or
  Arrow-compatible scan output selected during research. Tradeoff: the oracle
  is source-specific evidence, not the Loom decode path.
- Recommended: accepted reports must require `SourceOracleEvidence` and
  `SourceArtifactVerificationSummary::accepted`, matching Phase 26. Tradeoff:
  no source adapter can claim accepted archival readability from facts alone.
- Recommended: equivalence should compare rows from verified Loom artifacts
  against source-native/Arrow oracle output for the supported primitive/table
  slice. Tradeoff: row-level equality is narrow but concrete and reviewable.

### Reports And Gates
- Recommended: write a final `27-ARCHIVAL-READABILITY-REPORT.md` describing
  supported Lance/Parquet slices, source fact mapping, accepted/unsupported/
  rejected matrices, oracle evidence, archival-readability proof, dependency
  guards, tradeoffs, non-goals, and Phase 28 handoff.
- Recommended: add `scripts/lance-parquet-ingress-test.sh` or equivalent and
  wire it into `scripts/mvp0-verify.sh` only after focused adapter tests pass.
  Tradeoff: release evidence becomes one-command reproducible, but the script
  must stay bounded and skip/fixture-aware if source SDK tooling is unavailable.
- Recommended: all new source SDK package choices must be researched against
  current primary sources before implementation. Tradeoff: current crate/API
  reality is unstable enough that planning must verify versions rather than
  trusting memory.

### the agent's Discretion
- Choose exact crate names and module structure during research/planning, based
  on workspace patterns and dependency hygiene.
- Choose the smallest stable fixture strategy that proves current-version and
  legacy readability without adding remote services or credentials.
- Choose whether Lance and Parquet share helper functions only after a second
  adapter makes duplication real; do not introduce a broad plugin framework.
- Prefer local fixtures, deterministic generated data, and focused tests over
  external services.

### Deferred Ideas (OUT OF SCOPE)

- Iceberg table/ref metadata binding: Phase 28.
- StarRocks + DuckDB dual query surface: Phase 29.
- Full arbitrary Vortex semantic compatibility: Phase 30.
- Embedding Loom artifacts into Lance manifests, Parquet footers, or source
  writer internals.
- Object-store credentials, remote IO policy, dataset catalog semantics,
  index semantics, predicate pushdown, projection pushdown, parallel split
  execution, nested/list/struct extension type coverage beyond the minimal
  primitive/table slice, public SQL/API changes, and new native kernels.
</user_constraints>

## Project Constraints (from AGENTS.md)

- `loom-core` and `loom-ffi` must remain free of source SDK dependencies; source-specific SDKs belong only in isolated adapter crates. [VERIFIED: AGENTS.md; VERIFIED: Cargo.toml; VERIFIED: scripts/source-ingress-contract-test.sh]
- Arrow remains the Rust/C++ FFI and artifact output contract, and the workspace pins the Arrow family to `=58.3.0`. [VERIFIED: AGENTS.md; VERIFIED: Cargo.toml]
- Vortex crates are allowed only in oracle, fixture, and ingress boundaries, and Phase 27 must not weaken that precedent by leaking Lance or Parquet into core, FFI, DuckDB, or public headers. [VERIFIED: AGENTS.md; VERIFIED: scripts/source-ingress-contract-test.sh]
- MVP1 scope favors narrow verifier-gated slices over broad compatibility claims. [VERIFIED: AGENTS.md; VERIFIED: .planning/PROJECT.md]
- Do not edit `ROADMAP.md` or `STATE.md` for this research task. [VERIFIED: user request]

## Summary

Phase 27 should implement two source-specific adapter crates, `loom-lance-ingress` and `loom-parquet-ingress`, that consume the Phase 26 `loom-source-ingress` contract and emit only verifier-accepted `LMC1(LMP1)` or `LMC1(LMT1)` artifacts for non-null Arrow primitive single-column/simple-table shapes. [VERIFIED: 27-CONTEXT.md; VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md; VERIFIED: crates/loom-source-ingress/src/lib.rs] The adapters should canonicalize decoded Arrow rows into existing Loom raw/table payloads rather than representing Lance or Parquet physical encodings as new Loom semantics. [VERIFIED: 26-SOURCE-INGRESS-REPORT.md; VERIFIED: crates/loom-core/src/layout_codec.rs; VERIFIED: crates/loom-core/src/table_codec.rs]

The value proof should be archival readability: current-version source files/datasets can be read, converted, verified, decoded, and row-compared against source-native or Arrow-scan output; legacy source fixtures paired with Loom artifacts remain readable through Loom and can be rewritten to current source formats for the supported slice. [VERIFIED: 27-CONTEXT.md; CITED: https://docs.rs/lance/latest/lance/; CITED: https://docs.rs/parquet/58.3.0/parquet/] The phase should not embed Loom bytes in Lance manifests or Parquet footers, should not add remote IO, and should not add public SQL/API routes. [VERIFIED: 27-CONTEXT.md]

**Primary recommendation:** Use exact pins `lance = "=7.0.0"` with `default-features = false`, `parquet = "=58.3.0"` with `default-features = false` and `features = ["arrow"]`, plus focused test-only `tokio`, `futures`, and `tempfile` dependencies where needed; keep every package inside source-specific adapter crates or dev-dependencies. [VERIFIED: cargo info; VERIFIED: docs.rs/lance; VERIFIED: docs.rs/parquet; VERIFIED: slopcheck]

## Constraints

- Accepted reports must require trusted facts, `LMP1` or `LMT1` emission, accepted artifact verification, non-empty artifact bytes, and accepted oracle evidence. [VERIFIED: crates/loom-source-ingress/src/lib.rs]
- Unsupported valid sources may expose facts but must emit no bytes and no accepted oracle evidence. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md; VERIFIED: crates/loom-source-ingress/tests/source_ingress_contract.rs]
- Rejected malformed sources must expose diagnostics only, with no trusted facts, no artifact bytes, and no oracle evidence. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md; VERIFIED: crates/loom-source-ingress/tests/source_ingress_contract.rs]
- Current Loom artifact targets are `LMC1` wrapping `LMP1` for single-column payloads or `LMT1` for table payloads. [VERIFIED: crates/loom-core/src/container_codec.rs; VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md]
- Existing `LMP1` supports Arrow `Boolean`, `Int32`, `Int64`, `Utf8`, `Float32`, and `Float64`, but Phase 27 should accept only non-null primitive `Int32`, `Int64`, `Float32`, and `Float64` to match the requested narrow slice. [VERIFIED: crates/loom-core/src/layout_codec.rs; VERIFIED: user request]
- Nyquist validation is explicitly disabled, so the research does not add the full GSD Validation Architecture section. [VERIFIED: .planning/config.json]

## Current External Source Findings

### Lance

- The `lance` crate is current at `7.0.0`, requires Rust `1.91.0`, and depends on Arrow `^58.0.0`, which is compatible with the workspace's exact Arrow `58.3.0` pin under Cargo's resolver. [VERIFIED: cargo info lance; CITED: https://docs.rs/lance/latest/lance/]
- Lance documentation shows `Dataset::write` accepting an Arrow `RecordBatchReader`, a destination URI, and optional `WriteParams`. [CITED: https://docs.rs/lance/latest/lance/; CITED: https://docs.rs/lance/latest/lance/dataset/struct.Dataset.html]
- Lance documentation shows `Dataset::open`, `Dataset::scan`, and `Scanner::try_into_stream()` as the documented read path for collecting Arrow `RecordBatch` output. [CITED: https://docs.rs/lance/latest/lance/]
- Lance `Dataset` exposes `schema`, `version_id`, `version`, `count_fragments`, `get_fragments`, `manifest`, and `manifest_location` accessors that can map to source-neutral schema/version/fragment facts. [CITED: https://docs.rs/lance/latest/lance/dataset/struct.Dataset.html]
- Lance `FileFragment` exposes `id`, `num_data_files`, `schema`, `metadata`, `count_rows`, `physical_rows`, and `validate`, which are enough for descriptive fragment facts without leaking SDK objects into generic reports. [CITED: https://docs.rs/lance/latest/lance/dataset/fragment/struct.FileFragment.html]
- Lance default features include cloud/provider features (`aws`, `azure`, `gcp`, `oss`, `huggingface`, `tencent`) and `geo`, so Phase 27 should disable default features and use local path fixtures only. [VERIFIED: cargo info lance]
- Lance still has a normal `object_store` dependency even with cloud feature defaults disabled, so dependency guards should permit `object_store` only inside `loom-lance-ingress` transitive trees and should forbid credential/config APIs in source reports, CLI, DuckDB, FFI, and core. [CITED: https://docs.rs/lance/latest/lance/; VERIFIED: 27-CONTEXT.md]

### Parquet

- The `parquet` crate is current at `58.3.0`, is the official native Rust implementation under Apache Arrow, and its optional Arrow dependencies are also `58.3.0`. [VERIFIED: cargo info parquet; CITED: https://docs.rs/parquet/58.3.0/parquet/]
- Parquet docs describe row groups, column chunks, strongly typed schema, metadata, and optional statistics as the natural metadata/fact model for a file. [CITED: https://docs.rs/parquet/58.3.0/parquet/]
- Parquet docs recommend `ArrowWriter` for writing Arrow `RecordBatch` values and `ParquetRecordBatchReaderBuilder` for reading synchronous IO sources such as files or in-memory buffers. [CITED: https://docs.rs/parquet/58.3.0/parquet/; CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_reader/type.ParquetRecordBatchReaderBuilder.html; CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_writer/struct.ArrowWriter.html]
- `ParquetRecordBatchReaderBuilder::try_new` can build from a `File` or `Bytes`, exposes metadata such as row-group count through its builder metadata, and builds a `ParquetRecordBatchReader` for batch iteration. [CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_reader/type.ParquetRecordBatchReaderBuilder.html]
- `ParquetMetaData` exposes `file_metadata`, `num_row_groups`, `row_group`, `row_groups`, and optional column/page index accessors, which map directly to source-neutral file/row-group/page-adjacent facts. [CITED: https://docs.rs/parquet/58.3.0/parquet/file/metadata/struct.ParquetMetaData.html]
- `ArrowWriter` can write `RecordBatch` values, close the file, and expose flushed row-group metadata; Phase 27 should use explicit uncompressed current fixtures to avoid broad compression support claims. [CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_writer/struct.ArrowWriter.html]

## Recommended Dependency Strategy

### Workspace Pins

```toml
# New source-specific workspace dependencies.
lance = { version = "=7.0.0", default-features = false }
parquet = { version = "=58.3.0", default-features = false, features = ["arrow"] }
futures = { version = "=0.3.32" }
tokio = { version = "=1.52.3", default-features = false, features = ["rt", "macros"] }
tempfile = { version = "=3.27.0" }
```

- `lance` should be used only by `crates/loom-lance-ingress`; disabling default features avoids pulling provider feature flags into the direct dependency declaration. [VERIFIED: cargo info lance; VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md]
- `parquet` should be used only by `crates/loom-parquet-ingress`; `default-features = false, features = ["arrow"]` keeps the adapter on the RecordBatch API and avoids optional object-store/async/compression expansion. [VERIFIED: cargo info parquet; CITED: https://docs.rs/parquet/58.3.0/parquet/]
- `futures` is needed in `loom-lance-ingress` for collecting Lance scanner streams, matching the Lance docs example that imports `futures::StreamExt`. [CITED: https://docs.rs/lance/latest/lance/; VERIFIED: cargo info futures]
- `tokio` should be a dev-dependency for Lance adapter tests, with only `rt` and `macros` enabled for `#[tokio::test]` and local async execution. [CITED: https://docs.rs/tokio/latest/tokio/; VERIFIED: cargo info tokio]
- `tempfile` should be a dev-dependency for local source fixture directories/files and should not become part of core/runtime artifact APIs. [VERIFIED: cargo info tempfile; CITED: https://docs.rs/tempfile/latest/tempfile/]

### Crate Layout

- Add `crates/loom-lance-ingress` with dependencies on `loom-core`, `loom-source-ingress`, workspace Arrow crates, `lance`, and `futures`; keep `tokio` and `tempfile` as dev-dependencies. [VERIFIED: workspace Cargo.toml patterns; VERIFIED: docs.rs/lance]
- Add `crates/loom-parquet-ingress` with dependencies on `loom-core`, `loom-source-ingress`, workspace Arrow crates, and `parquet`; keep `tempfile` as a dev-dependency. [VERIFIED: workspace Cargo.toml patterns; VERIFIED: docs.rs/parquet]
- Do not add Lance or Parquet to `loom-core`, `loom-ffi`, `loom-source-ingress`, `loom-cli`, DuckDB extension code, or public headers. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md; VERIFIED: scripts/source-ingress-contract-test.sh]

### Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---|---|---:|---:|---|---|---|
| `lance` | crates.io | since 2022-07-28 | 1,504,988 total / 605,485 recent | https://github.com/lance-format/lance | OK | Approved for `loom-lance-ingress` only [VERIFIED: crates.io API; VERIFIED: slopcheck; VERIFIED: docs.rs/lance] |
| `parquet` | crates.io | since 2018-04-01 | 57,948,062 total / 12,314,838 recent | https://github.com/apache/arrow-rs | OK | Approved for `loom-parquet-ingress` only [VERIFIED: crates.io API; VERIFIED: slopcheck; VERIFIED: docs.rs/parquet] |
| `futures` | crates.io | since 2016-07-31 | 598,489,625 total / 133,007,410 recent | https://github.com/rust-lang/futures-rs | OK | Approved for Lance stream collection [VERIFIED: crates.io API; VERIFIED: slopcheck; CITED: https://docs.rs/futures/latest/futures/] |
| `tokio` | crates.io | since 2016-07-01 | 722,504,844 total / 165,827,735 recent | https://github.com/tokio-rs/tokio | OK | Approved as Lance adapter dev-dependency with `rt,macros` only [VERIFIED: crates.io API; VERIFIED: slopcheck; CITED: https://docs.rs/tokio/latest/tokio/] |
| `tempfile` | crates.io | since 2015-04-14 | 611,409,715 total / 133,079,565 recent | https://github.com/Stebalien/tempfile | OK | Approved as local fixture dev-dependency [VERIFIED: crates.io API; VERIFIED: slopcheck; CITED: https://docs.rs/tempfile/latest/tempfile/] |

**Packages removed due to slopcheck [SLOP] verdict:** none. [VERIFIED: slopcheck]
**Packages flagged as suspicious [SUS]:** none. [VERIFIED: slopcheck]
**Node postinstall audit:** not applicable because this phase uses Rust crates, not npm packages. [VERIFIED: package ecosystem]

## Adapter Model

### Shared Shape

- Each adapter should expose an inspect function returning `SourceIngressReport` and an emit function returning `Result<SourceIngressAcceptedArtifact, SourceIngressReport>`, matching the Vortex handoff pattern. [VERIFIED: crates/loom-vortex-ingress/src/source_contract.rs]
- Each adapter should build `SourceIdentity`, `SourceFacts`, `SourceSchemaFact`, `SourceLayoutFact`, `SourceSegmentFact`, `SourceSplitFact`, and `SourceCoverage` using only strings, primitive counts, ranges, and booleans. [VERIFIED: crates/loom-source-ingress/src/lib.rs]
- Each accepted adapter path should decode source rows into Arrow arrays, reject nullable arrays, reject non-primitive/nested/extension/dictionary/logical-only shapes for Phase 27, encode existing Loom `Raw` layouts, wrap them in `LMC1`, call `verify_artifact`, decode the verified Loom artifact, and compare rows against the oracle before returning bytes. [VERIFIED: crates/loom-core/src/layout_codec.rs; VERIFIED: crates/loom-core/src/table_codec.rs; VERIFIED: crates/loom-core/src/artifact_verifier.rs]
- Single-column accepted output should use `encode_layout_payload` and `wrap_layout_payload`; simple table accepted output should use `encode_table_payload` and `wrap_table_payload`. [VERIFIED: crates/loom-core/src/layout_codec.rs; VERIFIED: crates/loom-core/src/table_codec.rs; VERIFIED: crates/loom-core/src/container_codec.rs]

### Lance Adapter

- Use `Dataset::open` for local dataset paths and `Dataset::scan().try_into_stream()` for source-native/Arrow-compatible oracle batches. [CITED: https://docs.rs/lance/latest/lance/; CITED: https://docs.rs/lance/latest/lance/dataset/struct.Dataset.html]
- Map `dataset.schema()`, `dataset.version_id()`, `dataset.count_fragments()`, and `dataset.get_fragments()` into schema, version, layout, and split/fragment facts. [CITED: https://docs.rs/lance/latest/lance/dataset/struct.Dataset.html]
- Map fragment `id`, `num_data_files`, `count_rows`, `physical_rows`, and `validate` outputs into descriptive fragment diagnostics/facts where those calls succeed. [CITED: https://docs.rs/lance/latest/lance/dataset/fragment/struct.FileFragment.html]
- Treat Lance deletion vectors, indices, namespace writes, object-store options, schema metadata mutation, and SQL/DataFusion integration as unsupported or out of scope. [VERIFIED: 27-CONTEXT.md; CITED: https://docs.rs/lance/latest/lance/dataset/struct.Dataset.html]

### Parquet Adapter

- Use `ParquetRecordBatchReaderBuilder::try_new(File)` for local reads and Arrow `RecordBatch` oracle output. [CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_reader/type.ParquetRecordBatchReaderBuilder.html]
- Use builder metadata and `ParquetMetaData` methods to expose file schema, row-group count, row-group row counts, column counts, optional statistics presence, and optional page-index presence as facts. [CITED: https://docs.rs/parquet/58.3.0/parquet/; CITED: https://docs.rs/parquet/58.3.0/parquet/file/metadata/struct.ParquetMetaData.html]
- Use `ArrowWriter` only for deterministic current-version fixtures and rewrite proof output; do not use Parquet writer internals or footer embedding. [CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_writer/struct.ArrowWriter.html; VERIFIED: 27-CONTEXT.md]
- Treat nested schemas, nullable fields, decimal/date/timestamp semantics, dictionary-encoded semantic preservation, compression beyond generated uncompressed fixtures, page-level decoding, predicate filtering, and async/object-store readers as unsupported for Phase 27. [VERIFIED: user request; CITED: https://docs.rs/parquet/58.3.0/parquet/]

## Fixture and Legacy Readability Strategy

- Generate current-version fixtures in tests from deterministic Arrow `RecordBatch` values: one single-column fixture and one two-column simple table fixture per source family. [VERIFIED: docs.rs/lance; VERIFIED: docs.rs/parquet; VERIFIED: crates/loom-core/src/table_codec.rs]
- Use exact row sets such as `Int32 [7, -1, 42]`, `Int64 [10, 20, 30]`, `Float32 [1.25, -2.5, 3.75]`, and `Float64 [1.5, 2.5, 3.5]`, with every Arrow `Field` marked non-nullable. [ASSUMED]
- For Parquet current fixtures, write uncompressed local files with `ArrowWriter`, then read them back with `ParquetRecordBatchReaderBuilder` before Loom emission. [CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_writer/struct.ArrowWriter.html; CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_reader/type.ParquetRecordBatchReaderBuilder.html]
- For Lance current fixtures, write local temporary datasets with `Dataset::write` and read them back through `Dataset::open` plus scanner stream collection. [CITED: https://docs.rs/lance/latest/lance/]
- For legacy readability, commit tiny license-safe fixture directories/files under adapter test fixture paths with a manifest text file that records source format family, generator crate/version, schema, rows, expected row fixture, and the paired Loom artifact bytes generated from those rows. [VERIFIED: 27-CONTEXT.md; ASSUMED]
- Legacy tests should always verify and decode the paired Loom artifact without invoking the legacy source reader, then rewrite decoded rows into a current Lance dataset or Parquet file and read that current file back through the current source reader. [VERIFIED: 27-CONTEXT.md; VERIFIED: crates/loom-core/src/artifact_verifier.rs; CITED: https://docs.rs/lance/latest/lance/; CITED: https://docs.rs/parquet/58.3.0/parquet/]
- If a legacy source fixture is still readable by the current source SDK, the test should also run the adapter and oracle path; if it is not readable, the test should record that source-reader drift in `27-ARCHIVAL-READABILITY-REPORT.md` while preserving the Loom paired-artifact proof. [VERIFIED: 27-CONTEXT.md; ASSUMED]

## Oracle/Equivalence Strategy

- Accepted Lance reports should use `SourceOracleStrategy::SourceNativeScan` because Lance owns the dataset scan path and returns Arrow `RecordBatch` values. [CITED: https://docs.rs/lance/latest/lance/; VERIFIED: crates/loom-source-ingress/src/lib.rs]
- Accepted Parquet reports should use `SourceOracleStrategy::ArrowScan` because the official Parquet crate exposes Arrow `RecordBatch` reading through `ParquetRecordBatchReaderBuilder`. [CITED: https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_reader/type.ParquetRecordBatchReaderBuilder.html; VERIFIED: crates/loom-source-ingress/src/lib.rs]
- Equivalence should compare decoded Loom arrays/tables to oracle `RecordBatch` values by exact type, row count, field name, null count equal to zero, and element values. [VERIFIED: crates/loom-vortex-ingress/tests/source_ingress_handoff.rs; VERIFIED: crates/loom-core/src/container_codec.rs]
- Float comparisons should be exact for deterministic fixture values that have exact binary representation where possible; if non-exact decimal values are used, tests should switch to bitwise Arrow buffer comparison or documented epsilon comparison. [ASSUMED]
- Oracle evidence should set `row_count_checked`, `nulls_checked = true`, and notes stating that source-native/Arrow scan is evidence only and not the Loom decode path. [VERIFIED: crates/loom-vortex-ingress/src/source_contract.rs]

## Testing/Gates

- Add focused tests for `loom-lance-ingress`: accepted non-null primitive single column, accepted simple table, unsupported nullable primitive, unsupported string/nested shape, malformed/non-dataset rejected path, and legacy paired-artifact readability. [VERIFIED: 27-CONTEXT.md; VERIFIED: crates/loom-vortex-ingress/tests/source_ingress_handoff.rs]
- Add focused tests for `loom-parquet-ingress`: accepted non-null primitive single column, accepted simple table, unsupported nullable/nested/logical shape, malformed file rejected path, row-group fact extraction, and legacy paired-artifact readability. [VERIFIED: 27-CONTEXT.md; CITED: https://docs.rs/parquet/58.3.0/parquet/]
- Add `scripts/lance-parquet-ingress-test.sh` that runs both adapter test suites, `loom-core --test artifact_verifier`, dependency-boundary guards, source-feature guards, public API creep guards, and final report marker checks. [VERIFIED: scripts/source-ingress-contract-test.sh; VERIFIED: 27-CONTEXT.md]
- Wire the Phase 27 gate into `scripts/mvp0-verify.sh` after `scripts/source-ingress-contract-test.sh` and before `scripts/duckdb-smoke-test.sh`. [VERIFIED: scripts/mvp0-verify.sh; VERIFIED: 26-SOURCE-INGRESS-REPORT.md]
- Dependency guards should fail if `lance`, `parquet`, cloud provider crates, object credential markers, source route SQL markers, predicate/pushdown markers, parallel split markers, or new native-kernel markers appear outside the intended adapter crates/tests. [VERIFIED: scripts/source-ingress-contract-test.sh; VERIFIED: 27-CONTEXT.md]
- The final report should be `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-ARCHIVAL-READABILITY-REPORT.md` and should include supported/unsupported/rejected matrices, dependency tradeoffs, legacy fixture disposition, oracle evidence, verifier evidence, and Phase 28 handoff. [VERIFIED: 27-CONTEXT.md]

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---:|---|---|
| Rust compiler | Adapter crates and tests | yes | `rustc 1.92.0` | none needed; Lance requires Rust 1.91.0 [VERIFIED: rustc --version; VERIFIED: cargo info lance] |
| Cargo | Dependency resolution and tests | yes | `cargo 1.92.0` | none needed [VERIFIED: cargo --version] |
| slopcheck | Package legitimacy audit | yes | `0.6.1` | none needed [VERIFIED: slopcheck --version] |
| jq | crates.io API audit parsing | yes | `1.8.1` | manual JSON inspection [VERIFIED: jq --version] |
| Context7 CLI | Documentation lookup fallback | no | unavailable | used primary docs.rs and official docs instead [VERIFIED: command -v ctx7] |

**Missing dependencies with no fallback:** none for research. [VERIFIED: environment probes]
**Missing dependencies with fallback:** Context7 CLI is missing; primary docs.rs and official project docs were used. [VERIFIED: environment probes]

## Risks/Tradeoffs

| Risk / Tradeoff | Recommendation | Reason |
|---|---|---|
| Lance default features include cloud/provider capabilities. | Disable default features and guard public surfaces against credentials/remote IO. | The phase is local-file only and object-store credentials are out of scope. [VERIFIED: cargo info lance; VERIFIED: 27-CONTEXT.md] |
| Lance still has a normal `object_store` dependency. | Allow it only inside `loom-lance-ingress` transitive dependencies and forbid object-store API exposure. | The official crate uses it internally, but Phase 27 must not expose remote IO policy. [CITED: https://docs.rs/lance/latest/lance/; VERIFIED: 27-CONTEXT.md] |
| Parquet default features include compression and optional expansion. | Use `default-features = false, features = ["arrow"]` and generate uncompressed fixtures. | The value proof needs Arrow RecordBatch readability, not broad compression compatibility. [VERIFIED: cargo info parquet; CITED: https://docs.rs/parquet/58.3.0/parquet/] |
| Paired artifacts are less integrated than manifest/footer embedding. | Pair source files/datasets with Loom artifacts and reports in this phase. | Embedding source-format internals is explicitly deferred and would broaden compatibility risk. [VERIFIED: 27-CONTEXT.md] |
| Legacy writer tooling can become build archaeology. | Prefer checked-in tiny legacy fixtures with recorded provenance and expected rows. | This proves Loom readability without forcing old SDK toolchains into the release gate. [VERIFIED: 27-CONTEXT.md; ASSUMED] |
| Canonical raw/table emission can be mistaken for source semantic compatibility. | Name disposition `canonical-raw` or `canonical-table` and record unsupported matrices. | Phase 26 defines canonical emission as a verifier-backed bridge, not arbitrary source semantic coverage. [VERIFIED: 26-SOURCE-INGRESS-REPORT.md] |

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---|---:|---|
| V2 Authentication | no | No auth or credentials should be introduced. [VERIFIED: 27-CONTEXT.md] |
| V3 Session Management | no | No sessions should be introduced. [VERIFIED: 27-CONTEXT.md] |
| V4 Access Control | no | Local test fixtures only; no user/tenant authorization model. [VERIFIED: 27-CONTEXT.md] |
| V5 Input Validation | yes | Classify every source as accepted/unsupported/rejected and fail closed through `SourceIngressReport`. [VERIFIED: crates/loom-source-ingress/src/lib.rs] |
| V6 Cryptography | no | No signatures, encryption, or credential handling in Phase 27. [VERIFIED: 27-CONTEXT.md] |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---|---|---|
| Malformed source file causes trusted facts or partial bytes to escape. | Tampering | Rejected reports carry diagnostics only; bytes are returned only in `SourceIngressAcceptedArtifact` after verification and oracle evidence. [VERIFIED: crates/loom-source-ingress/src/lib.rs; VERIFIED: crates/loom-vortex-ingress/src/source_contract.rs] |
| Source SDK dependency leaks into core/FFI/public API. | Elevation of privilege / Tampering | Add cargo-tree and grep guards patterned after `source-ingress-contract-test.sh`. [VERIFIED: scripts/source-ingress-contract-test.sh] |
| Remote IO or credential settings creep in through Lance provider features. | Information disclosure | Disable Lance default features and grep for credential/storage option markers outside adapter internals. [VERIFIED: cargo info lance; VERIFIED: 27-CONTEXT.md] |
| Oracle path becomes the decode path. | Tampering | Keep oracle evidence separate from Loom artifact verification and decode. [VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md] |

## Proposed Plan Inputs

1. **Adapter crate scaffolding:** Add `loom-lance-ingress` and `loom-parquet-ingress` workspace crates with exact pins and dependency guards; no code outside adapters should gain Lance/Parquet dependencies. [VERIFIED: workspace Cargo.toml; VERIFIED: 26-SOURCE-INGRESS-CONTRACT.md]
2. **Fact extraction:** Implement source-neutral fact mapping for Lance schema/version/fragments and Parquet schema/row groups/page-adjacent metadata. [CITED: https://docs.rs/lance/latest/lance/dataset/struct.Dataset.html; CITED: https://docs.rs/parquet/58.3.0/parquet/file/metadata/struct.ParquetMetaData.html]
3. **Accepted emission:** Convert non-null primitive source batches into `LayoutDescription`/`TableDescription`, encode/wrap as `LMC1`, run `verify_artifact`, decode, and compare rows before returning accepted bytes. [VERIFIED: crates/loom-core/src/layout_codec.rs; VERIFIED: crates/loom-core/src/table_codec.rs; VERIFIED: crates/loom-core/src/artifact_verifier.rs]
4. **Unsupported/rejected matrices:** Add tests for nullable, nested/string/logical, malformed, verifier-failed, and oracle-failed cases with no emitted bytes. [VERIFIED: crates/loom-source-ingress/tests/source_ingress_contract.rs; VERIFIED: 27-CONTEXT.md]
5. **Legacy readability:** Add checked-in tiny legacy fixture pairs and tests proving paired Loom artifacts remain verifier-readable and rewriteable to current Lance/Parquet output. [VERIFIED: 27-CONTEXT.md; ASSUMED]
6. **Release gate/report:** Add `scripts/lance-parquet-ingress-test.sh`, wire it into `mvp0-verify.sh`, and write `27-ARCHIVAL-READABILITY-REPORT.md`. [VERIFIED: scripts/mvp0-verify.sh; VERIFIED: 27-CONTEXT.md]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|---|---|---|
| A1 | The exact deterministic fixture values listed are sufficient and ergonomic for all adapter row-equality tests. | Fixture and Legacy Readability Strategy | Tests may need adjusted values for float exactness or clearer edge coverage. |
| A2 | Checked-in tiny legacy fixtures are acceptable and license-safe once generated with recorded provenance. | Fixture and Legacy Readability Strategy | Planner may need a fixture-generation task or user confirmation if binary fixture policy is stricter. |
| A3 | Current SDK failure to read an older source fixture can still count as archival-reader drift if the paired Loom artifact remains readable and rewriteable. | Fixture and Legacy Readability Strategy | Acceptance criteria may need tightening if the user requires current SDK readability for every legacy fixture. |
| A4 | Bitwise or epsilon float comparison can be chosen during implementation if exact fixture float values are not used. | Oracle/Equivalence Strategy | Floating tests may be flaky or over-permissive without a locked comparison rule. |

## Open Questions (RESOLVED)

1. **Which legacy source versions should be represented?**  
   What we know: Phase 27 requires a legacy-file-with-Loom proof where feasible. [VERIFIED: 27-CONTEXT.md]  
   What's unclear: The user has not named exact Lance or Parquet historical versions. [VERIFIED: 27-CONTEXT.md]  
   RESOLVED default: Use one tiny checked-in fixture per family generated by the oldest version that can be produced without remote services or brittle build steps during implementation; record exact generator versions in the final report. If no stable old writer path is feasible, use a paired legacy-style fixture manifest and explicitly record the gap. [ASSUMED]

2. **Should Lance adapter APIs be async-only or expose a test-only sync wrapper?**  
   What we know: Lance `Dataset::open`, `Dataset::write`, scanner stream collection, and fragment row counts are async. [CITED: https://docs.rs/lance/latest/lance/; CITED: https://docs.rs/lance/latest/lance/dataset/fragment/struct.FileFragment.html]  
   What's unclear: No existing generic source-ingress trait requires async or sync adapter functions. [VERIFIED: crates/loom-source-ingress/src/lib.rs]  
   RESOLVED default: Keep Lance adapter functions async and use `#[tokio::test]` in tests; do not create a public blocking wrapper. [CITED: https://docs.rs/tokio/latest/tokio/; ASSUMED]

## Sources

### Primary (HIGH confidence)

- `./AGENTS.md` - project dependency boundaries and GSD workflow constraints. [VERIFIED: codebase read]
- `.planning/phases/27-lance-parquet-archival-readability-dataset-ingress/27-CONTEXT.md` - locked Phase 27 decisions, discretion, and deferred scope. [VERIFIED: codebase read]
- `.planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-CONTRACT.md` - normative source-ingress contract. [VERIFIED: codebase read]
- `.planning/phases/26-external-source-ingress-contract/26-SOURCE-INGRESS-REPORT.md` - Phase 26 handoff and gate evidence. [VERIFIED: codebase read]
- `crates/loom-source-ingress/src/lib.rs` - source report vocabulary and invariants. [VERIFIED: codebase read]
- `crates/loom-vortex-ingress/src/source_contract.rs` - reference adapter mapping and verifier/oracle handoff pattern. [VERIFIED: codebase read]
- `crates/loom-core/src/container_codec.rs`, `layout_codec.rs`, `table_codec.rs`, `artifact_verifier.rs` - artifact emission and verifier APIs. [VERIFIED: codebase read]
- `https://docs.rs/lance/latest/lance/` - Lance crate version, dependencies, write/scan examples. [CITED]
- `https://docs.rs/lance/latest/lance/dataset/struct.Dataset.html` - Lance dataset API facts. [CITED]
- `https://docs.rs/lance/latest/lance/dataset/fragment/struct.FileFragment.html` - Lance fragment API facts. [CITED]
- `https://docs.rs/parquet/58.3.0/parquet/` - Parquet crate version, feature/API overview. [CITED]
- `https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_reader/type.ParquetRecordBatchReaderBuilder.html` - Parquet Arrow reader API. [CITED]
- `https://docs.rs/parquet/58.3.0/parquet/arrow/arrow_writer/struct.ArrowWriter.html` - Parquet Arrow writer API. [CITED]
- `https://docs.rs/parquet/58.3.0/parquet/file/metadata/struct.ParquetMetaData.html` - Parquet metadata API. [CITED]

### Registry and Tool Verification

- `cargo search lance`, `cargo info lance` - `lance 7.0.0`, Rust version, features, repository. [VERIFIED: crates.io]
- `cargo search parquet`, `cargo info parquet` - `parquet 58.3.0`, Arrow feature/dependencies, repository. [VERIFIED: crates.io]
- `cargo info tokio`, `cargo info futures`, `cargo info tempfile` - supporting package versions and features. [VERIFIED: crates.io]
- `slopcheck install lance parquet tokio futures tempfile` - all packages returned `[OK]`; command then failed to run `cargo add` because no workspace package was specified, leaving no target package modification. [VERIFIED: slopcheck; VERIFIED: git status]
- crates.io API for `lance`, `parquet`, `tokio`, `futures`, and `tempfile` - package age, download, and repository metadata. [VERIFIED: crates.io API]

### Tertiary (LOW confidence)

- Assumptions listed in the Assumptions Log. [ASSUMED]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - versions, features, and legitimacy were checked against docs.rs, crates.io, cargo, and slopcheck. [VERIFIED: docs.rs; VERIFIED: crates.io; VERIFIED: slopcheck]
- Architecture: HIGH - adapter boundaries and report invariants are dictated by Phase 26 code/docs and existing Vortex adapter tests. [VERIFIED: codebase read]
- Pitfalls: MEDIUM - dependency and API creep risks are well evidenced, while legacy fixture version choice remains open until implementation selects concrete fixtures. [VERIFIED: codebase read; ASSUMED]

**Research date:** 2026-06-09 [VERIFIED: system date]
**Valid until:** 2026-07-09 for dependency/API pinning; re-check sooner if Lance releases a new major/minor version before implementation. [ASSUMED]
