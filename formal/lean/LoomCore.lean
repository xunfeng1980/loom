/-
Phase 13 Lean scaffold for the tiny Loom L2Core verifier slice.

This file is a mechanized scaffold, not a complete final Loom soundness proof.
It names the core language objects and theorem targets that the Rust verifier,
SMT obligations, and future proof work must align with.

Important limitation: the Phase 13 predicates `builder_events_typed` and
`no_ambient_authority` are intentionally `True` placeholders. Therefore
`accepted_program_safe` is a tautological scaffold theorem over names and shape,
not load-bearing safety evidence. Current load-bearing verifier evidence lives
in the Rust executable verifier and the Phase 19 Bitwuzla-backed SMT discharge.

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

def finite_bounds (p : Program) : Prop :=
  p.maxRows >= 0

def builder_events_typed (_p : Program) : Prop :=
  True

def no_ambient_authority (_p : Program) : Prop :=
  True

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
