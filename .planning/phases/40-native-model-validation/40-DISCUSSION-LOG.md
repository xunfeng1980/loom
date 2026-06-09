# Phase 40 Discussion Log

**Date:** 2026-06-09
**Mode:** Autonomous, using previously approved recommended defaults

> Decisions are captured in CONTEXT.md - this log preserves alternatives
> considered.

## Topics Discussed

**Areas discussed:** model coverage, float tags, native/model trace comparison,
fail-closed routing, TCB language

### Q1: What must be covered?

| Option | Description | Selected |
|---|---|---|
| Bool/int subset only | Validate the part already expressible in Phase 39 reference traces. | |
| Full Phase 35 supported matrix | Include nullable Boolean, Int32, Int64, Float32, Float64. | yes |
| Expand Arrow coverage | Add Utf8/logical/nested while validating. | |

**Decision:** Cover the full Phase 35 supported primitive matrix, but do not
expand native coverage.

### Q2: How to handle Float32/Float64 in the model?

| Option | Description | Selected |
|---|---|---|
| Skip floats | Treat floats as still Phase 35-only evidence. | |
| Add scalar tags only | Add Float32/Float64 bit-pattern constants and trace names. | yes |
| Add float arithmetic | Model float operations and semantics. | |

**Decision:** Add scalar tags only. Phase 40 needs builder-event type evidence,
not float arithmetic.

### Q3: What is positive native/model evidence?

| Option | Description | Selected |
|---|---|---|
| RecordBatch equality | Keep Phase 35 value equivalence as sufficient. | |
| Reference trace comparison | Compare native output event trace to reference executor trace. | yes |
| Host SQL success | Treat DuckDB query success as native correctness. | |

**Decision:** Use exact reference trace comparison. RecordBatch equality remains
supporting evidence only.

### Q4: What happens on divergence?

| Option | Description | Selected |
|---|---|---|
| Emit diagnostics but keep native route | Observability only. | |
| Fail closed and block cache/native route | Disable native route for divergent shape. | yes |
| Silent fallback | Hide divergence behind interpreter fallback. | |

**Decision:** Divergence fails closed and must be covered with an injected
mismatch test.

### Q5: How to describe MLIR/LLVM?

| Option | Description | Selected |
|---|---|---|
| Verified compiler claim | Treat toolchain output as proven correct. | |
| Permanent TCB + per-run validation | Trust toolchain, validate per-run output. | yes |
| Omit the claim boundary | Avoid TCB language. | |

**Decision:** MLIR/LLVM/native lowering remains a permanent TCB assumption.
Phase 40 is translation validation only.
