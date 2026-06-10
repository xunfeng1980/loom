# Phase 17 Artifact Verifier Report

**Status:** Complete
**Date:** 2026-06-08

## Scope

Phase 17 unifies the current `LMC1` / `LMP1` / `LMT1` structural verifier line
and the future `L2Core` verifier foundation into one artifact-facing report
pipeline. The deliverable is an executable verifier surface and release-gated
evidence, not a complete solver-backed formal verifier.

The shipped API is centered on:

- `verify_artifact` for `LMC1` container, manifest, and L1 structural facts.
- `verify_artifact_with_l2_core` for optional `L2Core` verification, facts
  fusion, constraint status, and lowering readiness.

## Unified Pipeline

The unified report follows this fail-closed order:

```text
LMC1 artifact bytes
  -> container shape/version/section checks
  -> feature, schema, kernel manifest, and payload-kind facts
  -> L1 structural verification through the existing payload verifier
  -> optional L2Core verification through the full-verifier foundation
  -> constraint/facts collection
  -> VerifiedArtifactFacts fusion
  -> lowering-readiness decision
```

Malformed containers, unsupported required features, invalid payloads, rejected
`L2Core` programs, and unsupported lowering targets all stop before successful
output or native artifact creation.

## Report and Facts Model

`loom_core::artifact_verifier` adds stable report types for artifact-facing
verification:

- `ArtifactVerificationReport`
- `ArtifactVerificationStatus`
- `ArtifactVerificationStage`
- `ArtifactVerificationDiagnostic`
- `ArtifactVerificationFacts`
- `ConstraintDischargeStatus`
- `ArtifactLoweringReadiness`

Accepted reports are the only reports that carry facts. Rejected and unsupported
reports carry diagnostics and no facts. This keeps `VerifiedArtifactFacts`
inside the accepted report boundary instead of treating copied facts as
standalone trust tokens.

## Container and L1 Verification

`verify_artifact` starts from raw artifact bytes and requires a valid `LMC1`
container. It records container version, required/optional features, section
presence, and payload kind, then delegates structural layout/table validation to
the existing verifier. Structural diagnostics are normalized into artifact
pipeline stages.

The current supported payload kinds are:

- `LMP1 layout`
- `LMT1 table`

Unsupported or malformed artifacts fail closed with deterministic diagnostics.

## L2Core Adapter

`verify_artifact_with_l2_core` composes a valid artifact report with a supplied
`L2CoreProgram`. The adapter runs `verify_l2_core`, preserves rejected-program
diagnostics, and fuses accepted `VerifiedArtifactFacts` into
`ArtifactVerificationFacts`.

This remains an internal Rust data-model integration. A stable external
`L2Core` artifact codec/parser is deferred.

## Constraint Status

Phase 17 records solver-neutral constraint status. Current accepted reports use:

- `not-required` when no obligations are present.
- `collected-only` when proof obligations are collected from `L2Core`.

Real SMT discharge is deferred. Phase 17 does not add Z3, cvc5, SMT-LIB process
execution, or an equivalent solver backend.

## Lowering Readiness

Lowering readiness is reported separately from artifact acceptance. A valid
artifact can be accepted while `lowering_ready` remains false.

Readiness can become true only when the unified report has accepted artifact
facts, accepted `L2Core` facts, and the existing bounded Int32 copy support
predicate accepts the program for the requested backend. Unsupported programs
fail closed before MLIR, LLVM, JIT, or native artifacts are created.

## CLI and Release Gate

The CLI now exposes:

```text
loom verify-artifact <payload>
```

The command prints a compact reviewer-facing status, facts summary, lowering
readiness, and diagnostics. `scripts/artifact-verifier-test.sh` checks the
contract docs, focused artifact verifier tests, CLI visibility, accepted
fixture reporting, and malformed-container rejection. `scripts/mvp0-verify.sh`
runs the Phase 17 gate as part of the repository release gate.

## Commands Run

- `cargo test --workspace`
- `cargo test -p loom-core --test artifact_verifier`
- `cargo run --bin loom -- --help | rg -q "verify-artifact"`
- `bash scripts/artifact-verifier-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

Phase 16's optional `melior`/LLVM/JIT evidence remains skip-aware in normal
release gates when the local LLVM/MLIR major version is incompatible. That skip
is recorded as optional backend evidence, not as production native compiler
support.

## Deferred Work

Deferred beyond Phase 17:

- real SMT discharge for symbolic offset/range/overflow obligations;
- stable external `L2Core` artifact codec/parser;
- deeper value-dependent semantic checks beyond static verification and runtime
  guards;
- publishable Lean/Rocq proof depth;
- complete Vortex reader support, owned by Phase 18;
- production MLIR decode dialect, Arrow/raw-buffer native writes, vectorization,
  and broad native kernel expansion, owned by Phase 19;
- host-engine native execution, owned by later runtime/integration phases.

## Requirement Closure

Phase 17 closes the verifier-pipeline gap identified after Phase 16: Loom now
has one artifact-facing path from `LMC1` bytes through L1 structural checks,
optional `L2Core` verification, facts, constraint status, and lowering
readiness. It does not claim full production verification, complete solver
discharge, complete Vortex file coverage, or production native execution.
