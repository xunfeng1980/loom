# Phase 37: Lean Stage B - Lean Rust Verifier Correspondence - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous discuss; recommended defaults selected from roadmap, Phase 36 contract, and prior Lean verifier caveats

<domain>
## Phase Boundary

Phase 37 is a correspondence phase. Its job is to make the Lean checker mirror
the executable Rust verifier's current accepted L2Core surface and to
continuously cross-check the two. It must consume the Phase 36 verified-lineage
contract and produce evidence for the Lean<->Rust verifier seam only.

This phase must not add operational semantics, a soundness theorem, native
execution validation, model-to-interpreter validation, or a broader L2Core
language than Rust already accepts. Those belong to later phases. A Lean model
that accepts or rejects a different language from Rust is the primary risk this
phase removes.

</domain>

<decisions>
## Implementation Decisions

### Lean AST And Typing Parity

- **D-37-01:** Enrich `formal/lean/LoomCore.lean` with `ScalarExpr` and
  `LetScalar` concepts that mirror the Rust verifier slice in
  `crates/loom-core/src/l2_core.rs` / `full_verifier.rs`.
- **D-37-02:** Lean `ReadInput`, `AppendValue`, loop bounds, cursor limits, and
  cursor progress should carry scalar expressions where Rust does. Avoid the
  current flattened `Nat` / explicit-output-type projection for new parity
  evidence.
- **D-37-03:** `builder_events_typed` must derive appended value types from the
  value expression through a Lean scalar type environment, matching Rust
  `type_of_expr`, instead of accepting an explicit `Stmt.appendValue` type.
- **D-37-04:** Lean must model `LetScalar` insertion and `UnknownVariable`
  rejection for the static checker slice. Unknown variables are no longer an
  allowed Lean non-modeling caveat for Phase 37 parity.
- **D-37-05:** Overflow and solver-only obligations remain delegated to the
  existing Rust/Bitwuzla evidence unless a plan deliberately models them as
  rejected/delegated obligations. Phase 37 should not invent a Lean proof of
  bitvector arithmetic soundness.

### Differential Harness

- **D-37-06:** Build a deterministic repo-local Lean<->Rust differential harness
  over the current full verifier fixture matrix plus a generated bounded fuzz
  corpus.
- **D-37-07:** The harness must compare both accept/reject result and stable
  reject classification. At minimum it must cover
  `MissingInputCapability`, `MissingOutputBuilder`, `InvalidLoopBounds`,
  `NonMonotoneCursorLoop`, and `ResourceBudgetExceeded`.
- **D-37-08:** Include additional current Rust verifier codes when practical:
  `UnknownVariable`, `OutputTypeMismatch`, `OutputNullabilityMismatch`, and
  `ConstraintBudgetExceeded`. Required roadmap codes remain the hard acceptance
  floor.
- **D-37-09:** Fail closed on any divergence, including cases where one checker
  accepts and the other rejects, or both reject with different classifications.
- **D-37-10:** Prefer shared fixtures or a shared fixture manifest consumed by
  both Lean and Rust. Do not require a verified extraction pipeline or a new
  proof language in Phase 37.

### Gate Wiring

- **D-37-11:** Add a focused correspondence gate, then wire it into the existing
  verifier/release path. `scripts/full-verifier-test.sh` is the current natural
  integration point, but the planner may choose a new focused script if that
  keeps the checks clearer.
- **D-37-12:** The gate must run `lean formal/lean/LoomCore.lean` and the
  differential comparison in CI/release-gate style, not as an optional advisory
  report.
- **D-37-13:** The gate should remain deterministic and bounded. Randomized fuzz
  is acceptable only if generated from a stable seed and materialized or
  reproducible in a way that keeps failures debuggable.

### Scope Boundary

- **D-37-14:** Phase 37 proves correspondence evidence only. It does not claim
  real executor safety, native backend correctness, source correctness, or a
  full formal soundness theorem.
- **D-37-15:** Any mismatch between Lean and Rust should be resolved by changing
  Lean to match the current Rust verifier unless a genuine Rust verifier bug is
  found and fixed with tests. Do not broaden Rust L2Core to satisfy Lean.
- **D-37-16:** Documentation should say that Lean is no longer a lossy AST
  projection for the covered static checker slice, while preserving explicit
  non-claims for later soundness and executor phases.

### the agent's Discretion

- Choose whether the differential corpus is encoded as Lean declarations, JSON,
  Rust fixture data, or generated text artifacts, provided both sides consume the
  same cases or an auditable shared source.
- Choose exact helper names and module layout for Lean scalar typing.
- Choose whether additional reject codes beyond the roadmap floor are included
  in the first or second plan.
- Keep edits scoped to `formal/lean`, `loom-core` tests/helpers, scripts, and
  planning/docs needed for Phase 37.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope And Contract

- `.planning/ROADMAP.md` - Phase 37 goal, success criteria, non-goals, and
  ordering decision.
- `.planning/STATE.md` - Current focus and existing caveats around the Lean
  scaffold and Rust executable verifier evidence.
- `.planning/REQUIREMENTS.md` - LINEAGE-03 and LINEAGE-04 requirements.
- `.planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` -
  Canonical evidence-lineage contract and seam ownership matrix.

### Lean And Rust Verifier Sources

- `formal/lean/LoomCore.lean` - Current Lean checker. It explicitly calls out the
  lossy `Nat`-grounded projection and missing `ScalarExpr` / `LetScalar` /
  `UnknownVariable` modeling.
- `crates/loom-core/src/l2_core.rs` - Rust L2Core AST, including `ScalarExpr` and
  `L2CoreStmt::LetScalar`.
- `crates/loom-core/src/full_verifier.rs` - Executable Rust verifier,
  `type_of_expr`, scalar environment handling, bounds/resource diagnostics, and
  stable `FullVerificationCode` values.
- `crates/loom-core/tests/full_verifier.rs` - Current verifier acceptance and
  negative fixture matrix.
- `crates/loom-core/tests/l2_core_model.rs` - Current L2Core model fixture
  coverage and sample construction patterns.
- `scripts/full-verifier-test.sh` - Current Lean/Rust/TLA verifier gate and the
  likely Phase 37 integration point.

### Prior Evidence And Non-Claims

- `.planning/phases/13-full-verifier-foundation/13-FINAL-REPORT.md` - Original
  Lean scaffold caveats and verifier foundation closeout.
- `.planning/phases/19-solver-backed-full-artifact-verifier/19-SOLVER-CONTRACT.md` -
  Solver-backed obligation semantics and Bitwuzla trust boundary.
- `.planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-CONTEXT.md` -
  Phase 37 assignment for the Lean<->Rust verifier seam.

</canonical_refs>

<code_context>
## Existing Code Insights

### Current Lean Shape

- `formal/lean/LoomCore.lean` has load-bearing Boolean checkers, but over a
  pre-resolved AST projection:
  - `Stmt.readInput` takes concrete `Nat` offset/width.
  - `Stmt.appendValue` takes an explicit `L2Ty`.
  - `Stmt.forRange` and `Stmt.cursorLoop` carry concrete `Nat` bounds.
  - There is no scalar expression environment and no `LetScalar`.
- The file explicitly says `UnknownVariable` and `LetScalar` are not modeled.
  Phase 37 should remove that caveat for the covered static verifier slice.

### Current Rust Shape

- `ScalarExpr` currently includes constants, variables, arithmetic, min/max, and
  comparison expressions: `Const`, `Var`, `Add`, `Sub`, `Mul`, `Min`, `Max`,
  `Eq`, `Lt`, and `Le`.
- `L2CoreStmt::LetScalar { name, expr }` inserts scalar types into the verifier
  state.
- `ReadInput` uses scalar expressions for `offset` and `width`, and binds a
  scalar read type derived from width.
- `AppendValue` uses a scalar expression and compares the expression-derived
  type to the declared output builder type.
- Stable Rust reject codes include:
  `MissingInputCapability`, `MissingOutputBuilder`, `UnknownVariable`,
  `OutputTypeMismatch`, `OutputNullabilityMismatch`, `InvalidLoopBounds`,
  `NonMonotoneCursorLoop`, `ResourceBudgetExceeded`, and
  `ConstraintBudgetExceeded`.

### Integration Points

- The current verifier gate already runs:
  - `cargo test -p loom-core --test l2_core_model`
  - `cargo test -p loom-core --test full_verifier`
  - `lean formal/lean/LoomCore.lean`
  - TLA checks
- Phase 37 should add correspondence evidence without removing existing Rust,
  Lean, or TLA gate coverage.
- If a harness needs generated fixtures, keep generation deterministic and
  bounded so reviewers can reproduce the exact divergence case.

</code_context>

<specifics>
## Specific Ideas

Recommended plan split:

| Plan | Scope | Acceptance Focus |
|---|---|---|
| 37-01 | Lean AST enrichment and typing parity | `ScalarExpr`, `LetScalar`, type environment, expression-derived append typing, unknown-variable rejection, Lean compile gate |
| 37-02 | Differential harness and gate wiring | Shared corpus, Rust/Lean classification comparison, required reject-code coverage, release-gate integration |

Recommended Lean helper shape:

- `ScalarTy` or reuse `L2Ty` for scalar expression typing where Rust does.
- `ScalarEnv := List (String × L2Ty)` or another simple lookup structure.
- `typeOfExpr? : ScalarEnv -> ScalarExpr -> Option L2Ty`.
- `checkTypedStmt` should thread scalar environment through `letScalar` and
  derive `appendValue` type from `typeOfExpr?`.
- Negative examples should make the expected code visible enough for the
  differential harness to compare classification, not just Boolean acceptance.

Recommended harness posture:

- Start from existing Rust verifier examples and add a small deterministic fuzz
  generator for expressions/statements within the current Rust verifier surface.
- Materialize or snapshot the classification rows if that makes Lean integration
  simpler.
- Compare `accepted` plus `reject_code` string. Treat missing, extra, or
  differently classified cases as failures.

</specifics>

<deferred>
## Deferred Ideas

- Operational semantics and soundness theorem remain Phase 38.
- Model-to-Rust interpreter event trace validation remains Phase 39.
- Native-to-model validation remains Phase 40.
- Proof extraction, verified checker lineage, Rocq fallback, and solver proof
  objects remain deferred unless a later roadmap phase explicitly activates
  them.
- Broader L2Core language design is out of scope; Phase 37 mirrors current Rust,
  not a future IR.

</deferred>

---

*Phase: 37-Lean Stage B - Lean Rust Verifier Correspondence*
*Context gathered: 2026-06-09*
