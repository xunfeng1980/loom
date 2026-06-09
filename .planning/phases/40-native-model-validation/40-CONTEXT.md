# Phase 40: Native Model Validation - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous discuss; defaults selected from Phase 35 native Arrow
semantic evidence, Phase 38 soundness remediation, and Phase 39 reference
executor closeout

<domain>
## Phase Boundary

Phase 40 owns the `native<->model` seam from the verified-lineage contract.
Phase 35 shipped engine-neutral native Arrow semantic execution for accepted
one-batch nullable fixed-width primitive `LMC2(LMA1)` artifacts, plus explicit
direct `LMA1` bridge/regression inputs. Phase 39 shipped a Rust transcription
of the Lean modeled executor as a separate differential oracle with stable
builder-event/fail-closed traces.

This phase must connect those two evidence lines. Native Arrow semantic
execution should be accepted as a native route only when its output can be
validated against a model/reference builder-event trace. This is per-run
translation validation, not verified compilation and not source-data
correctness.
</domain>

<decisions>
## Implementation Decisions

### Model Coverage

- **D-40-01:** Cover every shape Phase 35 currently claims as supported:
  one-record-batch nullable Boolean, Int32, Int64, Float32, and Float64 columns
  in default `LMC2(LMA1)`, plus explicit direct `LMA1` bridge evidence.
- **D-40-02:** Extend the L2Core/reference scalar tag vocabulary only as needed
  for Phase 35 coverage: add Float32/Float64 bit-pattern constants and scalar
  type names. Do not add float arithmetic, new encodings, or new source formats.
- **D-40-03:** Derive a model/reference trace from the supported Arrow semantic
  batch by constructing a narrow L2Core append/null program and running
  `l2_core_reference_executor::execute_reference`. This keeps the reference
  executor, not native output equality alone, as the model side of the check.

### Native/Model Trace Check

- **D-40-04:** Native output must be converted into the same stable
  builder-event trace vocabulary and compared exactly with the reference trace.
  Final `RecordBatch` equality can remain supporting evidence, but it is not
  sufficient positive Phase 40 evidence.
- **D-40-05:** Include injected divergence coverage where the native output
  trace differs from the model trace and produces `native-model-trace-mismatch`
  diagnostics.
- **D-40-06:** Unsupported accepted Arrow semantic shapes remain fail-closed or
  explicit interpreter-fallback policy cases; they must not seed native/model
  cache keys or positive validation reports.

### Runtime And TCB

- **D-40-07:** Add a validation-aware runtime/cache path. Native route
  candidacy and cache identity should require a successful native/model
  validation report, not merely `execution.is_supported()`.
- **D-40-08:** Record MLIR/LLVM/native lowering as a permanent TCB assumption.
  Phase 40 is translation validation of observed per-run output; it is not a
  compiler correctness proof.
- **D-40-09:** Keep evidence engine-neutral. DuckDB can consume the validated
  native route later, but DuckDB query success is not the correctness source in
  this phase.

### Non-Claims

- **D-40-10:** Do not claim verified compilation, arbitrary Arrow-shape native
  support, source-data correctness, performance, host ABI correctness, or
  DuckDB-native integration.

### the agent's Discretion

- Choose exact report/API names, provided "native model validation" is visible
  in code and gate markers.
- Choose stable trace line format, provided reference and native sides are
  column/row/type aware and deterministic.
- Choose whether validation helpers live in `native_arrow_semantic.rs` or an
  adjacent module. Prefer the existing module unless it becomes unwieldy.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before implementation.**

- `.planning/ROADMAP.md` - Phase 40 scope, success criteria, non-goals.
- `.planning/REQUIREMENTS.md` - LINEAGE-09 and LINEAGE-10.
- `.planning/phases/35-native-arrow-semantic-execution/35-CONTEXT.md` -
  supported native Arrow semantic matrix.
- `.planning/phases/35-native-arrow-semantic-execution/35-NATIVE-ARROW-SEMANTIC-REPORT.md` -
  Phase 35 positive/fail-closed evidence.
- `.planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md` -
  evidence-layer and TCB language.
- `.planning/phases/38-lean-stage-c-operational-semantics-and-soundness-theorem/38-02-SUMMARY.md` -
  remediated modeled soundness theorem scope.
- `.planning/phases/39-model-rust-interpreter-consistency/39-02-SUMMARY.md` -
  reference executor and trace-consistency handoff.
- `formal/lean/LoomCore.lean` - modeled scalar/event vocabulary.
- `crates/loom-core/src/l2_core.rs` - Rust L2Core scalar type/value model.
- `crates/loom-core/src/l2_core_reference_executor.rs` - reference trace oracle.
- `crates/loom-core/src/native_arrow_semantic.rs` - native Arrow semantic
  execution/runtime/cache APIs.
- `crates/loom-core/tests/native_arrow_semantic.rs` - Phase 35 supported and
  unsupported native Arrow semantic tests.
- `scripts/full-verifier-test.sh` - verified-lineage broad gate.
</canonical_refs>

<code_context>
## Existing Code Insights

- `native_arrow_semantic` supports Boolean, Int32, Int64, Float32, and Float64,
  but its current equivalence report compares native output to decoded Arrow
  reference values only.
- `l2_core_reference_executor` currently emits stable append/null traces for
  Boolean, Int32, Int64, Bytes, and row-index builder types. Float32/Float64 are
  absent, so Phase 40 must extend the model tag vocabulary before claiming full
  Phase 35 coverage.
- `formal/lean/LoomCore.lean` has just been remediated so modeled safety
  predicates consume `execProgram p` state evidence and `Verified p` premises.
  Any new float tags must preserve the no-`sorry` Lean gate and correspondence
  matrix.
- Runtime/cache identity currently keys off supported native execution. Phase
  40 should add validation-aware helpers while leaving Phase 35 helpers in place
  for backward tests.
</code_context>

<specifics>
## Plan Split

| Plan | Scope | Acceptance Focus |
|---|---|---|
| 40-01 | Native/model trace check | Extend float scalar tags, build reference trace from Arrow semantic batches, compare native output trace exactly, include injected divergence |
| 40-02 | Fail-closed routing + TCB record | Validation-aware runtime/cache helpers, focused and broad gates, LINEAGE-09/10 closeout, explicit permanent TCB/non-claim docs |

Recommended focused gate: `scripts/native-model-validation-test.sh`.
</specifics>

<deferred>
## Deferred Ideas

- Verified compilation of MLIR/LLVM remains permanently out of scope and in the
  TCB.
- General Arrow semantic to L2Core compilation is not required; this phase only
  needs a narrow supported-batch trace bridge.
- DuckDB consumption of validated native/model evidence remains later work.
- Utf8, logical, nested, multi-batch, and additional format coverage remain
  Phase 42 or later.
</deferred>

---

*Phase: 40-Native Model Validation*
*Context gathered: 2026-06-09*
