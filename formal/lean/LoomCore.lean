/-
Phase 13 Lean scaffold for the tiny Loom L2Core verifier slice.

This file is a mechanized scaffold, not a complete final Loom soundness proof.
It names the core language objects and theorem targets that the Rust verifier,
SMT obligations, and future proof work must align with.

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

