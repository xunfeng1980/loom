# Phase 17 Artifact Verifier Contract

**Status:** Plan 17-01 contract
**Date:** 2026-06-08
**Scope:** Unified artifact-facing verifier report model and trust boundary

## Scope

Phase 17 unifies Loom's current structural artifact verifier and future
`L2Core` verifier foundation into one report pipeline. The first contract is a
Rust-owned artifact report that later plans can fill from `LMC1`, L1
structural verification, optional `L2Core` verification, constraint status, and
lowering readiness.

The contract does not replace the existing `verifier.rs`, `full_verifier.rs`, or
`native_lowering.rs` modules. It gives later plans a single artifact-facing
surface that can reuse them without forking their logic.

## Pipeline Stages

The unified report uses explicit stages:

- `container` for `LMC1` magic/version/header/section-directory checks.
- `manifest` for feature, schema, kernel manifest, and payload-kind facts.
- `l1-structural` for current `verify_container`, `verify_layout`, and
  `verify_table` diagnostics.
- `l2core` for `verify_l2_core` diagnostics and accepted-program facts.
- `constraint-discharge` for collected or future solver-backed obligations.
- `facts` for report/facts fusion invariants.
- `lowering-readiness` for backend support decisions before MLIR/native artifact
  creation.

## Report Invariants

An `ArtifactVerificationReport` has one status:

- `accepted`
- `rejected`
- `unsupported`

Facts are emitted only for accepted reports. Rejected and unsupported reports
must return `None` from `ArtifactVerificationReport::facts()` and
`into_facts()`.

Accepted reports may still have `lowering_ready.ready == false`. A structurally
valid artifact is not automatically a lowerable artifact.

Diagnostics must keep stable stage, code, path, and message fields. Stage names
are part of the reviewer-facing contract.

## Facts Trust Boundary

`ArtifactVerificationFacts` are verifier-tied evidence. They are not independent
trust tokens and are not sufficient when copied out of their accepted report.

L2 facts must come from the same `FullVerificationReport` produced by
`verify_l2_core` during the unified verification call. Later plans may attach
`VerifiedArtifactFacts` inside the artifact facts object only after the L2Core
verifier accepts.

## Constraint Discharge Status

Phase 17 records solver-neutral constraint status:

- `NotRequired`
- `CollectedOnly`
- `Discharged`
- `Failed`
- `Unknown`
- `Skipped`

Current Phase 17 work may mark obligations as `CollectedOnly`; real SMT solver
discharge is deferred. This phase does not add Z3, cvc5, SMT-LIB subprocess
execution, or any solver runtime dependency.

## Lowering Readiness

Lowering readiness is a report property, not a replacement for verification.

For the current textual/native path, readiness may become true only when:

- the artifact report is accepted;
- L2Core verification has accepted the associated program;
- `VerifiedArtifactFacts` are present in that accepted report;
- the Phase 14/16 bounded Int32 copy support predicate accepts the program and
  facts.

Unsupported programs must fail closed before MLIR, LLVM, JIT, or native
artifacts are created.

## Runtime Semantic Guards

Static verification and runtime semantic guards are complementary.

The artifact verifier handles static container, structural, L2Core, and facts
preconditions. Value-dependent checks that require materialization remain in
typed decode/runtime paths and should continue to report fail-closed decode
errors. Oracle/equivalence tests remain separate evidence.

## Non-Goals

Phase 17 does not implement:

- real SMT solver discharge;
- a stable external `L2Core` artifact codec/parser;
- production MLIR decode dialect;
- Arrow/raw-buffer native writes;
- native kernel expansion;
- vectorization;
- complete Vortex reader support;
- DuckDB native execution;
- StarRocks or Iceberg integration;
- complete Lean/Rocq proof depth;
- a compiler correctness proof.
