# Engine-Integrated Native Execution Split Research

Date: 2026-06-08

## Goal

The former engine-integrated native execution placeholder mixed three different risks:

- a stable native runtime contract that host engines can call safely
- one concrete host-engine integration over complete-reader artifacts
- hard evidence that native execution, interpreter fallback, caching, and failure behavior remain equivalent and fail closed

Those are separate proof obligations. Keeping them separate avoids turning the first host integration into an unreviewable productization phase.

## Source Notes

- DuckDB extensions are the right first host surface because DuckDB explicitly supports dynamically loaded extensions and the project already owns a DuckDB table-function path. Source: https://duckdb.org/docs/stable/extensions/overview
- DuckDB's community extension workflow is C++ based, uses the extension template, SQL-based tests, and CI/CD tooling for supported platforms. That argues for a narrow DuckDB-native MVP before broader engine claims. Sources: https://duckdb.org/community_extensions/development and https://github.com/duckdb/extension-template
- Apache Arrow C Data Interface is a stable C ABI for columnar interchange, supports zero-copy sharing between independent runtimes, and has explicit release callbacks. That makes it the right memory/output boundary to preserve or deliberately evolve. Source: https://arrow.apache.org/docs/format/CDataInterface.html
- Vortex files are stable from version 0.36.0 and separate the small physical file envelope from layout-reader complexity. That supports keeping "complete reader" before native host execution. Source: https://docs.vortex.dev/specs/file-format
- MLIR lowering to LLVM IR and ExecutionEngine/ORC JIT add separate ABI and toolchain risk. Those risks belong behind the verifier/native artifact boundary, not directly inside the host engine phase. Sources: https://mlir.llvm.org/docs/TargetLLVMIR/ and https://llvm.org/docs/ORCv2.html

## Recommended Split

### Phase 22: Host Native Runtime ABI and Execution Policy

Define the engine-independent ABI and policy:

- native callable signatures and memory ownership
- artifact identity and verified-facts handoff
- cache keys and invalidation inputs
- fail-closed diagnostics and unsupported-program behavior
- interpreter fallback semantics
- Arrow/raw-buffer output contract for a host to consume

This phase should not integrate DuckDB, Iceberg, or StarRocks.

### Phase 23: DuckDB Native Execution Integration MVP

Wire the Phase 22 runtime into the existing DuckDB table-function path:

- native execution selected only for accepted verifier/native facts
- interpreter fallback where policy allows
- complete-reader artifact input, not synthetic-only fixtures
- SQL smoke tests that compare native and interpreter results
- deterministic diagnostics for unsupported programs

DuckDB stays first because Loom already has this host seam and release gate.

### Phase 24: Native Equivalence, Cache, and Fallback Hardening

Close the native execution story before table-format binding:

- interpreter/native/Vortex oracle equivalence matrix
- cache reuse and invalidation semantics
- negative coverage for unsupported programs, stale facts, and malformed artifacts
- release-gate integration
- performance smoke evidence without making speed the correctness criterion

## Roadmap Effect

Iceberg binding should move after this hardening, because Iceberg metadata should point at a credible execution/artifact contract rather than at an experimental host-specific native path.
