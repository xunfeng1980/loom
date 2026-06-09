# Phase 38: Lean Stage C - Operational Semantics and Soundness Theorem - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous discuss; recommended defaults selected from Phase 36 contract, Phase 37 correspondence closeout, and roadmap scope

<domain>
## Phase Boundary

Phase 38 owns the `static<->dynamic` seam from the Phase 36 verified-lineage
contract. It must define an executable or inductive Lean operational semantics
for the modeled L2Core executor and prove a scoped soundness theorem: verifier
acceptance implies modeled execution safety.

This phase is about the Lean model only. It must not claim Rust interpreter
correctness, native correctness, DuckDB correctness, source correctness,
performance, or model-to-real executor validation. Those seams are assigned to
Phase 39/40 or the TCB.

</domain>

<decisions>
## Implementation Decisions

### Semantics Shape

- **D-38-01:** Define modeled execution semantics in Lean for the current Phase
  37 checker surface: `readInput`, `appendValue`, `appendNull`, `letScalar`,
  bounded `forRange`, bounded `cursorLoop`, and `failClosed`.
- **D-38-02:** Prefer a small, proof-friendly model over a byte-accurate
  interpreter. Inputs may be abstract values satisfying declared capabilities;
  builder outputs may be modeled as typed builder events rather than concrete
  Arrow buffers.
- **D-38-03:** The semantics must account for termination through the existing
  `maxRows`/finite-bounds model. Do not add unbounded recursion or executable
  semantics that Lean cannot prove total.
- **D-38-04:** `failClosed` should be modeled as a safe terminal/failure state,
  not as unsafe behavior or emitted Arrow output.

### Soundness Theorem

- **D-38-05:** Re-prove `accepted_program_safe` as a semantic theorem over the
  modeled executor, not merely the previous structural `Verified -> Safe`
  projection.
- **D-38-06:** The theorem statement must cover the roadmap's four obligations:
  execution never reads outside declared input slices, emits only well-typed
  builder events, terminates within the `maxRows` budget, and yields
  well-formed Arrow by construction.
- **D-38-07:** Keep theorem assumptions explicit: input environments satisfy
  declared capabilities, scalar typing follows the Phase 37 checker, and
  SMT-only overflow/range obligations remain delegated unless directly modeled.
- **D-38-08:** The final Lean file must compile with 0 `sorry`.

### Scope Notes And Non-Claims

- **D-38-09:** The theorem must carry a visible scope note that it holds over
  the modeled executor only.
- **D-38-10:** Documentation and summaries must state that modeled-to-real Rust
  interpreter consistency remains Phase 39, and native-to-model validation
  remains Phase 40.
- **D-38-11:** Do not weaken the Phase 36 red line: Loom guarantees safety and
  Arrow well-formedness evidence lineage, never source-data correctness.

### Gate Wiring

- **D-38-12:** Keep `lean formal/lean/LoomCore.lean` as the primary proof gate.
  It is already run by `scripts/full-verifier-test.sh`; add marker checks only
  if they make the Phase 38 theorem and non-claim easier to audit.
- **D-38-13:** Avoid adding an advisory-only proof check. If Phase 38 adds a new
  proof artifact or script, it must be reachable from the existing verifier
  gate.

### the agent's Discretion

- Choose inductive small-step, big-step/fueled execution, or a hybrid, provided
  the theorem is machine-checked with 0 `sorry` and termination is explicit.
- Choose exact names for modeled input state, execution state, builder events,
  and safety predicates.
- Choose whether to split semantics and theorem into helper sections inside
  `LoomCore.lean` or a new Lean file, provided the existing gate compiles it.
- Keep the proof focused on the current modeled slice; do not expand Rust L2Core
  or source/native surfaces to make the theorem more ambitious.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope And Prior Evidence

- `.planning/ROADMAP.md` - Phase 38 goal, success criteria, non-goals, and
  ordering decision.
- `.planning/STATE.md` - Current position and last verified Phase 37 gate.
- `.planning/REQUIREMENTS.md` - LINEAGE-05 and LINEAGE-06 requirement targets.
- `.planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` -
  Evidence-layer definitions and `static<->dynamic` seam ownership.
- `.planning/phases/37-lean-stage-b-lean-rust-verifier-correspondence/37-02-SUMMARY.md` -
  Completed Lean/Rust correspondence evidence and residual non-claims.

### Lean And Verifier Artifacts

- `formal/lean/LoomCore.lean` - Current Lean checker, scalar expression model,
  classification corpus, and existing structural `accepted_program_safe`.
- `crates/loom-core/src/full_verifier.rs` - Rust verifier semantics that Phase
  37 mirrored for static acceptance and diagnostics.
- `crates/loom-core/src/l2_core.rs` - Rust AST surface and scalar expression
  vocabulary.
- `scripts/full-verifier-test.sh` - Existing verifier gate that runs Lean and
  the Phase 37 correspondence diff.
- `scripts/lean-rust-correspondence-test.sh` - Phase 37 differential evidence
  that Phase 38 consumes but does not replace.

</canonical_refs>

<code_context>
## Existing Code Insights

### Current Lean Shape

- `Verified p` currently remains a conjunction of checker predicates:
  `finite_bounds`, `builder_events_typed`, and `no_ambient_authority`.
- `accepted_program_safe` currently proves only `Verified p -> Safe p` where
  `Safe` is still a structural predicate, not a modeled execution theorem.
- Phase 37 added expression typing, `LetScalar`, classifier rows, and
  Lean/Rust correspondence output. Those should remain intact.

### Semantics Targets

- A useful modeled event type can mirror existing `ArrowEvent`:
  `appendValue`, `appendNull`, and `finish`, but Phase 38 only needs enough
  structure to prove emitted events are well-typed for declared builders.
- A useful modeled execution state can track scalar environment, builder events,
  consumed row budget, and terminal/fail-closed status.
- A useful input model can abstract over values by capability/range rather than
  storing real bytes. The theorem needs safe access, not source correctness.

### Integration Points

- `lean formal/lean/LoomCore.lean` must remain the proof gate.
- The `#eval IO.println correspondenceReport` output from Phase 37 is expected;
  scripts filter it for correspondence rows.
- Phase 38 may add marker checks for `OperationalSemantics`, `modeled executor`,
  `0 sorry`, or theorem names if needed, but the Lean compile check is the main
  load-bearing gate.

</code_context>

<specifics>
## Specific Ideas

Recommended plan split:

| Plan | Scope | Acceptance Focus |
|---|---|---|
| 38-01 | Lean modeled operational semantics | Modeled inputs, execution state, statement/body execution, terminal/fail-closed semantics, termination/fuel evidence, Lean compile |
| 38-02 | Semantic soundness theorem and closeout | `accepted_program_safe` semantic theorem, no `sorry`, scope note, LINEAGE-05/06, gate/docs closeout |

Recommended theorem posture:

```lean
-- Names are illustrative, not mandatory.
def ModeledExecutionSafe (p : Program) : Prop := ...

theorem accepted_program_safe (p : Program) :
    Verified p -> ModeledExecutionSafe p := ...
```

The theorem should explicitly be about modeled execution and should not mention
Rust interpreter traces or native execution as proven behavior.

</specifics>

<deferred>
## Deferred Ideas

- Rust interpreter/model event-trace validation remains Phase 39.
- Native/model validation remains Phase 40.
- Solver proof objects, compiler correctness, host correctness, and ABI
  correctness remain outside Phase 38.
- Any source-data correctness or performance theorem remains out of scope.

</deferred>

---

*Phase: 38-Lean Stage C - Operational Semantics and Soundness Theorem*
*Context gathered: 2026-06-09*
