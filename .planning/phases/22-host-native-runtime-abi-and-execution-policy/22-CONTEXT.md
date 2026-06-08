# Phase 22 Context: Host Native Runtime ABI and Execution Policy

## Locked Direction

Phase 22 defines the host-neutral runtime ABI and policy layer that later
engines call. It must not become DuckDB integration, StarRocks integration,
Iceberg binding, compiled dialect work, or production JIT implementation.

The runtime contract should consume:

- accepted `ArtifactVerificationReport` / `ArtifactVerificationFacts`
- Bitwuzla-backed `ConstraintDischargeStatus::Discharged` or `NotRequired`
- Phase 20 production lowering support facts
- Phase 21 reader support, emission disposition, and lowering disposition
- host projection, predicate, split, concurrency, fallback, and cache policy

## Source Inputs

- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RESEARCH.md`
- `.planning/research/ENGINE-INTEGRATION-SPLIT.md`
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-REPORT.md`
- `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-LOWERING-CONTRACT.md`
- `crates/loom-core/src/artifact_verifier.rs`
- `crates/loom-core/src/production_native_lowering.rs`
- `crates/loom-vortex-ingress/src/lib.rs`

## Design Constraints

- Runtime planning starts from verifier/facts reports, never from raw trusted
  host assertions.
- Native execution is a decision outcome, not the default path.
- Interpreter fallback must be explicit and policy-controlled.
- Projection, predicate, split, concurrency, and cache identity are part of the
  ABI contract, not host-engine details.
- Host-neutral model types must not expose DuckDB, StarRocks, Vortex, MLIR, LLVM,
  or Rust-specific ownership types.
- Engine independence is a design claim until Phase 27 validates the same
  contract through a second query surface.

## Non-Goals

- No DuckDB `DataChunk` or table-function implementation.
- No StarRocks connector or executor implementation.
- No Iceberg metadata binding.
- No compiled ODS dialect, production `melior` pass pipeline, LLVM lowering, or
  JIT execution.
- No arbitrary Vortex semantic compatibility.
- No new solver backend.

## Recommended Plan Split

1. Runtime ABI contract and lifecycle model.
2. Verified facts handoff and execution decision policy.
3. Projection, predicate, and split planning envelope.
4. Cache key, diagnostics, and ABI sketch.
5. Report, release gate, and Phase 23/24/26/27 handoff.
