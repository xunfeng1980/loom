# Phase 35: Native Arrow Semantic Execution - Patterns

**Gathered:** 2026-06-09
**Status:** Complete

## Patterns To Reuse

- Keep new native evidence inside `loom-core`; host adapters consume it later.
- Use stable diagnostic codes, JSON-ish paths, and fail-closed status reports.
- Preserve public API discipline: no new SQL function and no public FFI claim is
  needed for the first engine-neutral native proof.
- Use focused Rust tests first, then wire a script gate into the broad release
  verifier.
- Update ROADMAP/STATE/REQUIREMENTS only after each implemented slice is proven.

## Naming

- Module: `native_arrow_semantic`
- Backend identity: `loom-native-arrow-semantic`
- Positive report type: `NativeArrowSemanticExecutionReport`
- Diagnostic codes:
  - `verifier-rejected`
  - `unsupported-artifact`
  - `unsupported-payload`
  - `unsupported-batch-shape`
  - `unsupported-type`
  - `native-output-mismatch`

## Minimum Positive Matrix

| Shape | Expected |
|-------|----------|
| wrapped `LMC2(LMA1)` one-batch Int32/Int64/Float64/Boolean nullable | native supported and equivalent |
| direct `LMA1` same shape | native supported as regression bridge |
| Utf8 | fail closed unsupported type |
| Date32 | fail closed unsupported type |
| Struct/List | fail closed unsupported type |
| multi-batch | fail closed unsupported batch shape |
| malformed / verifier rejected | fail closed before native output |

## Verification Pattern

1. Generate in-test Arrow record batches.
2. Encode to `LMC2(LMA1)` or direct `LMA1`.
3. Execute through the native Arrow semantic module.
4. Assert backend, artifact kind, row count, column count, supported route, and
   array equality.
5. Assert unsupported shapes expose stable diagnostics and no output batch.
