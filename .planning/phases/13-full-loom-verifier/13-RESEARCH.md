# Phase 13 Research: Full Loom Verifier

**Status:** Research report
**Date:** 2026-06-08
**Phase:** 13 — Full Loom Verifier
**Depends on:** Phase 12 current-boundary Safety Proof MVP

## Executive Summary

Phase 13 should not jump directly to one proof assistant or one theorem-proving style. The full Loom verifier needs a layered assurance stack:

1. **Normative Loom spec:** a small mathematical core for distribution IR, L1 layout, L2 total-function language, module contracts, resource bounds, and Arrow builder events.
2. **Executable verifier:** a practical Rust verifier that rejects unsafe artifacts before decode/lowering.
3. **Mechanized meta-theory:** a proof assistant model showing that verifier acceptance implies safety properties such as memory capability discipline, output well-formedness, and termination.
4. **SMT/abstract-interpretation engine:** an automated checker for bounds, ranges, monotone progress, and resource budgets.
5. **Translation-validation hook:** later, Phase 14 native lowering should prove or check that lowered MLIR/native code refines the verified Loom semantics.

Recommended Phase 13 direction:

- Use **abstract interpretation + type/effect checking** as the verifier algorithm.
- Use **SMT** for local arithmetic/range obligations.
- Use **Lean or Rocq/Coq** for mechanized language meta-theory, with Lean favored if the project wants stronger math-library ergonomics and algebraic formalization, and Rocq favored if extraction/verified interpreter lineage matters more.
- Use **TLA+** only for lifecycle/state-machine aspects: container loading, feature negotiation, module cache/trust states, proof-carrying artifact workflow, and verifier/lowering pipeline invariants.
- Treat **geometric algebra / Clifford algebra formalization** as evidence that Lean/Rocq can formalize rich algebraic structures, but not as the core model for Loom unless future L2 kernels need algebraic simplification/proof of vectorized identities.

The first Phase 13 MVP should be a complete verifier for a deliberately tiny future Loom language subset, not the whole final design at once.

## Loom-Specific Target

Phase 12 proved only the current implementation boundary:

```text
LMC1/LMP1/LMT1 bytes
  -> checked parse
  -> structural verifier
  -> decode helpers
  -> Arrow output
```

Phase 13 expands the target to the **full Loom verifier**:

```text
Loom distribution artifact
  -> version/feature/container validation
  -> L1 declarative layout verification
  -> L2 total-function language verification
  -> module/kernel contract verification
  -> resource-bound proof
  -> Arrow builder event type/effect proof
  -> lowering preconditions for Phase 14
```

Important: Phase 13 should still verify **safety and well-formedness**, not semantic correctness of arbitrary third-party decoder intent. Correctness remains outside the core verifier unless separately supplied by oracle tests, checksums, or producer-specific proofs.

## Evaluation Criteria

| Criterion | What Loom Needs |
|---|---|
| Soundness | Accepted artifacts cannot read outside declared input, escape capabilities, write arbitrary output memory, or produce malformed Arrow events. |
| Termination | L2 loops must be count-bounded or data-monotone with a verifier-visible decreasing measure. |
| Resource bounds | Verifier computes finite upper bounds for input ranges, output rows, scratch memory, and kernel work. |
| Distribution stability | Spec must survive decades; avoid encoding host-specific details or MLIR/LLVM details in the distribution layer. |
| Automation | Common cases must verify without human proof work. |
| Explainability | Rejection diagnostics must map to artifact locations and proof obligations. |
| Implementability | The verifier must be practical in Rust and gate artifacts before decode/lowering. |
| Future lowering | Phase 14 must be able to consume verifier facts as lowering preconditions. |

## Method Family 1: TLA+ / Temporal Modeling

### What It Is

TLA+ is a high-level formal specification language based on Temporal Logic of Actions. It is especially strong for state machines, invariants, refinement, and safety/liveness reasoning over behaviors. Lamport's material frames TLA+ as a language for specifying and checking systems, while TLC and Apalache provide model-checking workflows.

Sources:

- TLA+ Hyperbook: https://lamport.azurewebsites.net/tla/hyperbook.html
- Lamport, *Specifying and Verifying Systems With TLA+*: https://lamport.org/pubs/spec-and-verifying.pdf
- Apalache supported features: https://apalache-mc.org/docs/apalache/features.html
- TLC model checking paper: https://www.microsoft.com/en-us/research/wp-content/uploads/2016/12/Model-Checking-TLA-Specifications.pdf

### Fit For Loom

Good for:

- Artifact lifecycle: fetched, parsed, verified, cached, lowered, invalidated.
- Feature negotiation: required vs optional feature flags.
- Trust workflow: unsigned artifact, signed artifact, hash-known artifact, locally verified artifact.
- Pipeline invariants: "native lowering only happens after verifier acceptance".
- Refinement model: raw artifact state -> verified semantic artifact state -> lowerable artifact state.

Weak for:

- Byte-level parser correctness.
- Full L2 language soundness.
- Arrow builder event typing.
- Unbounded mathematical proofs; TLC is finite-state, and Apalache is symbolic but still model-checking oriented.

### Recommendation

Use TLA+ as an **architecture-level guardrail**, not as the core verifier proof. It should answer "can the system reach a bad workflow state?" rather than "is the L2 type system sound?"

Deliverable idea:

- `13-TLA-PIPELINE.md`
- `specs/tla/LoomVerifierPipeline.tla`
- Model invariant: `Lowered => Verified /\ FeatureSetAccepted /\ ResourceBounded`

## Method Family 2: Mathematical Axiomatic Formalization

### What It Is

This family defines Loom's semantics as mathematical objects: inductive syntax, typing judgments, small-step or big-step semantics, safety theorems, and proof obligations. It can be done in dependent type theory, higher-order logic, or first-order logic with theories.

Relevant tools:

- **Rocq/Coq:** interactive theorem prover, dependent type theory, program extraction.
- **Lean:** dependent type theory with a large math library.
- **Isabelle/HOL:** higher-order logic, mature for large systems proofs.
- **ACL2/PVS/HOL family:** useful but less obviously aligned with this Rust/IR project.

Sources:

- Rocq official overview and extraction: https://rocq-prover.org/ and https://docs.rocq-prover.org/master/refman/addendum/extraction.html
- Lean theorem proving and mathlib: https://leanprover.github.io/theorem_proving_in_lean/ and https://lean-lang.org/use-cases/mathlib
- Isabelle official site: https://isabelle.in.tum.de/
- seL4 assumptions/proof stack using Isabelle/HOL: https://sel4.systems/Verification/assumptions.html
- CompCert verified compiler in Coq/Rocq: https://compcert.org/ and https://compcert.org/compcert-C.html
- WebAssembly mechanizations in Isabelle and Coq: https://www.doc.ic.ac.uk/~pg/publications/Watt2021Two.html

### Fit For Loom

Good for:

- Defining final Loom IR precisely.
- Proving type soundness: accepted L2 programs cannot go wrong.
- Proving progress/preservation-style properties.
- Proving termination by structural recursion or ranking functions.
- Proving Arrow builder event well-formedness.
- Proving verifier soundness for a small core language.

Weak for:

- Rapid iteration.
- Whole production verifier proof in one phase.
- Day-to-day Rust code proof unless paired with a Rust verifier tool.

### Rocq/Coq vs Lean vs Isabelle

| Tool | Strength | Loom Risk |
|---|---|---|
| Rocq/Coq | Strong lineage for verified compilers, mechanized programming languages, extraction, CompCert, WasmCert-Coq. | Proof engineering can be heavy; math-library ergonomics often less smooth than Lean for algebra-heavy work. |
| Lean | Excellent mathlib, good for algebraic/spec formalization, growing automation, pleasant theorem statements. | Less historical weight for verified compiler-scale implementation than Coq/Isabelle; extraction/story for production verifier is not the main path. |
| Isabelle/HOL | Mature large-systems proof environment; seL4-scale evidence; good for operational semantics and refinement. | Separate ecosystem from Rust and dependent-type programming; less direct extraction into project workflow. |

### Recommendation

For Phase 13, choose one mechanized proof kernel target:

- **Lean-first** if the proof is mainly a clean mathematical specification of the Loom language, algebraic laws, and verifier theorem statements.
- **Rocq-first** if the proof will grow toward executable verified checker extraction or CompCert/WasmCert-style verified interpreter/compiler lineage.
- **Isabelle-first** only if the project expects seL4-style refinement proofs to dominate.

My recommendation for Loom is **Lean for the spec and soundness model, plus Rust SMT/abstract interpretation for implementation**, unless Phase 13 explicitly aims to extract a certified verifier, in which case choose Rocq.

## Method Family 3: SMT / Deductive Verification

### What It Is

SMT solvers discharge first-order proof obligations over decidable theories such as integer arithmetic, bitvectors, arrays, algebraic datatypes, and uninterpreted functions. Deductive verifiers generate verification conditions and send them to SMT solvers.

Sources:

- Z3 official page: https://www.microsoft.com/en-us/research/project/z3-3/
- SMT-LIB standard: https://smt-lib.org/
- Boogie intermediate verification language: https://www.microsoft.com/en-us/research/project/boogie-an-intermediate-verification-language/
- Dafny reference: https://dafny.org/dafny/DafnyRef/DafnyRef
- Why3: https://www.why3.org/
- F*: https://fstar-lang.org/ and https://fstar-lang.org/tutorial/book/intro.html

### Fit For Loom

Good for:

- Range checks: `offset + length <= input_len`, no integer overflow.
- Loop-bound obligations: `variant` decreases, `count` finite.
- Buffer slicing proof obligations.
- Feature set constraints.
- Table/schema consistency.
- Local L2 instruction constraints.

Weak for:

- Deep language meta-theory with binders/effects unless carefully encoded.
- Nonlinear arithmetic or quantified array properties.
- Trustworthy proof objects, unless proof production/checking is added.

### Recommendation

Use SMT as the **automation backend**, not the foundational story. The full verifier should produce small, local SMT queries whose models can be surfaced in diagnostics.

Likely Rust implementation options:

- Direct SMT-LIB generation.
- Z3 via a Rust crate.
- A small internal constraint IR that can target Z3/cvc5 later.

Deliverable idea:

- `loom-core::verifier::constraints`
- `LoomConstraint::{Le, Lt, AddNoOverflow, InRange, Decreases, FeatureImplies}`
- optional `--explain-smt` CLI mode for debugging rejected artifacts.

## Method Family 4: Rust-Centric Program Verification

### What It Is

These tools verify Rust implementations directly or semi-directly.

Sources:

- Verus guide: https://verus-lang.github.io/verus/guide/
- Creusot: https://creusot.rs/
- Kani: https://model-checking.github.io/kani/usage.html
- Prusti verification pipeline: https://viperproject.github.io/prusti-dev/dev-guide/pipeline/summary.html
- RustBelt/Iris/Coq paper: https://plv.mpi-sws.org/rustbelt/popl18/paper.pdf

### Fit For Loom

Good for:

- Proving properties of the actual Rust verifier implementation.
- Checking bounded parser/verifier functions.
- Preventing panics/overflows beyond tests.
- Incrementally verifying hot safety functions.

Weak for:

- Full language soundness theorem.
- Tool maturity and setup friction.
- Heavy dependency/toolchain integration into normal Rust CI.

### Tool Notes

| Tool | Best Use In Loom |
|---|---|
| Kani | Bounded model checking of parser/verifier edge cases, bitvector-heavy invariants, panic/overflow checks. |
| Creusot | Deductive verification of Rust functions via Why3, especially pure-ish verifier logic. |
| Verus | Strong candidate for writing or mirroring a verified Rust verifier core with specs close to code. |
| Prusti | Useful reference point; likely less central than Verus/Creusot for new work. |
| RustBelt/Iris | Foundational background for Rust safety, not a day-one implementation tool. |

### Recommendation

Phase 13 should run a **small Verus or Creusot spike** against one verifier component:

- container feature validation,
- byte-range section directory checks,
- L2 loop variant checker,
- Arrow builder event type checker.

Do not try to verify the entire Rust codebase in Phase 13.

## Method Family 5: Abstract Interpretation And Type/Effect Systems

### What It Is

Abstract interpretation computes conservative facts about all possible executions. It underlies many practical verifiers and static analyzers. Type/effect systems encode safety rules in a compositional way.

Sources:

- eBPF verifier overview: https://docs.ebpf.io/linux/concepts/verifier/
- Linux kernel BPF verifier docs: https://docs.kernel.org/bpf/verifier.html
- Astrée static analyzer: https://www.astree.ens.fr/
- Cousot, formal verification by abstract interpretation: https://www.di.ens.fr/~cousot/publications.www/Cousot-NFM2012.pdf

### Fit For Loom

This is the strongest match for the **actual full Loom verifier algorithm**.

Loom's L2 language should be deliberately non-general. That means the verifier can maintain abstract states such as:

- input capabilities and allowed byte ranges,
- output builder state,
- known row counts,
- scratch budget,
- per-loop ranking measure,
- nullable/value validity facts,
- monotone input/output cursor progress,
- kernel contract pre/postconditions.

Good for:

- Fully automatic rejection/acceptance.
- Explaining diagnostics.
- Scaling to artifacts without human proof work.
- eBPF-like "pass verifier, then run fast" workflow.

Weak for:

- The abstract interpreter implementation itself may be unsound unless separately proved or heavily tested.
- False rejects are possible if the abstraction is too coarse.

### Recommendation

Make abstract interpretation/type checking the **core executable verifier**:

```text
parse artifact
  -> check module signatures/features
  -> type/effect check L1/L2
  -> abstract-interpret L2 control flow
  -> solve local SMT obligations
  -> emit VerifiedArtifact facts
```

The mechanized proof target should be: if the abstract interpreter accepts, the Loom small-step semantics cannot violate safety.

## Method Family 6: Rewriting Logic / Executable Semantics

### What It Is

The K Framework defines executable language semantics using rewriting rules and matching logic. It can generate interpreters and analysis tools from semantics.

Sources:

- K Framework: https://kframework.org/
- K user manual: https://kframework.org/docs/user_manual/
- KPHP executable semantics example: https://phpsemantics.org/

### Fit For Loom

Good for:

- Rapidly defining executable semantics for a new IR.
- Testing semantic rules through execution.
- Potential symbolic execution.
- Producing a reference semantics independent of Rust.

Weak for:

- Adds a separate toolchain and semantics ecosystem.
- Not obviously the best fit for durable proof artifacts compared with Lean/Rocq/Isabelle.
- May be heavier than needed if Loom's L2 is intentionally tiny.

### Recommendation

K is useful as a **semantics prototyping spike** if the team wants executable specs quickly. It should not be the primary Phase 13 proof system unless K-generated semantics becomes a project goal.

## Method Family 7: Geometric Algebra / Clifford Algebra Formalization

### What It Is

Geometric algebra formalization usually means mechanizing Clifford algebras and related structures in proof assistants. This is not a verifier method by itself, but it is relevant to the user's question because it shows how rich algebraic structures can be formalized.

Sources:

- Formalizing Geometric Algebra in Lean: https://arxiv.org/abs/2110.03551
- Springer page for the Lean formalization: https://doi.org/10.1007/s00006-021-01164-1
- Lean mathlib Clifford algebra docs: https://leanprover-community.github.io/mathlib4_docs/Mathlib/LinearAlgebra/CliffordAlgebra/Basic.html
- Coq geometric algebra product formalization: https://www.researchgate.net/publication/268164315_Implementing_Geometric_Algebra_Products_with_Binary_Trees

### Fit For Loom

Good for:

- Proving algebraic identities for future vectorized kernels.
- Reasoning about Clifford/geometric algebra kernels if Loom ever supports them.
- Evidence that Lean/mathlib is strong for algebraic formalization.

Weak for:

- It does not solve memory safety, termination, capability discipline, or Arrow output well-formedness.
- It is not a natural foundation for a decoder verifier.

### Recommendation

Do not use geometric algebra as the core formalism for Phase 13. Keep it as:

- a reference point for Lean's algebra ecosystem,
- a future kernel-proof pattern,
- a possible proof style for algebraic rewrite validation.

## Method Family 8: Translation Validation And Verified Compilation

### What It Is

Verified compilation proves a compiler preserves semantics for all programs. Translation validation checks each compiled artifact/output against the source semantics.

Sources:

- CompCert verified C compiler: https://compcert.org/
- Alive2 LLVM optimization validation: https://github.com/AliveToolkit/alive2
- Alive2 memory-model paper: https://sf.snu.ac.kr/publications/alive2-mem.pdf
- CIRCT/MLIR SMT dialect: https://circt.llvm.org/docs/Dialects/SMT/
- MLIR SMT dialect: https://mlir.llvm.org/docs/Dialects/SMT/

### Fit For Loom

Good for Phase 14, not Phase 13 core:

- Validate Loom-to-MLIR lowering.
- Validate MLIR optimization passes for decode kernels.
- Check equivalence of native fast paths against verified Loom semantics.

Weak for Phase 13:

- Requires a defined lowering target.
- Does not by itself prove source artifact safety.

### Recommendation

Phase 13 must emit verifier facts in a form Phase 14 can consume:

- row count facts,
- range/capability facts,
- loop ranking facts,
- builder state facts,
- kernel contract facts.

Phase 14 can then either prove lowering once, or translation-validate each lowered program.

## Comparative Matrix

Scores: 5 = strongest fit, 1 = weakest fit.

| Method | Sound Core Spec | Automation | Rust Integration | Diagnostic Fit | Long-Term Assurance | Recommended Role |
|---|---:|---:|---:|---:|---:|---|
| TLA+ | 3 | 4 | 1 | 3 | 3 | Pipeline/workflow model |
| Lean | 5 | 3 | 1 | 2 | 5 | Mathematical semantics and soundness proof |
| Rocq/Coq | 5 | 3 | 2 | 2 | 5 | Mechanized semantics, verified checker/extraction option |
| Isabelle/HOL | 5 | 3 | 1 | 2 | 5 | Large refinement proof option |
| SMT/Z3 | 3 | 5 | 4 | 4 | 3 | Local arithmetic/range proof backend |
| Why3/Dafny/F* | 4 | 4 | 2 | 3 | 4 | Deductive verifier prototypes |
| Verus/Creusot | 3 | 4 | 5 | 3 | 3 | Actual Rust verifier component proofs |
| Kani | 2 | 5 | 5 | 3 | 2 | Bounded Rust safety harnesses |
| Abstract interpretation | 4 | 5 | 5 | 5 | 4 | Core executable verifier algorithm |
| K Framework | 4 | 4 | 1 | 3 | 3 | Executable semantics spike |
| Geometric algebra formalization | 2 | 2 | 1 | 1 | 4 | Future algebraic kernel proof pattern |
| Translation validation | 4 | 4 | 3 | 3 | 4 | Phase 14 lowering validation |

## Recommended Phase 13 Architecture

```text
                +-----------------------------+
                | Normative Loom Core Spec    |
                | syntax/types/effects/events |
                +--------------+--------------+
                               |
                               v
 +----------------+    +---------------+    +-------------------+
 | TLA+ pipeline  |    | Mechanized    |    | Rust executable   |
 | lifecycle spec |    | soundness     |    | verifier          |
 +----------------+    | Lean/Rocq     |    | abstract interp   |
                       +-------+-------+    +---------+---------+
                               |                      |
                               v                      v
                       theorem: accepted      VerifiedArtifact facts
                       implies safety         diagnostics/resource bounds
                                                      |
                                                      v
                                           Phase 14 lowering preconditions
```

## Recommended Proof Obligations For Phase 13

These should become Phase 13 requirements/plans later:

| ID | Obligation |
|---|---|
| `VERIFIER-01` | Define canonical Loom artifact grammar and version/feature semantics beyond MVP0 `LMC1`. |
| `VERIFIER-02` | Define L1 layout semantics as pure, finite, declarative decode descriptions. |
| `VERIFIER-03` | Define L2 total-function language syntax, type system, effect system, and allowed loop forms. |
| `VERIFIER-04` | Define capability model: input ranges, scratch arena, output builders, no ambient authority. |
| `VERIFIER-05` | Define Arrow builder event semantics and prove well-formedness by construction. |
| `VERIFIER-06` | Implement abstract interpretation/type-effect checker for the L2 subset. |
| `VERIFIER-07` | Add SMT-backed local obligations for arithmetic, ranges, overflow, and loop variants. |
| `VERIFIER-08` | Produce stable diagnostics and proof-obligation traces for rejected artifacts. |
| `VERIFIER-09` | Mechanize a small core soundness theorem: accepted core programs cannot violate memory/capability/output/termination rules. |
| `VERIFIER-10` | Emit verified facts as lowering preconditions for Phase 14. |

## Recommended Phase 13 MVP Slice

Do not attempt the whole final verifier at once. Build one vertical slice:

1. Define a tiny `L2Core`:
   - finite input slices,
   - `for i in 0..N`,
   - monotone byte cursor loop,
   - scalar arithmetic over bounded integers,
   - typed Arrow builder events.
2. Add a verifier:
   - type/effect checker,
   - abstract state,
   - resource bound computation,
   - SMT range obligations.
3. Add a mechanized spec:
   - Lean or Rocq definitions for syntax, types, and semantics,
   - theorem statement for accepted programs,
   - at least one proved sub-theorem, preferably progress/preservation or builder well-formedness.
4. Add project gates:
   - `scripts/full-verifier-test.sh`,
   - negative artifacts,
   - proof source build/check if a proof assistant is introduced.

## Recommended Tool Choice

For the next step, I recommend:

1. **Primary implementation:** Rust verifier in `loom-core`, using type/effect checking + abstract interpretation.
2. **Constraint backend:** internal constraint IR with optional Z3/SMT-LIB output.
3. **Mechanized proof:** Lean first, unless extraction of a verified checker is a hard requirement, then Rocq.
4. **Workflow model:** small TLA+ spec for artifact lifecycle and lowering gate invariants.
5. **Rust implementation proof spike:** Verus or Creusot on a single verifier component.

This gives Loom a practical verifier path now and leaves room to strengthen assurance later without letting proof-tool selection dominate the IR design too early.

## Risks

| Risk | Mitigation |
|---|---|
| Proof scope explodes | Start with a tiny L2Core and one theorem. |
| SMT queries become opaque | Keep local obligations small and attach diagnostics to source spans. |
| Abstract interpreter unsoundness | Mechanize the abstract state relation for the core subset. |
| Toolchain friction | Keep proof assistant build optional at first, then gate once stable. |
| Future MLIR lowering mismatches verifier facts | Define `VerifiedArtifactFacts` in Phase 13 specifically for Phase 14. |
| False rejects | Treat precision improvements as verifier completeness work, not soundness work. |

## Final Recommendation

Phase 13 should be framed as:

> Build and partially mechanize the full Loom verifier architecture by defining a tiny but representative future Loom core language, implementing a Rust abstract-interpreting verifier for it, proving the accepted-core safety theorem in Lean or Rocq, and emitting verifier facts that become Phase 14 lowering preconditions.

TLA+, geometric algebra formalization, and K-style executable semantics are useful, but they should be supporting tracks. The load-bearing center should be:

```text
Loom core semantics + abstract interpretation/type-effect verifier + SMT obligations + mechanized soundness theorem
```

