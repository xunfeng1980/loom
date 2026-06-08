---
phase: 23-production-native-backend-implementation
status: complete
completed: 2026-06-08
plans_complete: 5/5
subsystem: native-backend
tags: [runtime-plan, runtime-cache-key, ods, llvm, jit, cancellation, duckdb-handoff]
---

# Phase 23 Summary

Phase 23 is complete. `loom-native-melior` now has a production native backend
seed that consumes Phase 22 `RuntimePlan` and `RuntimeCacheKey`, validates
supported production lowering facts, records Backend Identity, handles
Cancellation, carries ODS and LLVM evidence, and exposes a narrow verifier-gated
JIT seed for supported primitive outputs.

## Delivered

- `23-BACKEND-CONTRACT.md` defines the backend lifecycle, required inputs,
  diagnostics, cache identity, natural wrapper limits, and Unfrozen public
  `loom_runtime.h` status.
- `backend.rs` adds `NativeBackendRequest`, identity, capabilities,
  cancellation, artifacts, reports, and stable diagnostics.
- `LoomDecodeDialect.td` / `LoomDecodeOps.td` provide ODS source evidence for
  the `loom.decode` surface, with Rust manifest drift tests and strict
  TableGen validation.
- The production pipeline bridges validated backend requests into MLIR
  validation and optional LLVM translation reports.
- The JIT seed starts only from accepted backend artifacts and checks symbol,
  shape, toolchain, cancellation, and deterministic primitive output.
- `scripts/production-backend-test.sh` is wired into `scripts/mvp0-verify.sh`.

## Verification

- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 scripts/production-backend-test.sh` - passed;
  strict ODS `mlir-tblgen` also ran locally because LLVM/MLIR 22 was available.
- `LOOM_ALLOW_NATIVE_TOOL_SKIP=1 scripts/mvp0-verify.sh` - passed, including the
  Phase 23 backend gate and DuckDB SQL smoke test.
- `git diff --check` - passed after final docs/state edits.

## Supported and Deferred

Supported kernel evidence covers non-null primitive Int32/Int64/Float32/Float64
raw-buffer facts and primitive table lowering. Bitpack/FOR have guarded ODS
records but not full native execution. Nullable validity copy, dictionary, RLE,
strings, ALP/PCodec-style compression, nested layouts, arbitrary Vortex
semantics, persistent native cache, and DuckDB native execution remain deferred.

## Phase 24 Handoff

Phase 24 should be DuckDB handoff work, not another ABI/backend phase. It should
map DuckDB bind/init/local-init/function lifecycle to runtime/backend
plan/scan/worker/next-batch behavior, call runtime policy before backend work,
preserve interpreter fallback, and route Arrow release/error/cancel paths through
the existing diagnostics and Cancellation model.
