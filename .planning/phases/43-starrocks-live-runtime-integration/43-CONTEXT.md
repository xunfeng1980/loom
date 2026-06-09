# Phase 43: StarRocks Live Runtime Integration - Context

**Gathered:** 2026-06-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 43 upgrades Phase 30's StarRocks-compatible offline descriptors into a
runtime evidence surface for a second query engine. It must query the same
accepted Loom-bound artifact identity used by DuckDB/oracle evidence, compare
live StarRocks rows and aggregates against that matrix, and preserve
fail-closed behavior for unsupported shapes. It informs the still-unfrozen ABI;
it does not freeze the ABI, add a third engine, or productize StarRocks catalog,
credential, or object-store flows.

</domain>

<decisions>
## Implementation Decisions

### Runtime Evidence Boundary
- Treat Phase 30 descriptors as inputs to a StarRocks runtime harness, not as
  accepted runtime evidence by themselves.
- Use an explicit external runtime/client boundary for live StarRocks evidence;
  no default StarRocks server, JDBC/ODBC/MySQL client, Docker, credential, or
  catalog dependency is added to neutral crates.
- Missing runtime configuration must be reported as missing live evidence, not
  as a passing live-runtime claim.
- Keep runtime evidence tied to the accepted Phase 29 binding identity,
  artifact SHA-256, schema ID, snapshot ID, row count, and expected digest.

### Equivalence and Fail-Closed Matrix
- Compare StarRocks runtime output against the existing canonical query matrix
  and DuckDB evidence for ordered rows, projection, predicate, count, and sum.
- Record runtime output as typed evidence rows with status values that
  distinguish accepted, unsupported, rejected, skipped/missing-runtime, and
  mismatch.
- Unsupported StarRocks shapes must produce typed diagnostics and no accepted
  runtime evidence.
- Drift in descriptor identity, result digest, projection, or query kind must
  fail closed before a live result can be accepted.

### ABI Findings
- Document every place the current engine-independent contract remains
  DuckDB-shaped: path-oriented scan surface, one-shot table function lifecycle,
  single-thread/single-batch assumptions, projection/filter ownership, error
  text expectations, and Arrow adoption semantics.
- Fix only narrow unfrozen ABI assumptions when the code already has a neutral
  place to do so; otherwise record accepted asymmetries for Phase 44.
- Do not add public `loom_scan_starrocks`, StarRocks catalog/credential routes,
  or a StarRocks-specific public C ABI.
- Keep Phase 43 evidence adapter-local so Phase 44 can freeze the shared ABI
  from two-engine findings instead of from StarRocks-specific code.

### the agent's Discretion
Implementation details, helper names, report formatting, and gate shape are at
the agent's discretion as long as the live-evidence vs skipped-evidence
distinction remains explicit.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-dual-query-surface` already owns Phase 30 accepted fixture
  generation, canonical query matrix derivation, StarRocks-compatible
  descriptors, descriptor validation, and DuckDB evidence.
- `scripts/dual-query-surface-test.sh` already contains optional
  `LOOM_STARROCKS_RUNTIME_SMOKE` logic that runs StarRocks-compatible SQL via an
  operator-provided MySQL-compatible client, but it is explicitly supplemental.
- Phase 42 added `scripts/mvp2-verify.sh` and a coverage matrix gate that Phase
  43 can extend after inherited MVP2 checks.

### Established Patterns
- Source/query-engine work is adapter-local first, with public/neutral surfaces
  guarded by grep and dependency-boundary tests.
- Positive evidence requires accepted verifier/oracle/source identity and stable
  diagnostics; skipped or unsupported paths cannot seed accepted claims.
- Broad release gates call focused phase gates, and phase reports name
  non-claims bluntly.

### Integration Points
- Extend `loom-dual-query-surface` with runtime evidence data structures and
  tests.
- Add a Phase 43 focused script that checks planning/report markers, runs Rust
  contract tests, and optionally runs a live StarRocks runtime check when env is
  provided.
- Wire the focused gate into `scripts/mvp2-verify.sh` only once the phase can
  distinguish local contract evidence from genuine live-runtime evidence.

</code_context>

<specifics>
## Specific Ideas

User preference from prior review: do not let scaffolding masquerade as the
hard claim. For Phase 43, a skipped StarRocks runtime must remain visibly
non-accepted live evidence; only actual runtime query results may satisfy the
live-runtime claim.

</specifics>

<deferred>
## Deferred Ideas

StarRocks cluster lifecycle management, object-store/catalog integration,
credential routing, external table DDL, distributed execution, and ABI freeze
remain out of scope for Phase 43.

</deferred>
