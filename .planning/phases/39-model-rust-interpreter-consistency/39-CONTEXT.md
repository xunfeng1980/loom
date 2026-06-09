# Phase 39: Model Rust Interpreter Consistency - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous discuss; recommended defaults selected from Phase 36 contract, Phase 38 soundness closeout, and roadmap scope

<domain>
## Phase Boundary

Phase 39 owns the `modeled-executor<->real-executor` seam from the Phase 36
verified-lineage contract. Phase 38 proved safety over the Lean modeled
executor; Phase 39 must validate that the real Rust interpreter behavior agrees
with a faithful Rust transcription of that model at builder-event trace
granularity.

This phase is differential validation, not a proof of verified compilation. It
must not claim native/model validation, compiler correctness, source-data
correctness, performance, DuckDB correctness, or production GA readiness. Native
validation remains Phase 40.

</domain>

<decisions>
## Implementation Decisions

### Reference Executor

- **D-39-01:** Implement a Rust reference executor that transcribes the Phase 38
  Lean modeled operational semantics one-to-one. It is the differential oracle,
  not the production interpreter.
- **D-39-02:** Keep the reference executor visibly separate from production
  execution code. Do not let production paths call the reference executor as a
  fallback or correctness shortcut.
- **D-39-03:** The reference executor should emit stable trace records for reads,
  append-value events, append-null events, terminal status, and fail-closed
  diagnostics where applicable.

### Production Trace

- **D-39-04:** The production Rust interpreter under test must be treated as the
  subject, not adjusted to match the reference oracle. Divergence is a finding.
- **D-39-05:** If the current repository lacks a fully separate L2Core runtime
  interpreter for the modeled slice, the plan must first define the narrow
  production/interpreter surface under test and document that boundary. Any new
  trace hook must be observer-only and must not alter behavior.
- **D-39-06:** Compare full builder-event traces and fail-closed behavior, not
  only final values or accepted/rejected classifications.

### Corpus And Gate

- **D-39-07:** Use the supported Phase 37/38 matrix plus deterministic generated
  cases. Include positive event traces, append-null traces, fail-closed traces,
  loop/cursor traces, and negative diagnostics.
- **D-39-08:** The gate must fail closed on trace divergence, missing cases,
  differently classified fail-closed behavior, or nondeterministic output.
- **D-39-09:** Keep the harness repo-local and deterministic. Random fuzz is
  acceptable only through a stable seed and reproducible corpus generation.

### Extraction Evaluation

- **D-39-10:** Evaluate Lean extraction as optional additive evidence. Adopt it
  only if it is straightforward and keeps the gate deterministic; otherwise
  record it as deferred with a concrete reason.
- **D-39-11:** Do not block Phase 39 on extraction if the Rust transcription
  gives auditable trace-level differential evidence.

### Scope Notes And Non-Claims

- **D-39-12:** Documentation must say Phase 39 validates model-to-Rust
  interpreter consistency per run; it does not prove equivalence for all
  possible programs.
- **D-39-13:** Phase 40 remains responsible for native-to-model validation.

### the agent's Discretion

- Choose exact Rust module names and trace record shape.
- Choose whether the production trace hook lives behind test-only code,
  internal diagnostics, or a small public-in-crate helper.
- Choose corpus representation, provided reference and production paths consume
  the same cases and comparison output is stable.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope And Prior Evidence

- `.planning/ROADMAP.md` - Phase 39 goal, success criteria, non-goals, and
  ordering decision.
- `.planning/STATE.md` - Current position and Phase 38 handoff.
- `.planning/REQUIREMENTS.md` - LINEAGE-07 and LINEAGE-08 requirement targets.
- `.planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` -
  Evidence-layer definitions and `modeled-executor<->real-executor` seam.
- `.planning/phases/38-lean-stage-c-operational-semantics-and-soundness-theorem/38-02-SUMMARY.md` -
  Modeled executor theorem and explicit Phase 39 handoff.

### Code And Tests

- `formal/lean/LoomCore.lean` - Phase 38 modeled executor definitions:
  `ModeledState`, `ModeledEvent`, `execProgram`, `ModeledExecutionSafe`.
- `crates/loom-core/src/l2_core.rs` - Rust L2Core AST and scalar expression
  vocabulary.
- `crates/loom-core/src/full_verifier.rs` - Rust verifier semantics and stable
  diagnostics that govern accepted/fail-closed cases.
- `crates/loom-core/tests/full_verifier.rs` - Phase 37 correspondence corpus
  and deterministic matrix patterns.
- `scripts/full-verifier-test.sh` - Current verifier/Lean/correspondence proof
  gate.

</canonical_refs>

<code_context>
## Existing Code Insights

### Current State

- Phase 38 added a modeled Lean executor, but the repo does not yet expose a
  trace-level Rust executor that obviously corresponds one-to-one with that
  model.
- Existing Rust verifier tests already build representative `L2CoreProgram`
  values and stable diagnostics; those should seed the Phase 39 corpus.
- Existing gates already use deterministic report-line comparison for
  Lean/Rust verifier correspondence. Phase 39 can reuse that style for trace
  comparison.

### Trace Shape

Recommended trace rows:

| Field | Purpose |
|---|---|
| case id | stable corpus identity |
| source | `reference` or `production` |
| event index | total ordering |
| event kind | read, append-value, append-null, fail-closed, finish/terminal |
| builder/capability | target identifier |
| type/classification | scalar/builder type or fail-closed diagnostic |
| status | finished or fail-closed |

The comparison should be exact and deterministic.

</code_context>

<specifics>
## Specific Ideas

Recommended plan split:

| Plan | Scope | Acceptance Focus |
|---|---|---|
| 39-01 | Rust reference executor | One-to-one transcription of Lean modeled semantics, stable trace records, corpus seed, extraction evaluation note |
| 39-02 | Trace-level differential gate | Production trace hook, reference vs production trace diff, deterministic fuzz/corpus, release-gate wiring, LINEAGE-07/08 closeout |

Recommended script name: `scripts/model-rust-interpreter-consistency-test.sh`.

</specifics>

<deferred>
## Deferred Ideas

- Native-to-model validation remains Phase 40.
- Verified compilation/extraction as the only oracle remains deferred unless
  extraction is adopted cheaply in this phase.
- DuckDB/native/source correctness and performance remain out of scope.

</deferred>

---

*Phase: 39-Model Rust Interpreter Consistency*
*Context gathered: 2026-06-09*
