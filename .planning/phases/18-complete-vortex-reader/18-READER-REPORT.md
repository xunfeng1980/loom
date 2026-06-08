# Phase 18 Reader Report

## Scope

Phase 18 expands the Phase 15 real-ingress slice into a Loom-owned complete-reader boundary for real Vortex files. The phase owns reader facts, support classification, supported artifact emission, CLI visibility, oracle evidence, release-gate coverage, and the handoff into Phase 19.

This phase does not claim arbitrary native decode of every Vortex encoding, solver-backed proof discharge, production MLIR/native lowering, host-engine native execution, object-store ingress, signatures, attestation, or correctness beyond oracle/equivalence evidence for emitted fixtures.

## Complete Reader Boundary

The complete-reader boundary means Loom can open a real Vortex file through the isolated `loom-vortex-ingress` crate, summarize its file/schema/layout/segment/statistics facts, classify the input as accepted, unsupported, or rejected, and fail closed before emitting any partial artifact.

Accepted emission is intentionally finite:

- Non-null single-column primitive arrays: Int32, Int64, Float32, Float64.
- Non-null root struct/table arrays whose fields are all supported primitive single-column arrays.
- Emitted artifacts are wrapped as `LMC1` and use `LMP1` for single-column payloads or `LMT1` for table payloads.

Unsupported valid files remain inspectable through reader facts, but emit no Loom artifact. Malformed files are rejected.

## Facts Model

Phase 18 adds stable Loom-owned facts and diagnostics in `loom-vortex-ingress`:

- `VortexReaderFacts` combines source kind, row count, root dtype, support status, emission kind, recursive layout facts, dtype facts, segment facts, split facts, and diagnostics.
- `VortexReaderLayoutFact` records stable recursive layout paths and layout summaries.
- `VortexReaderDTypeFact` records primitive, nullable, and struct field shape information without leaking Vortex crate types across the boundary.
- `VortexReaderSegmentFact` records deterministic segment ranges and overlap/order information.
- `VortexReaderSplitFact` records discovered split ranges when available; split discovery failures are non-fatal diagnostics.
- Stable diagnostic codes keep malformed/rejected and unsupported/fail-closed cases distinct.

`vortex-file` and `vortex-layout` remain scoped to `crates/loom-vortex-ingress`; `loom-core` and `loom-ffi` remain Vortex-free.

## Supported Emission Matrix

| Input shape | Emission | Status |
|---|---|---|
| Non-null Int32 column | `LMC1(LMP1)` | Accepted |
| Non-null Int64 column | `LMC1(LMP1)` | Accepted |
| Non-null Float32 column | `LMC1(LMP1)` | Accepted |
| Non-null Float64 column | `LMC1(LMP1)` | Accepted |
| Non-null struct/table with supported primitive fields | `LMC1(LMT1)` | Accepted |
| UTF-8/string single column | None | Unsupported, fail-closed |
| Struct/table with unsupported field | None | Unsupported, fail-closed |
| Malformed Vortex bytes | None | Rejected, fail-closed |

This matrix is the Phase 18 accepted emission surface. Reader facts may describe more files than this matrix emits.

## Artifact Verifier Handoff

Every emitted artifact is intended to pass the Phase 17 artifact verifier:

```text
Vortex file
  -> VortexReaderFacts
  -> supported emission matrix
  -> LMC1(LMP1 or LMT1)
  -> loom verify-artifact
  -> ArtifactVerificationReport
```

`loom ingest-vortex --inspect` prints reader support, emission kind, reader fact counts, and `reader_artifact_verification: pass` for accepted emitted artifacts. `loom ingest-vortex --emit-loom` writes only supported artifacts and rejects unsupported shapes without output.

Phase 19 should consume `VortexReaderFacts`, emitted `LMC1` artifact facts, Phase 17 `ArtifactVerificationReport`, and any collected obligations that need solver discharge.

## Oracle Evidence

The tests use Vortex scan/execution as oracle evidence, not as a trusted core dependency:

- Single-column emitted artifacts are decoded and compared against typed Vortex scans for Int32, Int64, Float32, and Float64.
- Table emitted artifacts are decoded through Loom table decoding and compared against Vortex struct field scans.
- Unsupported and malformed cases prove fail-closed behavior before successful output.

This is equivalence evidence for the accepted matrix, not a correctness proof for all Vortex semantics.

## CLI and Release Gate

Reviewer-facing commands:

- `loom ingest-vortex --inspect <input.vortex>` prints reader facts, support status, emission kind, and artifact verifier status where applicable.
- `loom ingest-vortex --emit-loom <input.vortex> <output.loom>` emits only supported `LMC1` artifacts.
- `loom verify-artifact <output.loom>` verifies emitted artifacts through the Phase 17 artifact verifier.

Release-gate wiring:

- `scripts/complete-vortex-reader-test.sh` covers docs, implementation markers, accepted tests, unsupported tests, malformed tests, CLI inspect/emit, artifact verifier handoff, and dependency-boundary guards.
- `scripts/mvp0-verify.sh` invokes the Phase 18 gate after the Phase 17 artifact verifier gate.

## Commands Run

Final verification commands:

- `cargo test --workspace`
- `cargo test -p loom-vortex-ingress`
- `cargo test -p loom-core --test artifact_verifier`
- `bash scripts/complete-vortex-reader-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

Result: all commands passed on 2026-06-08. `scripts/melior-jit-test.sh`, as invoked by `scripts/mvp0-verify.sh`, reported the expected normal-mode skip for local LLVM/MLIR major 21 versus expected major 22; this is recorded as optional backend skip evidence, not production JIT success.

## Deferred Work

- Solver-backed symbolic range/offset/overflow discharge remains Phase 19.
- Stable external `L2Core` artifact codec/parser remains Phase 19 or later.
- Production MLIR decode dialect and native kernel expansion remain Phase 20.
- Host native runtime ABI and execution policy remain Phase 21.
- DuckDB native execution integration remains Phase 22.
- Native equivalence/cache/fallback hardening remains Phase 23.
- Iceberg ref/table binding and StarRocks + DuckDB dual query surface remain Phase 24 and Phase 25.

## Phase 19 Handoff

Phase 19 should start from this invariant:

```text
LMC1 artifact + VortexReaderFacts + ArtifactVerificationReport
  -> solver-backed obligation discharge
  -> trusted VerifiedArtifactFacts only when all required obligations are discharged
  -> fail-closed unknown/unsupported obligations
```

The solver-backed verifier should not widen native lowering or host execution. Its job is to replace collected obligations with discharged verifier evidence that later production native phases can trust.
