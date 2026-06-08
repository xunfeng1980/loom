# Phase 19 Research: Solver-backed Full Artifact Verifier

**Refreshed:** 2026-06-08
**Local solver probe:** `z3` is available at `/opt/homebrew/bin/z3` as `Z3 version 4.15.4 - 64 bit`; `cvc5` and `bitwuzla` were not found on `PATH`. Phase 19 still selects Bitwuzla as the primary implementation backend, so the execution plan must install/probe Bitwuzla or fail clearly in strict mode.

## Executive Recommendation

Phase 19 should build a solver-backed verifier without making a solver the source of truth.

Recommended architecture:

```text
LMC1 artifact
  -> Phase 17 artifact verifier
  -> optional Phase 13 L2Core verifier facts
  -> Phase 18 complete-reader facts / emission facts
  -> Loom-owned SolverObligation set
  -> deterministic SMT-LIB v2.7 scripts
  -> optional solver subprocess backend
  -> SolverDischargeReport
  -> ArtifactVerificationFacts.constraint_status = Discharged only when all required obligations are proven
```

The best first implementation path is:

1. Keep `loom-core` solver-neutral.
2. Emit deterministic SMT-LIB scripts from Loom-owned obligation/report types.
3. Add an optional `loom-solver-smt` crate that runs solver binaries through a process boundary.
4. Define the backend trait from day one around three command-line backend declarations: `z3`, `cvc5`, and `bitwuzla`.
5. Implement Bitwuzla first as the Phase 19 primary backend, even though the local machine currently needs Bitwuzla installation/probing.
6. Keep Z3 and cvc5 as supported backend declarations, optional adapters, and future cross-check/evidence paths.
7. Treat `sat`, `unknown`, timeout, parse error, solver crash, missing strict solver, and solver disagreement as fail-closed.

Do not implement production MLIR decode dialects, native kernel expansion, host-engine execution, Iceberg binding, or expanded Vortex encoding coverage in Phase 19.

## Scope

Phase 19 upgrades the Phase 17 artifact verifier from "constraints collected" to "constraints discharged" for the Phase 18 complete-reader artifact boundary.

The phase exists because current Loom can say:

- this artifact is structurally valid,
- this tiny L2Core program type-checks / abstract-interprets,
- these proof obligation IDs and constraint IDs were collected,
- these facts look lowering-relevant.

It cannot yet say:

- every required offset/range/overflow/resource obligation was proven,
- the proof status is reproducible from a script,
- a reviewer can see why a solver rejected or could not decide an artifact,
- later native phases can distinguish collected evidence from discharged evidence.

## Current Local Baseline

Relevant shipped surfaces:

- `crates/loom-core/src/artifact_verifier.rs`
  - `ArtifactVerificationStage::ConstraintDischarge`
  - `ConstraintDischargeStatus::{NotRequired, CollectedOnly, Discharged, Failed, Unknown, Skipped}`
  - `ArtifactVerificationFacts.constraint_ids`
  - `ArtifactVerificationFacts.proof_obligation_ids`
  - `ArtifactVerificationFacts.l2_core`
  - `verify_artifact`
  - `verify_artifact_with_l2_core`
- `crates/loom-core/src/full_verifier.rs`
  - `verify_l2_core`
  - `FullVerificationReport.constraint_comments`
  - `ProofObligationTrace`
  - `VerifiedArtifactFacts`
- `crates/loom-core/src/l2_core/constraints.rs`
  - `ConstraintSet`
  - `ConstraintTerm`
  - `LoomConstraint`
  - `to_smtlib_comments()`, currently comments only, not executable SMT-LIB.
- Phase 18 complete-reader handoff:
  - `VortexReaderFacts`
  - explicit accepted/unsupported/rejected support classification
  - supported `LMC1(LMP1)` / `LMC1(LMT1)` emission matrix
  - Phase 17 artifact-verifier handoff for emitted artifacts

The key gap is not absence of a verifier. The gap is absence of discharged proof evidence.

## Source Findings

### SMT-LIB v2.7 should be Loom's stable solver boundary

The official SMT-LIB reference document lists Version 2.7 and describes solver interaction as scripts with commands such as `set-logic`, `assert`, `check-sat`, `get-value`, `get-model`, and `get-info`. The official examples include `QF_LIA`, `QF_BV`, `get-model`, `get-value`, named assertions, and `get-unsat-core`.

For Loom, this matters because a deterministic text artifact is easier to:

- inspect during review,
- replay outside Rust,
- store in test snapshots,
- feed to Z3, cvc5, Bitwuzla, or another SMT-LIB-compatible solver,
- compare across solver versions.

Recommendation:

- `loom-core` should own `SmtLibScript` generation.
- The scripts should be valid SMT-LIB v2.7-style text, but Phase 19 should keep to conservative v2 commands that common solvers already support.
- Every generated script must include stable metadata comments with artifact id, obligation id, logic, and expected interpretation.
- Deterministic ordering is mandatory: sort declarations, assertions, named assumptions, and report sections by stable IDs.

### Bitwuzla primary means QF_BV first, with QF_LIA as an optional Z3/cvc5 path

The official SMT-LIB logic catalog defines `QF_LIA` as unquantified linear integer arithmetic and `QF_BV` as quantifier-free fixed-size bitvectors. Loom's first proof obligations are mostly:

- `offset >= 0`
- `width >= 0`
- `offset + width <= input_length`
- `row_index < row_count_bound`
- `builder_events <= max_events`
- `steps_used <= max_steps`
- loop trip counts are finite and within budget

Those fit `QF_LIA` if Loom models machine integers as mathematical integers plus explicit range constraints. But Bitwuzla's official scope is fixed-size bit-vectors, floating-point arithmetic, arrays, uninterpreted functions, and their combinations. Therefore a Bitwuzla-primary Phase 19 should not depend on `QF_LIA` as the required path.

`QF_BV` becomes the primary required encoding when Phase 19 implements Bitwuzla. Range/resource obligations can be represented with sufficiently wide unsigned bit-vectors and explicit unsigned comparisons. This aligns Phase 19 with later exact machine-width wrap/overflow semantics, bitpack semantics, SIMD lane semantics, and native lowering equivalence.

Recommendation:

- Implement `QF_BV` emission first for the Bitwuzla primary backend.
- Encode row/resource/offset obligations as bounded unsigned bit-vectors with explicit width policy, e.g. 64-bit for file offsets/row counts unless a fact requires a narrower width.
- Encode no-overflow with bit-vector predicates or explicit widen-then-bound checks.
- Keep `QF_LIA` as an optional alternate script family for Z3/cvc5 cross-checks, not the Phase 19 required path.
- Avoid quantifiers in Phase 19. Quantifiers increase `unknown` risk and make replay/debug harder.

### Z3 is a useful installed cross-check, but not the Phase 19 primary backend

The Rust `z3` crate documents an idiomatic high-level Rust wrapper and a `Solver` API with methods including `new_for_logic`, `from_string`, `check`, `get_model`, `get_reason_unknown`, `get_statistics`, `get_unsat_core`, `push`, `pop`, and `to_smt2`. Z3's API docs also document unsat-core extraction after checks with assumptions.

The local machine already has:

```text
/opt/homebrew/bin/z3
Z3 version 4.15.4 - 64 bit
```

Recommendation:

- Keep Z3 in the `loom-solver-smt` backend declaration set from day one.
- Do not make Z3 the Phase 19 primary implementation backend; Bitwuzla is now the selected primary.
- Preserve Z3 as an optional installed backend/cross-check because it is locally available and useful for debugging QF_LIA scripts.
- If direct Rust `z3` API support is added later, it must consume the same Loom obligation model and produce the same `SolverDischargeReport` shape.

Risk:

- Z3 native/library versioning can become another toolchain axis.
- API parsing of SMT-LIB strings can diverge from full solver command processing; subprocess execution of `.smt2` scripts avoids that class of surprise.
- A Z3-only implementation could accidentally encode solver-specific assumptions into Loom's verifier.

### cvc5 is the right second solver and evidence enhancer

cvc5 documentation and tutorials show support for SMT-LIB input, models, unsat cores, proofs, and reason-unknown reporting. Its "interfaces for understanding cvc5" docs explain `get-unsat-core`, timeout cores, difficulty, and unknown explanations; the beginner output docs describe `get-model`, `get-value`, `get-unsat-core`, `get-proof`, and `get-info :reason-unknown`.

Recommendation:

- Do not require cvc5 for Phase 19's default release gate unless the project explicitly installs it.
- Add report fields that can already represent cvc5-style evidence: unsat core, proof available flag, reason unknown, timeout core/difficulty notes as optional diagnostics.
- Consider cvc5 strict cross-check mode if it is easy to install locally.

Risk:

- cvc5 proof objects and unsat cores are useful but not yet a checked proof inside Loom.
- Treat proof text as diagnostic evidence unless/until a later phase adds independent proof checking.

### Bitwuzla is the Phase 19 primary implementation backend

Bitwuzla's command-line documentation says it parses SMT-LIBv2 input and can print formulas, models, unsat cores, and unsat assumptions. Its command line also exposes model checking, unsat-core checking, time limits, memory limits, and bit-vector-specific output controls. Its C API can query model values, unsat cores, and unsat assumptions.

This means Bitwuzla has enough evidence surface for Phase 19's solver-discharge reports. Its strongest long-term fit is still QF_BV / fixed-width native exactness, but selecting it as Phase 19's first implementation backend intentionally aligns the verifier backend with the later native/bitvector path.

Recommendation:

- Make `bitwuzla` the primary implemented command-line backend in `loom-solver-smt`.
- Require the backend trait to support `z3`, `cvc5`, and `bitwuzla` declarations even if only Bitwuzla is fully implemented in Phase 19.
- Use normal mode to skip clearly if Bitwuzla is unavailable; use strict mode to fail if Bitwuzla is unavailable.
- Keep Z3/cvc5 as optional adapters or cross-check placeholders.
- Start with `QF_BV` scripts for the required Bitwuzla path; optional Z3/cvc5 adapters may add `QF_LIA` scripts as cross-check evidence.

### Process-based Rust wrappers fit Loom better than direct native FFI in Phase 19

`easy-smt` is documented as a crate for interacting with an SMT solver subprocess, building SMT-LIB 2 expressions/assertions, querying solver results, and replaying solver interactions. `rsmt2` documents a process-based SMT-LIB 2.x wrapper where solvers run in separate processes and communicate via pipes.

Recommendation:

- A tiny Loom-owned subprocess runner may be simpler than adopting a wrapper immediately, because Phase 19 initially needs batch `.smt2` scripts, timeouts, stdout/stderr capture, and first-token parsing.
- If an external wrapper is used, prefer one that preserves replayable SMT-LIB text and does not obscure solver I/O.
- Do not add solver wrapper dependencies to `loom-core`.

## Semantic Strategy

### Safety success should usually mean `unsat`

For each safety obligation, generate a bad-state query:

```text
assumptions + artifact facts + verifier facts + negated safety property
```

Then interpret:

| Solver result | Meaning for bad-state query | Loom artifact outcome |
|---|---|---|
| `unsat` | No counterexample exists | obligation discharged |
| `sat` | Counterexample exists | rejected or unsupported; include model/counterexample if available |
| `unknown` | Solver could not decide | fail closed |
| timeout | Solver did not produce evidence | fail closed |
| parse/error/crash | Evidence is invalid | fail closed |
| solver unavailable | no production discharge | non-strict tests may skip; strict mode fails |
| solver disagreement | evidence is not stable | fail closed in strict cross-check mode |

This avoids a common verifier bug: treating solver success as "the script ran" instead of "the right theorem was proven."

### Named assertions are mandatory

Every assertion that can appear in a report must be named:

```smt2
(assert (! (<= (+ input_offset read_width) input_length) :named ob.read.bounds.0001))
```

Benefits:

- unsat cores are useful,
- model/counterexample output can be mapped back to Loom diagnostics,
- flaky obligations can be isolated,
- reports can cite stable obligation IDs.

### Obligation categories

Phase 19 should classify obligations before emitting SMT:

| Category | Examples | Required Bitwuzla logic | Result needed |
|---|---|---|---|
| Bounds | read offset/width, segment range, buffer length | `QF_BV` with unsigned comparisons | `unsat` bad-state |
| Row/resource | row count, loop bound, builder event count | `QF_BV` with stable width policy | `unsat` bad-state |
| Arithmetic range | checked add/mul, non-negative casts | `QF_BV` with overflow-aware bad states | `unsat` bad-state |
| Feature implication | feature flag implies obligation/fallback | Boolean/bit-vector encoding; likely avoid initially | exact simple encoding |
| Native exactness | fixed-width operations, vector lanes | `QF_BV` | deferred to Phase 20+ unless needed |

### Solver discharge belongs beside, not inside, structural verification

Phase 17's structural verifier should still run first. It rejects malformed containers and unsupported shapes before solver work. Solver discharge should consume accepted structural facts and optional accepted L2Core facts.

This preserves a clean failure ladder:

1. Malformed bytes: reject before facts.
2. Unsupported artifact shape: unsupported before solver.
3. Structural/L2 verifier error: reject before solver.
4. Solver unsupported or unknown: fail closed at constraint-discharge stage.
5. All required obligations discharged: accepted solver-backed facts.

## Report and Trust Model

### Current problem

`VerifiedArtifactFacts` currently means "the Rust L2Core verifier accepted this program and emitted facts." It does not mean "all constraints were solver-discharged."

### Recommended Phase 19 model

Keep `ArtifactVerificationFacts` as the top-level facts shape, but add solver-owned evidence:

```text
ArtifactVerificationFacts
  constraint_status: ConstraintDischargeStatus
  solver_report: Option<SolverDischargeReport>
```

`constraint_status == Discharged` must require:

- every required obligation has a solver result,
- every required obligation is `unsat` for the bad-state query,
- no required obligation is `sat`, `unknown`, timed out, skipped, or errored,
- solver report IDs match the facts' `constraint_ids` / `proof_obligation_ids`,
- if strict cross-check is enabled, every required backend agrees.

Do not introduce a second "trusted facts" type unless implementation pressure demands it. One accepted report with explicit `constraint_status` and `solver_report` is simpler for Phase 20 to consume.

### Report fields

Recommended Loom-owned report vocabulary:

- `SolverObligation`
  - `id`
  - `kind`
  - `theory`
  - `bit_width_policy`
  - `query_semantics` (`bad-state-unsat`)
  - `source_stage`
  - `source_path`
  - `constraint_ids`
  - `required`
- `SmtLibScript`
  - `id`
  - `logic`
  - `text`
  - `expected_success` (`unsat` for safety)
  - `obligation_ids`
  - `deterministic_hash`
- `SolverBackend`
  - `name`
  - `version`
  - `path`
  - `strict`
  - `timeout_ms`
- `SolverObligationResult`
  - `obligation_id`
  - `backend`
  - `status` (`Discharged`, `Failed`, `Unknown`, `Timeout`, `Error`, `Skipped`)
  - `raw_result` (`unsat`, `sat`, `unknown`, etc.)
  - `model_excerpt`
  - `unsat_core_ids`
  - `reason_unknown`
  - `stdout_excerpt`
  - `stderr_excerpt`
- `SolverDischargeReport`
  - `status`
  - `backend_results`
  - `required_obligation_count`
  - `discharged_count`
  - `failed_count`
  - `unknown_count`
  - `skipped_count`
  - `scripts`
  - `diagnostics`

The report must be serializable enough for CLI/release-gate display, even if Phase 19 does not choose a stable external JSON format yet.

## Backend Design

### Crate split

Recommended split:

```text
loom-core
  owns obligation types, SMT-LIB emission, report types, artifact verifier integration

loom-solver-smt
  optional subprocess runner, solver discovery, backend trait for z3/cvc5/bitwuzla,
  Bitwuzla primary implementation, timeouts, stdout/stderr parsing

loom-cli
  exposes solver-backed verify-artifact command/status
```

`loom-core` should not depend on `z3`, `cvc5`, `bitwuzla`, `easy-smt`, or `rsmt2`.

### Normal vs strict modes

Mirror Phase 16's optional backend pattern:

| Mode | Solver missing | Solver returns unknown | Solver returns sat | Use case |
|---|---|---|---|---|
| normal | skip with explicit `Skipped` status | fail closed for that artifact; tests may assert skip wording | fail closed | developer machines without solvers |
| strict | command fails | command fails | command fails | release gate / CI proving solver evidence |

Suggested environment variables:

- `LOOM_REQUIRE_SOLVER=1` means solver-backed tests must not skip.
- `LOOM_SOLVER_BACKEND=bitwuzla` selects the Phase 19 primary backend.
- `LOOM_SOLVER_TIMEOUT_MS=...` pins timeout.
- `LOOM_SOLVER_CROSSCHECK=z3,cvc5` enables optional strict cross-checks if installed.

### Subprocess behavior

The runner should:

- write SMT-LIB to a temp file or stdin,
- run solver with deterministic timeout,
- capture stdout/stderr,
- parse the first decisive token: `unsat`, `sat`, or `unknown`,
- capture `get-model`, `get-unsat-core`, and `get-info :reason-unknown` outputs when present,
- cap report excerpts to avoid huge diagnostics,
- never accept success if the subprocess exits non-zero or produces malformed output.

Do not rely on wall-clock performance as correctness evidence.

## SMT-LIB Emission Rules

### Script skeleton

Recommended Bitwuzla-primary `QF_BV` script shape:

```smt2
(set-info :smt-lib-version 2.7)
(set-option :print-success false)
(set-option :produce-models true)
(set-option :produce-unsat-cores true)
(set-logic QF_BV)

; loom-artifact ...
; loom-obligation ...

(declare-const input_offset (_ BitVec 64))
(declare-const read_width (_ BitVec 64))
(declare-const input_length (_ BitVec 64))

(assert (! (= input_length #x0000000000001000) :named fact.input_length.bound))

; Bad state: read extends past input length.
; Either the unsigned addition overflows, or the end offset exceeds input_length.
(assert (! (or (bvult (bvadd input_offset read_width) input_offset)
               (bvugt (bvadd input_offset read_width) input_length))
           :named bad.read.out_of_bounds))

(check-sat)
(get-model)
(get-unsat-core)
(get-info :reason-unknown)
(exit)
```

Important detail: `(get-model)` after `unsat` is not valid for every solver/configuration. The final emitter may need per-result scripts or tolerate unsupported diagnostics. A robust first version can emit query scripts for `check-sat` only, then run follow-up commands conditionally in the backend.

For optional Z3/cvc5 `QF_LIA` cross-checks, the emitter may also generate integer scripts, but those are not the Phase 19 required Bitwuzla path.

### Determinism rules

- Stable sort of variable declarations.
- Stable sort of named facts.
- Stable sort of obligations.
- No timestamps inside scripts.
- Stable comment prefixes.
- Stable integer formatting.
- Stable symbol sanitization.
- Snapshot tests for expected scripts.

### Symbol hygiene

SMT-LIB symbols should be generated from stable Loom IDs, not raw user strings. Recommended:

```text
ob.<phase>.<kind>.<index>
fact.<source>.<field>.<index>
bad.<kind>.<index>
```

Raw paths can stay in comments/report fields.

## Artifact Verifier Integration

Recommended integration API:

```rust
verify_artifact_with_solver(
    bytes,
    registry,
    artifact_options,
    solver_options,
    solver_backend,
) -> ArtifactVerificationReport
```

or a smaller first step:

```rust
apply_solver_discharge(
    report: ArtifactVerificationReport,
    solver_report: SolverDischargeReport,
) -> ArtifactVerificationReport
```

The second API is safer for Phase 19 because it cleanly separates:

- structural verification,
- L2Core verification,
- solver discharge,
- facts trust update.

### Status mapping

| Existing artifact status | Solver report | Final status |
|---|---|---|
| Rejected | not run | Rejected |
| Unsupported | not run | Unsupported |
| Accepted + no constraints | not required | Accepted / `NotRequired` |
| Accepted + all required discharged | discharged | Accepted / `Discharged` |
| Accepted + any sat/failed | failed | Rejected or Accepted with no trusted facts? Recommendation: Rejected |
| Accepted + unknown/timeout/error | unknown/failed | Unsupported or Rejected. Recommendation: Unsupported for solver incompleteness, Rejected for counterexample |
| Accepted + solver skipped in normal mode | skipped | Accepted only as structural evidence; not lowering-ready for Phase 20 |

Native lowering after Phase 19 should require `constraint_status == Discharged` whenever `constraint_ids` is non-empty.

## Alternatives Considered

### Direct `z3` crate in `loom-core`

Pros:

- typed API,
- fewer subprocess parsing details,
- easy in-process model/unsat-core access.

Cons:

- adds native solver dependency to core verifier,
- makes replay harder,
- risks Z3-specific encoding becoming the verifier contract,
- may create build/toolchain friction like MLIR did.

Verdict: do not start here.

### cvc5 as the required solver

Pros:

- strong proof/unsat-core/reason-unknown surfaces,
- good future cross-check candidate.

Cons:

- not installed locally,
- likely adds setup friction,
- proof output is not the same as checked proof inside Loom.

Verdict: optional cross-check later.

### Z3 as the required solver

Pros:

- already installed locally,
- mature ecosystem,
- strong QF_LIA debugging path,
- useful model/unsat-core/reason-unknown surfaces.

Cons:

- less aligned with the later QF_BV/native exactness path than Bitwuzla,
- direct API/FFI dependency would add toolchain surface,
- making Z3 the primary backend would still require a later primary Bitwuzla backend when native bitvector obligations dominate.

Verdict: keep as supported backend declaration and optional cross-check; do not make it the Phase 19 primary implementation.

### Bitwuzla as the required solver

Pros:

- strong candidate for bitvector-heavy obligations.
- supports SMT-LIBv2 command-line input.
- supports model, unsat core, unsat assumptions, check-model, check-unsat-core, timeout, and memory-limit evidence surfaces.
- aligns Phase 19 verifier backend with later QF_BV/native exactness work.

Cons:

- not installed locally,
- Phase 19 first obligations are still mostly integer/range constraints,
- installation/probing must be part of execution,
- Z3 may remain easier for immediate QF_LIA debugging on this machine.

Verdict: selected Phase 19 primary implementation backend.

### No solver crate, CLI only

Pros:

- simplest implementation.

Cons:

- solver evidence becomes hard to reuse from tests/API,
- artifact verifier cannot programmatically update facts,
- Phase 20 cannot consume a stable report.

Verdict: insufficient for Phase 19.

## Proposed Phase 19 Plan Split

1. **19-01 Solver contract and obligation/report model**
   - Add Loom-owned `SolverObligation`, `SmtLibScript`, `SolverDischargeReport`, backend metadata, and status types.
   - Define report-to-facts trust rules.
   - Keep `loom-core` free of solver native/process dependencies.

2. **19-02 Deterministic SMT-LIB emitter**
   - Convert current `ConstraintSet` / `ProofObligationTrace` into executable SMT-LIB scripts.
   - Support Bitwuzla-primary `QF_BV` scripts first.
   - Define a stable bit-width policy for row counts, offsets, widths, and resource counters.
   - Add snapshot tests for deterministic scripts, symbol hygiene, and overflow-aware bad-state queries.

3. **19-03 Optional solver backend crate**
   - Add `crates/loom-solver-smt`.
   - Define backend declarations for `z3`, `cvc5`, and `bitwuzla`.
   - Implement `bitwuzla` as the first full backend.
   - Add optional Z3/cvc5 adapter placeholders or cross-check hooks where cheap.
   - Add timeout, stdout/stderr capture, first-token parsing, and normal/strict skip behavior.
   - Strict mode must fail if Bitwuzla is unavailable.

4. **19-04 Artifact verifier integration**
   - Wire solver discharge into the artifact verification path.
   - Set `ConstraintDischargeStatus::Discharged` only when all required obligations discharge.
   - Fail closed on `sat`, `unknown`, timeout, parse error, solver crash, missing strict solver, or cross-check disagreement.

5. **19-05 CLI, release gate, and final report**
   - Expose solver status in `loom verify-artifact`.
   - Add `scripts/solver-verifier-test.sh`.
   - Wire the solver gate into `scripts/mvp0-verify.sh`.
   - Document Phase 20 handoff: production native expansion may only trust discharged facts.

## Acceptance Criteria

- `loom-core` remains free of Z3/cvc5/Bitwuzla/native solver dependencies.
- SMT-LIB output is deterministic and testable without installed solvers.
- `loom-solver-smt` exposes backend declarations for `z3`, `cvc5`, and `bitwuzla`.
- Bitwuzla is the primary implemented backend for Phase 19.
- At least one Bitwuzla-backed strict gate can discharge bounded range/offset obligations after Bitwuzla is installed or otherwise available on `PATH`.
- The required SMT-LIB path uses `QF_BV` or another Bitwuzla-supported logic; `QF_LIA` is optional cross-check only.
- Solver `sat`, `unknown`, timeout, subprocess error, malformed output, and missing strict solver are fail-closed.
- `ArtifactVerificationFacts.constraint_status` reaches `Discharged` only from verified solver evidence.
- CLI and release gate make solver-backed status reviewer-visible.
- Optional backend skip is explicit and cannot be misread as production proof.
- Phase 20 handoff states that `CollectedOnly` obligations are insufficient for production native expansion.

## Open Technical Decisions

| Decision | Options | Recommendation |
|---|---|---|
| Backend boundary | subprocess SMT-LIB, direct solver APIs | subprocess SMT-LIB first |
| Backend declarations | Z3 only, Bitwuzla only, `z3/cvc5/bitwuzla` | declare all three command-line backend kinds from day one |
| First implemented solver | Z3, cvc5, Bitwuzla | Bitwuzla |
| Cross-check | none, Z3 strict, cvc5 strict | optional Z3/cvc5 cross-checks after Bitwuzla primary path works |
| Fact shape | extend `ArtifactVerificationFacts`, add `SolverBackedArtifactFacts` | extend `ArtifactVerificationFacts` with `solver_report` first |
| Logic | `QF_LIA`, `QF_BV`, mixed | `QF_BV` first for Bitwuzla; `QF_LIA` optional for Z3/cvc5 cross-checks |
| Query convention | prove property directly, prove bad-state unsat | bad-state unsat |
| Proof objects | require checked proof, record diagnostic proof/core | record unsat core/model/reason-unknown first; checked proof later |

## Final Recommendation

Phase 19 should be a solver-backed artifact verifier MVP, not a theorem-prover productization phase.

The crisp target is:

```text
ArtifactVerificationReport is accepted as solver-backed only when:
  structural verifier accepted,
  optional L2Core verifier accepted,
  every required solver obligation has replayable SMT-LIB,
  every required bad-state query returns unsat,
  report IDs line up with emitted facts,
  and all non-unsat outcomes fail closed.
```

This gives Phase 20 a trustworthy handoff: production native lowering may consume discharged facts, while collected-only facts remain useful diagnostics but not production proof.

## Sources

- SMT-LIB Standard v2.7 reference PDF: https://smt-lib.org/papers/smt-lib-reference-v2.7-r2025-02-05.pdf
- SMT-LIB examples: https://smt-lib.org/examples.shtml
- SMT-LIB logic catalog: https://smt-lib.org/logics.shtml
- Z3 Rust `Solver` docs: https://docs.rs/z3/latest/z3/struct.Solver.html
- Z3 crate docs: https://docs.rs/z3/latest/z3/
- Z3 crate release page: https://docs.rs/crate/z3/latest
- Z3 API docs for solver statistics / unsat cores / SMT-LIB output: https://z3prover.github.io/api/html/z3.z3.html
- Z3 Guide: bitvectors: https://microsoft.github.io/z3guide/docs/theories/Bitvectors/
- cvc5 "Interfaces for Understanding cvc5": https://cvc5.github.io/blog/2024/04/15/interfaces-for-understanding-cvc5.html
- cvc5 beginner tutorial: solver outputs: https://cvc5.github.io/tutorials/beginners/outputs.html
- cvc5 options docs: https://cvc5.github.io/docs/latest/options.html
- Bitwuzla command line docs: https://bitwuzla.github.io/docs/binary.html
- `easy-smt` docs: https://docs.rs/easy-smt/latest/easy_smt/
- `rsmt2` docs: https://docs.rs/rsmt2/latest/rsmt2/
