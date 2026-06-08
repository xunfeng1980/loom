# Phase 12: Formal Verifier / Safety Proof MVP - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-08
**Phase:** 12-Formal Verifier / Safety Proof MVP
**Areas discussed:** Proof target boundary, Proof artifact shape, Verifier contract, Totality/termination MVP, Release gate

---

## Proof Target Boundary

| Option | Description | Selected |
|--------|-------------|----------|
| Current boundary proof | Cover `LMC1/LMP1/LMT1 -> verifier -> decode/FFI/CLI/DuckDB` safety boundary. | ✓ |
| Complete Loom IR proof | Attempt to cover the README future IR design. | |
| Documentation only | Write only a prose argument without executable coverage. | |

**User's choice:** Recommended option.
**Notes:** Scope is the implemented boundary, not the future language.

---

## Proof Artifact Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Matrix + gate + safety doc | Proof-obligation matrix, executable gates, and written safety argument. | ✓ |
| Theorem prover/model checker | Introduce a formal-methods toolchain in Phase 12. | |
| Tests only | Add tests without proof documentation. | |

**User's choice:** Recommended option.
**Notes:** Keep artifacts reviewable and tied to concrete code/tests.

---

## Verifier Contract

| Option | Description | Selected |
|--------|-------------|----------|
| Stabilize existing contract | Lock typed diagnostics, fail-closed behavior, and no-panic malformed input boundary. | ✓ |
| Rewrite verifier API | Replace the current verifier surface. | |
| README only | Describe behavior publicly without changing contract/test coverage. | |

**User's choice:** Recommended option.
**Notes:** Phase 12 should harden the current verifier rather than rewrite it.

---

## Totality / Termination MVP

| Option | Description | Selected |
|--------|-------------|----------|
| Current interpreter loops | Cover bounded loops, counts, and buffer extents in the implemented parser/interpreter. | ✓ |
| Future L2 language proof | Try to prove totality for a language not yet implemented. | |
| Memory safety only | Exclude termination from Phase 12. | |

**User's choice:** Recommended option.
**Notes:** Termination scope is current code only.

---

## Release Gate

| Option | Description | Selected |
|--------|-------------|----------|
| Add safety-proof gate | Add a dedicated gate, likely `scripts/safety-proof-test.sh`, and wire it into `mvp0-verify.sh`. | ✓ |
| Manual only | Keep the gate outside the release script. | |
| Documentation only | No executable gate. | |

**User's choice:** Recommended option.
**Notes:** Gate must stay practical for local release verification.

---

## the agent's Discretion

- Exact proof matrix filename and format.
- Exact safety documentation layout.
- Exact focused tests added to close proof-obligation coverage gaps.

## Deferred Ideas

- Theorem prover/model checker adoption.
- Full future Loom IR totality proof.
- MLIR/native lowering correctness proof.
- Real Vortex file/container ingress proof.
