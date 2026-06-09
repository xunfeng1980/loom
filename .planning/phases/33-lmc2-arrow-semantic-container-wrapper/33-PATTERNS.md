# Phase 33: LMC2 Arrow Semantic Container Wrapper - Patterns

**Gathered:** 2026-06-09
**Status:** Complete

## Pattern Map

### Core Codec Pattern

**Target files:**

- `crates/loom-core/src/arrow_semantic_codec.rs`
- `crates/loom-core/tests/arrow_semantic.rs`

**Closest analogs:**

- `crates/loom-core/src/arrow_semantic_codec.rs` direct `LMA1` codec.
- `crates/loom-core/src/container_codec.rs` `LMC1` section/header parser.

**Pattern to follow:**

- Use fixed magic + little-endian version fields.
- Verify payloads before encoding and after decoding.
- Return `LoomDecodeError::MalformedLayoutPayload` or
  `LoomDecodeError::MalformedContainer` with stable short reason strings.
- Reject trailing bytes, truncated reads, length overflows, and unsupported
  versions in the codec, not in downstream callers.

### Artifact Verifier Pattern

**Target file:**

- `crates/loom-core/src/artifact_verifier.rs`

**Closest analogs:**

- Direct `verify_arrow_semantic_artifact` branch for `LMA1`.
- `LMC1` `verify_container` branch for wrapper metadata/facts.

**Pattern to follow:**

- Detect special artifact magic before legacy container fallback.
- Accepted reports must expose `ArtifactVerificationFacts`.
- Unsupported/rejected reports must include stage, code, path, and message.
- Lowering readiness is computed only when requested and must remain explicit.

### Source Adapter Pattern

**Target files:**

- `ingress/loom-parquet-ingress/src/source_contract.rs`
- `ingress/loom-lance-ingress/src/source_contract.rs`
- `ingress/loom-vortex-ingress/src/source_contract.rs`

**Closest analogs:**

- Existing `loom_artifact_from_batches` helpers for Parquet and Lance.
- Existing inline Vortex `ArrowSemanticPayload` construction.

**Pattern to follow:**

- Materialize source as Arrow first.
- Convert `RecordBatch` to `ArrowSemanticPayload`.
- Encode Loom bytes.
- Run `verify_artifact` immediately.
- Build source report from accepted verifier facts and separate oracle evidence.

### Gate Pattern

**Target files:**

- `scripts/full-arrow-semantic-compatibility-test.sh`
- `scripts/duckdb-source-e2e-test.sh`
- new `scripts/lmc2-arrow-semantic-container-test.sh`
- `scripts/mvp0-verify.sh`
- optionally `scripts/mvp1-verify.sh`

**Closest analogs:**

- `scripts/full-arrow-semantic-compatibility-test.sh` focused semantic gate.
- `scripts/container-negative-test.sh` malformed container gate.

**Pattern to follow:**

- Focused gate first; broad release wiring last.
- Use marker checks to prevent accidental API/path drift.
- Run targeted cargo tests before expensive DuckDB e2e.
- Keep assertions concrete: expected magic bytes, exact test names, and report
  marker strings.

### Documentation Pattern

**Target files:**

- `README.md`
- `README-zh.md`
- `.planning/PROJECT.md`
- `.planning/REQUIREMENTS.md`
- `.planning/ROADMAP.md`
- `.planning/STATE.md`
- `.planning/phases/33-lmc2-arrow-semantic-container-wrapper/33-LMC2-REPORT.md`

**Closest analogs:**

- Phase 31 compatibility report.
- Phase 32 release readiness and claim-truth wording.

**Pattern to follow:**

- State what is proven, bounded, unsupported, and deferred.
- Keep direct `LMA1` as bridge wording.
- Do not imply broad DuckDB SQL or native execution support.

