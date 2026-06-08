# Phase 23 Backend Report

## Status

Phase 23 is complete as a production native backend seed inside
`loom-native-melior`. The backend consumes the Phase 22 `RuntimePlan` and
`RuntimeCacheKey`, validates the Phase 20/21 lowering facts, records backend and
toolchain identity, carries cancellation, and produces verifier-gated
MLIR/LLVM/JIT reports for the supported primitive slice.

The public `loom_runtime.h` C ABI sketch remains **Unfrozen**. Phase 23 provides
natural Rust backend APIs and stable diagnostics for Phase 24, not a finalized
host ABI.

## Implemented Surface

- Runtime/backend bridge: `validate_backend_request` accepts only native
  `RuntimePlan` decisions with no runtime diagnostics, a present
  `RuntimeCacheKey`, supported `ProductionLoweringFacts`, backend identity, and
  non-cancelled state.
- Backend Identity: reports carry Loom runtime ABI version, backend version,
  expected/detected LLVM/MLIR version, toolchain compatibility, pipeline ids,
  target/layout when available, capabilities, and supported native kernel names.
- ODS evidence: `LoomDecodeDialect.td` and `LoomDecodeOps.td` describe the
  `loom.decode` operation surface, with Rust manifest drift tests and strict
  `mlir-tblgen` validation when the managed LLVM/MLIR toolchain is available.
- LLVM pipeline: validated backend requests flow into production MLIR validation
  and optional LLVM translation, with stable skipped/fail-closed diagnostics.
- JIT seed: accepted backend artifacts can enter
  `execute_prepared_production_jit`; the seed checks artifact status, symbol,
  cancellation, supported shape, toolchain availability, and deterministic
  primitive reference-output equivalence.
- Cancellation: backend preflight and JIT preparation return explicit
  cancellation diagnostics and produce no accepted executable artifact.

## Supported Kernel Paths

Supported today means verifier-gated backend preparation evidence plus focused
tests, not broad semantic compatibility:

- Non-null primitive raw-buffer table/single-column facts for Int32, Int64,
  Float32, and Float64.
- Multi-column primitive table lowering through the Phase 20 production lowering
  facts and backend pipeline.
- `loom.decode` structural ops and raw primitive builder/copy surface declared
  in ODS and checked against the textual dialect manifest.
- Deterministic primitive zero-buffer JIT seed output matching the current
  production MLIR semantics.

Declared but still guarded/deferred:

- Bitpack/FOR operation records exist as guarded dialect evidence, but full
  native execution remains deferred until stronger verified parameter extraction
  and lowering are implemented.
- Nullable validity-copy native execution is not supported yet.
- Dictionary, RLE, string, ALP/PCodec-style compression, nested layouts,
  arbitrary Vortex storage modes, and full Vortex semantic compatibility remain
  interpreter-only, fail-closed, or future compatibility work.

## Diagnostics and Cache Safety

Stable backend diagnostic codes distinguish runtime rejection, missing cache key,
missing lowering facts, unsupported facts, cancellation, skipped/missing/mismatched
toolchain, invalid backend artifact, missing JIT symbol, and native-output
mismatch.

`RuntimeCacheKey` remains the canonical cache input. Backend reports preserve the
consumed cache key and add cache-relevant Backend Identity fields: ABI version,
LLVM/MLIR identity, target/layout, pass pipeline id, artifact summary, and
capabilities. Phase 25 should derive persistent cache reuse and invalidation from
this report rather than inventing a separate identity model.

## Verification

The final Phase 23 gates passed on 2026-06-08:

- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 scripts/production-backend-test.sh` - passed.
  Local LLVM/MLIR 22 tooling was available, so the script also ran strict ODS
  `mlir-tblgen` validation.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 scripts/mvp0-verify.sh` - passed. This included
  workspace tests, phases 12-23 focused gates, the Phase 23 production backend
  gate, and the DuckDB SQL smoke test.

Focused checks from the implementation waves also passed:

- `cargo test -p loom-native-melior --test production_backend_contract`
- `cargo test -p loom-native-melior --test decode_dialect_manifest`
- `cargo test -p loom-native-melior --test production_backend_pipeline`
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 cargo test -p loom-native-melior --test production_backend_jit`
- `cargo test -p loom-core --test runtime_execution_policy`
- `cargo test -p loom-core --test production_native_lowering`
- `cargo test -p loom-native-melior pipeline`
- `cargo test -p loom-native-melior jit`

## DuckDB Handoff

Phase 24 should treat DuckDB as a natural adapter over the Phase 22
runtime/backend contract:

- DuckDB bind maps file/artifact metadata, projection, and table schema into
  runtime planning inputs.
- DuckDB global/local init maps scan state and worker ownership into split and
  concurrency planning.
- DuckDB function/next-batch execution calls runtime policy first, then the
  Phase 23 backend only for accepted native candidates.
- Interpreter fallback remains policy-controlled; unsupported native facts do
  not become host exceptions unless the runtime policy says fail closed.
- Arrow C Data release, DuckDB error propagation, and host cancellation should
  map to backend diagnostics and Cancellation without changing the public
  `loom_runtime.h` sketch.

## Non-Goals

Phase 23 did not freeze the public C ABI, implement DuckDB native execution,
ship a persistent native artifact cache, claim full ExecutionEngine production
codegen, prove compiler correctness, add Iceberg/StarRocks integration, or
support arbitrary Vortex semantics.
