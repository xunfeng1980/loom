# Phase 39: Model Rust Interpreter Consistency - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md - this log preserves the alternatives considered.

**Date:** 2026-06-09
**Phase:** 39-Model Rust Interpreter Consistency
**Areas discussed:** Reference executor, production trace, corpus/gate, extraction, non-claims
**Mode:** Autonomous discuss; selected recommended defaults because the user previously approved recommended defaults and the active autonomous workflow permits safe defaulting.

---

## Reference Executor

| Option | Description | Selected |
|--------|-------------|----------|
| Separate Rust transcription | Implement a Rust reference executor that mirrors the Lean model and acts only as differential oracle. | yes |
| Use production as oracle | Treat current production behavior as the expected model. | |
| Replace production with reference | Route production through the reference executor. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** The oracle and subject must remain separate or the differential gate proves little.

---

## Production Trace

| Option | Description | Selected |
|--------|-------------|----------|
| Observer-only trace hook | Capture full builder-event and fail-closed traces without changing behavior. | yes |
| Final values only | Compare only final rows/values. | |
| Auto-fix divergence | Modify production to match the model whenever the gate fails. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Roadmap requires full builder-event trace comparison, not final values only.

---

## Corpus And Gate

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministic matrix plus generated cases | Use supported matrix plus reproducible fuzz/generated cases and fail closed on diff. | yes |
| Manual smoke only | Compare a handful of examples without fuzz/corpus generation. | |
| Live random fuzz | Generate nondeterministic cases every run. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 37's deterministic diff style is the pattern to reuse.

---

## Extraction

| Option | Description | Selected |
|--------|-------------|----------|
| Evaluate optional extraction | Try/assess Lean extraction and adopt only if cheap and deterministic; otherwise record deferral. | yes |
| Require extraction | Block Phase 39 until extraction becomes the oracle path. | |
| Ignore extraction | Do not mention extraction despite roadmap optional criterion. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** The roadmap marks extraction optional/additive.

---

## Non-Claims

| Option | Description | Selected |
|--------|-------------|----------|
| Per-run validation only | State this is differential validation, not proof of all-program equivalence. | yes |
| Proof of real interpreter | Claim the Rust interpreter is proven equivalent to the model. | |
| Native validation | Include native backend/model validation in this phase. | |

**User's choice:** Recommended default selected autonomously.
**Notes:** Phase 40 owns native-to-model validation.
