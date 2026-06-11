# Phase 101: Retain only Loom sidecar mode — Context

**Gathered:** 2026-06-11
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase strips the Loom project down to the sidecar-only model described in `docs/repositioning.md`. It removes the container path (`.loom` files, LMC1/LMP1/LMT1 codecs), the full C ABI (`loom-ffi`), the native `.loom` file IO boundary (`loom-self-ingress`), and the Iceberg binding stub. It cleans up `loom-core` re-exports, `loom-fixtures` container-dependent tests, and hardcodes `LOOM_SIDECAR_ONLY` as the only mode in the DuckDB extension.

What remains: a single code path — Parquet/Vortex/Lance host files with Loom sidecar overlay → `loom-sidecar-ffi` (lean C ABI) → 4-gate routing → Loom-native or host-native-reader fallback.
</domain>

<decisions>
## Implementation Decisions

### Deletion
- **D-01:** Delete `contrib/loom-container` (loom-container-legacy) — LMC1/LMP1/LMT1 codecs, descriptor, artifact_verifier, verifier.
- **D-02:** Delete `ingress/loom-self-ingress` — native `.loom` file IO boundary (container path only).
- **D-03:** Delete `crates/loom-ffi` — full C ABI surface (replaced by `loom-sidecar-ffi`).
- **D-04:** Delete `contrib/loom-iceberg-binding` — stub crate, no consumer, sidecar model doesn't need it.

### Modification
- **D-05:** `crates/loom-core` — remove `loom-container-legacy` dependency and all container re-exports (container_codec, descriptor, layout_codec, table_codec, verified_lineage, artifact_verifier, verifier, arrow-*, fsst, ron, serde, fnv).
- **D-06:** `crates/loom-fixtures` — delete container-dependent code: `bin/emit_duckdb_payloads.rs` (uses container_codec/layout_codec/table_codec), `tests/descriptor_roundtrip.rs` (uses descriptor/layout_codec). Change `loom-core` dep to direct `loom-ir-core` + `loom-common` deps.
- **D-07:** `contrib/duckdb-ext` — CMakeLists.txt: remove `option(LOOM_SIDECAR_ONLY ...)`, hardcode sidecar-only path; remove LLVM/MLIR runtime linkage block; remove `#else` full-mode code from `loom_extension.cpp`; remove `loom-ffi/include` include path.
- **D-08:** Root `Cargo.toml` — remove workspace members: `contrib/loom-container`, `ingress/loom-self-ingress`, `crates/loom-ffi`, `contrib/loom-iceberg-binding`.

### No Changes
- **D-09:** `contrib/kloom` — already sidecar-compatible. kloom locks onto the L2Core decode IR, independent of packaging. No container references exist.
- **D-10:** `crates/loom-ir-core`, `crates/loom-common`, `crates/loom-sidecar-ffi`, `ingress/loom-*` (except self-ingress), `crates/loom-cli`, `crates/loom-fixtures` (post-cleanup), `crates/loom-native-melior` — unchanged, form the sidecar path.

### the agent's Discretion
- Exact ordering of operations (delete before modify to catch compile errors early).
- Test and release script updates to remove references to deleted crates/paths.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Repositioning
- `docs/repositioning.md` — Full sidecar model architecture: Decision One (separate IR from container) + Decision Two (sidecar overlay, host-native-reader fallback)

### Sidecar Implementation
- `crates/loom-ir-core/src/sidecar.rs` — SidecarOverlay, ChunkBinding, deterministic binary encode/decode, FNV-1a content-hash
- `crates/loom-ir-core/src/sidecar_routing.rs` — 4-gate fail-closed routing logic
- `crates/loom-sidecar-ffi/src/ffi.rs` — Lean C ABI entry points (extract, verify, route, free)
- `crates/loom-parquet-ingress/src/sidecar_parquet.rs` — Parquet sidecar extract/embed via KeyValue metadata

### DuckDB Extension
- `contrib/duckdb-ext/CMakeLists.txt` — Build config with LOOM_SIDECAR_ONLY option
- `contrib/duckdb-ext/loom_extension.cpp` — C++ table function, sidecar-only mode behind `#ifdef LOOM_SIDECAR_ONLY`

### Build
- `Cargo.toml` — Workspace members list
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/loom-sidecar-ffi` — already the production sidecar C ABI surface; no changes needed
- `contrib/duckdb-ext` — already has working LOOM_SIDECAR_ONLY=ON code path; just need to make it default/only

### Integration Points
- `crates/loom-core` depends on `loom-container-legacy` — breaking this requires ensuring downstream crates (loom-fixtures) don't rely on those re-exports
- `contrib/duckdb-ext/CMakeLists.txt` builds either `libloom_ffi.a` or `libloom_sidecar_ffi.a` — simplify to always build the latter
- Root `Cargo.toml` workspace member list — remove deleted crate paths
</code_context>

<specifics>
## Specific Ideas

- The sidecar path is already fully functional: `loom-sidecar-ffi` → 4-gate routing → Loom-native or fallback. This phase removes the dead code, not builds new capability.
- Release scripts under `scripts/` may reference deleted crates — update or remove those references.
- `LOOM_SIDECAR_ONLY` build of DuckDB extension must be tested after changes to ensure the extension still loads and functions.
</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.
</deferred>

---

*Phase: 101-retain-only-loom-sidecar-mode-remove-container-path-legacy-c*
*Context gathered: 2026-06-11*
