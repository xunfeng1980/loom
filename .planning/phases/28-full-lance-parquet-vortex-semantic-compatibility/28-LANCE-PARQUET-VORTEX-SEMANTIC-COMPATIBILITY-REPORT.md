# Phase 28 Lance + Parquet + Vortex Semantic Compatibility Report

## Scope

Phase 28 defines a bounded semantic compatibility matrix over existing Lance,
Parquet, and Vortex evidence. It does not add a new source reader, query
surface, public ABI, StarRocks route, Iceberg behavior, or broader Vortex
decoder. Its job is to classify current evidence honestly:

- accepted rows require oracle evidence plus verifier-accepted Loom emission;
- canonicalized rows preserve values through current raw/table artifacts but do
  not preserve original structured encoding semantics;
- unsupported and rejected rows produce no accepted bytes;
- native disposition is evidence-bearing only when production lowering and
  `native-arrow-semantic-codegen-output` evidence exist.

## Accepted Matrix

| Shape | Source family | Emitted Loom shape | Oracle | Verifier | Native disposition |
|---|---|---|---|---|---|
| Non-null primitive rows | Vortex, Lance, Parquet | `LMC1(LMP1)` canonical raw | value rows | artifact verifier accepted | production lowering supported where primitive runtime facts allow it |
| Non-null primitive table rows | Vortex, Lance, Parquet | `LMC1(LMT1)` canonical table | value rows / table rows | artifact verifier accepted | production lowering supported for current primitive table slice |
| Phase 27 archival current fixtures | Lance, Parquet | verifier-backed Loom artifacts | Arrow/source scan rows | artifact verifier accepted | source-ingress evidence only |
| Phase 27 actual older-version fixtures | Lance, Parquet | paired verifier-backed Loom artifacts | source scan rows | artifact verifier accepted | archival-readability evidence only |

Accepted means the emitted Loom artifact is verified and has an executable row
oracle. It does not imply arbitrary source-format coverage.

## Unsupported Matrix

| Shape | Current status | Reason |
|---|---|---|
| Nullable primitive values | unsupported / deferred | validity bitmap semantics are preserved as reader facts but not emitted as accepted Loom artifact rows yet |
| Strings and variable binary | unsupported / deferred | string parameter extraction and Loom-owned verified emission are not complete |
| Nested, extension, logical, or complex source schemas | unsupported | outside the current non-null primitive/table slice |
| Chunked or split-sensitive forms requiring structural preservation | unsupported or canonicalized only | current artifact emission may flatten values without proving original layout semantics |

The focused tests require `nullable-validity-emission-deferred` for nullable
primitive rows instead of silently treating them as accepted.

## Rejected Matrix

Malformed source inputs, missing identity, invalid artifacts, verifier-rejected
bytes, stale evidence, and mismatched oracle/source facts remain rejected or
fail-closed in their owning source gates. Rejected rows must not carry accepted
artifact bytes.

## Canonicalized Rows

Dictionary, run-end/RLE, sequence-like, bitpack, and frame-of-reference Vortex
rows can be accepted as canonical raw value rows only when the Vortex scan
oracle and Loom verifier evidence agree. The matrix records these rows as
canonicalized/interpreter evidence with explicit deferral reasons such as:

- `structured-dictionary-facts-deferred`
- `structured-run-end-facts-deferred`
- `structured-bitpack-facts-deferred`
- `structured-for-facts-deferred`

The validator rejects `canonical-raw-overclaim` when a canonical raw row is
marked as full structured semantics without shape-oracle evidence.

## Native Disposition

Native support is not inferred from route labels, fallback, skipped toolchains,
or raw-copy placeholders. A row may use native language only when the evidence
chain reaches the MLIR ExecutionEngine path and includes
`native-arrow-semantic-codegen-output`.

Rows marked `ExecutionEngineValidated` without that marker fail with
`native-evidence-missing`. Rows outside the current production native slice
remain interpreter-only or deferred.

## Phase 30 Tradeoff

Phase 30 remains partial. The DuckDB executable slice over Phase 29 accepted
bytes exists, but StarRocks runtime smoke, negative matrix expansion, main
release-gate wiring, and final dual-query closeout remain deferred. Phase 28
does not use Phase 30 as second-host proof.

The accepted tradeoff is to finish semantic compatibility classification before
claiming multi-engine query compatibility. This prevents later query reports
from using canonical raw or DuckDB-only evidence as a proxy for broad
source-format semantics.

## Release Gate Evidence

`scripts/vortex-semantic-compatibility-test.sh` is the focused Phase 28 gate. It
checks:

- required Phase 28 planning artifacts;
- `VortexSemanticCompatibilityRow` and native evidence model markers;
- `canonical-raw-overclaim` and `native-evidence-missing` diagnostics;
- nullable and structured deferral markers;
- focused tests for semantic matrix, nullable semantics, and structured
  encoding semantics;
- real Phase 21 Vortex coverage tests for nullable, dictionary/RLE, bitpack,
  and frame-of-reference rows;
- absence of production `native-raw-copy-output` acceptance markers;
- final report sections and no deferred Phase 30 overclaim language.

`scripts/mvp0-verify.sh` invokes the Phase 28 gate after Phase 27
Lance/Parquet ingress and before Phase 29 Iceberg binding and DuckDB smoke.

## Commands Run

- `bash -n scripts/vortex-semantic-compatibility-test.sh`
- `bash -n scripts/mvp0-verify.sh`
- `bash scripts/vortex-semantic-compatibility-test.sh`
- `RUSTC_WRAPPER= bash scripts/mvp0-verify.sh`
- `cargo test -p loom-vortex-ingress --test semantic_compatibility_matrix`
- `cargo test -p loom-vortex-ingress --test nullable_semantic_compatibility`
- `cargo test -p loom-vortex-ingress --test structured_encoding_semantics`
- `cargo test -p loom-vortex-ingress --test nullable_primitive_coverage`
- `cargo test -p loom-vortex-ingress --test dictionary_runend_coverage`
- `cargo test -p loom-vortex-ingress --test bitpack_for_coverage`

## Residual Risks

- The matrix is bounded by current Phase 21 and Phase 27 evidence; it is not a
  full Vortex, Lance, or Parquet semantic implementation.
- Nullable primitive emission remains deferred until Loom artifacts carry and
  verify validity semantics through decode and SQL output.
- Structured Vortex encodings are value-compatible through canonical raw rows
  only; original encoding-shape preservation remains deferred.
- Phase 30 dual-query proof remains partial and cannot be used as StarRocks
  evidence.

## Phase/Milestone Handoff

Phase 29 Iceberg binding may consume Phase 28 as a bounded source-family
semantic classification, not as broad source compatibility. Phase 30 must keep
using this matrix to avoid claiming second-host or structured-source semantics
from canonical raw DuckDB evidence alone.
