# Phase 20 Context: Production Decode Dialect and Native Kernel Expansion

**Gathered:** 2026-06-08  
**Status:** Ready for planning  
**Source:** Phase 20 research and roadmap ordering decision

## Phase Boundary

Phase 20 turns the Phase 14/16 native-lowering spikes into a production-shaped
lowering surface. It must remain a backend of the verifier: accepted structure is
not enough, and collected constraints are not enough. Production native lowering
may only consume accepted artifact reports whose constraints are discharged or
not required.

## Locked Decisions

- Phase 20 defines a `loom.decode` dialect contract and deterministic textual
  surface before host-engine integration.
- Production lowering requires `ArtifactVerificationReport::Accepted` plus
  `ConstraintDischargeStatus::Discharged` or `NotRequired`.
- `CollectedOnly`, failed, unknown, skipped, missing facts, unsupported features,
  and unsupported output shapes reject before MLIR/native artifact creation.
- Default workspace builds must remain free of mandatory MLIR/LLVM dependencies.
- LLVM/MLIR 22 validation is allowed as optional or strict gate evidence.
- Initial kernel expansion targets fixed-size primitive output and primitive
  multi-column table batches.

## Scope Fences

In scope:

- Production lowering gate and diagnostics.
- `loom.decode` dialect contract and textual/dialect-shaped output.
- Primitive Arrow/raw-buffer builder model and standard-MLIR lowering.
- Raw primitive copy across Int32, Int64, Float32, Float64.
- Optional bitpack/FOR primitive native lowering if existing facts make it safe.
- MLIR validation gate and Phase 20 closeout report.

Out of scope:

- Host runtime ABI and cache/fallback policy; defer to Phase 22.
- DuckDB native execution; defer to Phase 23.
- Native cache/equivalence hardening; defer to Phase 24.
- Expanded Vortex encoding coverage beyond the current accepted matrix; defer to
  Phase 21.
- New solver backend work or checked proof objects.

## Canonical References

- `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-RESEARCH.md`
- `.planning/phases/19-solver-backed-full-artifact-verifier/19-SOLVER-REPORT.md`
- `.planning/phases/18-complete-vortex-reader/18-SUMMARY.md`
- `.planning/phases/16-full-melior-llvm-jit-backend-integration/16-BACKEND-CONTRACT.md`
- `crates/loom-core/src/artifact_verifier.rs`
- `crates/loom-core/src/native_lowering.rs`
- `crates/loom-native-melior/src/builder.rs`
- `crates/loom-native-melior/src/pipeline.rs`

## Recommended Execution Order

1. Add the production lowering gate and discharged-facts contract.
2. Define the dialect contract and textual surface.
3. Add primitive Arrow/raw-buffer builder lowering.
4. Expand primitive native kernels and multi-column slices.
5. Wire validation, docs, and final release-gate evidence.
