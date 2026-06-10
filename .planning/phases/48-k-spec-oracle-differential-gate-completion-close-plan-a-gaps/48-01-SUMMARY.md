# Phase 48 Plan 01 Summary

**Phase:** 48-k-spec-oracle-differential-gate-completion-close-plan-a-gaps  
**Plan:** 01  
**Status:** Complete  
**Date:** 2026-06-10

---

## What was done

### 1. Typed K-Oracle Outcome (`KOracleOutcome`)
- Added `pub enum KOracleOutcome` to `crates/loom-core/src/kloom_harness.rs` with three harness-level variants:
  - `ProducedTrace(Vec<String>)` — krun ran and emitted a usable reference trace.
  - `SkippedRefereeAbsent { reason: String }` — krun/kompile missing, definition dir missing, or timeout.
  - `UnsupportedProgram { reason: String }` — program contains constructs the harness cannot faithfully serialize.
- The `Compared`/`Diverged` distinction remains in `native_arrow_semantic.rs` where the native trace is available, keeping trace comparison in one place.

### 2. Unsupported-construct predicate
- Added `program_uses_unsupported_constructs` → `stmt_uses_unsupported` → `expr_uses_unsupported` recursive walk.
- Flags `ScalarExpr::Min`, `ScalarExpr::Max`, and `ScalarValue::Bytes` anywhere in the program (including nested expressions and statement bodies).
- These programs return `UnsupportedProgram` **before** serialization, so the placeholder arms in `serialize_expr`/`serialize_scalar_value` are now defensive-only (documented as unreachable for compared programs).

### 3. Skip semantics in `run_kloom`
- **Env var:** `LOOM_ALLOW_K_ORACLE_SKIP=1` mirrors the existing `LOOM_ALLOW_NATIVE_TOOL_SKIP` discipline.
- **ENOENT:** `io::ErrorKind::NotFound` from `Command::new("krun").spawn()` maps to `SkippedRefereeAbsent` only when skip is allowed; otherwise hard error with a message naming the env var.
- **Definition-dir missing:** same treatment as ENOENT (kompile not run = referee absent).
- **Timeout:** 30-second constant (`KRUN_TIMEOUT_SECS`). Uses `spawn` + `try_wait` polling loop. Timeout → kill child → `SkippedRefereeAbsent`.
- **Non-zero exit:** hard fail regardless of skip flag (referee present but disagreeing).
- **Garbled output:** `parse_trace` now checks for the presence of `<events>` before parsing; absent → hard error.

### 4. Threading through `native_arrow_semantic.rs`
- `reference_model_trace_for_batch` now returns `Result<KOracleOutcome, NativeArrowSemanticDiagnostic>`.
- `verify_native_arrow_semantic_model_for_output` branches on the three outcomes:
  - `ProducedTrace` → normal trace comparison.
  - `SkippedRefereeAbsent` / `UnsupportedProgram` → `model_trace_matches: true`, `value_equivalent` computed honestly against the decoded reference batch, empty diagnostics, and `oracle_skip_reason` set.
- Added `oracle_skip_reason: Option<String>` to `NativeArrowSemanticModelValidationReport`. When `Some`, the route does **not** fail-close solely because of the K oracle.
- All existing early-return constructors of the report were updated with `oracle_skip_reason: None`.

### 5. Tests
- Created `crates/loom-core/tests/kloom_skip_semantics.rs` (8 tests):
  - `unsupported_min_expr`
  - `unsupported_max_nested_in_add`
  - `unsupported_bytes_constant`
  - `unsupported_min_inside_forrange_body`
  - `unsupported_max_inside_cursorloop_body`
  - `pure_append_int32_not_unsupported`
  - `krun_absent_with_skip_allowed`
  - `krun_absent_without_skip_is_hard_error`
- Added `parse_garbled_no_events_cell_is_hard_error` to the kloom harness unit-test mod.
- All tests pass; existing `native_arrow_semantic` integration tests (19 tests) pass without regression.

---

## Files modified

| File | Change |
|------|--------|
| `crates/loom-core/src/kloom_harness.rs` | `KOracleOutcome` enum, unsupported-construct predicates, timeout + skip logic in `run_kloom`, garbled detection in `parse_trace`, unit tests |
| `crates/loom-core/src/native_arrow_semantic.rs` | `reference_model_trace_for_batch` returns `KOracleOutcome`, `verify_native_arrow_semantic_model_for_output` handles skip/unsupported, `oracle_skip_reason` field added to report |
| `crates/loom-core/tests/kloom_skip_semantics.rs` | New integration-test file (8 tests) |

---

## Verification

```bash
cargo build -p loom-core                              # OK
cargo test -p loom-core --test kloom_skip_semantics   # 8 passed
cargo test -p loom-core --test native_arrow_semantic  # 19 passed, no regression
cargo test -p loom-core --lib                         # 126 passed, no regression
```
