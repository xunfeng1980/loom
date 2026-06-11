# Loom Assurance Roadmap — Phases A–D

Concise execution doc for Loom's **verification / trust posture** after the
`loom-solver-smt` (Bitwuzla) removal. This is the *assurance strategy* that sits
above the detailed GSD phase list in `ROADMAP.md`; it defines what is trusted,
what only provides evidence, and what each phase may and may not claim.

## Core principle

**Safety comes from restriction, not from proof.** Loom is a non-Turing-complete,
total-function decoder IR whose only output is well-formed Arrow. We do not prove
arbitrary programs safe; we *accept only a narrow slice* and *lower only ops that
have a fixed, hand-verified safe rule*. Anything outside → reject / fallback.

The K spec-oracle (`kloom`) and the Rust-interpreter differential are **evidence**,
not part of the Trusted Computing Base (TCB). They run offline / in CI to raise
confidence; they never gate a production decision.

> Resolved design question: a component declared "outside the TCB" must not decide
> a production verifier fact. `constraints_discharged` must therefore stop being
> sourced from `kloom_discharged`. See "Reconciliation debt" below.

## Trust boundary

| In-TCB (trusted, gates production) | Out-of-TCB (evidence only, never gates) |
|---|---|
| Rust artifact/container/schema verifier (fail-closed) | `kloom` K spec-oracle trace |
| Narrow verifier-accepted slice | Rust-interpreter differential |
| Fixed per-op safe lowering rules | Fuzz corpus / `corpus_validated` |
| Bounded resource budget (rows, steps, builder events) | Lean modeled-soundness theorem (modeled executor only) |

## Phases

| Phase | Goal | Delivers | Production gate | Does NOT claim |
|---|---|---|---|---|
| **A — Decoder IR MVP** | Total decoder IR, no SMT | `L2Core` verifier accepting a narrow slice; no Bitwuzla, no SMT discharge; `kloom` = CI differential only | Verifier **acceptance** of the narrow slice (deterministic, pure function of the artifact) | No proof of no-overflow; no end-to-end correctness |
| **B — Native lowering** | Lower only what is accepted | One **fixed safe lowering rule per supported op**; unsupported shape → fallback / reject | Lowering readiness = `accepted ∧ supported-shape-has-a-rule` (in-TCB, deterministic) | No general compiler correctness; no JIT/perf claim |
| **C — Evidence layer** | Confidence the rules + decoder are right | `kloom` trace oracle, Rust-interpreter differential, fuzz corpus → `corpus_validated` lineage evidence | **None** — evidence is recorded in `VerifiedLineageRecord`, never gates A/B | `corpus_validated ≠ proven`; bounded by corpus coverage |
| **D — Optional future proof** | Real machine-checked discharge (only if needed) | In-TCB **bounded prover** for AddNoOverflow / InRange / Decreases (everything is bounded uint64 + known row caps ⇒ decidable) | Would upgrade `discharged` to a genuine proof result | — (new milestone; **does not block MVP**) |

## Invariants (must hold across A–C)

1. **No out-of-TCB input gates a production fact.** `kloom` / interpreter / corpus
   feed evidence layers only. Production verify must not spawn `krun`.
2. **Production verdicts are deterministic** — a pure function of the artifact and
   the in-TCB verifier, independent of whether the oracle toolchain is installed.
3. **Lowering safety = acceptance × a fixed rule.** B never lowers a shape without a
   pre-verified rule; B and C are coupled — C is what validates B's rules.
4. **Honest naming.** With SMT removed, nothing in-TCB "discharges" constraints in
   A–C. Emitted obligations (AddNoOverflow, InRange, …) are **evidence targets**,
   not proof claims. Do not label them `discharged`.
5. **Total lowering semantics make "no SMT" sound.** Overflow/range are safe because
   the lowering uses total uint64 semantics (wrapping/checked) that match Arrow and
   are confirmed by the C evidence layer — not because a solver proved them.
6. **Non-claims stay loud.** Lineage keeps "no end-to-end / no verified compilation /
   no production-readiness" non-claims; C's optimistic wording must not erode them.

## Reconciliation debt (left by the solver removal; reconcile to land Phase A/B/C)

Current code still wires the old SMT model and must be migrated to the table above:

1. `constraints_discharged = kloom_discharged` — out-of-TCB signal gating production.
   → derive production facts from in-TCB acceptance; demote `kloom` result to a
   renamed evidence signal (e.g. `spec_oracle_trace_validated`).
2. `verify` spawns `krun` synchronously — move trace extraction to the C/CI
   differential harness; production verify stays oracle-free.
3. Lowering readiness gates on `constraints_discharged` — gate on
   `support.is_supported()` (accepted slice + fixed rule) instead.
4. `native_arrow_semantic` hardcodes `constraints_discharged: true` while the
   full-verifier path sources it from the oracle — unify under the new semantics.
5. Stale tests / docs assert the Bitwuzla-SMT contract (verified_lineage
   undischarged-gate test; README/Lean/`.mise.toml` "Bitwuzla-backed SMT evidence"
   wording) — update to the evidence-layer model.

## Out of scope for MVP (Phase D only)

- Re-introducing any in-TCB solver/prover.
- Claiming machine-checked no-overflow / no-bad-state in production.
- Treating `corpus_validated` as a soundness proof.
