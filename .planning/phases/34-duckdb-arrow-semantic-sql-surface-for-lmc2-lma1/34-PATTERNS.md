# Phase 34: DuckDB Arrow Semantic SQL Surface for LMC2(LMA1) - Patterns

**Gathered:** 2026-06-09
**Status:** Complete

## Patterns To Follow

### Keep Public And Internal FFI Separate

Public FFI stays in `loom.h` and continues to expose stable coarse entrypoints
such as `loom_decode`. DuckDB-only helpers live in
`crates/loom-ffi/include/loom_duckdb_internal.h` and are excluded from generated
public headers.

### Let Rust Own Artifact Semantics

Rust core already owns `LMC2` unwrap, `LMA1` Arrow semantic decode, verifier
acceptance, and fail-closed diagnostics. C++ should not duplicate wrapper or
Arrow semantic byte parsing.

### Let C++ Own DuckDB Vectors

The extension already knows how to map primitive Arrow C Data arrays into
DuckDB vectors. New work should reuse the existing `ArrowArray`/`ArrowSchema`
release discipline in `LoomScanState`.

### Preserve Existing SQL Entrypoint

`loom_scan(path)` remains the only public SQL surface. Bind-time schema
population and init-time projected decode may branch internally, but users
should not need a new function or option for `LMC2`.

### Fail Closed Before SQL Output

Malformed wrappers, malformed inner payloads, unsupported multi-batch payloads,
and unsupported field types should fail during bind/init with stable
`loom_scan:` diagnostics. Unsupported must not silently fall back to a wrong
single-column `value` result.

### Separate Queryability From Native Execution

Phase 34 proves DuckDB SQL queryability for Arrow semantic artifacts. Native
execution remains Phase 35 and must be described as separate evidence.

## Naming Guidance

- Use `LMC2(LMA1)` for the default product artifact.
- Use "direct LMA1 bridge" or "direct LMA1 regression" for compatibility
  fixtures.
- Prefer `arrow_semantic` in new internal FFI names.
- Avoid naming new source paths `lma1` if they emit wrappers.

## Verification Patterns

- Focused gate first, broad gate second.
- Positive tests for wrapped default artifacts and direct bridge artifacts.
- Negative tests for malformed and unsupported shapes.
- Header tests should check absence from public `loom.h`, not just presence in
  the internal header.

