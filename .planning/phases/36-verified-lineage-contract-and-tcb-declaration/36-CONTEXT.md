# Phase 36: Verified-Lineage Contract and TCB Declaration - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous discuss; recommended defaults selected from roadmap and prior claim-audit decisions

<domain>
## Phase Boundary

Phase 36 starts the MVP1.5 Verified Lineage milestone by defining what Loom is
allowed to mean by "verified" at MVP1.5 exit. It is a normative contract phase:
write the evidence taxonomy, obligation matrix, and TCB clause before any later
Lean/Rust/model/native proof or validation work proceeds.

This phase must preserve the standing red line: Loom guarantees safety and
well-formedness, never correctness. It must not add proofs, production code, new
execution features, broader format support, host integration, or native speed
claims. Its job is to name the evidence layers and trust gaps so later phases
cannot silently upgrade scaffolded or bounded evidence into a stronger product
claim.

</domain>

<decisions>
## Implementation Decisions

### Meaning Of Verified

- **D-36-01:** "Verified" must be defined as an evidence-lineage statement for
  safety and Arrow well-formedness only. It must never mean source-data
  correctness, semantic equivalence to an upstream format, performance, or
  production readiness.
- **D-36-02:** Each in-scope safety claim must map to exactly one named evidence
  layer: Rust verifier structural check, Bitwuzla SMT discharge, Lean soundness
  theorem, differential validation, or explicit TCB trust assumption.
- **D-36-03:** Scaffolded, bounded, skipped, fallback-only, and deferred evidence
  must remain labeled with those statuses. The Phase 32 claim-ledger vocabulary
  is the preferred prior art for this taxonomy.

### TCB Clause

- **D-36-04:** The TCB must explicitly list Rust compiler/std, LLVM + MLIR
  toolchain, the Rust<->C ABI boundary, DuckDB host process, and Arrow C Data
  Interface.
- **D-36-05:** Each TCB item needs a one-line "what is assumed" and one-line "why
  it is not proven here" statement. Avoid vague blanket trust language.
- **D-36-06:** The Rust+C++/MLIR/LLVM/toolchain gap remains a permanent TCB
  assumption unless a future phase explicitly narrows a specific sub-gap.

### Obligation Matrix

- **D-36-07:** The matrix must enumerate the three roadmap trust seams:
  Lean<->Rust verifier, static<->dynamic, and modeled-executor<->real-executor.
- **D-36-08:** Each seam must be assigned either to a later MVP1.5 phase or to
  the TCB. Do not leave an unowned seam.
- **D-36-09:** Phase assignments should follow the roadmap: Phase 37 handles
  Lean<->Rust verifier correspondence, Phase 38 handles modeled soundness,
  Phase 39 handles model<->Rust interpreter consistency, and Phase 40 handles
  native<->model validation. Anything not handled there must be named as TCB.

### Deliverable And Verification Shape

- **D-36-10:** Prefer one normative contract document plus a concise final
  summary, rather than scattering the definition across many artifacts. The
  contract should include sections named Scope, Evidence Layers, Claim Mapping,
  TCB, Obligation Matrix, Non-Claims, and Downstream Phase Handoff.
- **D-36-11:** Verification for this phase should be documentation/marker based:
  targeted `rg` checks for required sections and non-claim wording, plus
  `git diff --check`. No proof or execution gate should be invented in Phase 36.
- **D-36-12:** Public/planning docs may introduce LINEAGE-01/LINEAGE-02
  requirements, but must not rewrite the completed MVP1 story or weaken older
  non-claims.

### the agent's Discretion

- Choose the exact contract filename and table layouts, provided downstream
  phases have a single canonical file to read.
- Choose whether LINEAGE-01 and LINEAGE-02 are represented only in
  REQUIREMENTS.md or also in the contract document frontmatter.
- Choose concise wording for public docs if needed, provided the standing red
  line is visible and the phase remains docs-only.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope And Milestone Boundary

- `.planning/ROADMAP.md` — Phase 36 goal, success criteria, non-goals, ordering
  decision, and MVP1.5 standing red line.
- `.planning/STATE.md` — Current focus and accumulated caveats from Phase 12,
  Phase 13, Phase 19, Phase 32, and Phase 35.
- `.planning/PROJECT.md` — Project value statement, explicit non-claims,
  architecture constraints, and decisions on Lean scaffold and solver evidence.
- `.planning/REQUIREMENTS.md` — Existing safety/formal/solver requirements and
  exclusions that Phase 36 must not contradict.

### Claim-Audit Prior Art

- `.planning/phases/32-mvp1-architecture-and-code-review/32-CLAIM-LEDGER.md` —
  Status taxonomy for proven/bounded/fallback/scaffold/skipped/deferred/
  unsupported/incorrect claims.
- `.planning/phases/32-mvp1-architecture-and-code-review/32-MVP1-RELEASE-READINESS.md` —
  Bounded GO decision and required handling for unsupported/bounded native,
  query, LMC2, and StarRocks claims.
- `.planning/phases/32-mvp1-architecture-and-code-review/32-ARCHITECTURE-BOUNDARY-REVIEW.md` —
  Architecture boundary findings and trust-boundary review input.

### Solver And Lean Evidence

- `.planning/phases/19-solver-backed-full-artifact-verifier/19-SOLVER-CONTRACT.md` —
  Solver-backed obligation semantics, Bitwuzla primary path, and facts trust
  rule.
- `.planning/phases/19-solver-backed-full-artifact-verifier/19-SOLVER-REPORT.md` —
  Actual solver evidence, skip policy, and Phase 20 discharged-facts handoff.
- `formal/lean/LoomCore.lean` — Current Lean checker scope and explicit note
  that it is a bounded lossy AST projection, not full L2Core soundness.
- `scripts/full-verifier-test.sh` — Current Lean scaffold/checker gate.
- `scripts/solver-verifier-test.sh` — Current Bitwuzla-backed solver gate.

### Recent Native And Arrow Semantic Boundaries

- `.planning/phases/35-native-arrow-semantic-execution/35-CONTEXT.md` — Native
  Arrow semantic route is engine-neutral and bounded to primitive nullable
  shapes.
- `.planning/phases/35-native-arrow-semantic-execution/35-NATIVE-ARROW-SEMANTIC-REPORT.md` —
  Completed native evidence and explicit non-claims.
- `.planning/phases/34-duckdb-arrow-semantic-sql-surface-for-lmc2-lma1/34-CONTEXT.md` —
  Queryability is separate from native execution.
- `.planning/phases/33-lmc2-arrow-semantic-container-wrapper/33-CONTEXT.md` —
  `LMC2(LMA1)` distribution wrapper decisions and direct `LMA1` bridge boundary.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `formal/lean/LoomCore.lean` already documents the current Lean checker as a
  bounded projection and names obligations that remain SMT-only or unmodeled.
- `crates/loom-core/tests/solver_contract.rs`,
  `crates/loom-core/tests/smtlib_emitter.rs`, and
  `crates/loom-core/tests/artifact_solver_discharge.rs` provide current solver
  obligation vocabulary and discharge invariants.
- Existing phase reports use stable sections and command lists; Phase 36 should
  reuse that style for a reviewer-readable contract.

### Established Patterns

- Loom documents narrow positive evidence and explicit non-claims rather than
  inferring broad guarantees from phase names.
- Accepted artifact facts become trusted only after the relevant verifier or
  solver evidence has accepted them.
- Skipped toolchain evidence is allowed only when explicitly labeled and never
  upgrades a proof/discharge claim.
- Public docs should say what a gate proves and what it does not prove.

### Integration Points

- `.planning/REQUIREMENTS.md` needs LINEAGE-01 and LINEAGE-02 entries once the
  contract is executed.
- `.planning/ROADMAP.md` and `.planning/STATE.md` should move Phase 36 from not
  started to complete only after the contract and matrix exist.
- Future Phase 37-40 plans should cite the Phase 36 contract as their authority
  for evidence naming and seam ownership.

</code_context>

<specifics>
## Specific Ideas

Recommended contract title: `36-VERIFIED-LINEAGE-CONTRACT.md`.

Recommended claim-map shape:

| Claim Family | Evidence Layer | Current Source | MVP1.5 Owner | Non-Claim |
|---|---|---|---|---|
| Artifact structural acceptance | Rust verifier structural check | `verify_artifact` / tests | Existing + Phase 37 parity | Source correctness |
| Arithmetic/range bad states | Bitwuzla SMT discharge | Phase 19 | Existing + later cross-checks | Checked proof objects |
| Modeled executor safety | Lean soundness theorem | Future Phase 38 | Phase 38 | Rust/native correctness |
| Real executor consistency | Differential validation | Future Phase 39/40 | Phase 39/40 | Performance |
| Toolchain/host ABI assumptions | Explicit TCB | Phase 36 | TCB | Proven compiler/host correctness |

</specifics>

<deferred>
## Deferred Ideas

- Lean AST enrichment and Rust verifier correspondence belong to Phase 37.
- Operational semantics and soundness theorem belong to Phase 38.
- Model-to-Rust interpreter validation belongs to Phase 39.
- Native-to-model validation belongs to Phase 40.
- Any production hardening, ABI freeze, signing, remote fetch, or broader
  format/native coverage belongs to later roadmap phases.

</deferred>

---

*Phase: 36-Verified-Lineage Contract and TCB Declaration*
*Context gathered: 2026-06-09*
