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
| VERIFIER-01 | A normative Phase 13 verifier/spec document defines the tiny `L2Core` subset, artifact assumptions, and safety theorem target. | Spec | `13-VERIFIER-SPEC.md` sections `Scope`, `L2Core Syntax`, and `Dynamic Semantics`; `13-VERIFIER-REPORT.md` | `rg -n "L2Core Syntax\|Dynamic Semantics" 13-VERIFIER-SPEC.md` | `scripts/full-verifier-test.sh` | Complete | Full production verifier for all future Loom features remains deferred outside Phase 13. |
| VERIFIER-02 | L1 declarative layout semantics are finite, pure data descriptions that compose with `L2Core` through explicit input capabilities and output facts. | Spec | `13-VERIFIER-SPEC.md` sections `Scope`, `Capability Model`, and `VerifiedArtifactFacts`; Phase 12 safety contract for current L1 boundary; `13-VERIFIER-REPORT.md` | `rg -n "InputSlice\|VerifiedArtifactFacts" 13-VERIFIER-SPEC.md` | `scripts/full-verifier-test.sh` | Complete | Future integration can map more `LayoutDescription` forms into `L2Core` input facts. |
| VERIFIER-03 | `L2Core` syntax, static semantics, dynamic semantics, and allowed loop forms are defined. | Spec/Rust/Lean/Rocq | `13-VERIFIER-SPEC.md`; `crates/loom-core/src/l2_core.rs`; `formal/lean/LoomCore.lean` | `cargo test -p loom-core --test l2_core_model`; Lean check when installed | `scripts/full-verifier-test.sh` | Complete | Complete Lean/Rocq metatheory remains future work. |
| VERIFIER-04 | The capability/resource model covers input ranges, scratch bounds, output builders, no ambient authority, and fail-closed errors. | Spec/Rust | `13-VERIFIER-SPEC.md`; `Capability`, `ResourceBudget`, and `verify_l2_core` | `cargo test -p loom-core --test full_verifier` | `scripts/full-verifier-test.sh` | Complete | Broader future-language features remain outside Phase 13. |
| VERIFIER-05 | Arrow builder event semantics are specified so output well-formedness can be checked or proved by construction. | Spec/Rust/Lean/Rocq | `13-VERIFIER-SPEC.md`; `ArrowEventType`; `builder_events_well_formed` | `cargo test -p loom-core --test l2_core_model`; Lean check when installed | `scripts/full-verifier-test.sh` | Complete | Complete builder theorem over all Arrow nested types remains future work. |
| VERIFIER-06 | A Rust verifier prototype or architecture uses type/effect checking plus abstract interpretation for `L2Core`. | Rust | `crates/loom-core/src/full_verifier.rs` | `cargo test -p loom-core --test full_verifier` | `scripts/full-verifier-test.sh` | Complete | Production hardening can expand the accepted language later. |
| VERIFIER-07 | Local arithmetic, range, overflow, loop-variant, and resource-bound obligations are represented as SMT-ready constraints. | SMT/Rust | `crates/loom-core/src/l2_core/constraints.rs`; `crates/loom-core/src/full_verifier.rs` | `cargo test -p loom-core --test full_verifier` checks `AddNoOverflow`, `InRange`, and `Decreases` proof traces | `scripts/full-verifier-test.sh` | Complete | Solver integration remains intentionally deferred. |
| VERIFIER-08 | Verifier diagnostics and proof-obligation traces are stable enough for reviewer-facing rejection reports. | Rust | `FullVerificationCode` and `ProofObligationTrace` in `crates/loom-core/src/full_verifier.rs` | `cargo test -p loom-core --test full_verifier`; `cargo run --bin loom -- verify-l2core --sample` | `scripts/full-verifier-test.sh` | Complete | Final production UX can expand beyond the sample CLI path. |
| VERIFIER-09 | A Lean or Rocq proof scaffold defines core syntax and states the accepted-program safety theorem target. | Lean/Rocq | `formal/lean/LoomCore.lean` defines `builder_events_well_formed` and `accepted_program_safe` | `bash scripts/full-verifier-test.sh` checks Lean theorem names and runs Lean when installed | `scripts/full-verifier-test.sh` | Complete as scaffold only | `builder_events_typed` and `no_ambient_authority` are currently `True` placeholders, so `accepted_program_safe` is not load-bearing safety evidence; complete final Loom soundness remains future work. |
| VERIFIER-10 | Phase 13 emits verifier facts/proof obligations that Phase 14 can use as native-lowering preconditions. | Gate/Rust/TLA+ | `VerifiedArtifactFacts`; `verify_l2_core`; `LoweredImpliesVerified`; `13-VERIFIER-REPORT.md` | `cargo test -p loom-core --test full_verifier`; `cargo run --bin loom -- verify-l2core --sample`; TLC when installed | `scripts/full-verifier-test.sh` | Complete | Phase 14 must implement and prove actual lowering separately. |

## Phase 14 Handoff

`VerifiedArtifactFacts` are the Phase 14 handoff for `VERIFIER-10`. Native
lowering may consume these facts only after the artifact is verifier-accepted
and the facts identify the accepted artifact version, feature set, input ranges,
output schema, loop bounds, resource bounds, builder event types, constraint
IDs, and proof-obligation IDs.

Phase 13 does not claim that the future native lowering is correct. It creates
the precondition surface that later lowering work must check before translating
an artifact to MLIR or native code.
