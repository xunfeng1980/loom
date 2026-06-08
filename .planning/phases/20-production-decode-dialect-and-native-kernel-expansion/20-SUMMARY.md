# Phase 20 Summary

## Shipped

- Production native-lowering support gate over `ArtifactVerificationReport`.
- `loom.decode` dialect contract and deterministic textual surface.
- Primitive Arrow/raw-buffer builder plans and standard-MLIR text emission.
- Raw primitive native-kernel matrix for non-null Int32, Int64, Float32, and
  Float64 single-column/table slices.
- Explicit deferred diagnostics for bitpack/FOR and complex encodings.
- Phase 20 production MLIR validation hook in `loom-native-melior`.
- `scripts/production-native-lowering-test.sh` release gate and strict
  `LOOM_REQUIRE_PRODUCTION_NATIVE=1` mode.
- Roadmap caveat that Phase 20/21 are coupled axes and Phase 22 must decide
  pushdown plus concurrency/thread ownership.

## Files

- `crates/loom-core/src/production_native_lowering.rs`
- `crates/loom-core/src/decode_dialect.rs`
- `crates/loom-core/src/arrow_buffer_lowering.rs`
- `crates/loom-core/tests/production_native_lowering.rs`
- `crates/loom-core/tests/decode_dialect.rs`
- `crates/loom-core/tests/arrow_buffer_lowering.rs`
- `crates/loom-core/tests/production_native_kernels.rs`
- `crates/loom-native-melior/src/pipeline.rs`
- `crates/loom-native-melior/tests/production_pipeline.rs`
- `scripts/production-native-lowering-test.sh`
- `scripts/mvp0-verify.sh`
- `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-LOWERING-CONTRACT.md`
- `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-DECODE-DIALECT.md`
- `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-NATIVE-LOWERING-REPORT.md`

## Commands

Final closeout commands:

- `cargo fmt -p loom-core -p loom-native-melior`
- `cargo test -p loom-core --test production_native_lowering`
- `cargo test -p loom-core --test decode_dialect`
- `cargo test -p loom-core --test arrow_buffer_lowering`
- `cargo test -p loom-core --test production_native_kernels`
- `cargo test -p loom-native-melior --test production_pipeline`
- `bash scripts/production-native-lowering-test.sh`
- `LOOM_REQUIRE_PRODUCTION_NATIVE=1 bash scripts/production-native-lowering-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

Status: all final closeout commands passed on 2026-06-08.

## Deviations

- Phase 20 treats `loom.decode` as a stable textual/semantic seed, not as a
  mandatory compiled C++/ODS dialect.
- Bitpack and frame-of-reference native lowering are explicitly deferred instead
  of being partially implemented without sufficient facts.

## Residual Risks

- Phase 21 will need paired encoding and lowering decisions as the Vortex matrix
  widens.
- Phase 22's engine-independent ABI remains a design claim until a second engine
  validates it.
- Current builder lowering supports all-valid primitive fixed-size output only.

## Self-Check

Self-Check: PASSED
