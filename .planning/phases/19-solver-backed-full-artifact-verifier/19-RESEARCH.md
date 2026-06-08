# Phase 19 Research: Solver-backed Full Artifact Verifier

## Scope

Phase 19 upgrades the Phase 17 artifact verifier from "constraints collected" to "constraints discharged" for the Phase 18 complete-reader artifact boundary.

The target pipeline is:

```text
LMC1 artifact
  -> Phase 17 artifact verifier
  -> optional L2Core verifier facts
  -> Phase 18 VortexReaderFacts / emission facts
  -> solver obligations
  -> SMT discharge report
  -> trusted VerifiedArtifactFacts only if required obligations discharge
```

Phase 19 must not become production MLIR/native kernel work, host-engine integration, or arbitrary Vortex decode expansion. Those remain Phase 20+.

## Current Local Baseline

Relevant shipped surfaces:

- `crates/loom-core/src/artifact_verifier.rs`
  - `ArtifactVerificationStage::ConstraintDischarge`
  - `ConstraintDischargeStatus::{NotRequired, CollectedOnly, Discharged, Failed, Unknown, Skipped}`
  - `ArtifactVerificationFacts.constraint_ids`
  - `ArtifactVerificationFacts.proof_obligation_ids`
  - `ArtifactVerificationFacts.l2_core`
- `crates/loom-core/src/full_verifier.rs`
  - `verify_l2_core`
  - `FullVerificationReport.constraint_comments`
  - `ProofObligationTrace`
  - `VerifiedArtifactFacts`
- `crates/loom-core/src/l2_core/constraints.rs`
  - SMT-ready constraint IR
  - current output is comments, not executable SMT-LIB queries
- Phase 18 reader handoff
  - `VortexReaderFacts`
  - supported emission matrix
  - `LMC1(LMP1)` / `LMC1(LMT1)` verifier handoff

Current gap: the verifier can collect obligation IDs and constraint IDs, but no solver backend decides `unsat`/`sat`/`unknown`, no report records solver model/counterexample/unknown reason, and no trust rule prevents later lowering from treating collected-only obligations as discharged evidence.

## Source Findings

### SMT-LIB

SMT-LIB is the right portable interchange layer. The official SMT-LIB site lists Version 2.7 as the latest official v2 standard, and its logic catalog includes quantifier-free integer and bit-vector fragments such as `QF_LIA`, `QF_IDL`, and `QF_BV`.

For Loom, this matters because the first obligations are mostly:

- offset + width <= input length
- row index within row count bound
- builder events <= max events
- loop bounds finite and within resource budget
- arithmetic overflow excluded or modeled explicitly

Recommendation: Phase 19 should emit deterministic SMT-LIB v2.7 scripts first. Prefer `QF_LIA` for row/offset/resource constraints while values are mathematical integers. Use `QF_BV` only when proving machine-width overflow behavior is specifically required.

### Z3

The current Rust `z3` crate is mature enough for optional in-process solver evidence. Docs.rs lists `z3 0.20.0` as high-level Rust bindings for Microsoft Research Z3, and the upstream `prove-rs/z3.rs` repository separates high-level `z3` from low-level `z3-sys`.

Useful properties:

- High-level API for direct term construction.
- `Solver::from_string` can parse SMT-LIB2 strings into the solver.
- `Solver::new_for_logic` supports logic-specialized solving.
- Solver API exposes satisfiability result, model, proof, unsat core, statistics, and reason-unknown surfaces.

Risk:

- In-process FFI dependency is heavier than a text contract.
- Exact native Z3 availability and build strategy can create toolchain friction.
- A direct Z3-only encoding can accidentally become the source of truth instead of the Loom obligation IR.

Recommendation: use Z3 as the first optional backend, but make SMT-LIB text and Loom-owned discharge reports the stable contract.

### cvc5

The `cvc5` Rust crate provides safe, high-level Rust bindings. cvc5 documentation describes cvc5 as an open-source SMT theorem prover supporting many theories and combinations; cvc5 quickstart states SMT-LIB v2 is its primary input language and documents logic selection such as `QF_BV` / `QF_AUFBV`.

Useful properties:

- Rust API exposes `TermManager`, `Solver`, `InputParser`, `Proof`, `Statistics`, and result/unknown explanation types.
- Official docs include proof production, resource limits, statistics, and options.
- Good cross-check candidate for solver disagreement.

Risk:

- Rust crate surface is newer than the Z3 Rust ecosystem.
- Adding cvc5 as a hard dependency would widen build surface too early.

Recommendation: keep cvc5 as secondary optional backend or strict cross-check mode after the SMT-LIB contract is stable.

### Process-based SMT-LIB Wrappers

`rsmt2` is a Rust wrapper around SMT-LIB 2.x-compliant solver subprocesses. `easy-smt` similarly talks to a solver subprocess.

Useful properties:

- Keeps `loom-core` free of solver native dependencies.
- Matches the recommended SMT-LIB text contract.
- Supports multiple external solver binaries through one process boundary.

Risk:

- Parser/model handling becomes the project's responsibility.
- Subprocess timeout/resource isolation must be explicit.
- Less typed than direct Z3/cvc5 APIs.

Recommendation: Phase 19 should strongly consider a separate optional crate, e.g. `loom-solver-smt`, that runs solver binaries through SMT-LIB and produces Loom-owned discharge reports. This should be the release-gated default before any direct in-process solver dependency.

## Design Recommendation

### 1. Keep `loom-core` solver-neutral

`loom-core` should define:

- `SolverObligation`
- `SolverObligationKind`
- `SolverTheory`
- `SmtLibScript`
- `SolverDischargeStatus`
- `SolverDiagnostic`
- `SolverDischargeReport`

`loom-core` may print SMT-LIB, but should not depend on `z3`, `cvc5`, or subprocess crates.

### 2. Add an optional solver crate

Add one optional crate for execution:

```text
crates/loom-solver-smt
```

Initial backend:

- subprocess `z3` if present
- optional `cvc5` cross-check in strict mode
- deterministic timeout
- deterministic parsing of `sat`, `unsat`, `unknown`, and reason-unknown
- fail-closed mapping into `SolverDischargeReport`

This mirrors the Phase 16 optional backend pattern: normal mode can skip unavailable toolchains, strict mode fails.

### 3. Prove safety by unsat queries

Safety obligations should be encoded as "bad state exists" queries:

```text
assumptions + verifier facts + negated safety property
```

Interpretation:

| Solver result | Meaning | Artifact outcome |
|---|---|---|
| `unsat` | No counterexample exists | obligation discharged |
| `sat` | Counterexample/model exists | rejected or unsupported |
| `unknown` | Solver cannot decide | fail closed |
| timeout/error | No trustworthy proof | fail closed |
| solver unavailable | skip only in non-strict tests; no production discharge |

Do not treat `sat` as success. For Loom safety, success is normally `unsat` for the negated bad-state query.

### 4. Gate `VerifiedArtifactFacts`

Phase 19 should introduce a trust distinction:

- `VerifiedArtifactFacts` from structural/L2 verifier remains collected evidence.
- `SolverBackedArtifactFacts` or an added `solver_discharge` field marks facts as trusted for production native phases only when all required obligations are discharged.

Phase 20+ must consume solver-backed facts, not `CollectedOnly`.

### 5. Start with QF_LIA, add QF_BV only when needed

Most current obligations are row/resource/offset inequalities and fit `QF_LIA`. Bit-vector modeling is necessary only when Loom wants to prove exact fixed-width wrap/overflow behavior. Use the simpler logic first to reduce solver fragility.

## Proposed Phase 19 Plan Split

1. **19-01 Solver contract and obligation model**
   - Add Loom-owned solver obligation/report types.
   - Define trust rules for `Discharged`, `Failed`, `Unknown`, `Skipped`.
   - No external solver dependency.

2. **19-02 Deterministic SMT-LIB emitter**
   - Convert current `ConstraintSet` / `ProofObligationTrace` into executable SMT-LIB v2 scripts.
   - Support `QF_LIA` first.
   - Add snapshot tests for deterministic scripts.

3. **19-03 Optional solver backend crate**
   - Add `loom-solver-smt`.
   - Detect `z3` and optionally `cvc5`.
   - Run SMT-LIB scripts with timeout and parse `sat` / `unsat` / `unknown`.
   - Normal mode skip, strict mode fail.

4. **19-04 Artifact verifier integration**
   - Wire solver discharge into `verify_artifact_with_l2_core` or a new `verify_artifact_with_solver`.
   - Mark `constraint_status = Discharged` only when all required obligations discharge.
   - Preserve fail-closed behavior for `sat`, `unknown`, timeout, parse error, or missing required solver in strict mode.

5. **19-05 CLI, release gate, and final report**
   - Expose solver status in `loom verify-artifact`.
   - Add `scripts/solver-verifier-test.sh`.
   - Wire into `scripts/mvp0-verify.sh`.
   - Document Phase 20 handoff: production native expansion may only trust discharged facts.

## Acceptance Criteria

- `loom-core` remains free of Z3/cvc5/native solver dependencies.
- SMT-LIB output is deterministic and testable without installed solvers.
- At least one local solver backend can discharge the bounded copy/offset obligations when available.
- Solver `sat`, `unknown`, timeout, and subprocess errors are fail-closed.
- `ArtifactVerificationFacts.constraint_status` reaches `Discharged` only from verified solver evidence.
- CLI and release gate make solver-backed status reviewer-visible.
- Optional backend skip is clearly recorded and cannot be misread as production proof.

## Open Technical Decisions

1. **Direct API vs subprocess first**
   - Recommendation: subprocess SMT-LIB first for portability and auditability.

2. **Z3-only vs Z3+cvc5**
   - Recommendation: Z3 primary, cvc5 optional strict cross-check later in the phase if cost stays low.

3. **Fact type shape**
   - Option A: extend `ArtifactVerificationFacts` with `solver_report`.
   - Option B: introduce `SolverBackedArtifactFacts`.
   - Recommendation: Option A for Phase 19 MVP, with a strong `constraint_status == Discharged` gate.

4. **Logic selection**
   - Recommendation: `QF_LIA` first, `QF_BV` only for explicit fixed-width overflow obligations.

5. **Proof objects**
   - Recommendation: record unsat core / model / reason-unknown first. Full proof-object checking is out of Phase 19 unless it falls out cheaply from cvc5/Z3 tooling.

## Sources

- SMT-LIB language standard: https://smt-lib.org/language.shtml
- SMT-LIB logic catalog: https://smt-lib.org/logics.shtml
- SMT-LIB all logics, including Version 2.7 declarations: https://smt-lib.org/logics-all.shtml
- Z3 Rust crate docs: https://docs.rs/z3/latest/z3/
- Z3 crate release page: https://docs.rs/crate/z3/latest
- Z3 Rust bindings repository: https://github.com/prove-rs/z3.rs
- Z3 guide basic SMT-LIB commands: https://microsoft.github.io/z3guide/docs/logic/basiccommands/
- cvc5 Rust crate docs: https://docs.rs/cvc5
- cvc5 documentation: https://cvc5.github.io/docs/cvc5-1.1.2/
- cvc5 quickstart / SMT-LIB input: https://cvc5.github.io/docs/cvc5-1.0.2/binary/quickstart.html
- cvc5 solver outputs / unknown handling: https://cvc5.github.io/tutorials/beginners/outputs.html
- rsmt2 docs: https://docs.rs/rsmt2/latest/rsmt2/
- easy-smt docs: https://docs.rs/easy-smt
