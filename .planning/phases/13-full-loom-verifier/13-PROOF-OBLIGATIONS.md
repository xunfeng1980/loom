# Phase 13 Proof Obligations

**Status:** Initial matrix for Phase 13 execution
**Spec:** `13-VERIFIER-SPEC.md`
**Scope:** Full Loom verifier foundation over the tiny `L2Core` slice

## Layer Separation

Phase 13 deliberately does not treat one formal method as authoritative for the
whole verifier:

- `Spec` defines the normative target and exclusions.
- `Rust` provides the executable verifier, diagnostics, abstract
  interpretation, and emitted facts.
- `SMT` handles local arithmetic, range, overflow, loop-variant, and resource
  obligations.
- `Lean/Rocq` gives the core language semantics and accepted-program soundness
  scaffold.
- `TLA+` covers lifecycle and pipeline invariants.
- `Gate` ties source evidence and executable evidence into release checks.

## Obligation Matrix

| ID | Claim | Layer | Source evidence | Executable evidence | Gate evidence | Status | Gap |
|---|---|---|---|---|---|---|---|
| VERIFIER-01 | A normative Phase 13 verifier/spec document defines the tiny `L2Core` subset, artifact assumptions, and safety theorem target. | Spec | `13-VERIFIER-SPEC.md` sections `Scope`, `L2Core Syntax`, and `Dynamic Semantics` | `rg -n "L2Core Syntax\|Dynamic Semantics" 13-VERIFIER-SPEC.md` | `13-01` verification commands | Planned in 13-01 | Downstream Rust and Lean/Rocq evidence planned after the spec is stable. |
| VERIFIER-02 | L1 declarative layout semantics are finite, pure data descriptions that compose with `L2Core` through explicit input capabilities and output facts. | Spec | `13-VERIFIER-SPEC.md` sections `Scope`, `Capability Model`, and `VerifiedArtifactFacts`; Phase 12 safety contract for current L1 boundary | `rg -n "InputSlice\|VerifiedArtifactFacts" 13-VERIFIER-SPEC.md` | `13-01` proof-obligation matrix check | Planned in 13-01 | A future integration spec can map existing `LayoutDescription` nodes into `L2Core` input facts. |
| VERIFIER-03 | `L2Core` syntax, static semantics, dynamic semantics, and allowed loop forms are defined. | Spec/Rust | `13-VERIFIER-SPEC.md` sections `L2Core Syntax`, `Static Semantics`, and `Dynamic Semantics`; `crates/loom-core/src/l2_core.rs` | `cargo test -p loom-core --test l2_core_model`; `rg -n "ForRange\|CursorLoop\|Static Semantics" 13-VERIFIER-SPEC.md` | `13-01` docs gate, `13-02` Rust model gate, then `13-04` formal model gate | Model evidence added in 13-02 | Lean/Rocq model still needs to mirror the spec. |
| VERIFIER-04 | The capability/resource model covers input ranges, scratch bounds, output builders, no ambient authority, and fail-closed errors. | Spec/Rust | `13-VERIFIER-SPEC.md` sections `Capability Model` and `Resource Model`; `Capability` and `ResourceBudget` in `crates/loom-core/src/l2_core.rs` | `cargo test -p loom-core --test l2_core_model` | `13-01` docs gate, `13-02` model gate, then `13-03` verifier tests | Model evidence added in 13-02 | Abstract interpretation and diagnostics are planned in `13-03`. |
| VERIFIER-05 | Arrow builder event semantics are specified so output well-formedness can be checked or proved by construction. | Spec/Rust | `13-VERIFIER-SPEC.md` section `Arrow Builder Event Semantics`; `ArrowEventType` and output schema facts in `crates/loom-core/src/l2_core.rs` | `cargo test -p loom-core --test l2_core_model` | `13-01` docs gate, `13-02` model gate, then `13-04` formal scaffold gate | Model evidence added in 13-02 | Lean/Rocq builder-event theorem scaffold remains planned. |
| VERIFIER-06 | A Rust verifier prototype or architecture uses type/effect checking plus abstract interpretation for `L2Core`. | Rust | `13-VERIFIER-SPEC.md` type/effect judgment; `crates/loom-core/src/full_verifier.rs` | `cargo test -p loom-core --test full_verifier` covers accepted/rejected `L2Core` programs | `13-03` verifier gate, future `scripts/full-verifier-test.sh` | Executable verifier evidence added in 13-03 | Broader formal linkage remains planned in `13-04`. |
| VERIFIER-07 | Local arithmetic, range, overflow, loop-variant, and resource-bound obligations are represented as SMT-ready constraints. | SMT/Rust | `13-VERIFIER-SPEC.md` static semantics and resource model; `crates/loom-core/src/l2_core/constraints.rs`; `crates/loom-core/src/full_verifier.rs` | `cargo test -p loom-core --test full_verifier` checks `AddNoOverflow`, `InRange`, and `Decreases` proof traces | `13-02` model gate, `13-03` verifier gate, then future full-verifier gate | Constraint emission evidence added in 13-03 | Solver integration remains intentionally deferred. |
| VERIFIER-08 | Verifier diagnostics and proof-obligation traces are stable enough for reviewer-facing rejection reports. | Rust | Existing Phase 9/12 diagnostic pattern in `13-PATTERNS.md`; `FullVerificationCode` and `ProofObligationTrace` in `crates/loom-core/src/full_verifier.rs` | `cargo test -p loom-core --test full_verifier` asserts stable diagnostic codes; `cargo run --bin loom -- --help` exposes `verify-l2core` | `13-03` CLI/test gate and future final report | Diagnostic and trace evidence added in 13-03 | Final docs and release wiring remain planned in `13-05`. |
| VERIFIER-09 | A Lean or Rocq proof scaffold defines core semantics and states or proves an accepted-program safety theorem. | Lean/Rocq | `13-VERIFIER-SPEC.md` dynamic semantics theorem target | Future Lean/Rocq file typecheck or syntax gate | Future `13-04` full-verifier gate | Planned in 13-04 | Mechanized scaffold has not been added. |
| VERIFIER-10 | Phase 13 emits verifier facts/proof obligations that Phase 14 can use as native-lowering preconditions. | Gate/Rust | `13-VERIFIER-SPEC.md` sections `VerifiedArtifactFacts` and `Lowering Preconditions`; `VerifiedArtifactFacts` in `crates/loom-core/src/l2_core.rs`; `verify_l2_core` in `crates/loom-core/src/full_verifier.rs` | `cargo test -p loom-core --test full_verifier` checks accepted facts and rejected-program fact absence; `cargo run --bin loom -- verify-l2core --sample` prints facts | `13-01` facts definition, `13-02` fact model gate, `13-03` executable facts gate, then `13-05` final closeout | Executable fact emission evidence added in 13-03 | Final Phase 14 handoff report remains planned. |

## Phase 14 Handoff

`VerifiedArtifactFacts` are the Phase 14 handoff for `VERIFIER-10`. Native
lowering may consume these facts only after the artifact is verifier-accepted
and the facts identify the accepted artifact version, feature set, input ranges,
output schema, loop bounds, resource bounds, builder event types, constraint
IDs, and proof-obligation IDs.

Phase 13 does not claim that the future native lowering is correct. It creates
the precondition surface that later lowering work must check before translating
an artifact to MLIR or native code.
