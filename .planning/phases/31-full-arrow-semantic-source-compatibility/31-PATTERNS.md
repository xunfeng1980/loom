# Phase 31 Patterns

## Preferred Patterns

- Treat Arrow `ArrayData` as the semantic tree: buffers + nulls + child arrays +
  data type.
- Keep source SDKs adapter-local. Source adapters emit Loom-owned facts and
  Arrow semantic artifacts.
- Use property-style fixture matrices for schema coverage instead of one-off
  scalar examples.
- Separate semantic preservation from physical encoding preservation.
- Separate source compatibility from query engine support.
- Accepted reports require verifier acceptance and oracle/source equality.

## Anti-Patterns

- Adding one `LayoutNode` variant per Arrow type.
- Serializing opaque bytes without verifier visibility and calling it accepted.
- Treating DuckDB SQL success over a subset as arbitrary schema compatibility.
- Treating Vortex canonical raw rows as Vortex encoding-shape preservation.
- Letting Lance/Parquet/Vortex SDK types leak into `loom-core`, `loom-ffi`, or
  source-neutral public contracts.

## Naming

- New container: `LMC2`.
- New payload: `LMA1` (Loom Arrow semantic payload v1).
- Suggested crate/module names:
  - `loom_core::arrow_semantic`
  - `loom_core::arrow_semantic_codec`
  - `loom_core::arrow_semantic_verifier`
  - `scripts/full-arrow-semantic-compatibility-test.sh`
