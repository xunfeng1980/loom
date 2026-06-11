---
phase: 51-sidecar-duckdb-decoupling-and-loom-self-ingress
gathered: 2026-06-11
status: ready
mode: auto-generated (infrastructure phase — smart discuss skipped)
---

<domain>
## Phase Boundary

**Goal:** Decouple the DuckDB sidecar path from `loom-container` so that DuckDB can read Parquet files with embedded Loom IR sidecars using only `loom-ir-core`. Reposition `loom-container` as the exclusive handler of the Loom native `.loom` format, and introduce `loom-self-ingress` as its IO boundary.

**Success Criteria:**
1. A new `loom-sidecar-ffi` (or feature-gated `loom-ffi` lean path) exports sidecar decode/verify/routing functions via C ABI, depending only on `loom-ir-core` and `loom-parquet-ingress` — zero dependency on `loom-container`.
2. The DuckDB extension can load this lean FFI surface and query a Parquet file with embedded `loom.sidecar.v1` metadata through `loom_scan(...)` without linking any container/codec/verifier modules.
3. A new `loom-self-ingress` crate wraps `loom-container` codecs for ingress/egress of `.loom` files — all `.loom`-format IO flows through this boundary.
4. `loom-cli` splits into two compilation units so the `sidecar embed` command compiles without `loom-container`.
5. Full workspace build and test pass with new dependency boundaries enforced.

**Non-goals:** No changes to the `.loom` container format. No changes to the MLIR/LLVM native lowering path.
</domain>

<decisions>
## Implementation Decisions

### the agent's Discretion
All implementation choices are at the agent's discretion — infrastructure refactoring phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions. Key principles:
- `loom-ir-core` must remain zero Arrow-dependency
- The lean sidecar-FFI path must not transitively pull in `loom-container`
- `loom-self-ingress` is the single IO boundary for `.loom` files
- Existing DuckDB extension continues to work via both paths
</decisions>

<code_context>
## Existing Code Insights

Key files and crates involved:
- `crates/loom-ir-core/` — Zero-dep IR crate (sidecar, routing, l2core_codec, full_verifier)
- `crates/loom-container/` — 19-module container crate (codecs, verifier, native lowering, lineage)
- `crates/loom-core/` — Re-export shim (38 `pub use` delegating to ir-core + container)
- `crates/loom-ffi/` — C ABI boundary (imports 20+ container modules via loom-core)
- `crates/loom-cli/` — CLI binary (imports from both ir-core and container)
- `ingress/loom-parquet-ingress/` — Parquet sidecar extract/embed (production code uses only ir-core)
- `contrib/duckdb-ext/` — C++ DuckDB extension (links `libloom_ffi.a`)

Phase 50 dependency analysis shows:
- DuckDB → loom-ffi → loom-container (hard, unavoidable currently)
- sidecar embed path only needs loom-ir-core + loom-parquet-ingress
- All container deps in loom-ffi are for decode/verify/native-lowering, not sidecar
</code_context>

<specifics>
## Specific Ideas

1. **Lean FFI approach:** Create `loom-sidecar-ffi` as a new `staticlib` crate exporting `extern "C"` functions for sidecar extract/verify/routing. Depends only on `loom-ir-core` and `loom-parquet-ingress`.
2. **Self-ingress crate:** Create `loom-self-ingress` wrapping `loom-container` codecs — provides `read_loom_file`, `write_loom_file`, `verify_loom_file` as the single IO boundary for `.loom` format.
3. **CLI split:** Extract sidecar embed into a separate binary or feature-gate container imports behind a `full` feature flag.
4. **DuckDB lean path:** Build a second FFI surface (`libloom_sidecar_ffi.a`) that the DuckDB extension can link alongside or instead of `libloom_ffi.a`.

Minimize breaking changes — existing DuckDB path through `loom-ffi` → `loom-container` must continue working.
</specifics>

<deferred>
## Deferred Ideas

None — pure infrastructure phase.
</deferred>
