---
phase: 52-container-split-loom-common-core-and-contrib-loom-container-legacy
gathered: 2026-06-11
status: ready
mode: auto-generated (infrastructure phase)
---

<domain>
## Phase Boundary

Split `crates/loom-container` (19 modules) into:
- `crates/loom-common` — 13 production-core modules (arrow_semantic*, native*, runtime_abi, artifact_verifier, l1_model, l2_kernel_registry, verifier)
- `contrib/loom-container` — 6 legacy modules (container_codec, layout_codec, table_codec, descriptor, verified_lineage, kloom_harness, fsst_params, alp_params, arrow_builder_output)

After split: DuckDB extension + native codegen depend only on loom-common. Legacy .loom format in contrib/.
</domain>

<decisions>
## Implementation Decisions

### the agent's Discretion
Pure infrastructure refactoring. Module categories:
- **Category A (→ loom-common):** arrow_semantic, arrow_semantic_codec, arrow_semantic_verifier, native_arrow_semantic, arrow_buffer_lowering, native_lowering, production_native_lowering, decode_dialect, runtime_abi, artifact_verifier, l1_model, l2_kernel_registry, verifier
- **Category B (→ contrib/loom-container):** container_codec, layout_codec, table_codec, descriptor, verified_lineage, kloom_harness, fsst_params, alp_params, arrow_builder_output

Key principles:
- Zero logic changes — only file moves and import path updates
- `loom-core` switches from `loom-container` to `loom-common` dependency
- `contrib/loom-container` depends on `loom-common` for shared types
- `cargo tree` confirms zero `contrib/loom-container` in production deps of loom-ffi, loom-native-melior, loom-sidecar-ffi
</decisions>

<code_context>
Key files to modify:
- crates/loom-container/src/lib.rs (module declarations)
- crates/loom-core/Cargo.toml (switch dep from container → common)
- crates/loom-core/src/lib.rs (re-export paths)
- ingress/loom-self-ingress/Cargo.toml (path to contrib)
- ingress/loom-vortex-ingress/Cargo.toml (path update)
- crates/loom-fixtures/Cargo.toml (path update)
- crates/loom-cli/Cargo.toml (full feature path update)
- Cargo.toml (workspace members)
</code_context>

<specifics>
No specific requirements — infrastructure phase. Follow existing codebase patterns.
</specifics>

<deferred>
None.
</deferred>
