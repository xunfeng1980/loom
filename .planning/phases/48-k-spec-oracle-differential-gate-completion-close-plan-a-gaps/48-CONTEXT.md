# Phase 48: K Spec-Oracle Differential Gate Completion (方案 A) - Context

**Gathered:** 2026-06-10
**Status:** Ready for planning
**Source:** PRD Express Path (user-provided 方案 A document: "Lean 守核心命题 ‖ K 主导独立 oracle 与差分对账")

<domain>
## Phase Boundary

Complete the Plan-A (方案 A) formal assurance scheme on top of the already-landed kloom v4 K Framework spec-oracle (commit 77d1bc4). Loom currently has the vertical leg (Lean `accepted_program_safe` over the abstract L2Core model) and a partially-built horizontal leg (K oracle replacing the Phase 39 ReferenceExecutor). This phase closes the remaining horizontal-redundancy gaps so Loom moves from "single implementation + self-reconciliation" to "multi-implementation differential + small-kernel-backed proof".

The gap analysis (2026-06-10, against commit 77d1bc4) found:

1. **krun-absent skip semantics MISSING** — `kloom_harness.rs` treats krun unavailability/timeout as a hard error; Plan-A decision 3 requires "referee absent = recorded skip, production gate not blocked".
2. **Per-shape native-route disable MISSING** — on native↔K trace divergence, only a `NativeModelTraceMismatch` diagnostic is recorded (`native_arrow_semantic.rs:1311`); Plan-A adjudication requires the divergent shape's native route to be disabled (fall back to interpreter) using existing schema/cache fingerprint identity (`RuntimeCacheKey`, `schema_fingerprint`).
3. **Three-way reconciliation INCOMPLETE** — only K↔native trace comparison exists; the Rust interpreter trace must also be reconciled against K per builder event (K = spec baseline; interpreter and native are both systems-under-test).
4. **kloom harness semantic holes** — `ScalarExpr::Min`/`Max` are lowered to placeholder `0` (`kloom_harness.rs:276-284`); silent placeholders can cause false agreement/false divergence. Must become real K rules or explicit fail-closed unsupported.
5. **Corpus is 12 hand-written tests** — Plan-A step 4 requires near-exhaustive generated input coverage over the kloom-modeled L2Core matrix (the state space is small; differential coverage is cheap).
6. **Four-place sync checklist MISSING** — no gate enforces that an L2Core change updates kloom.k, the Rust interpreter, native codegen, and the Lean theorem together (Plan-A decision 4).

</domain>

<decisions>
## Implementation Decisions

### Adjudication rules (locked, from 方案 A §3-4)
- K semantics (kloom) is the spec baseline (规范基准 oracle); the Rust interpreter and native output are both systems-under-test.
- Any SUT diverging from K → fail-closed for that artifact/run.
- Native divergence additionally triggers: disable the native route for that shape (keyed by existing schema/cache fingerprint identity) and fall back to the interpreter. The disable decision must persist for the process lifetime (in-process registry is sufficient; no new persistent format).
- Reconciliation granularity is per builder event (reuse the existing `TracedOutputBuilder` trace infrastructure), not final-value-only comparison.

### K failure = referee absent (locked, from 方案 A 决策 3)
- If krun/kompile is unavailable or times out, production native path proceeds unaffected; the differential round is recorded as an explicit skip (consistent with the project's existing skip-aware discipline, e.g. `LOOM_ALLOW_NATIVE_TOOL_SKIP`).
- Skip must be explicit and observable (diagnostic/report), never silent, and must be distinguishable from "compared and matched".
- In CI where K is installed, the gate runs strict (no skip allowed); skip-tolerance is for local/dev environments without K.

### Scope red lines (locked, from 方案 A §2)
- K never enters the production path: no K dependency in `loom-core`'s build graph beyond invoking the external `krun` binary from test/CI harness code; nothing K-related crosses the C ABI or reaches DuckDB.
- Native remains the only default execution route; performance path untouched.
- Lean `accepted_program_safe` retained as-is; minimal sync only.
- No K reachability-logic proofs (future option, explicitly out of scope).
- No correctness claims — safety/well-formedness and divergence detection only; correctness remains oracle-based.

### Semantic holes (locked)
- `Min`/`Max` placeholder-to-0 lowering in `kloom_harness.rs` is forbidden: either implement real `min`/`max` rules in kloom.k v5, or make the harness fail-closed/explicitly-unsupported for programs containing them. Never a silent placeholder. (Real K rules preferred — keeps the differential surface maximal.)
- Same rule for any other placeholder lowering discovered (bytes/string constants currently lower to 0): explicit unsupported classification, not silent placeholder.
- Unsupported-type boundary is explicit: UInt32/UInt64/Bytes/RowIndex remain out of kloom's modeled matrix; programs containing them are classified out-of-scope for the differential gate (skip with reason), not silently compared.

### Corpus (locked, from 方案 A 实施次序 4)
- Systematic generator (deterministic enumeration and/or seeded randomized generation) over the kloom-modeled L2Core matrix: statement kinds × types (int32/int64/float32/float64/bool) × nullability × budget boundaries × in/out-of-bounds reads × loop shapes (forRange/cursorLoop incl. non-monotone) × expression operators (incl. new min/max).
- Generated corpus runs in the differential gate; hand-written 12 tests remain as anchors.
- Corpus generation must be deterministic/reproducible (seeded) so CI failures replay.

### Four-place sync checklist (locked, from 方案 A 决策 4)
- A script-enforced check tied to L2Core changes: when the L2Core surface changes (AST/semantics), the gate requires synchronized touch-points in kloom.k, the Rust interpreter, native codegen, and the Lean theorem statement, plus an explicit checklist doc update.
- Mechanism at Claude's discretion (e.g., fingerprint of L2Core AST surface recorded in a manifest that the four places' guards check against), but it must fail loudly on drift, not rely on convention.

### Claude's Discretion
- Exact module/file layout for the shape-disable registry and skip reporting.
- Whether the three-way comparison is one test path comparing three traces or composed pairwise comparisons, as long as both SUTs are reconciled against K per event and adjudication is K-baseline.
- Corpus generator implementation (Rust test-side enumeration preferred over new deps; proptest acceptable only if already in the dependency tree).
- How CI strictness vs local skip-tolerance is expressed (env var convention consistent with existing `LOOM_ALLOW_NATIVE_TOOL_SKIP` discipline).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing kloom integration (the substrate this phase extends)
- `contrib/kloom/src/kloom.k` — K semantics v4 (statements, expressions, budget, trace emission)
- `crates/loom-core/src/kloom_harness.rs` — L2Core→kloom serialization, krun invocation, trace parsing (incl. Min/Max placeholder hole at ~276-284)
- `crates/loom-core/src/native_arrow_semantic.rs` — `reference_model_trace_for_batch` / `native_model_trace_for_batch` / trace comparison (~1244-1314)
- `contrib/kloom/scripts/kloom-diff.sh` — kompile/krun/cargo differential gate script
- `scripts/model-rust-interpreter-consistency-test.sh` — Phase 39 successor gate
- `.github/workflows/ci.yml` — kloom Spec-Oracle CI job (lines ~204-246)
- `contrib/kloom/docs/SEMANTICS.md`, `contrib/kloom/README.md` — coverage claims (stale "v0/pure-append" wording needs sync)
- `contrib/kloom/tests/semantics/` — 12 anchor tests

### Shape/cache identity for per-shape disable
- `crates/loom-core/src/runtime_abi.rs` — `RuntimeCacheKey`, `schema_fingerprint`, `production_lowering_fingerprint` (~451-460)

### Lean leg (retain, minimal sync)
- `formal/lean/LoomCore.lean` — `accepted_program_safe` (~1105), `Verified` (~486)
- `scripts/full-verifier-test.sh` — Lean/verifier gate wiring

### Project discipline
- `.planning/phases/40-native-model-validation/` — Phase 40 native↔model validation contracts this phase upgrades
- `.planning/ROADMAP.md` Phase 48 entry — goal + non-goals

</canonical_refs>

<specifics>
## Specific Ideas

- Architecture (方案 A §3): production native (MLIR/LLVM/JIT) stays default; per-event traces flow to three-way differential reconciliation with K as spec baseline; Lean runs in parallel with an independent trust root.
- Phase positioning (方案 A §5): this replaces/strengthens Phase 39 (self-transcribed Rust reference executor → independent-lineage K oracle — already landed) and feeds Phase 40 (native↔model translation validation becomes native↔K reconciliation).
- The value proposition is statistical independence: the "K semantics faithful to L2Core intent" seam is a new seam, but it is independent of the Rust implementation's seam — that independence is the point (以太坊多客户端冗余的单项目版).

</specifics>

<deferred>
## Deferred Ideas

- K reachability-logic second soundness proof cross-validating Lean (方案 A 决策 2 可选增强) — future option, out of scope.
- Modeling UInt32/UInt64/Bytes/RowIndex in kloom — out of scope for this phase; boundary stays explicit fail-closed/skip.
- Full decode-surface coverage in Lean (FSST/ALP/dict/RLE/Vortex/Lance/Parquet interpreter paths) — acknowledged honest-list item (方案 A §8), not addressed here.
- Any persistent (cross-process) native-route disable store — in-process registry only this phase.

</deferred>

---

*Phase: 48-k-spec-oracle-differential-gate-completion-close-plan-a-gaps*
*Context gathered: 2026-06-10 via PRD Express Path (user 方案 A document)*
