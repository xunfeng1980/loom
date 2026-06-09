# Phase 30 Pattern Map

## Closest Existing Patterns

| Phase 30 Need | Existing Pattern | How to Reuse |
|---|---|---|
| Compatibility matrix vocabulary | `crates/loom-vortex-ingress/src/lib.rs` `VortexEncodingCoverage` | Extend with semantic-compatibility rows instead of inventing a separate support taxonomy. |
| Matrix/report documentation | `21-COVERAGE-MATRIX.md`, `21-COVERAGE-REPORT.md` | Use the same accepted/unsupported/rejected and disposition separation, but make rows executable and broader. |
| Reader facts and fail-closed emission | `18-READER-REPORT.md`, `scripts/complete-vortex-reader-test.sh` | Keep valid unsupported files fact-bearing with no emitted bytes; malformed files rejected. |
| Artifact verifier handoff | `crates/loom-core/src/artifact_verifier.rs` tests | Every emitted compatibility row must pass existing artifact verification. |
| Native disposition guard | `crates/loom-core/src/production_native_lowering.rs`, `crates/loom-native-melior/src/jit.rs` | Native-supported means production lowering plus ExecutionEngine evidence, not fallback. |
| Focused release gate | `scripts/vortex-encoding-coverage-test.sh`, `scripts/native-hardening-test.sh`, `scripts/iceberg-binding-test.sh` | Build a strict shell gate with marker checks, focused cargo tests, negative checks, and no-overclaim grep guards. |

## Recommended File Shape

| New/Modified File | Purpose | Pattern Source |
|---|---|---|
| `crates/loom-vortex-ingress/src/lib.rs` | Add semantic compatibility row/report types and mapping from existing coverage facts. | Existing coverage enums and report-style structs. |
| `crates/loom-vortex-ingress/tests/semantic_compatibility_matrix.rs` | Positive and negative matrix tests for accepted/unsupported/canonicalized rows. | Existing `*_coverage.rs` tests. |
| `crates/loom-vortex-ingress/tests/nullable_semantic_compatibility.rs` | Nullable primitive semantic gap closure or explicit unsupported proof. | `nullable_primitive_coverage.rs`, core buffer layout tests. |
| `crates/loom-vortex-ingress/tests/structured_encoding_semantics.rs` | Dictionary/run-end/bitpack/FOR structured-vs-canonical evidence. | `dictionary_runend_coverage.rs`, `bitpack_for_coverage.rs`, core L1 tests. |
| `scripts/vortex-semantic-compatibility-test.sh` | Focused Phase 30 gate. | `scripts/vortex-encoding-coverage-test.sh`, `scripts/native-hardening-test.sh`. |
| `.planning/phases/30-full-vortex-semantic-compatibility/30-VORTEX-SEMANTIC-COMPATIBILITY-REPORT.md` | Final accepted/unsupported/deferred matrix and tradeoff report. | `21-COVERAGE-REPORT.md`, `25-NATIVE-HARDENING-REPORT.md`. |

## Dependency Boundary Pattern

- `loom-core` and `loom-ffi` remain Vortex-free.
- Vortex APIs stay isolated to `loom-vortex-ingress` and fixture/oracle tests.
- New compatibility types should expose Loom-owned strings/enums, not Vortex crate types.
- Do not add StarRocks, catalog, credential, object-store, or new public query dependencies.

## Gate Pattern

The focused Phase 30 gate should prove:

1. `30-CONTEXT.md`, `30-RESEARCH.md`, `30-PATTERNS.md`, and final report exist.
2. Semantic compatibility row/report types exist.
3. Positive matrix tests cover existing Phase 21 rows.
4. Negative tests prove unsupported/rejected/canonicalized rows cannot overclaim.
5. Nullable and structured semantic target tests pass or record explicit unsupported/deferred evidence.
6. Native support markers require `native-execution-engine-output` or equivalent ExecutionEngine evidence.
7. Public API/query-surface creep guards pass.
8. `mvp0-verify.sh` invokes the focused gate after prior source/table/native gates and before final DuckDB smoke.

## Current-Phase Tradeoffs

- Matrix-first over blanket support is mandatory. It is the only defensible way to call the phase "full compatibility" without hiding unsupported Vortex shapes.
- Canonical raw support is useful but lower fidelity than structured semantic support. Reports and tests must keep those states separate.
- Interpreter-only compatibility can be complete semantic evidence for a row; native support remains an additional disposition, not a required condition for every accepted row.
- Phase 29 skip/defer reduces second-host validation. Phase 30 must document that gap instead of compensating with broader claims.
