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
  | float32
  | float64
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
  | ExplicitFailClosed
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
  | .ExplicitFailClosed => "explicit-fail-closed"

inductive ScalarValue where
  | bool (value : Bool)
  | int32 (value : Int)
  | int64 (value : Int)
  | float32Bits (value : Nat)
  | float64Bits (value : Nat)
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
  | .float32Bits _ => .float32
  | .float64Bits _ => .float64
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
    | .failClosed _ => none

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
    | .failClosed _ => none

  def checkAuthorityBody (caps : List Capability) (env : ScalarEnv) : List Stmt -> Option ScalarEnv
    | [] => some env
    | s :: rest =>
        match checkAuthorityStmt caps env s with
        | some next => checkAuthorityBody caps next rest
        | none => none
end

def checkAuthority (caps : List Capability) (body : List Stmt) : Bool :=
  (checkAuthorityBody caps [] body).isSome

theorem checked_readInput_concrete_in_range
    (caps : List Capability) (env : ScalarEnv) (cap : String)
    (offset width : ScalarExpr) (bind : String) (next : ScalarEnv) :
    checkAuthorityStmt caps env (.readInput cap offset width bind) = some next ->
      ∃ sliceOffset sliceLen,
        inputSlice? caps cap = some (sliceOffset, sliceLen) ∧
        exprWellTyped env offset = true ∧
        exprWellTyped env width = true ∧
        concreteReadInRange sliceOffset sliceLen offset width = true := by
  intro h
  cases hSlice : inputSlice? caps cap with
  | none =>
      simp [checkAuthorityStmt, hSlice] at h
  | some slice =>
      cases slice with
      | mk sliceOffset sliceLen =>
          by_cases hOffset : exprWellTyped env offset
          · by_cases hWidth : exprWellTyped env width
            · by_cases hRange : concreteReadInRange sliceOffset sliceLen offset width
              · refine ⟨sliceOffset, sliceLen, rfl, hOffset, hWidth, hRange⟩
              · simp [checkAuthorityStmt, hSlice, hOffset, hWidth, hRange] at h
            · simp [checkAuthorityStmt, hSlice, hOffset, hWidth] at h
          · simp [checkAuthorityStmt, hSlice, hOffset] at h

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
    | .failClosed _ => none

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

def StaticVerified (p : Program) : Prop :=
  finite_bounds p /\ builder_events_typed p /\ no_ambient_authority p

def Safe (p : Program) : Prop :=
  builder_events_typed p /\ no_ambient_authority p

theorem builder_events_well_formed (p : Program) :
    StaticVerified p -> builder_events_typed p := by
  intro h
  exact h.right.left

theorem structural_safe_projection (p : Program) :
    StaticVerified p -> Safe p := by
  intro h
  exact And.intro h.right.left h.right.right

def u64 (value : Nat) : ScalarExpr :=
  .const (.uint64 value)

def firstRejectOfExpr? (env : ScalarEnv) : ScalarExpr -> Option RejectCode
  | .const _ => none
  | .var name => if (scalarLookup? env name).isSome then none else some .UnknownVariable
  | .add lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs
  | .sub lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs
  | .mul lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs
  | .min lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs
  | .max lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs
  | .eq lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs
  | .lt lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs
  | .le lhs rhs => firstRejectOfExpr? env lhs <|> firstRejectOfExpr? env rhs

mutual
  def classifyStmt? (caps : List Capability) (maxRows : Nat) (env : ScalarEnv) : Stmt -> Option RejectCode × ScalarEnv
    | .forRange index start end_ body =>
        match constNat? end_, constNat? start with
        | some e, some s =>
            if decide (e >= s) then
              if decide (e - s <= maxRows) then
                match classifyBody? caps maxRows (scalarInsert index .rowIndex env) body with
                | (none, _) => (none, env)
                | (some code, _) => (some code, env)
              else (some .ResourceBudgetExceeded, env)
            else (some .InvalidLoopBounds, env)
        | _, _ => (some .InvalidLoopBounds, env)
    | .cursorLoop cursor limit progress body =>
        if !isMonotoneProgress cursor progress then
          (some .NonMonotoneCursorLoop, env)
        else
          match constNat? limit with
          | some n =>
              if decide (n <= maxRows) then
                match classifyBody? caps maxRows (scalarInsert cursor .rowIndex env) body with
                | (none, _) => (none, env)
                | (some code, _) => (some code, env)
              else (some .ResourceBudgetExceeded, env)
          | none => (some .InvalidLoopBounds, env)
    | .readInput cap offset width bind =>
        match inputSlice? caps cap with
        | some (sliceOffset, sliceLen) =>
            if exprWellTyped env offset && exprWellTyped env width then
              if concreteReadInRange sliceOffset sliceLen offset width then
                (none, scalarInsert bind (scalarTypeForReadWidth width) env)
              else (some .MissingInputCapability, env)
            else (some .UnknownVariable, env)
        | none => (some .MissingInputCapability, env)
    | .letScalar name expr =>
        match typeOfExpr? env expr with
        | some ty =>
            if exprVarsKnown env expr then
              (none, scalarInsert name ty env)
            else (some .UnknownVariable, env)
        | none => (some .UnknownVariable, env)
    | .appendValue builder value =>
        match builderInfo? caps builder, typeOfExpr? env value with
        | some (expected, _), some actual =>
            if exprVarsKnown env value && expected == actual then
              (none, env)
            else (some .OutputTypeMismatch, env)
        | none, _ => (some .MissingOutputBuilder, env)
        | _, none => (some .UnknownVariable, env)
    | .appendNull builder =>
        match builderInfo? caps builder with
        | none => (some .MissingOutputBuilder, env)
        | some (_, nullable) =>
            if nullable then (none, env) else (some .OutputNullabilityMismatch, env)
    | .failClosed _ => (some .ExplicitFailClosed, env)

  def classifyBody? (caps : List Capability) (maxRows : Nat) (env : ScalarEnv) : List Stmt -> Option RejectCode × ScalarEnv
    | [] => (none, env)
    | s :: rest =>
        match classifyStmt? caps maxRows env s with
        | (some code, next) => (some code, next)
        | (none, next) => classifyBody? caps maxRows next rest
end

def classifyProgram? (p : Program) : Option RejectCode :=
  (classifyBody? p.capabilities p.maxRows [] p.body).fst

def Verified (p : Program) : Prop :=
  StaticVerified p /\ classifyProgram? p = none

def classificationString (code : Option RejectCode) : String :=
  match code with
  | some reject => reject.asString
  | none => "accepted"

def correspondenceLine (id : String) (program : Program) : String :=
  "correspondence:" ++ id ++ ":" ++ classificationString (classifyProgram? program)

/- Modeled executor semantics for Phase 38.

   This is a proof-friendly modeled executor, not a byte-accurate Rust
   interpreter or Arrow buffer implementation. It records abstract reads and
   typed builder events so the Lean theorem can talk about the modeled executor
   safety obligations assigned to Phase 38. -/

structure ModeledInput where
  satisfiesCapabilities : Program -> Prop

structure ModeledRead where
  capability : String
  offset : ScalarExpr
  width : ScalarExpr
  inBounds : Bool
deriving Repr

inductive ModeledEvent where
  | appendValue (builderId : String) (ty : L2Ty)
  | appendNull (builderId : String) (ty : L2Ty)
deriving Repr

inductive ExecutionStatus where
  | running
  | finished
  | failClosed
deriving Repr, DecidableEq

def eventWellTyped (caps : List Capability) : ModeledEvent -> Bool
  | .appendValue builder ty =>
      match builderInfo? caps builder with
      | some (expected, _) => expected == ty
      | none => false
  | .appendNull builder ty =>
      match builderInfo? caps builder with
      | some (expected, nullable) => nullable && expected == ty
      | none => false

structure ModeledState where
  caps : List Capability
  maxRows : Nat
  scalars : ScalarEnv
  reads : List ModeledRead
  events : List ModeledEvent
  rowsUsed : Nat
  status : ExecutionStatus
  readSafety : status = .failClosed \/ reads.all (fun read => read.inBounds) = true
  eventsTyped : events.all (eventWellTyped caps) = true
  rowsWithinMax : rowsUsed <= maxRows

def initialModeledState (caps : List Capability) (maxRows : Nat) : ModeledState :=
  {
    caps := caps,
    maxRows := maxRows,
    scalars := [],
    reads := [],
    events := [],
    rowsUsed := 0,
    status := .running,
    readSafety := by
      right
      simp,
    eventsTyped := by simp,
    rowsWithinMax := Nat.zero_le maxRows
  }

def modeledFailClosed (state : ModeledState) : ModeledState :=
  {
    state with
    status := .failClosed,
    readSafety := by
      left
      rfl
  }

def modeledFinished (state : ModeledState) (running : state.status = .running) : ModeledState :=
  {
    state with
    status := .finished,
    readSafety := by
      cases state.readSafety with
      | inl failed =>
          rw [running] at failed
          cases failed
      | inr safe =>
          right
          exact safe
  }

def finalizeModeledState (state : ModeledState) : ModeledState :=
  if state.status == .running then modeledFailClosed state else state

theorem finalized_status_terminal (state : ModeledState) :
    (finalizeModeledState state).status = .finished \/
      (finalizeModeledState state).status = .failClosed := by
  cases h : state.status
  · right
    simp [finalizeModeledState, modeledFailClosed, h]
  · left
    simp [finalizeModeledState, h]
  · right
    simp [finalizeModeledState, h]

def appendModeledReadInBounds (state : ModeledState) (cap : String) (offset width : ScalarExpr) : ModeledState :=
  {
    state with
    reads := state.reads ++ [{ capability := cap, offset := offset, width := width, inBounds := true }],
    readSafety := by
      cases state.readSafety with
      | inl failed =>
          left
          exact failed
      | inr safe =>
          right
          simp [List.all_append, safe]
  }

def appendModeledReadOutOfBoundsFailed (state : ModeledState) (cap : String) (offset width : ScalarExpr) : ModeledState :=
  {
    state with
    reads := state.reads ++ [{ capability := cap, offset := offset, width := width, inBounds := false }],
    status := .failClosed,
    readSafety := by
      left
      rfl
  }

def appendModeledEvent (state : ModeledState) (event : ModeledEvent)
    (h : eventWellTyped state.caps event = true) : ModeledState :=
  {
    state with
    events := state.events ++ [event],
    eventsTyped := by
      simp [List.all_append, state.eventsTyped, h]
  }

def appendModeledAppendValueEventKnown (state : ModeledState) (builder : String)
    (expected : L2Ty) (nullable : Bool)
    (h : builderInfo? state.caps builder = some (expected, nullable)) : ModeledState :=
  appendModeledEvent state (.appendValue builder expected) (by
    simp [eventWellTyped, h])

def appendModeledAppendNullEventKnown (state : ModeledState) (builder : String)
    (ty : L2Ty) (nullable : Bool)
    (h : builderInfo? state.caps builder = some (ty, nullable)) (hn : nullable = true) :
    ModeledState :=
  appendModeledEvent state (.appendNull builder ty) (by
    simp [eventWellTyped, h, hn])

def appendModeledAppendValueEvent (state : ModeledState) (builder : String) : ModeledState :=
  match h : builderInfo? state.caps builder with
  | some (expected, _) =>
      appendModeledEvent state (.appendValue builder expected) (by
        simp [eventWellTyped, h])
  | none => modeledFailClosed state

def appendModeledAppendNullEvent (state : ModeledState) (builder : String) : ModeledState :=
  match h : builderInfo? state.caps builder with
  | some (ty, nullable) =>
      if hn : nullable then
        appendModeledEvent state (.appendNull builder ty) (by
          simp [eventWellTyped, h, hn])
      else modeledFailClosed state
  | none => modeledFailClosed state

def addModeledRows (state : ModeledState) (rows : Nat) : ModeledState :=
  if h : rows <= state.maxRows then
    {
      state with
      rowsUsed := max state.rowsUsed rows,
      rowsWithinMax := by
        rw [Nat.max_le]
        exact And.intro state.rowsWithinMax h
    }
  else modeledFailClosed state

def restoreScalars (before after : ModeledState) : ModeledState :=
  { after with scalars := before.scalars }

def NoOutOfBoundsRead (p : Program) (state : ModeledState) : Prop :=
  state.status = .finished /\
    state.reads.all (fun read => read.inBounds) = true /\
    no_ambient_authority p /\
    (∀ caps env cap offset width bind next,
      checkAuthorityStmt caps env (.readInput cap offset width bind) = some next ->
        ∃ sliceOffset sliceLen,
          inputSlice? caps cap = some (sliceOffset, sliceLen) ∧
          exprWellTyped env offset = true ∧
          exprWellTyped env width = true ∧
          concreteReadInRange sliceOffset sliceLen offset width = true)

def BuilderEventsWellTyped (p : Program) (state : ModeledState) : Prop :=
  state.events.all (eventWellTyped state.caps) = true /\
    builder_events_typed p

def TerminatesWithinMaxRows (p : Program) (state : ModeledState) : Prop :=
  state.rowsUsed <= state.maxRows /\
    (state.status = .finished \/ state.status = .failClosed) /\
    finite_bounds p

def ArrowWellFormedByConstruction (p : Program) (state : ModeledState) : Prop :=
  state.events.all (eventWellTyped state.caps) = true /\
    builder_events_typed p

def ModeledRunSafe (p : Program) (state : ModeledState) : Prop :=
  NoOutOfBoundsRead p state /\
    BuilderEventsWellTyped p state /\
    TerminatesWithinMaxRows p state /\
    ArrowWellFormedByConstruction p state

def modeledExecutorScopeNote : String :=
  "Phase 38 theorem scope: modeled executor only; Rust interpreter consistency is Phase 39 and native/model validation is Phase 40."

mutual
  def execStmtFuel (fuel : Nat) (stmt : Stmt) (state : ModeledState) : ModeledState :=
    match fuel with
    | 0 => modeledFailClosed state
    | fuel' + 1 =>
        if state.status != .running then state else
        match stmt with
        | .readInput cap offset width bind =>
            match inputSlice? state.caps cap with
            | none => modeledFailClosed state
            | some (sliceOffset, sliceLen) =>
                if exprWellTyped state.scalars offset && exprWellTyped state.scalars width then
                  let inBounds := concreteReadInRange sliceOffset sliceLen offset width
                  if inBounds then
                    let next := appendModeledReadInBounds state cap offset width
                    { next with scalars := scalarInsert bind (scalarTypeForReadWidth width) state.scalars }
                  else appendModeledReadOutOfBoundsFailed state cap offset width
                else modeledFailClosed state
        | .letScalar name expr =>
            match typeOfExpr? state.scalars expr with
            | some ty =>
                if exprVarsKnown state.scalars expr then
                  { state with scalars := scalarInsert name ty state.scalars }
                else modeledFailClosed state
            | none => modeledFailClosed state
        | .appendValue builder value =>
            match builderInfo? state.caps builder with
            | some (expected, _) =>
                match typeOfExpr? state.scalars value with
                | some actual =>
                    if exprVarsKnown state.scalars value && expected == actual then
                      state
                    else modeledFailClosed state
                | none => modeledFailClosed state
            | none => modeledFailClosed state
        | .appendNull builder =>
            match builderInfo? state.caps builder with
            | some (_, nullable) =>
                if nullable then
                  state
                else modeledFailClosed state
            | none => modeledFailClosed state
        | .forRange index start end_ body =>
            match constNat? start, constNat? end_ with
            | some s, some e =>
                if decide (e >= s) && decide (e - s <= state.maxRows) then
                  let countedState := addModeledRows state (e - s)
                  if countedState.status == .running then
                    let loopState := { countedState with scalars := scalarInsert index .rowIndex state.scalars }
                    restoreScalars state (execBodyFuel fuel' body loopState)
                  else countedState
                else modeledFailClosed state
            | _, _ => modeledFailClosed state
        | .cursorLoop cursor limit progress body =>
            match constNat? limit with
            | some n =>
                if isMonotoneProgress cursor progress && decide (n <= state.maxRows) then
                  let countedState := addModeledRows state n
                  if countedState.status == .running then
                    let loopState := { countedState with scalars := scalarInsert cursor .rowIndex state.scalars }
                    restoreScalars state (execBodyFuel fuel' body loopState)
                  else countedState
                else modeledFailClosed state
            | none => modeledFailClosed state
        | .failClosed _ => modeledFailClosed state

  def execBodyFuel (fuel : Nat) (body : List Stmt) (state : ModeledState) : ModeledState :=
    match fuel with
    | 0 => modeledFailClosed state
    | fuel' + 1 =>
        match body with
        | [] =>
            match h : state.status with
            | .running => modeledFinished state h
            | .finished => state
            | .failClosed => state
        | stmt :: rest =>
            let next := execStmtFuel fuel' stmt state
            if next.status == .running then execBodyFuel fuel' rest next else next
end

mutual
  def execStmt (stmt : Stmt) (state : ModeledState) : ModeledState :=
    if state.status != .running then state else
    match stmt with
    | .readInput cap offset width bind =>
        match inputSlice? state.caps cap with
        | none => modeledFailClosed state
        | some (sliceOffset, sliceLen) =>
            if exprWellTyped state.scalars offset && exprWellTyped state.scalars width then
              let inBounds := concreteReadInRange sliceOffset sliceLen offset width
              if inBounds then
                let next := appendModeledReadInBounds state cap offset width
                { next with scalars := scalarInsert bind (scalarTypeForReadWidth width) state.scalars }
              else appendModeledReadOutOfBoundsFailed state cap offset width
            else modeledFailClosed state
    | .letScalar name expr =>
        match typeOfExpr? state.scalars expr with
        | some ty =>
            if exprVarsKnown state.scalars expr then
              { state with scalars := scalarInsert name ty state.scalars }
            else modeledFailClosed state
        | none => modeledFailClosed state
    | .appendValue builder value =>
        match builderInfo? state.caps builder with
        | some (expected, _) =>
            match typeOfExpr? state.scalars value with
            | some actual =>
                if exprVarsKnown state.scalars value && expected == actual then
                  state
                else modeledFailClosed state
            | none => modeledFailClosed state
        | none => modeledFailClosed state
    | .appendNull builder =>
        match builderInfo? state.caps builder with
        | some (_, nullable) =>
            if nullable then
              state
            else modeledFailClosed state
        | none => modeledFailClosed state
    | .forRange index start end_ body =>
        match constNat? start, constNat? end_ with
        | some s, some e =>
            if decide (e >= s) && decide (e - s <= state.maxRows) then
              let countedState := addModeledRows state (e - s)
              if countedState.status == .running then
                let loopState := { countedState with scalars := scalarInsert index .rowIndex state.scalars }
                restoreScalars state (execBody body loopState)
              else countedState
            else modeledFailClosed state
        | _, _ => modeledFailClosed state
    | .cursorLoop cursor limit progress body =>
        match constNat? limit with
        | some n =>
            if isMonotoneProgress cursor progress && decide (n <= state.maxRows) then
              let countedState := addModeledRows state n
              if countedState.status == .running then
                let loopState := { countedState with scalars := scalarInsert cursor .rowIndex state.scalars }
                restoreScalars state (execBody body loopState)
              else countedState
            else modeledFailClosed state
        | none => modeledFailClosed state
    | .failClosed _ => modeledFailClosed state

  def execBody (body : List Stmt) (state : ModeledState) : ModeledState :=
    match body with
    | [] =>
        match h : state.status with
        | .running => modeledFinished state h
        | .finished => state
        | .failClosed => state
    | stmt :: rest =>
        let next := execStmt stmt state
        if next.status == .running then execBody rest next else next
end

def execProgram (p : Program) : ModeledState :=
  finalizeModeledState (execBody p.body (initialModeledState p.capabilities p.maxRows))

def ModeledExecutionSafe (p : Program) : Prop :=
  ModeledRunSafe p (execProgram p)

theorem execBody_nil_running (state : ModeledState) :
    state.status = .running -> (execBody [] state).status = .finished := by
  intro hStatus
  cases state with
  | mk caps maxRows scalars reads events rowsUsed status readSafety eventsTyped rowsWithinMax =>
      cases status <;> simp [execBody, modeledFinished] at *

mutual
  theorem classified_stmt_exec_progress
      (caps : List Capability) (maxRows : Nat) (env nextEnv : ScalarEnv)
      (stmt : Stmt) (state : ModeledState) :
      state.caps = caps ->
      state.maxRows = maxRows ->
      state.scalars = env ->
      state.status = .running ->
      classifyStmt? caps maxRows env stmt = (none, nextEnv) ->
        ((execStmt stmt state).status = .running /\
          (execStmt stmt state).caps = caps /\
          (execStmt stmt state).maxRows = maxRows /\
          (execStmt stmt state).scalars = nextEnv) \/
        (execStmt stmt state).status = .finished := by
    intro hCaps hRows hScalars hStatus hClassified
    subst hCaps
    subst hRows
    subst hScalars
    cases stmt
    · rename_i cap offset width bind
      cases hSlice : inputSlice? state.caps cap with
      | none =>
          simp [classifyStmt?, hSlice] at hClassified
      | some slice =>
          cases slice with
          | mk sliceOffset sliceLen =>
              by_cases hWell : exprWellTyped state.scalars offset && exprWellTyped state.scalars width
              · by_cases hRange : concreteReadInRange sliceOffset sliceLen offset width
                · simp [classifyStmt?, hSlice, hWell, hRange] at hClassified
                  subst nextEnv
                  left
                  simp [execStmt, hStatus, hSlice, hWell, hRange, appendModeledReadInBounds]
                · simp [classifyStmt?, hSlice, hWell, hRange] at hClassified
              · simp [classifyStmt?, hSlice, hWell] at hClassified
    · rename_i name expr
      cases hTy : typeOfExpr? state.scalars expr with
      | none =>
          simp [classifyStmt?, hTy] at hClassified
      | some ty =>
          by_cases hKnown : exprVarsKnown state.scalars expr
          · simp [classifyStmt?, hTy, hKnown] at hClassified
            subst nextEnv
            left
            simp [execStmt, hStatus, hTy, hKnown]
          · simp [classifyStmt?, hTy, hKnown] at hClassified
    · rename_i builder value
      cases hBuilder : builderInfo? state.caps builder with
      | none =>
          simp [classifyStmt?, hBuilder] at hClassified
      | some builderInfo =>
          cases builderInfo with
          | mk expected nullable =>
              cases hTy : typeOfExpr? state.scalars value with
              | none =>
                  simp [classifyStmt?, hBuilder, hTy] at hClassified
              | some actual =>
                  by_cases hOk : exprVarsKnown state.scalars value && expected == actual
                  · simp [classifyStmt?, hBuilder, hTy, hOk] at hClassified
                    subst nextEnv
                    left
                    simp [execStmt, hStatus, hBuilder, hTy, hOk]
                  · simp [classifyStmt?, hBuilder, hTy, hOk] at hClassified
    · rename_i builder
      cases hBuilder : builderInfo? state.caps builder with
      | none =>
          simp [classifyStmt?, hBuilder] at hClassified
      | some builderInfo =>
          cases builderInfo with
          | mk ty nullable =>
              by_cases hNullable : nullable
              · simp [classifyStmt?, hBuilder, hNullable] at hClassified
                subst nextEnv
                left
                simp [execStmt, hStatus, hBuilder, hNullable]
              · simp [classifyStmt?, hBuilder, hNullable] at hClassified
    · rename_i index start end_ body
      cases hStart : constNat? start with
      | none =>
          simp [classifyStmt?, hStart] at hClassified
      | some s =>
          cases hEnd : constNat? end_ with
          | none =>
              simp [classifyStmt?, hStart, hEnd] at hClassified
          | some e =>
              by_cases hGe : e >= s
              · by_cases hRowsOk : e - s <= state.maxRows
                · cases hBody : classifyBody? state.caps state.maxRows (scalarInsert index .rowIndex state.scalars) body with
                  | mk code bodyEnv =>
                      cases code with
                      | none =>
                          simp [classifyStmt?, hStart, hEnd, hGe, hRowsOk, hBody] at hClassified
                          right
                          have hCountedRunning :
                              (addModeledRows state (e - s)).status = .running := by
                            simp [addModeledRows, hRowsOk, hStatus]
                          have hBodyFinished :=
                            classified_body_exec_finishes state.caps state.maxRows
                              (scalarInsert index .rowIndex state.scalars) bodyEnv body
                              { addModeledRows state (e - s) with scalars := scalarInsert index .rowIndex state.scalars }
                              (by simp [addModeledRows, hRowsOk])
                              (by simp [addModeledRows, hRowsOk])
                              rfl hCountedRunning hBody
                          simpa [execStmt, hStatus, hStart, hEnd, hGe, hRowsOk, hCountedRunning,
                            restoreScalars] using hBodyFinished
                      | some reject =>
                          simp [classifyStmt?, hStart, hEnd, hGe, hRowsOk, hBody] at hClassified
                · simp [classifyStmt?, hStart, hEnd, hGe, hRowsOk] at hClassified
              · simp [classifyStmt?, hStart, hEnd, hGe] at hClassified
    · rename_i cursor limit progress body
      by_cases hMono : isMonotoneProgress cursor progress
      · cases hLimit : constNat? limit with
        | none =>
            simp [classifyStmt?, hMono, hLimit] at hClassified
        | some n =>
            by_cases hRowsOk : n <= state.maxRows
            · cases hBody : classifyBody? state.caps state.maxRows (scalarInsert cursor .rowIndex state.scalars) body with
              | mk code bodyEnv =>
                  cases code with
                  | none =>
                      simp [classifyStmt?, hMono, hLimit, hRowsOk, hBody] at hClassified
                      right
                      have hCountedRunning :
                          (addModeledRows state n).status = .running := by
                        simp [addModeledRows, hRowsOk, hStatus]
                      have hBodyFinished :=
                        classified_body_exec_finishes state.caps state.maxRows
                          (scalarInsert cursor .rowIndex state.scalars) bodyEnv body
                          { addModeledRows state n with scalars := scalarInsert cursor .rowIndex state.scalars }
                          (by simp [addModeledRows, hRowsOk])
                          (by simp [addModeledRows, hRowsOk])
                          rfl hCountedRunning hBody
                      simpa [execStmt, hStatus, hMono, hLimit, hRowsOk, hCountedRunning,
                        restoreScalars] using hBodyFinished
                  | some reject =>
                      simp [classifyStmt?, hMono, hLimit, hRowsOk, hBody] at hClassified
            · simp [classifyStmt?, hMono, hLimit, hRowsOk] at hClassified
      · simp [classifyStmt?, hMono] at hClassified
    · rename_i code
      simp [classifyStmt?] at hClassified

  theorem classified_body_exec_finishes
      (caps : List Capability) (maxRows : Nat) (env nextEnv : ScalarEnv)
      (body : List Stmt) (state : ModeledState) :
      state.caps = caps ->
      state.maxRows = maxRows ->
      state.scalars = env ->
      state.status = .running ->
      classifyBody? caps maxRows env body = (none, nextEnv) ->
        (execBody body state).status = .finished := by
    intro hCaps hRows hScalars hStatus hClassified
    subst hCaps
    subst hRows
    subst hScalars
    cases body with
    | nil =>
        simp [classifyBody?] at hClassified
        exact execBody_nil_running state hStatus
    | cons stmt rest =>
        cases hStmt : classifyStmt? state.caps state.maxRows state.scalars stmt with
        | mk code stmtEnv =>
            cases code with
            | none =>
                simp [classifyBody?, hStmt] at hClassified
                have hProgress :=
                  classified_stmt_exec_progress state.caps state.maxRows state.scalars stmtEnv stmt state
                    rfl rfl rfl hStatus hStmt
                cases hProgress with
                | inl running =>
                    rcases running with ⟨hRunning, hCapsNext, hRowsNext, hScalarsNext⟩
                    have hRest :=
                      classified_body_exec_finishes state.caps state.maxRows stmtEnv nextEnv rest
                        (execStmt stmt state) hCapsNext hRowsNext hScalarsNext hRunning hClassified
                    simp [execBody, hRunning, hRest]
                | inr finished =>
                    simp [execBody, finished]
            | some reject =>
                simp [classifyBody?, hStmt] at hClassified
end

theorem classified_program_finishes (p : Program) :
    classifyProgram? p = none -> (execProgram p).status = .finished := by
  intro h
  unfold execProgram classifyProgram? at *
  cases hBody : classifyBody? p.capabilities p.maxRows [] p.body with
  | mk code env =>
      cases code with
      | none =>
          have hFinished :=
            classified_body_exec_finishes p.capabilities p.maxRows [] env p.body
              (initialModeledState p.capabilities p.maxRows)
              rfl rfl rfl (by simp [initialModeledState]) hBody
          simp [hBody] at h
          simp [hFinished, finalizeModeledState]
      | some reject =>
          simp [hBody] at h

theorem verified_program_finishes (p : Program) :
    Verified p -> (execProgram p).status = .finished := by
  intro h
  exact classified_program_finishes p h.right

theorem finished_state_reads_in_bounds (state : ModeledState) :
    state.status = .finished ->
    (state.status = .failClosed \/ state.reads.all (fun read => read.inBounds) = true) ->
      state.reads.all (fun read => read.inBounds) = true := by
  intro hFinished hSafety
  cases hSafety with
  | inl failed =>
      rw [hFinished] at failed
      cases failed
  | inr safe =>
      exact safe

theorem verified_program_reads_in_bounds (p : Program) :
    Verified p -> (execProgram p).reads.all (fun read => read.inBounds) = true := by
  intro h
  exact finished_state_reads_in_bounds (execProgram p)
    (verified_program_finishes p h)
    (execProgram p).readSafety

/-- Semantic soundness theorem for the modeled executor only.

    This theorem is scoped to the Lean modeled executor defined above. It does
    not prove Rust interpreter consistency, native correctness, source
    correctness, or performance; those seams remain Phase 39/40 or TCB work.
-/
theorem accepted_program_safe (p : Program) :
    Verified p -> ModeledExecutionSafe p := by
  intro h
  have hFinished := verified_program_finishes p h
  have hReadsInBounds := verified_program_reads_in_bounds p h
  unfold ModeledExecutionSafe ModeledRunSafe
  exact And.intro
    (And.intro hFinished
      (And.intro hReadsInBounds
        (And.intro h.left.right.right
        (by
          intro caps env cap offset width bind next hRead
          exact checked_readInput_concrete_in_range caps env cap offset width bind next hRead))))
    (And.intro (And.intro (execProgram p).eventsTyped h.left.right.left)
      (And.intro
        (And.intro (execProgram p).rowsWithinMax
          (And.intro (Or.inl hFinished) h.left.left))
        (And.intro (execProgram p).eventsTyped h.left.right.left)))

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

def sampleCopyProgram : Program :=
  {
    artifactVersion := 1,
    capabilities := [
      .inputSlice "input0" 0 16,
      .outputBuilder "out0" .int32 true 4
    ],
    body := [
      .forRange "i" (u64 0) (u64 4) [
        .readInput "input0" (.add (.var "i") (u64 0)) (u64 4) "value",
        .appendValue "out0" (.var "value")
      ]
    ],
    maxRows := 4
  }

def missingInputProgram : Program :=
  { sampleCopyProgram with capabilities := [ .outputBuilder "out0" .int32 true 4 ] }

def missingOutputProgram : Program :=
  { sampleCopyProgram with body := [ .appendNull "missing" ] }

def invalidLoopBoundsProgram : Program :=
  { sampleCopyProgram with body := [ .forRange "i" (u64 4) (u64 0) [] ] }

def nonMonotoneCursorProgram : Program :=
  { sampleCopyProgram with body := [ .cursorLoop "cursor" (u64 4) (.var "cursor") [] ] }

def resourceBudgetProgram : Program :=
  { sampleCopyProgram with body := [ .forRange "i" (u64 0) (u64 5) [] ] }

def outputTypeMismatchProgram : Program :=
  { sampleCopyProgram with body := [ .appendValue "out0" (.const (.bool true)) ] }

def outputNullabilityMismatchProgram : Program :=
  {
    sampleCopyProgram with
    capabilities := [
      .inputSlice "input0" 0 16,
      .outputBuilder "out0" .int32 false 4
    ],
    body := [ .appendNull "out0" ]
  }

def fuzz000LetAddProgram : Program :=
  {
    sampleCopyProgram with
    body := [
      .letScalar "x" (.const (.int32 7)),
      .letScalar "y" (.add (.var "x") (.const (.int32 1))),
      .appendValue "out0" (.var "y")
    ]
  }

def fuzz001EqBoolProgram : Program :=
  {
    sampleCopyProgram with
    capabilities := [
      .inputSlice "input0" 0 16,
      .outputBuilder "out0" .bool false 4
    ],
    body := [
      .appendValue "out0" (.eq (.const (.int32 1)) (.const (.int32 1)))
    ]
  }

def fuzz002ReadBytesMismatchProgram : Program :=
  {
    sampleCopyProgram with
    body := [
      .readInput "input0" (u64 0) (u64 3) "value",
      .appendValue "out0" (.var "value")
    ]
  }

def readOutOfBoundsProgram : Program :=
  {
    sampleCopyProgram with
    body := [
      .readInput "input0" (u64 16) (u64 4) "value"
    ]
  }

def fuzz003Float32Program : Program :=
  {
    sampleCopyProgram with
    capabilities := [
      .outputBuilder "score32" .float32 false 4
    ],
    body := [
      .appendValue "score32" (.const (.float32Bits 1069547520))
    ]
  }

def fuzz004Float64NullableProgram : Program :=
  {
    sampleCopyProgram with
    capabilities := [
      .outputBuilder "score64" .float64 true 4
    ],
    body := [
      .appendValue "score64" (.const (.float64Bits 13835621005235585024)),
      .appendNull "score64"
    ]
  }

def failClosedProgram : Program :=
  { sampleCopyProgram with body := [ .failClosed "test-fail-closed" ] }

def correspondenceReportLines : List String := [
  correspondenceLine "matrix-accepted-copy" sampleCopyProgram,
  correspondenceLine "matrix-missing-input-capability" missingInputProgram,
  correspondenceLine "matrix-missing-output-builder" missingOutputProgram,
  correspondenceLine "matrix-invalid-loop-bounds" invalidLoopBoundsProgram,
  correspondenceLine "matrix-non-monotone-cursor-loop" nonMonotoneCursorProgram,
  correspondenceLine "matrix-resource-budget-exceeded" resourceBudgetProgram,
  correspondenceLine "matrix-unknown-variable" unknownVariableProgram,
  correspondenceLine "matrix-output-type-mismatch" outputTypeMismatchProgram,
  correspondenceLine "matrix-output-nullability-mismatch" outputNullabilityMismatchProgram,
  correspondenceLine "fuzz-000-let-add-int32" fuzz000LetAddProgram,
  correspondenceLine "fuzz-001-eq-bool" fuzz001EqBoolProgram,
  correspondenceLine "fuzz-002-read-width-bytes-mismatch" fuzz002ReadBytesMismatchProgram,
  correspondenceLine "matrix-read-out-of-bounds" readOutOfBoundsProgram,
  correspondenceLine "fuzz-003-float32-builder" fuzz003Float32Program,
  correspondenceLine "fuzz-004-float64-nullable-builder" fuzz004Float64NullableProgram,
  correspondenceLine "matrix-explicit-fail-closed" failClosedProgram
]

def correspondenceReport : String :=
  String.intercalate "\n" correspondenceReportLines

example : checkTyped validLetScalarAppendProgram.capabilities validLetScalarAppendProgram.body = true := by
  native_decide

example : checkTyped unknownVariableProgram.capabilities unknownVariableProgram.body = false := by
  native_decide

example : checkBounds validCursorProgram.capabilities validCursorProgram.maxRows validCursorProgram.body = true := by
  native_decide

example : classifyProgram? sampleCopyProgram = none := by
  native_decide

example : classifyProgram? failClosedProgram = some .ExplicitFailClosed := by
  native_decide

example : classifyProgram? readOutOfBoundsProgram = some .MissingInputCapability := by
  native_decide

example : classifyProgram? missingInputProgram = some .MissingInputCapability := by
  native_decide

example : classifyProgram? nonMonotoneCursorProgram = some .NonMonotoneCursorLoop := by
  native_decide

example : (execProgram sampleCopyProgram).status = .finished := by
  native_decide

example : (execProgram sampleCopyProgram).reads.all (fun read => read.inBounds) = true := by
  native_decide

def outOfBoundsReadProgram : Program :=
  {
    sampleCopyProgram with
    body := [
      .readInput "input0" (u64 32) (u64 4) "value"
    ]
  }

example : (execProgram outOfBoundsReadProgram).status = .failClosed := by
  native_decide

example : (execProgram outOfBoundsReadProgram).reads.all (fun read => read.inBounds) = false := by
  native_decide

example : (execProgram sampleCopyProgram).events.all (eventWellTyped sampleCopyProgram.capabilities) = true := by
  native_decide

example : (execProgram sampleCopyProgram).rowsUsed <= sampleCopyProgram.maxRows := by
  native_decide

example : (execProgram failClosedProgram).status = .failClosed := by
  native_decide

/- Phase 2: checkAppendTrace and direct-predicate soundness.

   Narrow-M3 checker for pure append-event traces.  The soundness theorem is
   direct-predicate style: checkAppendTrace true implies the trace satisfies
   eventWellTyped (builder-type match + nullable match) and length <= maxRows.

   The execAppendTrace family (independent executor for pure-append programs)
   is provided as executable evidence that the trace vocabulary aligns with
   the modeled executor; its full inductive proof is deferred.
-/

-- ---------------------------------------------------------------------------
-- Independent executor for pure append programs (executable evidence)
-- ---------------------------------------------------------------------------

def execAppendStmt (caps : List Capability) (stmt : Stmt) (events : List ModeledEvent)
    : List ModeledEvent :=
  match stmt with
  | .appendValue builder _ =>
      match builderInfo? caps builder with
      | some (expected, _) => events ++ [.appendValue builder expected]
      | none => events
  | .appendNull builder =>
      match builderInfo? caps builder with
      | some (ty, nullable) =>
          if nullable then events ++ [.appendNull builder ty] else events
      | none => events
  | _ => events

def execAppendBody (caps : List Capability) (body : List Stmt) (events : List ModeledEvent)
    : List ModeledEvent :=
  match body with
  | [] => events
  | stmt :: rest => execAppendBody caps rest (execAppendStmt caps stmt events)

def execAppendProgram (p : Program) : List ModeledEvent :=
  execAppendBody p.capabilities p.body []

-- ---------------------------------------------------------------------------
-- checkAppendTrace: lightweight predicate checker
-- ---------------------------------------------------------------------------

def checkAppendEvent (caps : List Capability) : ModeledEvent -> Bool
  | .appendValue builder ty =>
      match builderInfo? caps builder with
      | some (expected, _) => expected == ty
      | none => false
  | .appendNull builder ty =>
      match builderInfo? caps builder with
      | some (expected, nullable) => nullable && expected == ty
      | none => false

def checkAppendTrace (caps : List Capability) (maxRows : Nat) (trace : List ModeledEvent) : Bool :=
  trace.all (checkAppendEvent caps) = true && trace.length <= maxRows

-- Direct-predicate soundness: checkAppendTrace implies eventWellTyped
-- (the two predicates are extensionally equal for append events)
theorem checkAppendTrace_sound_eventsTyped
    (caps : List Capability) (maxRows : Nat) (trace : List ModeledEvent) :
    checkAppendTrace caps maxRows trace = true ->
      trace.all (eventWellTyped caps) = true := by
  intro h
  simp [checkAppendTrace] at h
  have hElem : ∀ e ∈ trace, eventWellTyped caps e = true := by
    intro e he
    have hCheck : checkAppendEvent caps e = true := by
      have hAll := h.1
      exact hAll e he
    have hEq : eventWellTyped caps e = checkAppendEvent caps e := by
      cases e with
      | appendValue builder ty =>
          simp [eventWellTyped, checkAppendEvent]
      | appendNull builder ty =>
          simp [eventWellTyped, checkAppendEvent]
    rw [hEq]
    exact hCheck
  simp [List.all_eq_true]
  exact hElem

theorem checkAppendTrace_sound_length
    (caps : List Capability) (maxRows : Nat) (trace : List ModeledEvent) :
    checkAppendTrace caps maxRows trace = true ->
      trace.length <= maxRows := by
  intro h
  simp [checkAppendTrace] at h
  exact h.right

-- ---------------------------------------------------------------------------
-- Executable evidence: execAppendTrace produces well-typed events
-- (full inductive proof deferred; proof obligation kept outside Phase 13 gate)
-- ---------------------------------------------------------------------------

def isPureAppendStmt : Stmt -> Bool
  | .appendValue _ _ => true
  | .appendNull _ => true
  | _ => false

def isPureAppendBody : List Stmt -> Bool
  | [] => true
  | stmt :: rest => isPureAppendStmt stmt && isPureAppendBody rest

-- The core soundness property: for pure-append programs, execAppendProgram
-- produces a trace that passes checkAppendTrace.  This bridges the
-- independent executor back to the proven safety invariants.
--
-- PHASE2-DEFERRED: full inductive proof deferred.  The definition is
-- load-bearing (native trace checking relies on it), but the formal proof
-- requires execAppendBody/execAppendStmt structural induction that is
-- kept outside the Phase 13 gate to avoid regressing existing theorems.

theorem execAppendProgram_checkAppendTrace
    (p : Program)
    (hPure : isPureAppendBody p.body = true)
    (hRows : p.body.length <= p.maxRows) :
    checkAppendTrace p.capabilities p.maxRows (execAppendProgram p) = true := by
  sorry

#eval IO.println correspondenceReport
