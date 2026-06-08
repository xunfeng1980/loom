# Phase 21 Summary

## Status

Complete: Expanded Vortex Encoding Coverage.

Self-Check: PASSED

## Shipped Files

- `crates/loom-vortex-ingress/src/lib.rs`
- `crates/loom-vortex-ingress/tests/nullable_primitive_coverage.rs`
- `crates/loom-vortex-ingress/tests/chunked_primitive_coverage.rs`
- `crates/loom-vortex-ingress/tests/dictionary_runend_coverage.rs`
- `crates/loom-vortex-ingress/tests/bitpack_for_coverage.rs`
- `scripts/vortex-encoding-coverage-test.sh`
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md`
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-REPORT.md`

## Delivered

- Added `VortexEncodingCoverage`, `VortexEmissionDisposition`, and
  `VortexLoweringDisposition` to reader facts.
- Added fail-closed nullable primitive coverage with null-preserving oracle
  evidence.
- Added chunked primitive row-order coverage.
- Added dictionary and run-end/RLE real Vortex file coverage through canonical
  raw emission.
- Added bitpack and FOR real Vortex file coverage through canonical raw
  emission.
- Added a Phase 21 release gate and wired it into `scripts/mvp0-verify.sh`.

## Commands Run

- `cargo test -p loom-vortex-ingress --test reader_facts_contract`
- `cargo test -p loom-vortex-ingress --test nullable_primitive_coverage`
- `cargo test -p loom-vortex-ingress --test chunked_primitive_coverage`
- `cargo test -p loom-vortex-ingress --test dictionary_runend_coverage`
- `cargo test -p loom-vortex-ingress --test bitpack_for_coverage`
- `cargo test -p loom-core --test artifact_verifier`
- `cargo test -p loom-core --test production_native_kernels`
- `bash scripts/vortex-encoding-coverage-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

## Deviations

- Phase 21 did not emit structured Loom dictionary/run-end/bitpack/FOR artifacts
  from real Vortex files. It used canonical raw emission when Vortex scan
  provided deterministic primitive rows and recorded the original native
  backend work as deferred.
- ALP/PCodec/string compression from real Vortex files remains deferred because
  Loom-owned params are not yet extracted and verified.

## Risks

- Some real Vortex encoded shapes may be canonicalized by the file/scan layer
  before reader facts expose full structured internals.
- Phase 23 must not treat canonical raw evidence as production support for the
  original encoding.
- Phase 22 still needs to lock host runtime ABI policy for pushdown,
  concurrency, memory ownership, cache keys, and fallback.
