# Phase 17 Research: Unified Artifact Verification Pipeline

**Status:** Research report
**Date:** 2026-06-08
**Phase:** 17 - Unified Artifact Verification Pipeline
**Depends on:** Phase 11 distribution container, Phase 13 full-verifier foundation, Phase 14/16 verifier-gated lowering/backend evidence, Phase 15 real Vortex ingress

## Executive Summary

Phase 17 should turn Loom's current parallel verifier lines into one
artifact-facing, fail-closed verification pipeline.

Recommended first slice:

```text
LMC1 artifact bytes
  -> container decode and section manifest
  -> schema/features/kernel manifest facts
  -> L1 structural verification
  -> optional L2Core program association
  -> L2Core verification when present
  -> constraint/proof/facts fusion
  -> ArtifactVerificationReport
  -> lowering readiness decision
```

The key design choice is to make native lowering and future engine integration
consume one accepted artifact report instead of manually pairing:

- `verifier.rs` structural `VerificationReport`,
- `full_verifier.rs` `FullVerificationReport`,
- ad hoc `VerifiedArtifactFacts`,
- backend-specific support checks.

Phase 17 should not become a solver phase, production MLIR dialect phase, full
Vortex reader phase, or proof-completion phase. It should define the unified
report, connect the existing checkers, expose explicit stage outcomes, and make
unsupported/missing stages fail closed before lowering.

## Local Starting Point

### Implemented structural verifier

`crates/loom-core/src/verifier.rs` verifies implemented `LMP1`, `LMT1`, and
`LMC1` structural payloads:

- `verify_layout(desc, registry) -> VerificationReport`
- `verify_table(table, registry) -> VerificationReport`
- `verify_container(bytes, registry) -> VerificationReport`
- stable path-addressed diagnostics via `VerificationCode`

Its stated boundary is structural verification before decode. It deliberately
leaves value-dependent checks that require materialization to typed decode
errors.

### Implemented L2Core verifier foundation

`crates/loom-core/src/full_verifier.rs` verifies the Phase 13 `L2Core` model:

- `verify_l2_core(program) -> FullVerificationReport`
- stable diagnostics via `FullVerificationCode`
- abstract-state walking over capabilities, loops, scalar variables, builder
  events, resource budgets, and local constraints
- proof-obligation traces and SMT-LIB comment emission
- `VerifiedArtifactFacts` only for accepted programs

`crates/loom-core/src/l2_core.rs` currently defines the model in Rust. There is
not yet a stable external `L2Core` artifact codec/parser that an arbitrary
producer can submit.

### Implemented lowering gate

`crates/loom-core/src/native_lowering.rs` already enforces a verifier-gated
backend precondition:

- lowering requires an accepted `FullVerificationReport`
- `FullVerificationReport::facts()` must be present
- unsupported programs fail closed before textual MLIR or native backend
  artifact creation
- current accepted shape is only bounded non-null Int32 copy with feature
  `l2core.copy.v0`

Phase 16 keeps `melior`/LLVM/JIT optional and isolated in
`crates/loom-native-melior`, preserving `loom-core` as the verifier/lowering
contract owner.

## External Evidence

### WebAssembly validation

The WebAssembly 3.0 spec separates declarative validation rules from an
algorithmic validation appendix. The validation algorithm is described as a
sound and complete algorithm for instruction sequences, and it can be integrated
directly into binary decoding.

Source: https://webassembly.github.io/spec/core/valid/index.html
Source: https://webassembly.github.io/spec/core/appendix/algorithm.html

Implication for Loom:

- Put as much artifact validation as possible adjacent to artifact decoding.
- Keep the normative rules separate from implementation details.
- Make the accepted artifact state a typed/reportable outcome, not a side
  effect of decode.

### Linux eBPF verifier

The Linux kernel eBPF verifier determines program safety before load. The
official docs describe a two-step process: CFG/DAG validation first, then
execution simulation over all paths while tracking register and stack state.

Source: https://docs.kernel.org/bpf/verifier.html

Implication for Loom:

- Treat future native execution as privileged enough to require a fail-closed
  verifier gate.
- Separate cheap structural checks from deeper abstract interpretation.
- Make bounded loops, pointer/range-like facts, and unreadable/unknown states
  explicit in diagnostics and facts.

### MLIR verifier and dialect invariants

MLIR exposes recursive operation verification through `mlir::verify`, and its
trait system lets dialects attach invariant hooks to operations, attributes,
and types. MLIR's language reference also supports dialect-owned types,
attributes, and versioning.

Source: https://mlir.llvm.org/doxygen/Verifier_8h.html
Source: https://mlir.llvm.org/docs/Traits/
Source: https://mlir.llvm.org/docs/LangRef/

Implication for Loom:

- Phase 17 should produce Loom-owned artifact facts before Phase 19 creates a
  production decode dialect.
- Phase 19 dialect verification should be downstream of Phase 17 artifact
  verification, not a replacement for it.
- Facts and diagnostics should be stable enough to link to MLIR verifier
  failures later.

### SMT-LIB, Z3, and cvc5

SMT-LIB defines common input/output languages, background theories, logics, and
benchmarks for SMT solvers. Z3 positions SMT solving as a component for
software analysis and verification tools. cvc5 uses SMT-LIB v2 as its primary
input language and can produce models and unsat cores.

Source: https://smt-lib.org/index.shtml
Source: https://microsoft.github.io/z3guide/docs/logic/intro/
Source: https://cvc5.github.io/docs/latest/binary/quickstart.html

Implication for Loom:

- Phase 17 should define a solver-neutral discharge boundary and report status.
- It should not require Z3/cvc5 yet, but it should make future solver results
  first-class: `not_required`, `collected_only`, `discharged`, `failed`,
  `unknown`, or `skipped`.
- Constraint IDs emitted by `VerifiedArtifactFacts` need stable identities so
  future solver evidence can attach without changing report shape.

### Souffle / Datalog

Souffle documents Datalog as a declarative logic-based query language with
applications including program analysis and security. It supports recursive
relations and static typing for relation attributes.

Source: https://souffle-lang.github.io/tutorial

Implication for Loom:

- Datalog could later express cross-section artifact facts and feature-policy
  closure rules declaratively.
- It is not the right Phase 17 runtime dependency. The immediate need is a
  Rust-owned report model and fail-closed pipeline, not a second rule engine.

### Proof-carrying code lineage

Proof-carrying-code work is relevant as a long-term artifact model: producers
may eventually ship proof/certificate evidence that a small checker validates.

Source: https://people.eecs.berkeley.edu/~necula/Papers/pcc-popl97.pdf

Implication for Loom:

- Keep proof-obligation IDs, facts, and verifier decisions stable enough to
  support future certificates.
- Do not make Phase 17 depend on producer-supplied proofs; current trust should
  remain in Loom's verifier and runtime guards.

## Recommended Phase 17 Scope

### In Scope

- Add a unified artifact verifier contract and report type.
- Normalize diagnostics from structural verification, L2Core verification,
  constraint collection, facts fusion, and lowering readiness.
- Verify `LMC1` container shape and extract section facts:
  - version,
  - required/optional features,
  - section list,
  - payload kind,
  - schema/kernel manifest presence,
  - structural payload report.
- Reuse existing `verify_container`, `verify_layout`, and `verify_table` rather
  than duplicating L1 checks.
- Reuse existing `verify_l2_core` when a `L2CoreProgram` is available through a
  Phase 17 adapter.
- Emit one artifact-facing facts object that can contain:
  - container facts,
  - L1 payload facts,
  - L2Core `VerifiedArtifactFacts`,
  - constraint/proof IDs,
  - lowering readiness.
- Make lowering readiness an explicit predicate over the unified report.
- Add tests proving fail-closed behavior for:
  - malformed container,
  - unknown required feature,
  - structural verifier rejection,
  - missing required L2Core section for a lowering-required artifact,
  - rejected L2Core program,
  - missing facts,
  - unsupported-but-valid artifact.
- Add CLI/release gate visibility for the unified report.

### Out of Scope

- Real SMT solver discharge.
- Stable external `L2Core` binary/text codec beyond the adapter required for
  local tests.
- Full Vortex reader expansion.
- Production MLIR decode dialect and native kernel expansion.
- DuckDB native execution integration.
- Complete Lean/Rocq soundness proof.
- Replacing runtime semantic guards and oracle/equivalence tests.

## Proposed Data Model

Recommended new module:

```text
crates/loom-core/src/artifact_verifier.rs
```

Core types:

```rust
pub enum ArtifactVerificationStage {
    Container,
    Manifest,
    L1Structural,
    L2Core,
    ConstraintDischarge,
    Facts,
    LoweringReadiness,
}

pub enum ArtifactVerificationStatus {
    Accepted,
    Rejected,
    Unsupported,
}

pub enum ConstraintDischargeStatus {
    NotRequired,
    CollectedOnly,
    Discharged,
    Failed,
    Unknown,
    Skipped,
}

pub struct ArtifactVerificationDiagnostic {
    pub stage: ArtifactVerificationStage,
    pub code: String,
    pub path: String,
    pub message: String,
}

pub struct ArtifactVerificationFacts {
    pub artifact_kind: String,
    pub container_version: Option<u16>,
    pub required_features: Vec<String>,
    pub optional_features: Vec<String>,
    pub payload_kind: Option<String>,
    pub row_count_bound: Option<u64>,
    pub l2_core: Option<VerifiedArtifactFacts>,
    pub constraint_status: ConstraintDischargeStatus,
    pub lowering_ready: bool,
}

pub struct ArtifactVerificationReport {
    pub status: ArtifactVerificationStatus,
    pub diagnostics: Vec<ArtifactVerificationDiagnostic>,
    pub facts: Option<ArtifactVerificationFacts>,
}
```

Important API rule:

```text
ArtifactVerificationReport::facts() is Some only when status == Accepted.
lowering_ready can be true only when facts are present and all required stages
for the requested backend are accepted.
```

## Proposed Pipeline

### Stage 1: Container decode

Input: artifact bytes.

Action:

- call `decode_container` for `LMC1`;
- reject malformed container with stage `Container`;
- record version/features/sections;
- do not silently accept unknown required features.

### Stage 2: Manifest extraction

Input: decoded container sections.

Action:

- identify payload kind (`LMP1`, `LMT1`, future L2Core);
- extract schema/kernel manifest presence as facts;
- record unsupported combinations as `Unsupported`, not accepted.

### Stage 3: L1 structural verification

Input: current payload sections.

Action:

- call `verify_container(bytes, registry)` for existing L1 payloads;
- map `VerificationDiagnostic` into artifact diagnostics with stable stage and
  path prefix;
- reject on any structural diagnostic.

### Stage 4: L2Core verification

Input: optional `L2CoreProgram` adapter or future section.

Action:

- call `verify_l2_core(program)` when an L2Core program is present;
- map `FullVerificationDiagnostic` into artifact diagnostics;
- propagate proof obligations, constraint IDs, and `VerifiedArtifactFacts`;
- if lowering is requested and no L2Core facts exist, reject/unsupported
  fail-closed.

### Stage 5: Constraint discharge status

Input: collected constraint IDs/comments.

Action:

- Phase 17 marks current constraints as `CollectedOnly` unless no constraints
  are required;
- do not claim solver discharge;
- preserve stable IDs for Phase 18+ solver work.

### Stage 6: Facts fusion

Input: container facts, L1 facts, L2Core facts.

Action:

- produce `ArtifactVerificationFacts` only when all required stages accept;
- keep L2 `VerifiedArtifactFacts` verifier-tied, not independently trusted;
- include row-count bounds and feature set used by lowering support checks.

### Stage 7: Lowering readiness

Input: accepted artifact report and backend target.

Action:

- compute `lowering_ready` via backend-specific support predicates;
- for the current native path, require the same conditions as Phase 14/16:
  accepted L2Core report, present `VerifiedArtifactFacts`, supported bounded
  Int32 copy, and no unsupported optional features;
- unsupported programs are valid artifacts only if they are not being lowered.

## Layering Contract

Phase 17 reports should explicitly separate:

| Layer | Purpose | Phase 17 behavior |
|---|---|---|
| Static structural verifier | Container/payload shape and feature support | Required for `LMC1`/`LMP1`/`LMT1` artifacts |
| Static L2Core verifier | Type/effect/capability/resource safety | Required when a lowering-target L2Core program is present |
| Constraint discharge | Arithmetic/range/overflow/progress obligations | Record as collected/not required; real SMT deferred |
| Runtime semantic guard | Value-dependent checks that require materialization | Keep in decode/runtime; report as deferred guard layer |
| Oracle/equivalence evidence | Compare native/interpreter/Vortex outputs | Deferred to native hardening phases |
| Lowering readiness | Whether a backend may create artifacts | Explicit fail-closed predicate over accepted report |

This layering is the answer to the current shortfall: structural verification,
runtime guards, and oracle evidence are complementary, not interchangeable.

## Recommended Plan Split

### 17-01: Contract and report model

- Add `artifact_verifier` module with stage/status/diagnostic/facts types.
- Add report invariants and unit tests.
- Document which facts are trusted and when they may be absent.

### 17-02: Container and L1 structural pipeline

- Implement `verify_artifact_container(bytes, registry)`.
- Map existing `verify_container` diagnostics into artifact diagnostics.
- Add negative tests for malformed/unknown/structurally invalid artifacts.

### 17-03: L2Core adapter and facts fusion

- Add an explicit adapter API for verifying a decoded/associated
  `L2CoreProgram` alongside the artifact report.
- Fuse `VerifiedArtifactFacts` into `ArtifactVerificationFacts`.
- Preserve constraint/proof IDs and `ConstraintDischargeStatus`.

### 17-04: Lowering-readiness gate and CLI visibility

- Add backend readiness checks without creating MLIR/JIT artifacts.
- Add CLI output for accepted/rejected/unsupported artifact reports.
- Add release gate script covering fail-closed cases.

### 17-05: Docs and closeout

- Update README/roadmap/state with the unified verifier contract.
- Add final report and requirement closure.
- Run workspace and release gates.

## Acceptance Criteria

- There is one public artifact-verifier entrypoint for `LMC1` bytes.
- The entrypoint returns a structured `ArtifactVerificationReport`.
- Reports are fail-closed: rejected/unsupported artifacts never expose trusted
  accepted facts.
- Existing `verify_container` and `verify_l2_core` are reused, not forked.
- `VerifiedArtifactFacts` are reachable through the artifact report only after
  L2Core acceptance.
- Lowering readiness is explicit and false by default.
- Unsupported valid artifacts are distinguishable from invalid artifacts.
- CLI/release gates can show why an artifact is not lowering-ready.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Phase 17 grows into full solver integration | Keep solver status enum but defer Z3/cvc5 implementation |
| L2Core has no stable artifact codec yet | Use explicit adapter now; schedule codec/parser later |
| Facts become trusted out of context | Tie facts to accepted report status and artifact identity |
| Lowering support duplicates verifier logic | Make lowering readiness call existing support predicates and report stage outcome |
| Runtime guards are mistaken for verifier gaps | Document guard layer and keep typed decode errors intact |
| Production MLIR work disappears | Keep it explicitly in Phase 19 after verifier and complete-reader constraints |

## Recommendation

Proceed with Phase 17 as a verifier-unification phase. The first implementation
should be small but architectural:

```text
verify_artifact(bytes, registry, options) -> ArtifactVerificationReport
```

It should connect the implemented L1 structural verifier and Phase 13 L2Core
verifier foundation, expose a stable report/facts shape, and make lowering
readiness consume the unified report. This is the right blocker before Phase 18
complete Vortex reader and Phase 19 production MLIR/native kernel expansion.
