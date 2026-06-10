# Phase 48 Plan 02 Summary

**Phase:** 48-k-spec-oracle-differential-gate-completion-close-plan-a-gaps  
**Plan:** 02  
**Status:** Complete  
**Date:** 2026-06-10

---

## What was done

### 1. Disabled-shapes registry
- Added `static NATIVE_ROUTE_DISABLED_SHAPES: OnceLock<Mutex<HashSet<String>>>` to `crates/loom-native-melior/src/jit.rs`.
- Helpers:
  - `is_shape_disabled(schema_fingerprint: &str) -> bool`
  - `disable_shape(schema_fingerprint: &str)`
  - `reset_disabled_shapes()` — `#[doc(hidden)]` pub fn for test cleanup.
- Keyed by `schema_fingerprint` from `NativeArrowSemanticCodegenSupportReport`, composing correctly with Phase 43.2 cache admission.

### 2. Pre-check (fast fallback)
- Inserted in `execute_arrow_semantic_codegen_production_route_inner` after `support.is_supported()` and before JIT execution.
- If `is_shape_disabled(&support.schema_fingerprint)` → returns immediately with:
  - `status = fallback_or_fail_closed(policy)`
  - `cacheable = false`
  - `replay_evidence = None`
  - `NativeShapeDisabled` diagnostic
- No JIT runs, no krun invoked for already-disabled shapes.

### 3. Post-validation disable hook
- Inserted in `validate_arrow_semantic_codegen_production_route_output_with_cancellation` after `execution` is obtained.
- Detects **genuine divergence**: `execution.validation().is_some_and(|v| v.oracle_skip_reason.is_none() && v.diagnostics().iter().any(|d| d.code == NativeModelTraceMismatch))`.
- On divergence: calls `disable_shape(&execution.schema_fingerprint)`, forces `cacheable = false`, `replay_evidence = None`, status to fallback/fail-closed, and emits `NativeShapeDisabled` diagnostic.
- **Skip/unsupported outcomes do NOT disable**: the hook explicitly checks `oracle_skip_reason.is_none()`.

### 4. Diagnostic code
- Added `NativeShapeDisabled` to `NativeBackendDiagnosticCode` in `backend.rs` with string `"native-shape-disabled"`.

### 5. Tests
- Created `crates/loom-native-melior/tests/native_arrow_semantic_shape_disable.rs`:
  - `registry_disable_and_check` — always runs; tests the registry directly.
  - `divergence_disables_shape_and_fails_closed` — `#[cfg(feature = "melior")]`; mutates JIT output validity buffer to force trace divergence, asserts disable + non-cacheable + diagnostic.
  - `pre_check_fast_fallback_on_disabled_shape` — `#[cfg(feature = "melior")]`; artificially disables shape before route, asserts pre-check short-circuit.
  - `skip_does_not_disable_shape` — `#[cfg(feature = "melior")]`; clean run must not disable.
- Existing `production_arrow_semantic_jit` tests pass without regression.

---

## Files modified

| File | Change |
|------|--------|
| `crates/loom-native-melior/src/backend.rs` | Added `NativeShapeDisabled` to `NativeBackendDiagnosticCode` |
| `crates/loom-native-melior/src/jit.rs` | Registry static + helpers, pre-check in `execute_arrow_semantic_codegen_production_route_inner`, post-validation disable hook in `validate_arrow_semantic_codegen_production_route_output_with_cancellation` |
| `crates/loom-native-melior/tests/native_arrow_semantic_shape_disable.rs` | New integration-test file |

---

## Verification

```bash
cargo check -p loom-native-melior                              # OK
cargo test -p loom-native-melior --test native_arrow_semantic_shape_disable  # 1 passed (registry)
cargo test -p loom-native-melior --test production_arrow_semantic_jit        # 1 passed, no regression
```

Full melior-gated tests require `cargo test -p loom-native-melior --features melior --test native_arrow_semantic_shape_disable` (CI path).
