---
phase: quick-260609-lb2
plan: 01
subsystem: formal-verification
tags: [lean, l2core, verifier, formal]
requires:
  - "formal/lean/LoomCore.lean existing AST/theorem scaffold"
  - "crates/loom-core/src/full_verifier.rs accept/reject groups (mirrored, not imported)"
provides:
  - "Real decidable Lean L2Core checkers (checkTyped/checkAuthority/checkBounds)"
  - "Load-bearing builder_events_typed / no_ambient_authority / finite_bounds predicates"
  - "Honest header enumerating Lean-checked vs SMT-only obligations"
affects:
  - "scripts/full-verifier-test.sh Lean compile gate (still PASS)"
tech-stack:
  added: []
  patterns:
    - "Lean 4.30 mutual checkStmt/checkBody structural recursion over List Stmt"
    - "Bool-valued checker wrapped as `_ = true : Prop` to keep Decidable predicates"
key-files:
  created: []
  modified:
    - "formal/lean/LoomCore.lean"
decisions:
  - "Three independent checkers (1:1 with Rust check groups) per RESEARCH Open Question 2"
  - "Added thin checkTyped/checkAuthority/checkBounds body entry points so predicates pass p.body"
  - "Block comments (/- -/) before mutual blocks; doc-comments (/-- -/) are a syntax error there in Lean 4.30"
  - "Integer overflow / var-env / non-row budgets left SMT-only — no faithful Nat-grounded counterpart"
metrics:
  duration: ~3min
  completed: 2026-06-09
---

# Quick Task lb2: Replace placeholder `True` predicates in `LoomCore.lean` Summary

Replaced the three placeholder L2Core predicates in `formal/lean/LoomCore.lean`
(`builder_events_typed = True`, `no_ambient_authority = True`, vacuous
`finite_bounds = p.maxRows >= 0`) with real decidable `Bool`-valued checkers
mirroring the Rust `verify_l2_core` accept/reject groups, while keeping the two
projection theorems (`accepted_program_safe`, `builder_events_well_formed`) and the
`Verified` conjunction order byte-for-byte unchanged. The real CI gate
`scripts/full-verifier-test.sh` passes.

## What Was Built

- **Lookup helpers** over `List Capability`: `builderInfo?` (declared output builder
  type + nullability, via `findSome?`) and `inputSlice?` (declared input offset/length).
- **Three independent `mutual` checker blocks**, each pairing a `check<X>Stmt` with a
  `check<X>Body` (tail recursion + descent into the stmt helper — the structural shape
  Lean 4.30 accepts with no `termination_by`):
  - `checkTyped` (builder_events_typed): AppendValue type match (OutputTypeMismatch),
    AppendNull nullability (OutputNullabilityMismatch), declared builder
    (MissingOutputBuilder).
  - `checkAuthority` (no_ambient_authority): ReadInput declared capability
    (MissingInputCapability) + spatial in-range over the slice (faithful over concrete
    `Nat`), append targets declared (shared MissingOutputBuilder).
  - `checkBounds` (finite_bounds): ForRange `stop>=start` + `(stop-start)<=maxRows`;
    CursorLoop `progress>0` + `limit<=maxRows`.
- **Rewired predicates** delegate to the checkers as `<checker> ... = true : Prop`
  (keeps them `Decidable`); removed the `_p`/`_` ignored-argument forms.
- **Rewritten header**: drops the "intentionally `True` placeholders / not load-bearing"
  claim; honestly enumerates what Lean now machine-checks vs what remains SMT-only
  (Nat overflow `AddNoOverflow`/`MulNoOverflow`, unknown-variable/`ScalarExpr` var-env,
  non-row resource budgets `max_steps`/`max_builder_events`/per-builder `max_events`).

## Tasks Completed

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | Add lookup helpers and mutual recursive checkers | 18eb428 | formal/lean/LoomCore.lean |
| 2 | Rewire predicates, preserve theorems, rewrite header | 27b0a45 | formal/lean/LoomCore.lean |
| 3 | Run the real CI verifier gate | (no code change) | formal/lean/LoomCore.lean |

## Verification

- `lean formal/lean/LoomCore.lean` exits 0 with no `sorry` (LEAN_OK).
- `scripts/full-verifier-test.sh` PASSED end-to-end (GATE_EXIT=0): required artifacts,
  VERIFIER-01..10 IDs, formal scaffold names, Rust `l2_core_model` (3 tests) and
  `full_verifier` (7 tests), `loom verify-l2core --sample`, the Lean compile, and the
  TLC `LoomVerifierPipeline` model check.
- Protected names present: `accepted_program_safe`, `builder_events_well_formed`,
  `Verified`, `Safe`.
- `Verified` conjunction order preserved:
  `finite_bounds p /\ builder_events_typed p /\ no_ambient_authority p`.
- Vacuous `p.maxRows >= 0` placeholder removed; the three predicate bodies no longer
  read `True`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Doc-comments before `mutual` blocks are a Lean 4.30 syntax error**
- **Found during:** Task 1 (first `lean` compile)
- **Issue:** Placing a `/-- ... -/` doc-comment immediately before a `mutual` block
  produced `unexpected token 'mutual'` (doc-comments must attach to a single
  declaration; `mutual` is a block keyword).
- **Fix:** Converted the three pre-`mutual` doc-comments to plain block comments
  (`/- ... -/`). Doc-comments on individual `def`s (e.g. `builderInfo?`) were left as-is.
- **Files modified:** formal/lean/LoomCore.lean
- **Commit:** 18eb428

**2. [Rule 3 - Blocking] Added thin checker entry points to match predicate call sites**
- **Found during:** Task 2
- **Issue:** The RESEARCH wrap-as-Prop sketch calls `checkTyped`/`checkAuthority`/
  `checkBounds` on `p.body : List Stmt`, but the mutual blocks expose the body
  traversal as `check<X>Body`. Without a top-level alias the predicates would not
  resolve those names.
- **Fix:** Added `checkTyped` / `checkAuthority` / `checkBounds` thin wrappers
  delegating to the corresponding `*Body` helper (1:1 with the names the predicates use).
- **Files modified:** formal/lean/LoomCore.lean
- **Commit:** 27b0a45

No architectural (Rule 4) changes; no auth gates; no package installs (Lean-core only,
no Mathlib, no lakefile).

## Threat Model Compliance

- T-lb2-01 (overclaim): mitigated — header enumerates Lean-checked vs SMT-only
  obligations and does not claim full soundness.
- T-lb2-02 (CI string-grep contract): mitigated — protected names and `Verified`
  conjunction order preserved; verified by grep and the passing CI gate.
- T-lb2-03 (spurious proof via `sorry`): mitigated — `lean` exits 0 with no `sorry`.
- T-lb2-SC (package installs): n/a — no installs.

## Known Stubs

None. The predicates are now real checkers. The SMT-only obligations (overflow,
var-env, non-row budgets) are intentionally and explicitly documented as out of scope
for the `Nat`-grounded Lean AST, not stubbed within Lean.

## Self-Check: PASSED

- formal/lean/LoomCore.lean: FOUND (modified, typechecks exit 0)
- Commit 18eb428: FOUND
- Commit 27b0a45: FOUND
