# Phase 101 — Discussion Log

**Date:** 2026-06-11
**Mode:** default (interactive)

## Areas Discussed

### Area 1: 具体删除清单确认

- **Question:** Whether to also delete `contrib/loom-iceberg-binding` (nearly empty stub)
- **User choice:** 删除 (Recommended)
- **Decision:** Delete along with the other container-path crates.

### Area 2: loom-ffi 处理方式

- **Question:** Delete completely vs make it a stub re-exporting loom-sidecar-ffi
- **User choice:** 完全删除 (Recommended)
- **Decision:** Delete the entire `crates/loom-ffi` crate. `loom-sidecar-ffi` is the sole C ABI surface.

### Area 3: loom-fixtures 清理范围

- **Question:** How much to clean up — delete both container-dependent bin and tests, or just the bin
- **User choice:** 完全删除 container 相关 (Recommended)
- **Decision:** Delete `bin/emit_duckdb_payloads.rs` and `tests/descriptor_roundtrip.rs`. Change loom-fixtures to depend directly on loom-ir-core + loom-common instead of loom-core.

### Area 4: duckdb-ext 处理方式

- **Question:** Make LOOM_SIDECAR_ONLY default/only vs keep both modes
- **User choice:** LOOM_SIDECAR_ONLY 设为默认且唯一 (Recommended)
- **Decision:** Hardcode sidecar-only in CMakeLists.txt. Remove `#else` full-mode code from `loom_extension.cpp`. Remove `loom-ffi/include` reference.

### Area 5 (follow-up): kloom

- **Question:** Whether kloom needs sidecar-only adjustments
- **User choice:** Already fine — kloom locks onto L2Core IR, independent of packaging
- **Decision:** No changes needed. Verified zero container references in kloom source.

## Deferred Ideas

None.
