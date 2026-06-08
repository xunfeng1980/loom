# Phase 23 Research: Production Native Backend Implementation

## Recommendation

Implement Phase 23 as a backend boundary, not a host integration phase. The
backend should consume Phase 22 runtime plans/cache keys, Phase 20 production
lowering facts, and Phase 21 encoding dispositions, then produce verifier-gated
MLIR/LLVM/JIT evidence with stable diagnostics and backend identity. Keep the
public C ABI unfrozen until Phase 24 exercises a real host adapter and Phase 25
hardens cache/fallback behavior.

The highest-leverage order is:

1. Define a backend contract and typed request/report model around `RuntimePlan`
   and `RuntimeCacheKey`.
2. Add compiled `loom.decode` ODS/TableGen evidence and generated verifier/doc
   checks without making LLVM mandatory for default builds.
3. Promote the existing melior validation path into an explicit production
   backend pipeline with LLVM lowering reports and backend/toolchain identity.
4. Add verifier-gated JIT execution for a narrow primitive kernel slice and compare
   against interpreter output.
5. Close with a backend release gate, final report, and Phase 24 DuckDB handoff.

## Source Findings

- `melior` 0.27.0 is current for this workspace era and depends on `mlir-sys`
  `^220.0.1`, aligning with LLVM/MLIR 22. Source:
  https://docs.rs/crate/melior/latest
- The upstream melior project states that LLVM/MLIR must be installed and also
  warns that MLIR C API and melior are alpha/unstable. This reinforces the current
  strict-toolchain-gate design and argues against freezing a public ABI in Phase
  23. Source: https://github.com/mlir-rs/melior
- MLIR ODS/TableGen is the intended way to define operations, generate C++ op
  classes, verifier scaffolding, parsers/printers, and operation documentation.
  Phase 23 should therefore add ODS evidence for `loom.decode` rather than
  continuing with only textual string emission. Source:
  https://mlir.llvm.org/docs/DefiningDialects/Operations/
- MLIR dialect definitions use TableGen records and C++ registration machinery.
  This makes "compiled dialect" a toolchain artifact that should live behind a
  backend crate/build boundary, not in `loom-core`. Source:
  https://mlir.llvm.org/docs/DefiningDialects/
- MLIR's C ExecutionEngine API exposes a JIT engine boundary and invocation model.
  Phase 23 should keep this behind verifier-gated backend reports and never expose
  it directly to DuckDB or the public C ABI. Source:
  https://mlir.llvm.org/doxygen/mlir-c_2ExecutionEngine_8h.html

## ABI and API Lessons Applied

From the Phase 22 post-closeout C API / N-API / natural API research:

- Freeze the stable ABI late and keep it small. Phase 23 should define backend
  internals, not stabilize `loom_runtime.h`.
- Natural Rust/C++ wrappers are useful for tests and ergonomics, but they should
  be generated from or layered over the stable contract. They must not define the
  contract.
- Version and capability negotiation need to be explicit. Backend artifacts must
  record Loom ABI version, backend version, LLVM/MLIR version, target/layout facts,
  supported kernel set, and pass pipeline ID.
- Cancellation and long-running execution hooks belong in the backend request
  model before JIT kernels are introduced, even if the first implementation only
  checks cancellation before/after preparation.
- Cache identity must include more than the artifact hash: plan, projection,
  predicate envelope, splits/concurrency, ABI version, backend version, toolchain,
  target/layout, and pass pipeline identity all matter.

## Related Project Lessons

- The existing Loom Phase 16 melior backend proved optional toolchain probing,
  validation, and strict/skip behavior. Phase 23 should reuse that surface instead
  of adding a parallel LLVM integration.
- DuckDB's extension path should remain a consumer, not the owner of the runtime
  or backend contract. Phase 24 will map DuckDB bind/init/local-init/function
  lifecycle onto the Phase 22/23 contract.
- Vortex support remains a reader/oracle/ingress concern unless a Loom-owned
  encoding's parameters and verifier facts are extracted. Phase 23 should only
  compile kernels whose lowering disposition is explicitly supported.

## Risks

- LLVM/MLIR version drift can make local strict gates noisy. Mitigate with explicit
  backend identity, managed toolchain probing, and skip only by
  `LOOM_ALLOW_NATIVE_TOOL_SKIP=1`.
- A compiled ODS dialect can balloon the build. Mitigate by making ODS generation
  and dialect registration strict-tooling evidence first, with default builds
  remaining MLIR-free.
- JIT execution can bypass verifier assumptions. Mitigate by requiring the Phase
  22 runtime decision, accepted artifact facts, discharged constraints, production
  lowering support, and supported kernel matrix before preparation.
- Backend cache keys can be under-specified. Mitigate by including toolchain,
  target/layout, pass pipeline, capability, and runtime plan identity in every
  backend artifact/report.
- Natural wrappers can accidentally become ABI. Mitigate by marking wrappers
  internal/test-only until Phase 24/25 prove a stable C surface.

## Proposed Phase Split

- 23-01: Backend contract and runtime-plan bridge.
- 23-02: Compiled `loom.decode` ODS/TableGen evidence.
- 23-03: Production melior/LLVM lowering pipeline and backend identity.
- 23-04: Verifier-gated JIT execution seed and interpreter equivalence.
- 23-05: Backend release gate, report, docs, and Phase 24 handoff.

## Out of Scope

- DuckDB native integration.
- Public C ABI freeze.
- Persistent native cache reuse hardening.
- Iceberg/StarRocks integration.
- Arbitrary Vortex semantics or new broad encoding support.
