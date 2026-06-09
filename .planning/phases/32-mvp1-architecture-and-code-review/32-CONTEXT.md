# Phase 32: MVP1 Architecture and Code Review - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning
**Mode:** Text-mode discuss; user selected all review areas and accepted recommended defaults

<domain>
## Phase Boundary

Phase 32 is a review-first phase for the completed and partially deferred MVP1
track. It audits the design and implementation end to end: artifact contracts,
source-ingress semantics, DuckDB execution evidence, native/runtime claims,
ABI/FFI boundaries, dependency isolation, release gates, documentation
truthfulness, and concrete remediation items.

This phase should pause feature expansion. It may apply narrow, unambiguous
fixes discovered during review, but it must not add new source formats, broaden
DuckDB SQL coverage, resume StarRocks integration, redesign `LMA1`/`LMC2`, or
expand native MLIR execution scope.

</domain>

<decisions>
## Implementation Decisions

### Truth and Overclaim Audit

- **D-32-01:** Produce a claim ledger. Each public or planning claim reviewed
  must map to concrete evidence, a status, and any required correction.
- **D-32-02:** The ledger must explicitly distinguish proven execution,
  interpreter fallback, scaffold/seed evidence, skipped/toolchain-conditional
  evidence, deferred scope, and unsupported/fail-closed behavior.
- **D-32-03:** README, README-zh, ROADMAP, STATE, phase reports, and release-gate
  script names are in scope for claim truthfulness. Do not let successful
  scripts imply stronger semantics than the code and assertions prove.

### Execution Evidence Audit

- **D-32-04:** Use an evidence-first standard. Each gate must state what it
  proves, what it does not prove, and whether it depends on fallback, skip,
  scaffold, synthetic fixtures, or real source artifacts.
- **D-32-05:** Treat `scripts/mvp1-verify.sh` as the broad MVP1 gate. It proves
  `scripts/mvp0-verify.sh` plus Parquet/Lance/Vortex source-backed single-column
  `LMA1` artifacts queried through DuckDB SQL.
- **D-32-06:** Native evidence must be reviewed with special caution. Phase 24/25
  route/ABI/cache/fallback wiring is not equivalent to full native semantic
  decode for arbitrary source artifacts. Any zero-buffer, fallback, skip, or
  mismatch-only evidence must be labeled accurately.

### Architecture Boundary Audit

- **D-32-07:** Audit all major boundaries: `loom-core`, `loom-ffi`, source
  adapters, DuckDB extension, native backend, scripts, and documentation.
- **D-32-08:** Verify dependency isolation claims directly. `loom-core` and
  `loom-ffi` must remain free of source SDK and Vortex-file/reader leakage except
  where explicitly allowed by existing dependency-boundary decisions.
- **D-32-09:** Review ABI/FFI boundaries for ownership, panic safety, release
  callbacks, public header creep, internal DuckDB handle leakage, and natural
  API design. Do not freeze new public ABI unless the review explicitly justifies
  it.
- **D-32-10:** Review `LMA1` and `LMC2` wording. `LMA1` direct payload support is
  implemented; `LMC2` remains future wrapper unless code proves otherwise.

### Code Quality and Maintainability Audit

- **D-32-11:** Use review-first workflow. Produce review reports before applying
  fixes.
- **D-32-12:** Narrow fixes are allowed only for unambiguous defects, incorrect
  documentation, broken gates, dependency-boundary leaks, or small code-quality
  issues with low blast radius.
- **D-32-13:** Broader refactors, new feature work, StarRocks runtime work, native
  expansion, and semantic artifact redesign belong in follow-up phases or
  explicit remediation plans, not silent Phase 32 scope growth.

### Release Readiness Audit

- **D-32-14:** Produce a go/no-go matrix for the current MVP1 baseline.
- **D-32-15:** Classify findings as blocking, high, medium, low, or informational.
  Blocking/high findings must include concrete evidence and a proposed
  remediation path.
- **D-32-16:** Phase 30 must remain explicitly partial/deferred unless the review
  or a later execution phase actually completes StarRocks/full dual-surface
  evidence.

### the agent's Discretion

- Choose exact report file names and plan boundaries, provided the phase covers
  all five selected audit dimensions.
- Choose whether each plan is report-only or includes narrow fixes, provided any
  fixes are small, justified by review findings, and verified.
- Choose focused grep/static checks and script probes that make the audit
  repeatable without adding unnecessary dependencies.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project State and Scope

- `.planning/ROADMAP.md` — Current phase graph, Phase 30 partial status, Phase
  31 completion, and Phase 32 review boundary.
- `.planning/STATE.md` — Accumulated decisions, recent quick task, current
  focus, and known caveats.
- `.planning/PROJECT.md` — Project value statement, constraints, and key
  decisions.
- `.planning/REQUIREMENTS.md` — Requirement IDs and completion context.

### Recent Phase Context and Reports

- `.planning/phases/31-full-arrow-semantic-source-compatibility/31-CONTEXT.md`
  — Locked full Arrow semantic compatibility decisions and non-goals.
- `.planning/phases/31-full-arrow-semantic-source-compatibility/31-FULL-COMPATIBILITY-REPORT.md`
  — Final Phase 31 evidence and tradeoffs.
- `.planning/phases/30-starrocks-duckdb-dual-query-surface/30-CONTEXT.md`
  — Phase 30 query-surface decisions and deferred StarRocks scope.
- `.planning/phases/30-starrocks-duckdb-dual-query-surface/30-DUCKDB-EXECUTION-REPORT.md`
  — DuckDB executable evidence slice for Phase 30.
- `.planning/phases/30-starrocks-duckdb-dual-query-surface/30-SKIPPED.md`
  — Deferred/skipped Phase 30 work.
- `.planning/phases/29-iceberg-ref-table-binding/29-ICEBERG-BINDING-REPORT.md`
  — Iceberg binding evidence and non-goals.
- `.planning/phases/28-full-lance-parquet-vortex-semantic-compatibility/28-LANCE-PARQUET-VORTEX-SEMANTIC-COMPATIBILITY-REPORT.md`
  — Semantic compatibility matrix and no-overclaim evidence.
- `.planning/phases/25-native-equivalence-cache-and-fallback-hardening/25-NATIVE-HARDENING-REPORT.md`
  — Native equivalence/cache/fallback evidence and limitations.
- `.planning/phases/24-duckdb-native-execution-integration-mvp/24-CONTEXT.md`
  — DuckDB native integration decisions and tradeoffs.
- `.planning/phases/23-production-native-backend-implementation/23-BACKEND-REPORT.md`
  — Production backend seed evidence and deferred native scope.
- `.planning/phases/22-host-native-runtime-abi-and-execution-policy/22-RUNTIME-ABI-REPORT.md`
  — Runtime ABI/policy design and unfrozen boundary.

### Verification Gates

- `scripts/mvp1-verify.sh` — Broad MVP1 check entry point.
- `scripts/mvp0-verify.sh` — Existing release gate that now carries most phase
  gates.
- `scripts/duckdb-source-e2e-test.sh` — Parquet/Lance/Vortex source-backed
  `LMA1` DuckDB SQL e2e gate.
- `scripts/duckdb-native-integration-test.sh` — Phase 24 DuckDB native route
  gate.
- `scripts/native-hardening-test.sh` — Phase 25 native hardening gate.
- `scripts/full-arrow-semantic-compatibility-test.sh` — Phase 31 source semantic
  gate.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/loom-core/src/arrow_semantic*.rs` — `LMA1` semantic artifact model,
  codec, and verifier substrate.
- `crates/loom-ffi/src/ffi.rs` and `crates/loom-ffi/src/duckdb_runtime.rs` —
  FFI decode and DuckDB runtime/fallback routing.
- `duckdb-ext/loom_extension.cpp` — Public DuckDB `loom_scan(path)` table
  function, bind/scan behavior, and direct DataChunk output.
- `ingress/loom-parquet-ingress`, `ingress/loom-lance-ingress`, and
  `ingress/loom-vortex-ingress` — Source adapter patterns and semantic emission
  boundaries.
- `crates/loom-native-melior` — Native backend/JIT seed, diagnostics, and
  toolchain-dependent evidence.

### Established Patterns

- Accepted artifacts require verifier acceptance and explicit evidence; malformed
  or unsupported inputs fail closed.
- Source-specific dependencies are isolated in adapter crates; generic/core/ffi
  surfaces stay source-neutral.
- Public SQL surface remains `loom_scan(path)`; internal diagnostics and handles
  should not leak into public ABI.
- Focused gates are added before broad release-gate wiring.
- Reports must record current-phase tradeoffs and non-goals when evidence is
  intentionally bounded.

### Integration Points

- Documentation surfaces: `README.md`, `README-zh.md`, `.planning/ROADMAP.md`,
  `.planning/STATE.md`, and phase reports.
- Release/check surfaces: `scripts/mvp1-verify.sh`, `scripts/mvp0-verify.sh`,
  and focused phase gates.
- ABI/FFI surfaces: `crates/loom-ffi/include/loom.h`,
  `crates/loom-ffi/include/loom_duckdb_internal.h`, Rust extern functions, and
  DuckDB C++ ownership paths.
- Dependency surfaces: workspace manifests, adapter manifests, and dependency
  boundary tests/scripts.

</code_context>

<specifics>
## Specific Ideas

- Create a claim ledger that reviewers can scan quickly: claim, source file,
  evidence command/test, actual status, risk, action.
- Create an execution evidence matrix that separates real source e2e, pure Rust
  semantic equality, DuckDB SQL, interpreter fallback, native route scaffolding,
  toolchain-conditional native checks, and skipped/deferred StarRocks work.
- Treat Phase 32 as the place to correct misleading wording before more phases
  build on it.
- Keep Phase 32 plans review-sized and audit-oriented.

</specifics>

<deferred>
## Deferred Ideas

- Completing StarRocks runtime integration and full dual-surface closeout remains
  Phase 30 or a future explicit remediation phase.
- Broad DuckDB nested/logical SQL support for arbitrary `LMA1` Arrow schemas is
  outside this review unless documented as a finding.
- Native MLIR/ExecutionEngine semantic decode expansion for `LMA1` and arbitrary
  source artifacts is outside this review unless documented as a finding.
- `LMC2` container implementation/wrapping is outside this review unless
  documented as a claim mismatch or follow-up.

</deferred>

---

*Phase: 32-mvp1-architecture-and-code-review*
*Context gathered: 2026-06-09*
