# kloom Semantic Design Notes

## 1. Alignment with Lean `LoomCore.lean`

kloom is a **direct operational-semantics rendering** of the Lean model in K
rewriting rules.  Every K cell has a corresponding Lean `ModeledState` field:

| K Cell | Lean Field | Purpose |
|--------|-----------|---------|
| `<caps>` | `caps : List Capability` | Declared input columns and output builders |
| `<scalars>` | `scalars : Map String ScalarValue` | Variable environment (v0 empty) |
| `<events>` | `events : List ModeledEvent` | Builder-event trace (the oracle output) |
| `<reads>` | `reads : List ModeledRead` | Input read log (v0 empty) |
| `<rows>` | `rowsUsed : Nat` | Events emitted so far (budget counter) |
| `<maxRows>` | `maxRows : Nat` | Budget from `Program` structure |
| `<status>` | `status : ModeledStatus` | `running` / `finished` / `failClosed` |

## 2. Design Decisions

### 2.1 `maxRows` as event budget in v0

In Lean, `maxRows` bounds **loop iterations** (`forRange`, `cursorLoop`).  In
kloom v0 there are no loops, so `maxRows` is interpreted as the **builder-event
budget** for the pure-append slice.  This aligns with `checkAppendTrace`'s
`trace.length ≤ maxRows` check.

**Future (v1+)**: When loops are added, `maxRows` will revert to its Lean
semantics (loop-iteration bound), and a separate `<maxBuilderEvents>` cell will
be introduced for the event budget.

### 2.2 `input` and `builder` capabilities share the same `<caps>` map

In Lean, `Capability` is a single inductive type with `inputColumn` and
`outputBuilder` constructors.  kloom mirrors this: both declarations are stored
in `<caps>` under the same Id key.  `appendValue`/`appendNull` lookup only
matches `builder(_,_)` patterns, so an `input` declaration does not interfere
with builder lookup.

If a program declares the **same Id** as both `input` and `builder`, the last
declaration wins (standard Map overwrite).  This matches Lean's list-of-caps
semantics where later declarations shadow earlier ones.

### 2.3 Trace format alignment

kloom `TraceEvent` syntax is designed to be **byte-for-byte compatible** with
`TracedOutputBuilder` output:

```
K:   append-value : col0 : int32
Rust: append-value:col0:id:int32
```

The Rust format includes the field name (`:id:`) in the builder Id
(`col0:id`).  kloom uses the same Id token, so if the kloom program declares
`builder col0:id:int32`, the trace line is identical.

### 2.4 fail-closed semantics

Every divergence from the declared capability schema or budget produces
`<status>failClosed</status>` and **stops emitting events**.  This mirrors
Lean's `modeledFailClosed` behavior.

## 3. Known Limitations (v0)

| Feature | Lean | kloom v0 | Plan |
|---------|------|----------|------|
| `readInput` | ✅ | ❌ | v1 |
| `letScalar` | ✅ | ❌ | v1 |
| `forRange` | ✅ | ❌ | v2 |
| `cursorLoop` | ✅ | ❌ | v3 |
| Scalar expressions (arithmetic, comparisons) | ✅ | constants only | v4 |
| `failClosed` with user code | ✅ | implicit only | v5 |
| Float/bool scalar values | ✅ | integers only | v0.1 |

## 4. Trust Boundary

kloom is **outside the production TCB**:
- It does not run in the user-facing decode path.
- It does not touch `loom-core`, `loom-ffi`, or DuckDB extension code.
- It lives in `contrib/kloom/`, a clearly demarcated boundary.

The only trust assumption added by kloom is:
> **K rewriting rules faithfully encode the L2Core operational semantics intent.**

This assumption is **independent** of the Rust implementation assumption, giving
the differential gate its statistical value.
