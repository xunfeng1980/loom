# Phase 21: Expanded Vortex Encoding Coverage - Context

**Gathered:** 2026-06-08  
**Status:** Ready for planning  
**Source:** Research-led planning from `21-RESEARCH.md`

<domain>
## Phase Boundary

Phase 21 widens the real Vortex reader/support matrix beyond Phase 18's
non-null primitive and primitive-struct slice. The deliverable is a disciplined
coverage matrix over dtype, nullability, array/layout encoding, segmentation,
statistics, artifact emission, verifier/oracle evidence, and lowering
disposition.

This phase must not become a production backend phase. Compiled ODS dialects,
the production `melior` pass pipeline, LLVM lowering, and verifier-gated
LLVM/JIT execution are Phase 23.
</domain>

<decisions>
## Locked Decisions

### Coverage Model

- Coverage is tracked per Vortex shape, not as a single boolean.
- Each covered shape records reader classification, emission kind, verifier
  evidence, oracle evidence, and lowering disposition.
- Lowering disposition must be one of:
  - `interpreter-only`
  - `production-lowering-supported`
  - `fail-closed/deferred`

### Emission Policy

- Valid Vortex inputs may be `Unsupported` while still producing reader facts.
- `LMC1` bytes are emitted only when Loom-owned facts are sufficient for
  structural verification, artifact verification, and oracle evidence.
- Canonicalized raw emission is acceptable only when it is explicitly recorded
  as a semantic bridge, not as proof that Loom understands the original native
  encoding for lowering.

### Dependency Boundary

- `vortex-file`, `vortex-layout`, and Vortex API details stay isolated in
  `loom-vortex-ingress`.
- `loom-core` and `loom-ffi` remain Vortex-free.
- Phase 21 may extend Loom-owned facts/types in `loom-vortex-ingress` and Loom
  artifact models in `loom-core` only where required by accepted emission.

### Out of Scope

- Arbitrary Vortex encoding support.
- WASM decompression or extension encoding execution.
- New solver backend strategy.
- Production compiled dialect/JIT implementation.
- DuckDB native integration.
- Iceberg or multi-engine query surface.
</decisions>

<canonical_refs>
## Canonical References

### Phase Scope

- `.planning/ROADMAP.md` - Phase 21 ordering and boundaries.
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-RESEARCH.md` -
  recommended coverage matrix and plan split.

### Prerequisites

- `.planning/phases/18-complete-vortex-reader/18-SUMMARY.md` - current reader
  boundary and supported matrix.
- `.planning/phases/18-complete-vortex-reader/18-READER-CONTRACT.md` - reader
  fact contract.
- `.planning/phases/19-solver-backed-full-artifact-verifier/19-SUMMARY.md` -
  solver-backed artifact verifier status.
- `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-SUMMARY.md`
  - production lowering seed and deferred native kernels.
- `.planning/phases/20-production-decode-dialect-and-native-kernel-expansion/20-DECODE-DIALECT.md`
  - current `loom.decode` semantic seed.

### Code

- `ingress/loom-vortex-ingress/src/lib.rs` - isolated Vortex reader bridge.
- `ingress/loom-vortex-ingress/tests/reader_facts_contract.rs` - reader fact
  stability tests.
- `ingress/loom-vortex-ingress/tests/single_column_to_loom.rs` - current
  primitive matrix tests.
- `ingress/loom-vortex-ingress/tests/table_to_loom.rs` - current primitive table
  tests.
- `crates/loom-core/src/l1_model.rs` - Loom layout model and decode semantics.
- `crates/loom-core/src/layout_codec.rs` - `LMP1` layout codec.
- `crates/loom-core/src/production_native_lowering.rs` - Phase 20 lowering
  support gate and current deferred kernels.
</canonical_refs>

<specifics>
## Specific Ideas

- Add a coverage/disposition type in `loom-vortex-ingress`, not in
  `loom-core`, unless a fact must become part of a Loom artifact.
- Add a reviewer-readable `21-COVERAGE-MATRIX.md`.
- Extend release gates with a `scripts/vortex-encoding-coverage-test.sh`
  script wired into `scripts/mvp0-verify.sh`.
- Treat nullable primitives as the first pressure test because they force the
  distinction between reader facts, artifact emission, and Phase 20 all-valid
  native lowering.
- Treat chunk/split facts as Phase 22 ABI input even if Phase 21 does not yet
  exploit pushdown or parallel execution.
</specifics>

<deferred>
## Deferred Ideas

- Production ODS dialect and LLVM/JIT backend implementation: Phase 23.
- Host callable ABI and engine execution policy: Phase 22.
- DuckDB native execution: Phase 24.
- Native cache/equivalence hardening: Phase 25.
- Iceberg and dual-engine query surfaces: Phase 26 and Phase 27.
</deferred>

---

*Phase: 21-expanded-vortex-encoding-coverage*  
*Context gathered: 2026-06-08*
