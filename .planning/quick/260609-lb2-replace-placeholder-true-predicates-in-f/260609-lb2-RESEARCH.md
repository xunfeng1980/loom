# Quick Task lb2: Replace placeholder `True` predicates in `LoomCore.lean` — Research

**Researched:** 2026-06-09
**Domain:** Lean 4 verified-checker definitions mirroring the Rust `L2Core` verifier
**Confidence:** HIGH (all claims verified against repo source + a compiled Lean 4.30.0 scratch proof in this session)

## Summary

`formal/lean/LoomCore.lean` defines three semantic predicates as trivial placeholders:
`finite_bounds` is `p.maxRows >= 0` (vacuous over `Nat`), and `builder_events_typed` /
`no_ambient_authority` are literally `True`. The task is to replace all three with real,
decidable `Bool`-valued checkers over the existing Lean AST that mirror the accept/reject
logic of `verify_l2_core` in `crates/loom-core/src/full_verifier.rs`, while keeping the two
existing projection theorems (`accepted_program_safe`, `builder_events_well_formed`) provable
**unchanged** — they only project out of the `Verified` conjunction.

The key finding: this is achievable **without changing the Lean AST** for the structural
checks (type match, nullability, capability declaration, loop bounds, monotone progress, row
budget). The Rust checks that have *no* faithful counterpart on the current Lean AST are the
ones that depend on machinery the Lean AST does not model — `ScalarExpr`/`LetScalar` variable
environments, integer-overflow constraints (`AddNoOverflow`/`MulNoOverflow`), and SMT range
obligations (`InRange`). Those are correctly out of scope for a `Nat`-based Lean model and
should be documented as SMT-only obligations, not forced into Lean.

**Primary recommendation:** Define `Bool`-valued recursive checkers (`checkBody`/`checkStmt`
via a `mutual` block, plus `builderInfo?` / `inputDeclared` lookup helpers over
`List Capability`), wrap each predicate as `<checker> p = true : Prop`, keep `Verified` and
`Safe` as the same conjunction shapes, and verify with `cd <repo> && lean formal/lean/LoomCore.lean`.
Take the **minimal-AST-change path** — do not add `ScalarExpr` to Lean for this task.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| lb2 | Replace `builder_events_typed`, `no_ambient_authority`, `finite_bounds` placeholders with real checkers mirroring Rust verifier; keep both projection theorems provable unchanged | Sections "Exact Rust checks to mirror", "Abstraction gap", "Lean implementation" below; all idioms compiled in-session |

## Environment / Build (focus item 4)

| Fact | Value |
|------|-------|
| Lean installed | YES — `lean` and `lake` at `~/.elan/bin` (elan-managed) |
| Toolchain | `leanprover/lean4:v4.30.0` (pinned in `./lean-toolchain`) |
| Mathlib | NOT used — single-file, no `lakefile.{lean,toml}`, no `lake` project. Lean **core only**. |
| Build model | The CI gate runs `lean formal/lean/LoomCore.lean` **directly** (no `lake build`) |
| Current file status | Compiles clean, exit 0, ~2s (verified in-session) |

**Exact verification command an implementer/executor must run** (the `lean-toolchain` pin is
directory-scoped, so you MUST be inside the repo or the elan default-toolchain error fires):

```bash
export PATH="${HOME}/.elan/bin:${PATH}"
cd <repo-root>
lean formal/lean/LoomCore.lean   # exit 0 == typechecks, no `sorry`
```

**CI gates that consume this file (must still pass):**
- `scripts/full-verifier-test.sh` — checks the file exists, greps that `accepted_program_safe`
  and `builder_events_well_formed` are present (string match — so do NOT rename them), then runs
  `lean formal/lean/LoomCore.lean`.
- `scripts/install-formal-tools.sh` — also runs `lean formal/lean/LoomCore.lean`.
- `scripts/safety-proof-test.sh` is invoked by `scripts/mvp0-verify.sh` (Phase 12 gate); the
  Lean compile is reached through the full-verifier path.

Lean IS installed and CI DOES compile it — so the executor gets real machine-checking. Changes
need not be conservative-by-fear, but must keep the two theorem names and the conjunction shape.

## Exact Rust accept/reject checks to mirror (focus item 1)

Grouped by target Lean predicate. Source: `verify_l2_core` + helpers in `full_verifier.rs`.

### → `builder_events_typed`
| Rust code | Reject code | Faithful on current Lean AST? |
|-----------|-------------|-------------------------------|
| `AppendValue`: `scalar_type_for_arrow(builder.arrow_type) == Some(value_type)` (l.352–364) | `OutputTypeMismatch` | YES — Lean `appendValue` carries `ty : L2Ty` directly; compare to builder's declared `ty` |
| `AppendValue`/`AppendNull` on undeclared builder (l.343–350, 371–378) | `MissingOutputBuilder` | YES — `builderInfo? caps name` returns `none` |
| `AppendNull` on non-nullable builder (l.380–386) | `OutputNullabilityMismatch` | YES — builder's `nullable` flag is in the Lean `Capability.outputBuilder` |
| `value_type` derived from `ScalarExpr` via `verify_expr`/`LetScalar` env (l.342, 397–435) | (feeds the above) | NO direct counterpart — Lean already has the *resolved* `ty` on `appendValue`, so the env is pre-collapsed. This is the abstraction the Lean AST bakes in. |

Note Rust `scalar_type_for_arrow` maps `Boolean→Bool, Int32→Int32, Int64→Int64, Utf8→Bytes`,
and `Float32/64→None` (which would force a mismatch). The Lean `L2Ty` (bool/int32/int64/uint32/
uint64/bytes/rowIndex) is already the resolved scalar type, so the Lean check is a direct
`builderTy == appendTy` equality (`DecidableEq` on `L2Ty` exists).

### → `no_ambient_authority`
| Rust code | Reject code | Faithful on current Lean AST? |
|-----------|-------------|-------------------------------|
| `ReadInput`: capability not in `input_capabilities` (l.322–329) | `MissingInputCapability` | YES — `inputDeclared caps name` over `Capability.inputSlice` |
| `AppendValue`/`AppendNull` undeclared builder | `MissingOutputBuilder` | YES (shared with above) |
| Read bounds: `offset+width` within `[input.offset, input.offset+length]` via `InRange` constraint (l.486–491) | (SMT obligation, not a `report.push`) | PARTIAL — Lean `readInput` has concrete `Nat` offset/width, so a literal bounds check `offset + width <= sliceOffset + sliceLength && offset >= sliceOffset` IS expressible. Rust defers this to SMT (`InRange`) rather than rejecting inline. |
| Read add-no-overflow (`AddNoOverflow`, l.480–485) | (SMT obligation) | NO — `Nat` has no overflow; document as SMT-only |

**Recommendation for read-bounds:** Because Lean `readInput` uses concrete `Nat` offset/width
(unlike Rust's symbolic `ScalarExpr`), you *can* faithfully add the spatial bounds check in
Lean as a total `Nat` predicate. This is strictly stronger evidence than the placeholder and
costs nothing. Do it. The overflow obligation stays SMT-only (no `Nat` counterpart).

### → `finite_bounds`
| Rust code | Reject code | Faithful on current Lean AST? |
|-----------|-------------|-------------------------------|
| `ForRange`: `start`/`end` const & `end >= start` (l.237–268) | `InvalidLoopBounds` | PARTIAL — Lean `forRange` already has `start stop : Nat` (always finite consts), so "non-const" cannot happen; check `stop >= start` |
| `ForRange`: `(end-start) <= max_rows` (l.248–263) | `ResourceBudgetExceeded` | YES — `stop - start <= p.maxRows` |
| `CursorLoop`: progress = `cursor + positiveConst` (l.278–283, `is_monotone_progress`) | `NonMonotoneCursorLoop` | PARTIAL — Lean `cursorLoop` carries `progress : Nat` (the increment), so monotonicity reduces to `progress > 0` |
| `CursorLoop`: `limit` const (l.300–305) | `InvalidLoopBounds` | N/A — Lean `limit : Nat` is always a const |
| `CursorLoop`: `limit <= max_rows` (l.288–298) | `ResourceBudgetExceeded` | YES — `limit <= p.maxRows` |
| Step/builder-event budgets (`max_steps`, `max_builder_events`, per-builder `max_events`) | `ResourceBudgetExceeded` | DEFERRABLE — Lean `Program` has only `maxRows`, not the full `ResourceBudget`. Counting steps/events requires either AST enrichment or a stateful fold. Recommend folding event counts is optional; the row-bound checks above are the load-bearing finiteness evidence. |

## The abstraction gap, precisely (focus item 2)

The Lean AST is a **pre-resolved, `Nat`-grounded projection** of the Rust AST:

| Concept | Rust AST | Lean AST | Consequence |
|---------|----------|----------|-------------|
| Append value type | `ScalarExpr value` typed via `LetScalar` env | `ty : L2Ty` on `appendValue` | Type env already collapsed → Lean checks equality directly; no env needed |
| Read offset/width | `ScalarExpr` (symbolic) | `Nat` (concrete) | Lean can do literal spatial bounds; Rust defers to SMT |
| Loop bounds / progress | `ScalarExpr` (needs `const_u64`) | `Nat` | Lean bounds are always "const" — `InvalidLoopBounds`/non-const cases are unreachable in Lean |
| Resource limits | full `ResourceBudget` (6 fields) | only `maxRows` | step/event budgets not modeled in Lean |
| Scalar expressions / let-binding | `ScalarExpr`, `LetScalar` | absent | Overflow (`AddNoOverflow`/`MulNoOverflow`), `UnknownVariable` have NO Lean counterpart |

**What CAN be checked faithfully on the CURRENT AST (no change):** output type match,
output nullability, missing-builder, missing-input-capability, read spatial bounds (Nat),
ForRange `stop>=start` + `(stop-start)<=maxRows`, CursorLoop `progress>0` + `limit<=maxRows`.

**What CANNOT (must be deferred / noted as SMT-only):** integer overflow (no `Nat`
counterpart — `Nat` is unbounded), `UnknownVariable` (no var env in Lean), and the non-row
resource budgets (`max_steps`, `max_builder_events`, per-builder `max_events`) which would need
either AST enrichment or an event-count fold.

**Recommendation: keep the current AST. Scope this task to the predicates expressible over it.**
This is a quick task; enriching the Lean AST with `ScalarExpr`/`LetScalar`/full `ResourceBudget`
is a separate, larger piece of work. Add a short comment block in the file documenting the
SMT-only obligations (overflow, range proof, full budget) so the scaffold honestly states its
boundary — replacing the misleading "intentionally `True`" header note.

## Concrete Lean 4 implementation (focus item 3)

All idioms below were **compiled in-session** against Lean 4.30.0 (single file, no Mathlib).
Verified working: `List.findSome?`, `List.any`, `mutual` structural recursion over nested
`List Stmt`, `decide`/`&&` over `Nat` comparisons, and `= true` projection theorems.

### Lookup helpers (over the existing `List Capability`)

```lean
def builderInfo? (caps : List Capability) (name : String) : Option (L2Ty × Bool) :=
  caps.findSome? fun c => match c with
    | .outputBuilder id ty nullable _ => if id == name then some (ty, nullable) else none
    | _ => none

def inputSlice? (caps : List Capability) (name : String) : Option (Nat × Nat) :=
  caps.findSome? fun c => match c with
    | .inputSlice id offset length => if id == name then some (offset, length) else none
    | _ => none
```

### Mutual recursive checkers (handles `forRange`/`cursorLoop` bodies)

A `mutual` block with `checkBody : List Stmt → Bool` recursing via `checkStmt` terminates by
structural recursion (Lean accepts it; verified in-session). Thread `caps` and `maxRows`.

```lean
mutual
  def checkStmt (caps : List Capability) (maxRows : Nat) : Stmt → Bool
    | .appendValue builder ty =>
        match builderInfo? caps builder with
        | some (bty, _) => bty == ty            -- OutputTypeMismatch + MissingOutputBuilder
        | none          => false
    | .appendNull builder =>
        match builderInfo? caps builder with
        | some (_, nullable) => nullable         -- OutputNullabilityMismatch + Missing
        | none               => false
    | .readInput cap offset width _ =>
        match inputSlice? caps cap with          -- MissingInputCapability
        | some (sOff, sLen) => decide (offset >= sOff)
                                && decide (offset + width <= sOff + sLen)  -- read in-range
        | none              => false
    | .forRange _ start stop body =>
        decide (stop >= start) && decide (stop - start <= maxRows)         -- InvalidLoopBounds + budget
          && checkBody caps maxRows body
    | .cursorLoop _ limit progress body =>
        decide (progress > 0) && decide (limit <= maxRows)                 -- NonMonotone + budget
          && checkBody caps maxRows body
    | .failClosed _ => true
  def checkBody (caps : List Capability) (maxRows : Nat) : List Stmt → Bool
    | []          => true
    | s :: rest   => checkStmt caps maxRows s && checkBody caps maxRows rest
end
```

If you prefer one checker per predicate (cleaner separation, but three traversals), split into
`checkTyped`/`checkAuthority`/`checkBounds` each with its own `mutual` body-recursion. A single
combined checker is simpler and the predicates can still be defined as projections of distinct
sub-conditions. **Recommendation:** define three independent checkers so each predicate maps
1:1 to its Rust check group and reads clearly. Each follows the identical `mutual` shape above
but with only its relevant arms returning a real condition (others returning `true`).

### Wrap as `Prop` and keep the conjunction shape EXACTLY

```lean
def finite_bounds (p : Program) : Prop :=
  checkBounds p.capabilities p.maxRows p.body = true

def builder_events_typed (p : Program) : Prop :=
  checkTyped p.capabilities p.body = true

def no_ambient_authority (p : Program) : Prop :=
  checkAuthority p.capabilities p.body = true

def Verified (p : Program) : Prop :=
  finite_bounds p /\ builder_events_typed p /\ no_ambient_authority p

def Safe (p : Program) : Prop :=
  builder_events_typed p /\ no_ambient_authority p
```

### The two existing theorems stay byte-for-byte unchanged

Because `Verified` is still `finite_bounds /\ builder_events_typed /\ no_ambient_authority`
(right-nested `And`), `h.right.left` and `And.intro h.right.left h.right.right` still typecheck:

```lean
theorem builder_events_well_formed (p : Program) :
    Verified p -> builder_events_typed p := by
  intro h
  exact h.right.left

theorem accepted_program_safe (p : Program) :
    Verified p -> Safe p := by
  intro h
  exact And.intro h.right.left h.right.right
```

These compile only if the `And` nesting order is preserved (`finite_bounds` first, then
`builder_events_typed`, then `no_ambient_authority`). Keep that order.

### Lean 4.30 idioms / pitfalls (verified)
- `List.findSome?` and `List.any`/`List.all` are in Lean **core** (no Mathlib) — confirmed compiling.
- Use `==` (BEq) for `String` equality inside lambdas, `decide (a <= b)` for `Nat` comparisons
  inside a `Bool` context. `&&` is `Bool` AND.
- `mutual ... end` with `checkBody` recursing on the tail and into `checkStmt` for the head is
  accepted by the structural-recursion checker — no `termination_by` / `decreasing_by` needed
  (verified in-session). Do NOT try to recurse `checkStmt` directly over `List Stmt` without the
  `checkBody` indirection; the helper-over-lists pattern is what makes termination obvious to Lean.
- `L2Ty` already `deriving DecidableEq`, so `bty == ty` works. `Capability` derives only `Repr`
  in the current file — that's fine; the lookups pattern-match, they don't need `DecidableEq` on `Capability`.
- The predicates are `Decidable` for free (they are `_ = true`), so downstream `decide`/`#eval`
  sanity checks are possible if desired.

## Differential-test alignment (focus item 5 — optional, out of scope now)

The new Lean checkers are decidable and `#eval`-able, and the Rust verifier already has a
`verify-l2core --sample` CLI path (`scripts/full-verifier-test.sh` l.84–87) plus
`crates/loom-core/tests/full_verifier.rs`. A future differential harness could encode the same
sample programs as Lean `Program` literals and assert `#eval checkBody ... == <expected>`
matches the Rust accept/reject. **Treat as out of scope for this quick task** — the Lean AST is
a lossy projection (no `ScalarExpr`, no overflow), so only the structurally-shared decisions
would line up, and building the dual encoding is more work than the predicate replacement
itself. Note it as a follow-up, do not implement.

## Common Pitfalls

- **Renaming the theorems or predicates.** `scripts/full-verifier-test.sh` greps literal
  strings `accepted_program_safe` and `builder_events_well_formed`. Renaming breaks CI even if
  Lean compiles. Keep names.
- **Changing the `And` nesting order in `Verified`.** Breaks `h.right.left` projections.
- **Running `lean` outside the repo.** The `lean-toolchain` pin is directory-scoped; outside the
  repo you get `no default toolchain configured`. Always `cd <repo>` first (verified in-session).
- **Trying to model integer overflow in Lean.** `Nat` is unbounded — there is no overflow to
  catch. Document as SMT-only; do not invent a check.
- **Over-scoping into AST enrichment.** Adding `ScalarExpr`/`LetScalar`/full `ResourceBudget`
  to the Lean AST is tempting but is a separate, larger task. Stay on the minimal path.

## Open Questions

1. **Should the misleading header comment (lines 8–13: "intentionally `True` placeholders…not
   load-bearing safety evidence") be rewritten?**
   - Recommendation: YES. After this change the predicates ARE load-bearing for the structural
     checks they cover. Replace with an honest statement of what Lean now checks vs. what remains
     SMT-only (overflow, range proof, non-row budgets). This is a doc edit in the same file.
2. **Per-predicate checkers vs. one combined checker?**
   - Recommendation: three independent checkers for 1:1 mapping to Rust check groups. Minor
     perf cost (three traversals) is irrelevant for a scaffold.

## Sources

### Primary (HIGH confidence — repo source + in-session Lean compile)
- `formal/lean/LoomCore.lean` — current AST, placeholders, theorem shapes (read fully)
- `crates/loom-core/src/full_verifier.rs` — `verify_l2_core` and all helpers (read fully)
- `crates/loom-core/src/l2_core.rs` — `ScalarType`, `Capability`, `L2CoreStmt`, `ResourceBudget`
- `scripts/full-verifier-test.sh`, `scripts/install-formal-tools.sh` — the exact Lean compile gate
- `./lean-toolchain` — `leanprover/lean4:v4.30.0`
- In-session: `lean --version` (4.30.0 confirmed), current file compiles exit 0, and a scratch
  file exercising every recommended idiom (`findSome?`, `any`, `mutual`, `decide`, `= true`
  projection) compiled exit 0 inside the repo toolchain context.

## Metadata
- Standard stack: HIGH — Lean core only, version pinned and confirmed.
- Architecture (checker design): HIGH — full idiom set compiled in-session.
- Pitfalls: HIGH — CI gate strings and toolchain-scoping behavior directly observed.
- Research date: 2026-06-09. Valid until: stable (no external/fast-moving deps).
