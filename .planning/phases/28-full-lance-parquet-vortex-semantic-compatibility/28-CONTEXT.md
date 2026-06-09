# Phase 28: Full Lance + Parquet + Vortex Semantic Compatibility - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Autonomous smart discuss; recommended defaults accepted per user preference

<domain>
## Phase Boundary

Phase 28 targets full Vortex semantic compatibility as a staged compatibility
matrix over the existing reader, verifier, source-ingress, native-backend, and
DuckDB evidence. It must make Vortex semantic support explicit by dtype,
nullability, array encoding, layout wrapper, chunking/zoning/statistics,
projection/predicate interaction, artifact emission, oracle equivalence, and
native-lowering disposition.

This phase does not claim StarRocks or second-host query equivalence because
Phase 29 is skipped/deferred. It also must not become an ABI redesign,
Iceberg/catalog phase, source-ingress framework rewrite, or broad native JIT
rewrite. Any semantic shape that cannot be proven through accepted reader facts,
verifier evidence, oracle equivalence, and fail-closed diagnostics remains
unsupported or deferred with explicit evidence.

</domain>

<decisions>
## Implementation Decisions

### Compatibility Scope

- Treat "full compatibility" as a versioned matrix and gate, not a single
  blanket support claim. Every row must record original Vortex shape, emitted
  Loom shape, artifact verifier status, oracle evidence, DuckDB visibility when
  applicable, and native-lowering disposition.
- Start from Phase 18/21 accepted and unsupported matrices, then close gaps in
  priority order: nullable primitives, chunked primitives, dictionary/run-end
  structured semantics, bitpack/FOR facts, string/FSST-compatible paths, nested
  struct/table edge cases, and statistics/projection semantics.
- Preserve `loom-core`, `loom-ffi`, and public C/SQL surfaces as Vortex-free.
  Vortex crate APIs stay isolated to `loom-vortex-ingress` and adapter-local
  test/oracle code.
- Do not cite Phase 29 as evidence. Engine independence remains a weaker claim
  until a second-consumer phase is completed.

### Evidence and Trust Model

- A compatibility row is accepted only when real Vortex input yields stable
  Loom-owned reader facts, verifier-accepted artifact bytes where emission is
  supported, and Vortex oracle equivalence for row values and null semantics.
- Unsupported valid Vortex shapes must remain inspectable where possible but
  emit no artifact and carry stable unsupported diagnostics.
- Malformed or internally inconsistent Vortex inputs must reject fail-closed
  before partial artifact emission.
- Canonicalized raw emission is allowed only when the report records both the
  original Vortex shape and the emitted Loom shape; it must not be mistaken for
  structured semantic preservation or native lowering support.

### Native and Runtime Position

- Native execution support remains limited to shapes with verifier-backed
  production lowering and ExecutionEngine evidence. Interpreter-only semantic
  compatibility is valid when explicitly labeled and oracle-gated.
- The Phase 24/25 native fix is now load-bearing: native DuckDB primitive SQL
  evidence must use MLIR ExecutionEngine output over real artifact value
  buffers, not zero buffers or interpreter fallback.
- New compatibility shapes must choose one lowering disposition:
  `production-lowering-supported`, `interpreter-only`, or
  `fail-closed/deferred`.
- Predicate pushdown, distributed execution, and second-host query proof are
  out of scope unless already supported by existing runtime policy; record them
  as deferred, not implied.

### Verification and Release Gate

- Produce a Phase 28 compatibility matrix/report that distinguishes accepted,
  unsupported, rejected, interpreter-only, and native-supported rows.
- Add a focused `scripts/vortex-semantic-compatibility-test.sh` gate and wire it
  into `scripts/mvp0-verify.sh` after Phase 28/29 planning gates and before the
  final DuckDB smoke/native closeout, only after it passes.
- Include negative checks for silent artifact emission, conflated canonical raw
  output, missing oracle evidence, unsupported null/nested/string semantics,
  stale reader facts, and public API/query-surface creep.
- Keep the release proof deterministic on a clean checkout.

### Current-Phase Tradeoff

- Accepted tradeoff: Phase 28 starts without Phase 30 StarRocks/DuckDB
  dual-query evidence. This keeps momentum on Vortex semantics but weakens any
  claim that the runtime ABI has been validated by a second host.
- Accepted tradeoff: The first Phase 28 pass may complete "full semantic
  compatibility" as an explicit matrix with unsupported/deferred rows instead
  of implementing every Vortex shape. The report must make unsupported rows
  impossible to confuse with accepted support.
- Accepted tradeoff: native support is not required for every semantically
  accepted row. Interpreter-only rows are acceptable when the verifier/oracle
  evidence is strong and native disposition is explicit.

### the agent's Discretion

Implementation details, file splits, fixture shapes, and test factoring are at
the agent's discretion as long as the compatibility matrix remains explicit,
Vortex dependencies stay isolated, and every accepted row has real oracle and
verifier evidence.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets

- `ingress/loom-vortex-ingress` already owns Vortex APIs, reader facts, support
  classification, artifact emission, and oracle tests for primitive/table
  slices.
- `.planning/phases/21-expanded-vortex-encoding-coverage/21-COVERAGE-MATRIX.md`
  and `21-COVERAGE-REPORT.md` provide the starting compatibility matrix and
  disposition vocabulary.
- `loom_core::artifact_verifier`, `loom_core::runtime_abi`, and
  `loom_core::production_native_lowering` already model verifier acceptance,
  runtime planning, and native-lowering support/fail-closed diagnostics.
- `scripts/complete-vortex-reader-test.sh`,
  `scripts/vortex-encoding-coverage-test.sh`,
  `scripts/native-hardening-test.sh`, and `scripts/mvp0-verify.sh` are the main
  gate patterns to reuse.

### Established Patterns

- Adapter crates keep external SDK/API details out of `loom-core` and `loom-ffi`.
- Phase reports explicitly separate accepted, unsupported, rejected, deferred,
  interpreter-only, and native-supported evidence.
- Release gates combine implementation marker checks, focused Rust tests,
  negative tests, generated fixtures, CLI/SQL smoke where applicable, and
  public-surface creep guards.
- Canonical raw emission is allowed only with explicit original-shape evidence
  and oracle equivalence.

### Integration Points

- `ingress/loom-vortex-ingress/src/lib.rs` and its coverage tests are the likely
  implementation entry point for new Vortex semantic rows.
- `crates/loom-core/src/table_codec.rs`, `layout_codec.rs`,
  `artifact_verifier.rs`, and `production_native_lowering.rs` are the core
  verifier/emission/lowering checkpoints.
- `crates/loom-fixtures` can provide deterministic generated fixtures and
  oracle helpers where direct Vortex APIs are awkward.
- `scripts/mvp0-verify.sh` is the final release-gate integration point.

</code_context>

<specifics>
## Specific Ideas

- Prioritize explicit truth over phase-count progress: unsupported rows are
  acceptable only when clearly labeled and tested fail-closed.
- Keep the Phase 29 skip/defer visible in Phase 28 planning and final reporting.
- Treat the recent native zero-buffer fix as a regression guard: Phase 28 must
  not add compatibility gates that can pass by fallback while claiming native
  execution.

</specifics>

<deferred>
## Deferred Ideas

- StarRocks or any second-host query-surface proof remains deferred to Phase 29
  or an equivalent future phase.
- Production native lowering for every accepted Vortex shape is deferred unless
  a row has explicit Phase 23/24/25-compatible ExecutionEngine evidence.
- Remote catalogs, object-store credentials, distributed execution, and mutable
  Iceberg/catalog semantics remain out of scope.

</deferred>
