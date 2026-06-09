# Phase 18 Summary

## Shipped

- Reader facts contract and dependency boundary for complete Vortex reader work.
- Recursive dtype/layout/segment/split fact extraction with stable Loom-owned diagnostics.
- Supported single-column emission matrix for non-null Int32, Int64, Float32, and Float64.
- Supported struct/table emission for non-null primitive fields as `LMC1(LMT1)`.
- CLI inspect/emit visibility for reader support, emission kind, fact counts, and artifact-verifier handoff.
- `scripts/complete-vortex-reader-test.sh` and `scripts/mvp0-verify.sh` release-gate wiring.
- Final reader report and public/planning docs aligned to Phase 18.

## Files

- `ingress/loom-vortex-ingress/src/lib.rs`
- `ingress/loom-vortex-ingress/tests/reader_facts_contract.rs`
- `ingress/loom-vortex-ingress/tests/reader_recursive_facts.rs`
- `ingress/loom-vortex-ingress/tests/single_column_to_loom.rs`
- `ingress/loom-vortex-ingress/tests/table_to_loom.rs`
- `ingress/loom-vortex-ingress/tests/real_file_to_loom.rs`
- `crates/loom-cli/src/main.rs`
- `scripts/complete-vortex-reader-test.sh`
- `scripts/mvp0-verify.sh`
- `README.md`
- `README-zh.md`
- `.planning/PROJECT.md`
- `.planning/ROADMAP.md`
- `.planning/STATE.md`
- `.planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md`
- `.planning/phases/18-complete-vortex-reader/18-READER-REPORT.md`

## Commands

Final closeout commands:

- `cargo test --workspace`
- `cargo test -p loom-vortex-ingress`
- `cargo test -p loom-core --test artifact_verifier`
- `bash scripts/complete-vortex-reader-test.sh`
- `bash scripts/mvp0-verify.sh`
- `git diff --check`

Status: all final closeout commands passed on 2026-06-08. After the local LLVM/MLIR `22.1.7` upgrade, the Phase 16 optional backend gate now passes its feature-enabled `melior` JIT path rather than skipping for toolchain mismatch.

## Deviations

- "Complete reader" is interpreted as a complete reader boundary and facts model, not arbitrary Loom artifact emission for every Vortex encoding/layout.
- Vortex scan remains oracle evidence only; it is not imported into `loom-core` or `loom-ffi`.
- Solver discharge is intentionally deferred to Phase 19.

## Residual Risks

- The accepted emission matrix is finite and should remain explicit until solver-backed facts and native lowering can consume more shapes safely.
- Vortex crate API details remain isolated, but future reader expansion may require more adapter logic as Vortex layout APIs evolve.
- Complete correctness is still out of scope; emitted artifacts have oracle/equivalence evidence and verifier acceptance, not a proof of all Vortex semantics.

## Self-Check

Self-Check: PASSED
