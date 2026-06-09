# Phase 33: LMC2 Arrow Semantic Container Wrapper - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-09
**Phase:** 33-lmc2-arrow-semantic-container-wrapper
**Areas discussed:** LMC2 envelope shape, Verifier facts and diagnostics, Source-ingress cutover

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| All four | Discuss every wrapper decision, including visible gates/docs. | |
| Core + source | Focus on envelope, verifier facts, and source emission; leave CLI and gate details mostly to agent discretion. | ✓ |
| Core only | Discuss only the LMC2 envelope and verifier contract. | |
| Custom subset | User selected gray areas 1, 2, and 3. | ✓ |

**User's choice:** `123`, followed by "全部按照推荐进行" ("proceed with all recommended choices").
**Notes:** Interpreted as selecting the first three gray areas and accepting the recommended path for each selected area.

---

## LMC2 Envelope Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Semantic-specific wrapper | Small versioned wrapper with one required `LMA1` payload section plus minimal feature/metadata fields. | ✓ |
| Mirror `LMC1` sections | Reuse the broader `LMC1`-style section directory and feature bitset model, adapted for Arrow semantic payloads. | |
| Full new distribution container | Design `LMC2` as the future general artifact container now, beyond just wrapping `LMA1`. | |

**User's choice:** Recommended default.
**Notes:** Direct `LMA1` remains accepted as an explicit bridge; new source-distribution evidence should prefer `LMC2(LMA1)`.

---

## Verifier Facts and Diagnostics

| Option | Description | Selected |
|--------|-------------|----------|
| Wrapper plus inner semantic facts | Accepted facts expose `LMC2` wrapper version/features, payload kind, row/schema/batch summary, and inner `LMA1` acceptance. | ✓ |
| Minimal artifact kind only | Facts only show that the wrapper was accepted and defer semantic details to decode tests. | |
| Full future manifest facts | Add broader distribution facts such as signatures, remote identity, cache policy, and provenance now. | |

**User's choice:** Recommended default.
**Notes:** Lowering remains explicitly not ready; diagnostics should distinguish malformed wrapper, unsupported version, unknown feature, missing/malformed inner `LMA1`, trailing bytes, and offset/length failures.

---

## Source-Ingress Cutover

| Option | Description | Selected |
|--------|-------------|----------|
| Emit `LMC2(LMA1)` by default | Parquet, Lance, and Vortex accepted emission shifts to wrapper artifacts while keeping direct `LMA1` helper compatibility. | ✓ |
| Keep direct `LMA1` default | Implement verifier support for `LMC2`, but leave source emission unchanged until a later phase. | |
| Dual emit everywhere | Emit both direct and wrapped artifacts for every source path as equal first-class outputs. | |

**User's choice:** Recommended default.
**Notes:** Source reports should name `LMC2` as accepted artifact bytes and preserve separate source/oracle evidence. Existing semantic equality tests remain; Phase 33 adds wrapper acceptance and negative coverage.

---

## the agent's Discretion

- Exact Rust type/module/function names for the wrapper codec.
- Exact CLI/report/script updates needed to make `LMC2` visible.
- Focused gate name and broader release-gate wiring order.

## Deferred Ideas

- Phase 34 DuckDB Arrow semantic SQL expansion.
- Phase 35 native Arrow semantic execution.
- Live StarRocks runtime integration.
- Universal future distribution features such as signatures, attestation, remote fetch, encryption, and cache policy.
