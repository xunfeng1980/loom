/-
Lean mechanized checker for the Loom L2Core verifier slice.

This file is a bounded mechanized checker for the static verifier surface in
`crates/loom-core/src/full_verifier.rs`. It is still NOT a final Loom
operational semantics or soundness theorem, but the covered checker now mirrors
the Rust `ScalarExpr` / `LetScalar` shape instead of the older pre-resolved
`Nat` projection.

What Lean machine-checks now (real `Bool`-valued checkers wrapped as `_ = true`):
  - `builder_events_typed` (via `checkTyped`): AppendValue output type match
    derives the value type from `ScalarExpr` through a scalar type environment,
    AppendNull output nullability is checked, and every appended-to builder is a
    declared output builder. Mirrored reject vocabulary:
    MissingOutputBuilder, UnknownVariable, OutputTypeMismatch, and
    OutputNullabilityMismatch.
  - `no_ambient_authority` (via `checkAuthority`): every ReadInput targets a
    declared input capability (MissingInputCapability), ReadInput offset/width
    expressions have known variables, concrete read ranges are spatially
    in-range when offsets/widths are constants, and append targets are declared
    output builders.
  - `finite_bounds` (via `checkBounds`): ForRange constant bounds require
    `end >= start` (InvalidLoopBounds) and `(end - start) <= maxRows`
    (ResourceBudgetExceeded); CursorLoop requires monotone positive progress
    of the form `cursor + positive-constant` (NonMonotoneCursorLoop) and a
    constant `limit <= maxRows` (ResourceBudgetExceeded).

Obligations that remain SMT-only:
  - Integer overflow (`AddNoOverflow` / `MulNoOverflow`) and non-concrete read
    range obligations are delegated to the Rust SMT path and Phase 19
    Bitwuzla-backed `QF_BV` discharge. Lean records expression typing and known
    variables for these expressions, but does not prove bitvector arithmetic.
  - Non-row resource budgets (`max_steps`, `max_builder_events`, per-builder
    `max_events`) remain executable Rust verifier checks unless later phases
    explicitly lift them into the model.

This is therefore Phase 37 correspondence evidence for the covered Rust
verifier slice, not a claim of full L2Core soundness. Current load-bearing
evidence also includes the Rust executable verifier and the Phase 19
Bitwuzla-backed SMT discharge.

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

inductive RejectCode where
  | MissingInputCapability
  | MissingOutputBuilder
  | UnknownVariable
  | OutputTypeMismatch
  | OutputNullabilityMismatch
  | InvalidLoopBounds
  | NonMonotoneCursorLoop
  | ResourceBudgetExceeded
  | ConstraintBudgetExceeded
deriving Repr, DecidableEq

def RejectCode.asString : RejectCode -> String
  | .MissingInputCapability => "missing-input-capability"
  | .MissingOutputBuilder => "missing-output-builder"
  | .UnknownVariable => "unknown-variable"
  | .OutputTypeMismatch => "output-type-mismatch"
  | .OutputNullabilityMismatch => "output-nullability-mismatch"
  | .InvalidLoopBounds => "invalid-loop-bounds"
  | .NonMonotoneCursorLoop => "non-monotone-cursor-loop"
  | .ResourceBudgetExceeded => "resource-budget-exceeded"
  | .ConstraintBudgetExceeded => "constraint-budget-exceeded"

inductive ScalarValue where
  | bool (value : Bool)
  | int32 (value : Int)
  | int64 (value : Int)
  | uint32 (value : Nat)
  | uint64 (value : Nat)
  | bytes (value : List Nat)
deriving Repr, DecidableEq

inductive ScalarExpr where
  | const (value : ScalarValue)
  | var (name : String)
  | add (lhs rhs : ScalarExpr)
  | sub (lhs rhs : ScalarExpr)
  | mul (lhs rhs : ScalarExpr)
  | min (lhs rhs : ScalarExpr)
  | max (lhs rhs : ScalarExpr)
  | eq (lhs rhs : ScalarExpr)
  | lt (lhs rhs : ScalarExpr)
  | le (lhs rhs : ScalarExpr)
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
  | readInput (capability : String) (offset : ScalarExpr) (width : ScalarExpr) (bind : String)
  | letScalar (name : String) (expr : ScalarExpr)
  | appendValue (builder : String) (value : ScalarExpr)
  | appendNull (builder : String)
  | forRange (index : String) (start : ScalarExpr) (end_ : ScalarExpr) (body : List Stmt)
  | cursorLoop (cursor : String) (limit : ScalarExpr) (progress : ScalarExpr) (body : List Stmt)
  | failClosed (code : String)
deriving Repr

structure Program where
  artifactVersion : Nat
  capabilities : List Capability
  body : List Stmt
  maxRows : Nat
deriving Repr

abbrev ScalarEnv := List (String × L2Ty)

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

def scalarLookup? (env : ScalarEnv) (name : String) : Option L2Ty :=
  env.findSome? fun entry => if entry.fst == name then some entry.snd else none

def scalarInsert (name : String) (ty : L2Ty) (env : ScalarEnv) : ScalarEnv :=
  (name, ty) :: env

def typeOfConst : ScalarValue -> L2Ty
  | .bool _ => .bool
  | .int32 _ => .int32
  | .int64 _ => .int64
  | .uint32 _ => .uint32
  | .uint64 _ => .uint64
  | .bytes _ => .bytes

def firstSome (lhs rhs : Option L2Ty) : Option L2Ty :=
  match lhs with
  | some ty => some ty
  | none => rhs

def typeOfExpr? (env : ScalarEnv) : ScalarExpr -> Option L2Ty
  | .const value => some (typeOfConst value)
  | .var name => scalarLookup? env name
  | .add lhs rhs => firstSome (typeOfExpr? env lhs) (typeOfExpr? env rhs)
  | .sub lhs rhs => firstSome (typeOfExpr? env lhs) (typeOfExpr? env rhs)
  | .mul lhs rhs => firstSome (typeOfExpr? env lhs) (typeOfExpr? env rhs)
  | .min lhs rhs => firstSome (typeOfExpr? env lhs) (typeOfExpr? env rhs)
  | .max lhs rhs => firstSome (typeOfExpr? env lhs) (typeOfExpr? env rhs)
  | .eq lhs rhs =>
      if (typeOfExpr? env lhs).isSome && (typeOfExpr? env rhs).isSome then some .bool else none
  | .lt lhs rhs =>
      if (typeOfExpr? env lhs).isSome && (typeOfExpr? env rhs).isSome then some .bool else none
  | .le lhs rhs =>
      if (typeOfExpr? env lhs).isSome && (typeOfExpr? env rhs).isSome then some .bool else none

def exprVarsKnown (env : ScalarEnv) : ScalarExpr -> Bool
  | .const _ => true
  | .var name => (scalarLookup? env name).isSome
  | .add lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs
  | .sub lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs
  | .mul lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs
  | .min lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs
  | .max lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs
  | .eq lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs
  | .lt lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs
  | .le lhs rhs => exprVarsKnown env lhs && exprVarsKnown env rhs

def exprWellTyped (env : ScalarEnv) (expr : ScalarExpr) : Bool :=
  exprVarsKnown env expr && (typeOfExpr? env expr).isSome

def constNat? : ScalarExpr -> Option Nat
  | .const (.uint64 value) => some value
  | .const (.uint32 value) => some value
  | .const (.int32 value) => if value < 0 then none else some (Int.toNat value)
  | .const (.int64 value) => if value < 0 then none else some (Int.toNat value)
  | _ => none

def scalarTypeForReadWidth (width : ScalarExpr) : L2Ty :=
  match constNat? width with
  | some 4 => .int32
  | some 8 => .int64
  | _ => .bytes

def isMonotoneProgress (cursor : String) : ScalarExpr -> Bool
  | .add (.var name) rhs =>
      name == cursor &&
        match constNat? rhs with
        | some value => decide (value > 0)
        | none => false
  | _ => false

def concreteReadInRange (sliceOffset sliceLen : Nat) (offset width : ScalarExpr) : Bool :=
  match constNat? offset, constNat? width with
  | some off, some len =>
      decide (off >= sliceOffset) && decide (off + len <= sliceOffset + sliceLen)
  | _, _ => true

/- builder_events_typed checker: output type match + nullability + declared builder
   + expression-derived value typing. Reject vocabulary mirrored:
   MissingOutputBuilder, UnknownVariable, OutputTypeMismatch,
   OutputNullabilityMismatch. -/
mutual
  def checkTypedStmt (caps : List Capability) (env : ScalarEnv) : Stmt -> Option ScalarEnv
    | .appendValue builder value =>
        match builderInfo? caps builder, typeOfExpr? env value with
        | some (expected, _), some actual =>
            if exprVarsKnown env value && expected == actual then some env else none
        | _, _ => none
    | .appendNull builder =>
        match builderInfo? caps builder with
        | some (_, nullable) => if nullable then some env else none
        | none => none
    | .letScalar name expr =>
        match typeOfExpr? env expr with
        | some ty => if exprVarsKnown env expr then some (scalarInsert name ty env) else none
        | none => none
    | .readInput _ _ width bind =>
        if exprWellTyped env width then some (scalarInsert bind (scalarTypeForReadWidth width) env) else none
    | .forRange index _ _ body =>
        checkTypedBody caps (scalarInsert index .rowIndex env) body |>.map fun _ => env
    | .cursorLoop cursor _ _ body =>
        checkTypedBody caps (scalarInsert cursor .rowIndex env) body |>.map fun _ => env
    | .failClosed _ => some env

  def checkTypedBody (caps : List Capability) (env : ScalarEnv) : List Stmt -> Option ScalarEnv
    | [] => some env
    | s :: rest =>
        match checkTypedStmt caps env s with
        | some next => checkTypedBody caps next rest
        | none => none
end

def checkTyped (caps : List Capability) (body : List Stmt) : Bool :=
  (checkTypedBody caps [] body).isSome

/- no_ambient_authority checker: declared input capability + known ReadInput
   expressions + concrete read spatial bounds where constants are available +
   declared output builder. Reject vocabulary mirrored: MissingInputCapability,
   MissingOutputBuilder, UnknownVariable. -/
mutual
  def checkAuthorityStmt (caps : List Capability) (env : ScalarEnv) : Stmt -> Option ScalarEnv
    | .readInput cap offset width bind =>
        match inputSlice? caps cap with
        | some (sliceOffset, sliceLen) =>
            if exprWellTyped env offset && exprWellTyped env width &&
              concreteReadInRange sliceOffset sliceLen offset width then
              some (scalarInsert bind (scalarTypeForReadWidth width) env)
            else none
        | none => none
    | .appendValue builder value =>
        match builderInfo? caps builder with
        | some _ => if exprWellTyped env value then some env else none
        | none => none
    | .appendNull builder =>
        match builderInfo? caps builder with
        | some _ => some env
        | none => none
    | .letScalar name expr =>
        match typeOfExpr? env expr with
        | some ty => if exprVarsKnown env expr then some (scalarInsert name ty env) else none
        | none => none
    | .forRange index _ _ body =>
        checkAuthorityBody caps (scalarInsert index .rowIndex env) body |>.map fun _ => env
    | .cursorLoop cursor _ _ body =>
        checkAuthorityBody caps (scalarInsert cursor .rowIndex env) body |>.map fun _ => env
    | .failClosed _ => some env

  def checkAuthorityBody (caps : List Capability) (env : ScalarEnv) : List Stmt -> Option ScalarEnv
    | [] => some env
    | s :: rest =>
        match checkAuthorityStmt caps env s with
        | some next => checkAuthorityBody caps next rest
        | none => none
end

def checkAuthority (caps : List Capability) (body : List Stmt) : Bool :=
  (checkAuthorityBody caps [] body).isSome

/- finite_bounds checker: ForRange finite constant bounds + row budget;
   CursorLoop monotone progress + finite constant row budget. Reject vocabulary
   mirrored: InvalidLoopBounds, NonMonotoneCursorLoop, ResourceBudgetExceeded,
   UnknownVariable. -/
mutual
  def checkBoundsStmt (caps : List Capability) (maxRows : Nat) (env : ScalarEnv) : Stmt -> Option ScalarEnv
    | .forRange index start end_ body =>
        match constNat? start, constNat? end_ with
        | some s, some e =>
            if exprVarsKnown env start && exprVarsKnown env end_ && decide (e >= s) && decide (e - s <= maxRows) then
              checkBoundsBody caps maxRows (scalarInsert index .rowIndex env) body |>.map fun _ => env
            else none
        | _, _ => none
    | .cursorLoop cursor limit progress body =>
        match constNat? limit with
        | some n =>
            if isMonotoneProgress cursor progress && decide (n <= maxRows) then
              checkBoundsBody caps maxRows (scalarInsert cursor .rowIndex env) body |>.map fun _ => env
            else none
        | none => none
    | .readInput _ offset width bind =>
        if exprWellTyped env offset && exprWellTyped env width then
          some (scalarInsert bind (scalarTypeForReadWidth width) env)
        else none
    | .letScalar name expr =>
        match typeOfExpr? env expr with
        | some ty => if exprVarsKnown env expr then some (scalarInsert name ty env) else none
        | none => none
    | .appendValue _ value =>
        if exprWellTyped env value then some env else none
    | .appendNull _ => some env
    | .failClosed _ => some env

  def checkBoundsBody (caps : List Capability) (maxRows : Nat) (env : ScalarEnv) : List Stmt -> Option ScalarEnv
    | [] => some env
    | s :: rest =>
        match checkBoundsStmt caps maxRows env s with
        | some next => checkBoundsBody caps maxRows next rest
        | none => none
end

def checkBounds (caps : List Capability) (maxRows : Nat) (body : List Stmt) : Bool :=
  (checkBoundsBody caps maxRows [] body).isSome

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

def u64 (value : Nat) : ScalarExpr :=
  .const (.uint64 value)

def validLetScalarAppendProgram : Program :=
  {
    artifactVersion := 1,
    capabilities := [
      .outputBuilder "out" .int32 false 8
    ],
    body := [
      .letScalar "x" (.const (.int32 7)),
      .appendValue "out" (.var "x")
    ],
    maxRows := 8
  }

def unknownVariableProgram : Program :=
  {
    artifactVersion := 1,
    capabilities := [
      .outputBuilder "out" .int32 false 8
    ],
    body := [
      .appendValue "out" (.var "missing")
    ],
    maxRows := 8
  }

def validCursorProgram : Program :=
  {
    artifactVersion := 1,
    capabilities := [
      .outputBuilder "out" .rowIndex false 8
    ],
    body := [
      .cursorLoop "cursor" (u64 3) (.add (.var "cursor") (u64 1)) [
        .appendValue "out" (.var "cursor")
      ]
    ],
    maxRows := 8
  }

example : checkTyped validLetScalarAppendProgram.capabilities validLetScalarAppendProgram.body = true := by
  native_decide

example : checkTyped unknownVariableProgram.capabilities unknownVariableProgram.body = false := by
  native_decide

example : checkBounds validCursorProgram.capabilities validCursorProgram.maxRows validCursorProgram.body = true := by
  native_decide
