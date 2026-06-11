# kloom Semantic Design Notes

## 1. Alignment with Lean `LoomCore.lean`

kloom is a **direct operational-semantics rendering** of the Lean model in K
rewriting rules.  Every K cell has a corresponding Lean `ModeledState` field:

| K Cell | Lean Field | Purpose |
|--------|-----------|---------|
| `<caps>` | `caps : List Capability` | Declared input columns and output builders |
| `<scalars>` | `scalars : List (String × L2Ty)` | Variable environment |
| `<events>` | `events : List ModeledEvent` | Builder-event trace (the oracle output) |
| `<reads>` | `reads : List ModeledRead` | Input read log |
| `<rows>` | `rowsUsed : Nat` | Events emitted so far (budget counter) |
| `<maxRows>` | `maxRows : Nat` | Budget from `Program` structure |
| `<status>` | `status : ExecutionStatus` | `running` / `finished` / `failClosed` |

## 2. Design Decisions

### 2.1 `maxRows` as event budget in v0, loop bound in v1+

In Lean, `maxRows` bounds **loop iterations** (`forRange`, `cursorLoop`).  In
kloom v0 (pure-append only) there were no loops, so `maxRows` was interpreted as
the **builder-event budget**.  kloom v4 reverts to the Lean semantics:
`maxRows` bounds loop iterations, while the event budget is implicit in the
builder rules' `requires R <Int MaxR` check.

### 2.2 `input` and `builder` capabilities share the same `<caps>` map

In Lean, `Capability` is a single inductive type with `inputSlice` and
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
K:    append-value : col0 : int32
Rust: append-value:col0:id:int32
```

The Rust format includes the field name (`:id:`) in the builder Id
(`col0:id`).  kloom uses the same Id token, so if the kloom program declares
`builder col0:id:int32`, the trace line is identical.

### 2.4 fail-closed semantics

Every divergence from the declared capability schema or budget produces
`<status>failClosed</status>` and **stops emitting events**.  This mirrors
Lean's `modeledFailClosed` behavior.

## 3. Coverage Matrix (v4)

| Feature | Lean | kloom v4 | Notes |
|---------|------|----------|-------|
| `appendValue` / `appendNull` | ✅ | ✅ | Builder existence + type + nullable checked |
| `readInput` | ✅ | ✅ | In-bounds / out-of-bounds / unknown-constants rules per type |
| `letScalar` | ✅ | ✅ | TypeOf-driven scalar environment update |
| `forRange` | ✅ | ✅ | Constant bounds, row budget pre-check, body iteration |
| `cursorLoop` | ✅ | ✅ | Monotone progress check (`cursor + N`), limit budget |
| `failClosed` | ✅ | ✅ | Explicit user-code fail-closed |
| Scalar expressions (add/sub/mul/eq/lt/le) | ✅ | ✅ | `EvalConst` for constants; `TypeOf` for type derivation |
| Float/bool scalar values | ✅ | ✅ | Bit-pattern integers for float; `true`/`false` for bool |
| `Min` / `Max` | ✅ | ✅ | `EvalConst` selects the bound; `TypeOf` propagates operand type |
| `Bytes` constants | ✅ | ✅ | `bytes` builder type + `bytesConst("<hex>")` literal; content irrelevant to trace |
| `UInt32` / `UInt64` / `RowIndex` | ✅ | ⚠️ | Syntax declared; rule coverage partial |

## 4. Trust Boundary

kloom is **outside the production TCB**:
- It does not run in the user-facing decode path.
- It does not touch `loom-core`, `loom-ffi`, or DuckDB extension code.
- It lives in `contrib/kloom/`, a clearly demarcated boundary.

The only trust assumption added by kloom is:
> **K rewriting rules faithfully encode the L2Core operational semantics intent.**

This assumption is **independent** of the Rust implementation assumption, giving
the differential gate its statistical value.
