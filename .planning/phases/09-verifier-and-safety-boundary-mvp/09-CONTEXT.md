# Phase 9: Verifier and Safety Boundary MVP - Context

**Gathered:** 2026-06-08
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 9 delivers a first-pass structural verifier for MVP0 layout and table payloads. It should reject malformed or unsafe `LayoutDescription` / `TableDescription` inputs before decode where practical, expose typed diagnostics to reviewers, and add negative regression coverage to the release gate.

This is not the formal Loom verifier. Totality proofs, ranking functions, non-termination proofs, and the full sandbox story remain later work.

</domain>

<decisions>
## Implementation Decisions

### Verifier Boundary
- **D-01:** Build a cheap structural verifier for MVP0 payload descriptions, not a full semantic/formal verifier.
- **D-02:** Do not duplicate every existing decode-time check. If an existing decode path is the authoritative check for an invariant, document that routing and keep one source of truth.
- **D-03:** Verifier scope should cover layout/table invariants that can be checked without executing the full decode, and route deeper data-dependent checks deliberately.

### Diagnostics
- **D-04:** Add a lightweight verifier diagnostic type rather than overloading `LoomDecodeError` for every verifier result.
- **D-05:** Diagnostics should include at least a stable code, human-readable message, and layout/table path such as `root.values` or `columns[label].root`.
- **D-06:** Diagnostic severity can stay simple for Phase 9: verifier failures are errors. Warnings can be deferred unless an implementation issue requires them.

### CLI Visibility
- **D-07:** `loom inspect` should display verifier status by default for binary payloads and descriptors.
- **D-08:** Passing input should show a concise `verification: pass`. Failing input should print human-readable diagnostics.
- **D-09:** Machine-readable JSON output is not required in Phase 9.

### Negative Fixtures
- **D-10:** Use curated negative fixtures/tests rather than fuzzing in Phase 9.
- **D-11:** Required negative coverage includes truncated payloads, invalid row/count relationships, non-monotonic run ends, unknown kernels, unsupported type/layout combinations, and table column mismatches.
- **D-12:** Fuzzing and property-based malformed input generation are deferred to a later safety-hardening phase.

### FFI and DuckDB Boundary
- **D-13:** Rust decode helpers and the FFI entry path should invoke verifier checks before producing Arrow when practical.
- **D-14:** DuckDB should benefit through the existing `loom_decode` path; no new DuckDB-specific verifier API is required in Phase 9.
- **D-15:** Failure must remain fail-closed: malformed payloads return typed Rust errors / FFI decode failure codes and must not cross into successful DuckDB scan output.

### Folded Todos
- **D-16:** Fold `.planning/todos/pending/cr-02-decode-for-non-bitpack-reference.md` into Phase 9 as a stale invariant audit. The original bug appears addressed by Phase 4's FOR-over-Raw handling, but Phase 9 should explicitly verify or close that historical warning so it no longer misleads future planning.

### the agent's Discretion

The agent may choose exact module names, test file layout, and diagnostic enum/code naming, provided the public behavior above is preserved and `loom-core` remains Vortex/FastLanes-free.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` — Phase 9 goal, requirements, and success criteria.
- `.planning/REQUIREMENTS.md` — `SAFE-01` through `SAFE-04`, plus `VERIFY-06`.
- `.planning/PROJECT.md` — active Phase 9 scope and formal-verifier boundary.
- `.planning/STATE.md` — current phase status and deferred safety notes.

### Prior Phase Context
- `.planning/phases/08-multi-column-table-output-and-arrow-stream-evaluation/08-CONTEXT.md` — table payload constraints, single-column compatibility, and release-gate expectations.
- `.planning/phases/07-human-readable-layout-descriptor-and-cli/07-CONTEXT.md` — CLI inspect/decode surface and descriptor constraints.
- `.planning/phases/06-mvp0-hardening-and-release-baseline/06-CONTEXT.md` — release-gate and build-hygiene baseline.

### Folded Todo
- `.planning/todos/pending/cr-02-decode-for-non-bitpack-reference.md` — stale FOR-over-non-BitPack warning to audit and close/update during Phase 9.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-core/src/error.rs` — existing typed decode errors; verifier diagnostics should interoperate without erasing these errors.
- `crates/loom-core/src/layout_codec.rs` — checked `LMP1` payload parser; malformed binary payload handling already starts here.
- `crates/loom-core/src/table_codec.rs` — checked `LMT1` table parser and `TableDescription::validate`; Phase 9 should reuse/extend this table validation path.
- `crates/loom-core/src/l1_model.rs` — `LayoutNode`, `LayoutDescription`, and decode helpers; this is the core tree the verifier will walk.
- `crates/loom-cli/src/main.rs` — `inspect` and `decode` commands; `inspect` is the recommended verifier status surface.
- `crates/loom-ffi/src/ffi.rs` — `loom_decode_inner` is the FFI decode ingress and should route malformed payloads through verifier/decode failure before Arrow export.
- `scripts/mvp0-verify.sh` and `scripts/duckdb-smoke-test.sh` — release-gate integration points for negative verifier regression coverage.

### Established Patterns
- `loom-core` must stay independent of Vortex/FastLanes dependencies.
- Existing malformed input paths return typed Rust errors rather than panics.
- Current binary payload codecs are internal fixture formats, so Phase 9 can improve validation without claiming long-term ABI stability.
- `loom inspect` is reviewer-facing and should stay concise by default.
- DuckDB integration should remain thin and continue to rely on Rust/FFI decode behavior.

### Integration Points
- Add verifier module in `loom-core`, likely exported from `lib.rs`.
- Call verifier from Rust decode helpers and/or FFI ingress before Arrow C Data export.
- Extend CLI `inspect` to show verifier result for single-column payloads, descriptors, and table payloads.
- Add negative verifier tests under `loom-core` and/or `loom-fixtures`, then wire them into `scripts/mvp0-verify.sh`.

</code_context>

<specifics>
## Specific Ideas

- Phase 9 should make the safety boundary visible without overselling it as the final formal verifier.
- Preferred CLI text is simple: `verification: pass` for success, diagnostics for failure.
- Paths in diagnostics matter because recursive layout trees make root-cause location otherwise hard to inspect.

</specifics>

<deferred>
## Deferred Ideas

- Full formal totality/termination verifier.
- Non-termination safety demo.
- Fuzzing/property-based malformed payload generation.
- Machine-readable verifier output such as JSON.

</deferred>

---

*Phase: 9-verifier-and-safety-boundary-mvp*
*Context gathered: 2026-06-08*
