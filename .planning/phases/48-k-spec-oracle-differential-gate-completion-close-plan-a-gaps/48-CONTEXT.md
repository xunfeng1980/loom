# Phase 48: K Spec-Oracle Differential Gate Completion (方案 A, narrowed) - Context

**Gathered:** 2026-06-10
**Status:** Ready for planning
**Source:** PRD Express Path (user-provided 方案 A document) + user scope decisions 2026-06-10

<domain>
## Phase Boundary

Complete the reconciliation core of Plan-A (方案 A) on top of the already-landed kloom v4 K Framework spec-oracle (commit 77d1bc4). **User-narrowed scope (2026-06-10):**

- **No Rust L2Core interpreter leg.** The deleted ReferenceExecutor is NOT resurrected and no third trace leg is added. K semantics plays both roles (方案 1 + 方案 2 step one): it is the independent spec oracle AND the sole reference executor in the validation layer. The differential is a **two-way K ↔ native per-builder-event mirror reconciliation** (镜像对账).
- **Two-step K trajectory:** this phase keeps K out of the production binary path (krun invoked from test/CI harness only). Additionally, this phase must produce **kompile LLVM-backend feasibility evidence** — kloom.k compiles under the K LLVM backend and runs the existing semantics corpus with traces identical to the Haskell backend — recorded as groundwork for a future "extract production interpreter from K" phase. Not wired into production.
- **Reconciliation core only:** krun-absent skip semantics, per-shape native-route disable on divergence, and mirror-reconciliation hardening. Generated near-exhaustive corpus and the semantics sync-checklist gate are DEFERRED to a later phase.

Gap analysis (2026-06-10, against commit 77d1bc4) for the in-scope items:

1. **krun-absent skip semantics MISSING** — `kloom_harness.rs` treats krun unavailability as a hard error; Plan-A decision 3 requires "referee absent = recorded skip, production gate not blocked". Divergence or garbled K output stays HARD FAIL (referee present and disagreeing ≠ absent).
2. **Per-shape native-route disable MISSING** — on native↔K trace divergence, only a `NativeModelTraceMismatch` diagnostic is recorded (`native_arrow_semantic.rs:~1311`); Plan-A adjudication requires the divergent shape's native route to be disabled for the process lifetime (fall back to interpreter) using existing schema/cache fingerprint identity (`RuntimeCacheKey`, `schema_fingerprint`).
3. **Harness placeholder holes poison the gate** — `ScalarExpr::Min`/`Max` lower to placeholder `0` (`kloom_harness.rs:~276-284`), bytes constants likewise (~309-313). Silent placeholders can produce false agreement/false divergence inside the very comparison this phase hardens. Narrow fix only: classify such programs explicitly unsupported-for-K-oracle (skip-with-reason), never silently compare. Full Min/Max K rules are deferred.
4. **LLVM-backend feasibility evidence MISSING** — kloom currently kompiles with the Haskell backend only; no evidence the LLVM backend (the path to a fast extracted interpreter) accepts kloom.k.

</domain>

<decisions>
## Implementation Decisions

### Adjudication rules (locked, 方案 A §3-4 + user narrowing)
- K semantics (kloom) is the spec baseline; **native output is the system-under-test**. Two-way K ↔ native mirror reconciliation per builder event (reuse existing `TracedOutputBuilder`-derived trace comparison in `native_arrow_semantic.rs`). No Rust interpreter trace leg.
- Native↔K divergence → fail-closed for that run AND disable the native route for that shape (keyed by existing schema/cache fingerprint identity) with interpreter fallback. Disable persists for the process lifetime (in-process registry; no new persistent format, no public ABI change).
- Disabled-shape decisions must be observable (diagnostic/route report), and must prevent native cache/replay admission for that shape, consistent with Phase 43.2 admission discipline.

### K failure = referee absent (locked, 方案 A 决策 3)
- krun/kompile binary missing or timeout ⇒ explicit skip (recorded, observable, distinguishable from "compared and matched"); production native path and release gates proceed.
- K present but output unparseable, or traces compared and divergent ⇒ HARD FAIL (fail-closed). Never classify these as skip.
- CI with K installed runs strict (skip not allowed); skip-tolerance is for local/dev without K, expressed via env-var convention consistent with `LOOM_ALLOW_NATIVE_TOOL_SKIP` discipline.

### Scope red lines (locked)
- K never enters the production binary path this phase: external `krun` subprocess from test/CI harness code only; nothing K-related crosses the C ABI or reaches DuckDB.
- Native remains the only default execution route; performance path untouched.
- Lean `accepted_program_safe` retained untouched.
- No K reachability proofs.
- No correctness claims — safety/well-formedness and divergence detection only.

### Placeholder holes (locked, narrow)
- Programs whose L2Core AST contains constructs the kloom harness cannot faithfully serialize (Min/Max, bytes constants, any other placeholder lowering found) are classified explicitly unsupported-for-K-oracle: the harness returns a typed unsupported outcome (skip-with-reason in the gate), and never emits placeholder values into a compared trace.
- Implementing real min/max K rules is DEFERRED (out of scope this phase).

### kompile LLVM-backend feasibility (locked, user "两步走" decision)
- Deliverable: script-driven evidence that kloom.k kompiles with `--backend llvm` and `krun` (or the kompiled interpreter binary) reproduces the same traces as the Haskell backend over the existing 12-test semantics corpus.
- Evidence is recorded in the kloom build script + a short findings doc; it does NOT gate the release pipeline strictly (skip-aware like other optional-toolchain gates) and does NOT touch production code.

### Deferred (explicit, user decision 2026-06-10)
- Generated near-exhaustive corpus over the kloom-modeled matrix.
- Semantics sync-checklist gate (now three places after dropping the Rust interpreter leg: kloom.k / native codegen / Lean).
- Real min/max K rules; modeling UInt32/UInt64/Bytes/RowIndex in kloom.
- Extracting the K LLVM-backend interpreter into an actual Loom execution mode (next phase candidate).

### Claude's Discretion
- Exact module/file layout for the shape-disable registry and skip reporting (prefer integrating with existing `duckdb_runtime`/route-report fallback policy over inventing a parallel mechanism).
- Typed outcome enum shape for the K oracle (e.g., Compared/Diverged/SkippedRefereeAbsent/UnsupportedProgram) as long as the four states are distinguishable and divergence is fail-closed.
- How the LLVM-backend evidence integrates into `contrib/kloom/scripts/` (separate script vs kloom-diff.sh flag).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing kloom integration (the substrate this phase extends)
- `contrib/kloom/src/kloom.k` — K semantics v4
- `crates/loom-core/src/kloom_harness.rs` — L2Core→kloom serialization, krun invocation, trace parsing (placeholder holes ~276-284, ~309-313; hard-error krun handling ~322-357)
- `crates/loom-core/src/native_arrow_semantic.rs` — `reference_model_trace_for_batch` / `native_model_trace_for_batch` / trace comparison and `NativeModelTraceMismatch` (~1244-1314)
- `contrib/kloom/scripts/kloom-diff.sh` — kompile/krun/cargo differential gate script
- `scripts/model-rust-interpreter-consistency-test.sh` — Phase 39 successor gate
- `.github/workflows/ci.yml` — kloom Spec-Oracle CI job (~204-246)
- `contrib/kloom/tests/semantics/` — 12 anchor tests
- `contrib/kloom/docs/SEMANTICS.md`, `contrib/kloom/README.md` — stale "v0/pure-append" wording needs sync with this phase's outcome

### Shape/cache identity and route policy for per-shape disable
- `crates/loom-core/src/runtime_abi.rs` — `RuntimeCacheKey`, `schema_fingerprint`, `production_lowering_fingerprint` (~451-460)
- Phase 43.2 route reports / cache-replay admission discipline (`.planning/phases/43.2-*/`, `loom-native-melior` route code) — the disable check must compose with this, not bypass it

### Lean leg (retain, untouched)
- `formal/lean/LoomCore.lean` — `accepted_program_safe` (~1105)

### Project discipline
- `.planning/ROADMAP.md` Phase 48 entry — goal + non-goals
- `CLAUDE.md` — dependency boundaries (K never in loom-core build graph; loom-core/loom-ffi Vortex-free)

</canonical_refs>

<specifics>
## Specific Ideas

- 方案 1 (independent oracle) needs only K's EXECUTION ability — krun an L2Core program, emit builder-event traces comparable to production output. No proofs, no reachability logic, no proof-friendly rule shapes. This is the lightest half of the KEVM usage pattern.
- 方案 2 (extract production interpreter) is the future step: K semantics becomes the default-running interpreter implementation in some mode. This phase only proves backend feasibility (LLVM backend kompile + identical corpus traces), because integration/performance/FFI demands are much higher.
- The differential gate's value is statistical independence: the "K faithful to L2Core intent" seam is independent of the Rust/native implementation seam.

</specifics>

<deferred>
## Deferred Ideas

- Generated near-exhaustive corpus (was Plan-A step 4) — next phase candidate.
- Three-place semantics sync-checklist gate (kloom.k / native codegen / Lean) — next phase candidate.
- Real min/max K rules; kloom coverage for UInt32/UInt64/Bytes/RowIndex.
- K LLVM-backend extracted interpreter as an actual Loom execution mode (方案 2 full form).
- K reachability-logic second soundness proof (方案 A 决策 2 可选增强).
- Any persistent (cross-process) native-route disable store.

</deferred>

---

*Phase: 48-k-spec-oracle-differential-gate-completion-close-plan-a-gaps*
*Context gathered: 2026-06-10 via PRD Express Path + user scope decisions (no-Rust-interpreter-leg, 两步走 K trajectory, reconciliation-core-only)*
