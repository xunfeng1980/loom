/-
Lean mechanized checker for the Loom L2Core verifier slice.

This file is a bounded mechanized checker over a pre-resolved, `Nat`-grounded AST
projection of the Rust `verify_l2_core` logic (`crates/loom-core/src/full_verifier.rs`).
It is NOT a complete final Loom soundness proof, but the three semantic predicates
are now load-bearing decidable checkers, not `True` placeholders.

What Lean machine-checks now (real `Bool`-valued checkers wrapped as `_ = true`):
  - `builder_events_typed` (via `checkTyped`): AppendValue output type match
    (OutputTypeMismatch), AppendNull output nullability (OutputNullabilityMismatch),
    and that every appended-to builder is a declared output builder
    (MissingOutputBuilder).
  - `no_ambient_authority` (via `checkAuthority`): every ReadInput targets a declared
    input capability (MissingInputCapability), the read is spatially in-range over the
    declared slice (`offset >= sliceOffset && offset + width <= sliceOffset + sliceLen`,
    faithful because the Lean AST carries concrete `Nat` offset/width), and append
    targets are declared output builders (shared MissingOutputBuilder).
  - `finite_bounds` (via `checkBounds`): ForRange `stop >= start` (InvalidLoopBounds)
    and `(stop - start) <= maxRows` (ResourceBudgetExceeded, row budget); CursorLoop
    `progress > 0` (NonMonotoneCursorLoop) and `limit <= maxRows`
    (ResourceBudgetExceeded, row budget).

Obligations that remain SMT-only, with NO faithful counterpart on this `Nat`-grounded
Lean AST (intentionally NOT modeled here — modeling them would require enriching the
AST, a separate larger task):
  - Integer overflow (`AddNoOverflow` / `MulNoOverflow`): `Nat` is unbounded, so there
    is no overflow to catch in Lean; this is discharged by the Rust SMT path
    (Phase 19 Bitwuzla-backed `QF_BV`).
  - Unknown-variable / `ScalarExpr` variable environment (`UnknownVariable`,
    `LetScalar`): the Lean AST is pre-resolved (types/offsets already concrete), so it
    has no var-env to validate.
  - Non-row resource budgets (`max_steps`, `max_builder_events`, per-builder
    `max_events`): the Lean `Program` models only `maxRows`, not the full
    `ResourceBudget`.

This is therefore a bounded mechanized checker over a lossy AST projection, not a
claim of full L2Core soundness. Current load-bearing evidence also includes the Rust
executable verifier and the Phase 19 Bitwuzla-backed SMT discharge.

Rocq remains the fallback if extraction or verified-checker lineage becomes
mandatory for later milestones.
-/

inductive L2Ty where
  | bool
  | int32
  | int64
  | uint32
  | uint64
  | bytes
  | rowIndex
deriving Repr, DecidableEq

inductive Capability where
  | inputSlice (id : String) (offset : Nat) (length : Nat)
  | scratch (id : String) (maxBytes : Nat)
  | outputBuilder (id : String) (ty : L2Ty) (nullable : Bool) (maxEvents : Nat)
deriving Repr

inductive ArrowEvent where
  | appendValue (builderId : String) (ty : L2Ty)
  | appendNull (builderId : String) (ty : L2Ty)
  | finish (builderId : String)
deriving Repr

inductive Stmt where
  | readInput (capability : String) (offset : Nat) (width : Nat) (bind : String)
  | appendValue (builder : String) (ty : L2Ty)
  | appendNull (builder : String)
  | forRange (index : String) (start : Nat) (stop : Nat) (body : List Stmt)
  | cursorLoop (cursor : String) (limit : Nat) (progress : Nat) (body : List Stmt)
  | failClosed (code : String)
deriving Repr

structure Program where
  artifactVersion : Nat
  capabilities : List Capability
  body : List Stmt
  maxRows : Nat
deriving Repr

/-- Lookup the declared output builder's resolved scalar type and nullability.
    Mirrors the Rust `MissingOutputBuilder` / nullability lookup. -/
def builderInfo? (caps : List Capability) (name : String) : Option (L2Ty × Bool) :=
  caps.findSome? fun c => match c with
    | .outputBuilder id ty nullable _ => if id == name then some (ty, nullable) else none
    | _ => none

/-- Lookup the declared input slice's (offset, length).
    Mirrors the Rust `input_capabilities` lookup / `MissingInputCapability`. -/
def inputSlice? (caps : List Capability) (name : String) : Option (Nat × Nat) :=
  caps.findSome? fun c => match c with
    | .inputSlice id offset length => if id == name then some (offset, length) else none
    | _ => none

/- builder_events_typed checker: output type match + nullability + declared builder.
   Reject codes mirrored: OutputTypeMismatch, OutputNullabilityMismatch,
   MissingOutputBuilder. -/
mutual
  def checkTypedStmt (caps : List Capability) : Stmt → Bool
    | .appendValue builder ty =>
        match builderInfo? caps builder with
        | some (bty, _) => bty == ty                 -- OutputTypeMismatch + MissingOutputBuilder
        | none          => false
    | .appendNull builder =>
        match builderInfo? caps builder with
        | some (_, nullable) => nullable             -- OutputNullabilityMismatch + MissingOutputBuilder
        | none               => false
    | .readInput _ _ _ _ => true
    | .forRange _ _ _ body => checkTypedBody caps body
    | .cursorLoop _ _ _ body => checkTypedBody caps body
    | .failClosed _ => true
  def checkTypedBody (caps : List Capability) : List Stmt → Bool
    | []        => true
    | s :: rest => checkTypedStmt caps s && checkTypedBody caps rest
end

/-- builder_events_typed entry point over a program body. -/
def checkTyped (caps : List Capability) (body : List Stmt) : Bool :=
  checkTypedBody caps body

/- no_ambient_authority checker: declared input capability + read spatial bounds +
   declared output builder. Reject codes mirrored: MissingInputCapability,
   MissingOutputBuilder, plus the read in-range obligation (faithful here because
   Lean offset/width are concrete `Nat`). -/
mutual
  def checkAuthorityStmt (caps : List Capability) : Stmt → Bool
    | .readInput cap offset width _ =>
        match inputSlice? caps cap with             -- MissingInputCapability
        | some (sOff, sLen) =>
            decide (offset >= sOff) && decide (offset + width <= sOff + sLen)  -- read in-range
        | none => false
    | .appendValue builder _ =>
        match builderInfo? caps builder with        -- shared MissingOutputBuilder
        | some _ => true
        | none   => false
    | .appendNull builder =>
        match builderInfo? caps builder with        -- shared MissingOutputBuilder
        | some _ => true
        | none   => false
    | .forRange _ _ _ body => checkAuthorityBody caps body
    | .cursorLoop _ _ _ body => checkAuthorityBody caps body
    | .failClosed _ => true
  def checkAuthorityBody (caps : List Capability) : List Stmt → Bool
    | []        => true
    | s :: rest => checkAuthorityStmt caps s && checkAuthorityBody caps rest
end

/-- no_ambient_authority entry point over a program body. -/
def checkAuthority (caps : List Capability) (body : List Stmt) : Bool :=
  checkAuthorityBody caps body

/- finite_bounds checker: ForRange `stop>=start` + `(stop-start)<=maxRows`;
   CursorLoop `progress>0` + `limit<=maxRows`. Reject codes mirrored:
   InvalidLoopBounds, NonMonotoneCursorLoop, ResourceBudgetExceeded (row budget). -/
mutual
  def checkBoundsStmt (caps : List Capability) (maxRows : Nat) : Stmt → Bool
    | .forRange _ start stop body =>
        decide (stop >= start) && decide (stop - start <= maxRows)   -- InvalidLoopBounds + budget
          && checkBoundsBody caps maxRows body
    | .cursorLoop _ limit progress body =>
        decide (progress > 0) && decide (limit <= maxRows)           -- NonMonotone + budget
          && checkBoundsBody caps maxRows body
    | .readInput _ _ _ _ => true
    | .appendValue _ _ => true
    | .appendNull _ => true
    | .failClosed _ => true
  def checkBoundsBody (caps : List Capability) (maxRows : Nat) : List Stmt → Bool
    | []        => true
    | s :: rest => checkBoundsStmt caps maxRows s && checkBoundsBody caps maxRows rest
end

/-- finite_bounds entry point over a program body. -/
def checkBounds (caps : List Capability) (maxRows : Nat) (body : List Stmt) : Bool :=
  checkBoundsBody caps maxRows body

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

theorem builder_events_well_formed (p : Program) :
    Verified p -> builder_events_typed p := by
  intro h
  exact h.right.left

theorem accepted_program_safe (p : Program) :
    Verified p -> Safe p := by
  intro h
  exact And.intro h.right.left h.right.right
