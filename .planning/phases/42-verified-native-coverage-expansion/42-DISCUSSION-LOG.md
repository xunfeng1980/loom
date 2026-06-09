# Phase 42 Discussion Log

## 2026-06-09

Phase 42 starts after Phase 41 verified-lineage closeout and a Phase 38
remediation that made the Lean modeled theorem use a program-level
`Verified -> execProgram finished -> reads in bounds` bridge.

Working assumptions:

- Use the recommended route from the roadmap: coverage expansion before
  StarRocks integration and ABI freeze.
- Keep source format semantics, native execution eligibility, and interpreter
  fallback as separate columns in the matrix.
- Default source artifacts are `LMC2(LMA1)`; direct `LMA1` entries are legacy
  regression bridge evidence only.
- Prefer adding matrix/gate evidence over claiming broad new source-native
  execution.

Open risks:

- Existing Vortex source semantic tests are narrower than existing Phase 21/28
  coverage rows.
- Lance/Parquet already roundtrip several Arrow schema shapes, but native
  support is narrower than source semantic acceptance.
- A single row can be accepted for source semantic emission while still
  interpreter-only for native execution.
