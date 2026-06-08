# Phase 13 L2Core Verifier Specification

**Status:** Normative target for Phase 13 execution
**Scope:** Tiny `L2Core` verifier slice for future Loom distribution IR
**Depends on:** Phase 12 safety proof MVP

## Scope

Phase 13 defines the verifier foundation for a deliberately small `L2Core`
language slice. The verifier target is safety and well-formed output
construction, not semantic correctness of arbitrary producer intent.

An accepted `L2Core` artifact must satisfy this safety theorem target:

```text
For every finite artifact accepted by the Phase 13 verifier, execution either
fails closed through a typed error or produces only typed Arrow builder events
within declared input, scratch, output, and resource bounds. Accepted artifacts
have no ambient authority, cannot perform out-of-range reads, cannot rely on
unchecked arithmetic overflow, and cannot enter an unbounded loop.
```

The verification stack is layered:

- `Rust abstract interpretation` is the executable verifier boundary used by
  Loom tooling.
- `SMT` discharges local arithmetic, range, overflow, loop-variant, and
  resource-bound obligations through an internal constraint IR.
- `Lean/Rocq` defines the core syntax, static semantics, dynamic semantics, and
  accepted-program soundness theorem scaffold.
- `TLA+` models artifact lifecycle and pipeline invariants such as
  parse-before-verify and verify-before-lower.

Phase 13 does not implement MLIR/native lowering or real Vortex ingress. Those
remain later roadmap phases. Phase 13 emits verifier facts and obligations that
Phase 14 can use as native-lowering preconditions.

## L2Core Syntax

`L2Core` artifacts are finite modules with an artifact header, feature list,
input capabilities, output builders, resource budget, and a bounded statement
body.

```text
Artifact :=
  version
  required_features
  optional_features
  inputs
  outputs
  resource_budget
  body
```

Types:

- `Bool`
- `Int32`, `Int64`
- `UInt32`, `UInt64`
- `Bytes`
- `RowIndex`
- `ArrowScalar(type)`
- `ArrowBuilder(type)`

Capabilities:

- `InputSlice { id, offset, length }`
- `Scratch { id, max_bytes }`
- `OutputBuilder { id, arrow_type, max_events }`

Statements:

- `ForRange { index, start, end, body }`
- `CursorLoop { cursor, limit, progress, body }`
- `ReadInput { capability, offset, width, bind }`
- `LetScalar { name, expr }`
- `AppendValue { builder, value }`
- `AppendNull { builder }`
- `FailClosed { code }`

Expressions:

- constants and variables,
- checked integer `add`, `sub`, and `mul`,
- comparisons,
- `min` and `max`,
- offset and length expressions over `RowIndex` and bounded integer types.

Allowed loops are only finite `for i in 0..N`-style loops and monotone cursor
loops with an explicit progress expression. Recursive functions, unbounded
while loops, dynamic code loading, and host callbacks are not in `L2Core`.

## Static Semantics

The static semantics validates the artifact before execution:

- Every variable is defined before use and has one type.
- Every input read names an explicit `InputSlice` capability.
- Every input read proves `offset + width <= capability.length` without
  unchecked overflow.
- Every builder append names an explicit `OutputBuilder` capability.
- `AppendValue` values match the builder Arrow type.
- `AppendNull` is allowed only for nullable output positions represented by
  the builder type contract.
- Every loop has a finite bound or a monotone progress proof obligation.
- Every arithmetic expression is either statically bounded or represented as
  an SMT-ready checked arithmetic obligation.
- Statements have no ambient effects. They may read declared input slices,
  update declared scratch state, append to declared output builders, or fail
  closed.

The type-and-effect judgment is:

```text
Gamma; Caps; Budget |- stmt : Effects, Obligations
```

where `Effects` is limited to declared input reads, scratch writes, output
builder appends, and fail-closed exits.

## Dynamic Semantics

Dynamic semantics are defined over finite execution states:

```text
State :=
  env
  input_capabilities
  scratch_state
  builder_states
  resource_remaining
  emitted_facts
```

Execution steps either:

- evaluate a bounded expression,
- perform a capability-checked input read,
- append a typed Arrow builder event,
- decrease a loop or resource measure,
- enter a typed fail-closed state, or
- finish with a verified artifact fact set.

No dynamic step can access filesystem, network, clocks, environment variables,
foreign functions, or process-global state. Runtime failure is a verifier-owned
`FailClosed` outcome, not a panic or undefined behavior.

The Lean/Rocq scaffold should model this semantics small enough to state:

```text
accepted(program) and executes(program, input) = ok(output)
  implies safe_output(output) and within_bounds(program, input, output)
```

## Capability Model

`L2Core` uses explicit capabilities rather than ambient authority.

Allowed authority:

- read bytes from declared finite `InputSlice` ranges,
- write bounded scratch slots declared in the artifact,
- emit events to declared typed Arrow builders,
- return typed verifier or runtime failure codes.

Forbidden authority:

- filesystem,
- network,
- remote fetch,
- signatures or attestation checks,
- native code execution,
- dynamic library loading,
- host callbacks,
- process-global state,
- unbounded allocation.

Unknown required features fail closed during artifact negotiation. Optional
features may be ignored only if their absence cannot change safety obligations.

## Resource Model

Each artifact declares a finite resource budget:

```text
ResourceBudget :=
  max_steps
  max_input_bytes_read
  max_scratch_bytes
  max_builder_events
  max_rows
  max_constraint_count
```

The verifier computes conservative resource bounds by abstract interpretation:

- `ForRange` contributes `end - start` iterations when the bound is finite.
- `CursorLoop` requires a monotone progress obligation and a finite `limit`.
- Each input read contributes bounded bytes.
- Each builder append contributes one builder event.
- Scratch writes must fit declared scratch bounds.

Resource obligations that are not resolved syntactically become SMT-ready
constraints. If an obligation cannot be represented or discharged, the artifact
is rejected.

## Arrow Builder Event Semantics

Arrow output is represented as a finite event stream per builder:

```text
BuilderEvent :=
  AppendValue(builder_id, arrow_type, value)
  AppendNull(builder_id, arrow_type)
  Finish(builder_id)
```

Well-formed builder event streams satisfy:

- each event targets a declared builder,
- each `AppendValue` value has the builder Arrow type,
- null events are valid for the builder's declared nullability contract,
- event count does not exceed `max_builder_events`,
- all builders are finished at most once,
- final arrays have lengths consistent with artifact row-count facts,
- no raw Arrow buffer writes are expressible in `L2Core`.

This preserves the existing Loom invariant that decoded values are emitted
through typed Arrow builders rather than raw memory writes.

## VerifiedArtifactFacts

The Rust verifier emits `VerifiedArtifactFacts` as the stable handoff between
verification and later lowering:

```text
VerifiedArtifactFacts :=
  artifact_version
  required_features
  optional_features
  accepted_feature_set
  input_ranges
  output_schema
  row_count_bound
  loop_bounds
  resource_bounds
  builder_event_types
  capability_summary
  constraint_ids
  proof_obligation_ids
```

Facts are evidence, not trust by themselves. A later phase may consume them only
with the gate invariant that the corresponding artifact was accepted by the
Phase 13 verifier and the facts are tied to the accepted artifact version and
feature set.

## Lowering Preconditions

Phase 14 native-lowering work may begin only after these verifier preconditions
exist:

- the artifact has a successful verifier report,
- all unknown required features have failed closed,
- all input reads are tied to finite input capabilities,
- all loops have finite bounds or discharged monotone-progress obligations,
- all checked arithmetic/range/resource constraints have been represented,
- all output writes are typed Arrow builder events,
- `VerifiedArtifactFacts` include output schema, resource bounds, builder event
  types, and proof-obligation IDs,
- the TLA+ lifecycle model preserves the invariant that lowering cannot occur
  before verifier acceptance.

Phase 13 does not lower to MLIR, native code, SIMD kernels, or an external
runtime. It only defines the evidence those later paths must require.

## Explicit Exclusions

Phase 13 excludes:

- MLIR/native lowering implementation,
- MLIR/native lowering correctness proof,
- real Vortex file/container ingress,
- signatures, attestation, content-addressed remote lookup, encryption, and
  remote fetch,
- semantic correctness of arbitrary producer intent,
- a full future Loom language with recursion, dynamic dispatch, user-defined
  unbounded loops, or host callbacks,
- geometric-algebra-based verifier design,
- replacing the Rust executable verifier with a single formalism.

These exclusions keep Phase 13 focused on the full-verifier foundation: a tiny
`L2Core` language, executable Rust verifier path, SMT-ready obligations,
Lean/Rocq soundness scaffold, TLA+ lifecycle invariants, and stable lowering
facts.
