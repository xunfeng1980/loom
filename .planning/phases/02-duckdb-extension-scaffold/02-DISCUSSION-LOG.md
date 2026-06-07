# Phase 2: DuckDB Extension Scaffold - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-07
**Phase:** 02-duckdb-extension-scaffold
**Areas discussed:** Arrow→DuckDB import path, How DuckDB is obtained, Extension build harness/layout, loom_scan stub interface (all four selected)

---

## Arrow → DuckDB import path

| Option | Description | Selected |
|--------|-------------|----------|
| ArrowArrayStream + arrow_scan | Wrap the single array in a one-shot stream; use DuckDB's built-in Arrow scan (public/stable) | ✓ |
| Internal ArrowToDuckDB() helper | Direct internal converter; include path/signature version-fragile | |
| Hand-copy into DataChunk | Manual buffer reads; most control, most code | |

**User's choice:** ArrowArrayStream + arrow_scan.
**Notes:** Sidesteps the uncertain internal `ArrowToDuckDB()` header flagged in research/SUMMARY.md.

## How DuckDB is obtained

| Option | Description | Selected |
|--------|-------------|----------|
| Prebuilt 1.5.3 lib + CLI | Link prebuilt release lib+headers; load into matching 1.5.3 CLI (allow_unsigned_extensions) | ✓ |
| Build from source @ v1.5.3 | Build duckdb from pinned tag; exact ABI, heavy | |

**User's choice:** Prebuilt 1.5.3 lib + CLI.

## Extension build harness/layout

| Option | Description | Selected |
|--------|-------------|----------|
| Hand-rolled minimal CMake | Small CMakeLists linking libloom_ffi.a + loom.h against prebuilt DuckDB; pairs with prebuilt | ✓ |
| Official extension-template | Standard, ABI-safe, but vendors duckdb + pairs with build-from-source | |

**User's choice:** Hand-rolled minimal CMake.
**Notes:** Coheres with the prebuilt-lib choice; lighter repo.

## loom_scan stub interface

| Option | Description | Selected |
|--------|-------------|----------|
| Single path/string arg (ignored) | loom_scan(VARCHAR) matching ROADMAP `loom_scan('test.bin')`; arg ignored while decode is stubbed | ✓ |
| No-arg loom_scan() | Zero-arg stub; add path arg in Phase 3 | |

**User's choice:** Single path/string arg, ignored for now.

---

## Claude's Discretion

- Exact CMake structure + extension directory location.
- The `ArrowArrayStream` wrapper implementation (schema/next/release callbacks).
- `allow_unsigned_extensions` load mechanics + extension version/platform metadata stamping.
- How the ABI/version pin is asserted in CI (load smoke-test).

## Deferred Ideas

None. Flagged for research: confirm an unsigned locally-built extension loads into the prebuilt 1.5.3 CLI on this platform; if not, fall back to build-from-source (the rejected D-02 alternative).
