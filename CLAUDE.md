<!-- GSD:project-start source:PROJECT.md -->

## Project

**Loom — MVP0 (DuckDB demo)**

Loom is a distribution-oriented decoder IR: a deliberately non-Turing-complete,
total-function language whose only possible output is well-formed Apache Arrow
(full design in `design.md`). **This project is MVP0** — a runnable prototype that
proves the core chain end-to-end on a real engine: a single Vortex-encoded column
is decoded through Loom's declarative **L1 layout layer** plus one total-function
**L2 kernel (FSST)** into legal Arrow, handed to **DuckDB** via the Arrow C Data
Interface, and queried with SQL. It is for the author/systems audience evaluating
whether the L1/L2 + "output-as-typed-Arrow" idea actually works in practice.

**Core Value:** A user can run a SQL query in DuckDB over a Vortex-encoded column that was decoded
by the Loom interpreter, and get results that match Vortex's own decoder row-for-row.
If only one thing works, it is this end-to-end chain.

### Constraints

- **Tech stack**: Rust decoder core (Arrow via arrow-rs) — chosen for Vortex-ecosystem alignment and a path toward the eventual safety/memory model.
- **Tech stack**: C++ DuckDB extension (table function) — same language as DuckDB; thinnest possible wrapper over the Rust core.
- **Interop**: Arrow C Data Interface as the Rust↔C++ FFI boundary — zero-copy, language-neutral, matches the design's "output is Arrow" contract.
- **Dependencies**: Vortex (as reference decoder for verification and as the source of the encoding to decode); DuckDB (host engine + extension API); Apache Arrow (C Data Interface, arrow-rs).
- **Scope discipline**: MVP0 is a feasibility prototype, not production. Prefer the narrowest path that produces a correct, demonstrable SQL result over generality.

<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->

## Technology Stack

## Sub-Stack 1: Rust Library — Vortex Encoded Arrays → Arrow

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `vortex-array` | 0.74.0 | Core in-memory array model, encoding registry, Arrow conversion | The canonical Vortex crate; owns `ArrayRef`, encoding dispatch, and zero-copy Arrow round-trips. Directly models the L1 encodings MVP0 needs (bitpack, FOR, dict, RLE). |
| `vortex-fastlanes` | 0.74.0 | BitPacked and Frame-of-Reference (FOR) encoding implementations | SpiralDB's FastLanes port; provides `BitPackedEncoding` and `FoREncoding`. These are the two numeric L1 encodings MVP0 must decode. |
| `vortex-dict` | 0.74.0 | Dictionary encoding implementation | `DictEncoding` wraps a codes array + values array; required for L1 dictionary support. |
| `vortex-fsst` | 0.74.0 | FSST string encoding/decoding — the one L2 kernel in MVP0 | Wraps `fsst-rs`; exposes `FsstEncoding` and its decode path. This is the single L2 escape MVP0 exercises. |
| `fsst-rs` | 0.5.11 | Pure-Rust FSST symbol-table decompressor (transitive via vortex-fsst) | Maintained by the SpiralDB team (now vortex-data). `Decompressor` takes an 8-bit code stream and a symbol table, emits raw bytes. Zero external dependencies. |
| `arrow` (arrow-rs) | 58.3.0 | Arrow typed array builders, `ArrayData`, `ArrayRef`, `Schema` | The official Apache Arrow Rust implementation. `vortex-array` 0.74 depends on arrow-array/arrow-schema/arrow-cast at the **58.x** series. Must use the same major version to avoid duplicate type definitions across the FFI boundary. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `arrow-array` | 58.3.0 | Typed array builders (`Int32Builder`, `StringBuilder`, etc.) | Building the output Arrow array inside the decode path |
| `arrow-schema` | 58.3.0 | `Schema`, `Field`, `DataType` | Constructing the `FFI_ArrowSchema` to hand to DuckDB |
| `arrow-data` | 58.3.0 | `ArrayData` (the substrate for `to_ffi`) | Required by `arrow::ffi::to_ffi` |

### RLE Encoding

### How to Read a Vortex-Encoded Array

## Sub-Stack 2: Arrow C Data Interface FFI Export (Rust → C)

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `arrow::ffi` (feature `ffi`) | 58.3.0 | Exports `FFI_ArrowArray` + `FFI_ArrowSchema` across C ABI | Part of the official arrow-rs crate; no extra dependency. The `ffi` feature must be enabled explicitly. Exactly matches the Arrow C Data Interface specification. |

### Key API Pattern

#[no_mangle]

### What NOT to Use for FFI

- **`arrow2`**: A competing Arrow implementation with its own `ffi` module. Do not mix — `FFI_ArrowArray` from `arrow2` and from `arrow` are ABI-identical structs but Rust will see them as different types, causing confusing miscompilations.
- **`arrow-ipc`** for this step: IPC serialization is a different mechanism (serialize to bytes, then deserialize). For zero-copy C-boundary handoff, use the C Data Interface (`arrow::ffi`), not IPC.

## Sub-Stack 3: DuckDB C++ Extension — Table Function

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| DuckDB extension-template (C++) | targets DuckDB 1.5.x | CMake + CI scaffold for a loadable `.duckdb_extension` | The canonical starting point. Provides `ExtensionUtil::RegisterFunction`, CMake build wiring, and vcpkg integration. For MVP0 (no distribution needed) just use `make` locally. |
| DuckDB C++ API | 1.5.3 (latest) | `TableFunction`, `DataChunk`, `ClientContext`, `LogicalType` | The internal C++ API available inside an extension; gives the table-function bind/init/scan callback pattern and the `DataChunk` to fill. |

### Table Function Pattern (C++ side)

### Arrow C Data Interface Ingestion into DuckDB

### What NOT to Use

- **`duckdb-rs`** (the Rust client library): It is a Rust application-level client, not an extension SDK. Do not use it to build the extension.
- **`extension-template-rs`** (experimental Rust-only extension template): Only a few months old, still experimental, and does not support the full `TableFunction` bind/init/scan callback API needed for a scan-style table function as of June 2026. The C++ template path is more mature.
- **The deprecated `arrow` DuckDB extension**: Archived in DuckDB 1.3. Do not install or depend on it.

## Sub-Stack 4: Build / Link — Rust cdylib Consumed by C++ DuckDB Extension

### Approach: Static Library + cbindgen Header

# Rust Cargo.toml

# Link the Rust static library

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| cbindgen | 0.29.3 | Generates C header from `extern "C"` Rust functions | Zero-friction for simple C ABI. No need to maintain the header by hand. Runs in `build.rs`. |
| Rust `staticlib` crate type | — | Produces `.a` / `.lib` for linking into C++ extension | Avoids dylib-in-dylib complexity; the DuckDB extension is already the shared library boundary. |
| CMakeLists.txt (manual) | CMake 3.22+ | Links the static Rust output into the C++ extension | DuckDB's extension-template already uses CMake; just add one `find_library` + `target_link_libraries`. |

### Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `staticlib` + cbindgen | `cxx` crate | `cxx` is optimal when C++ types (std::string, std::vector) cross the boundary. For MVP0 the boundary is two raw C pointers (`FFI_ArrowArray*`, `FFI_ArrowSchema*`) — C ABI is sufficient and simpler. |
| `staticlib` + cbindgen | Corrosion (CMake-Rust integration) | Corrosion is excellent when you own the whole CMake project. For MVP0 using DuckDB's extension-template scaffold, invoking `cargo build` as an `ExternalProject` in CMake is simpler than pulling in Corrosion. |
| `staticlib` + cbindgen | `cdylib` | A `cdylib` inside another `cdylib` (DuckDB extension) works but requires runtime dylib resolution path configuration. `staticlib` is simpler and portable. |
| Raw C ABI | `duckdb-loadable-macros` / `quack-rs` | `quack-rs` v0.13 is actively maintained but generates a pure-Rust extension without a C++ wrapper layer. For MVP0 the design explicitly requires a C++ table function (thinnest wrapper) + Rust core. The two-language split is intentional. |

### What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `arrow2` | Parallel Arrow ecosystem, incompatible types with `vortex-array` which uses `arrow-rs` | `arrow` (arrow-rs) 58.x |
| `cdylib` nested in DuckDB extension `.duckdb_extension` | Runtime linker complexity, RPATH issues on macOS | `staticlib`, link at compile time |
| `cxx` for the Arrow FFI boundary | The boundary carries only C-ABI-compatible structs (`FFI_ArrowArray*`); `cxx` overhead not justified | `extern "C"` + cbindgen |
| `extension-template-rs` (pure Rust DuckDB extension) | Experimental; no mature DataChunk-level table function API; cannot easily intermix with Rust `vortex-array` | C++ extension-template + Rust staticlib |
| DuckDB `arrow` community extension (deprecated) | Archived as of DuckDB 1.3 | Use `arrow_scan` built-in or `nanoarrow` community extension |
| `vortex-file` / `vortex-ipc` | Full file-format container; MVP0 decodes a single in-memory column, not a file | `vortex-array` + encoding-specific crates only |

## Version Compatibility Matrix

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `vortex-array` 0.74.0 | `arrow` 58.x series | Vortex 0.74 pins arrow-array/arrow-schema at 58.x. Must match exactly in your workspace — use `[patch.crates-io]` if needed. |
| `arrow` 58.3.0 | `arrow-array` 58.3.0, `arrow-schema` 58.3.0, `arrow-data` 58.3.0 | All arrow-rs sub-crates are versioned together; pin the whole family to 58.3.0. |
| `fsst-rs` 0.5.11 | `vortex-fsst` 0.74.0 | `vortex-fsst` carries `fsst-rs` as a direct dependency; do not add `fsst-rs` independently unless you need its `Decompressor` API directly. |
| DuckDB 1.5.3 (C++ API) | extension-template (main branch) | Extension template targets the latest DuckDB stable. Pin `DUCKDB_GIT_VERSION=v1.5.3` in the Makefile for reproducibility. |
| cbindgen 0.29.3 | Rust 1.87+ (MSRV of quack-rs; cbindgen itself is more permissive) | Use `0.29` in `[build-dependencies]`; patch in `Cargo.lock`. |

## Installation Sketch

# Rust Cargo.toml (decoder crate)

# C++ extension CMakeLists.txt additions

## Sources

- [vortex-array on docs.rs](https://docs.rs/vortex-array) — version 0.74.0 confirmed, arrow-rs 58.x dependency
- [vortex-data/vortex GitHub](https://github.com/vortex-data/vortex) — crate list, 0.74.0 release June 2, 2026
- [vortex-data/vortex releases](https://github.com/vortex-data/vortex/releases) — version history confirmed
- [fsst-rs on docs.rs](https://docs.rs/fsst-rs/latest/fsst/) — version 0.5.11, SpiralDB maintainership, `Decompressor` API
- [arrow-rs releases](https://github.com/apache/arrow-rs/releases) — 58.3.0 released May 12, 2026 (latest)
- [arrow::ffi docs](https://docs.rs/arrow/latest/arrow/ffi/index.html) — `FFI_ArrowArray`, `FFI_ArrowSchema`, `to_ffi`, feature `ffi`
- [Apache Arrow FFI source](https://arrow.apache.org/rust/src/arrow_array/ffi.rs.html) — `to_ffi` signature and memory model
- [cbindgen crates.io](https://crates.io/crates/cbindgen) — 0.29.3 released 2026-05-28
- [DuckDB 1.5.0 announcement](https://duckdb.org/2026/03/09/announcing-duckdb-150) — DuckDB version history
- [DuckDB extension-template GitHub](https://github.com/duckdb/extension-template) — C++ extension scaffold
- [DuckDB extension-template-rs GitHub](https://github.com/duckdb/extension-template-rs) — experimental Rust template (noted as NOT recommended for MVP0)
- [quack-rs GitHub](https://github.com/tomtom215/quack-rs) — v0.13.0, DuckDB 1.4–1.5 support, pure-Rust (noted as alternative, not recommended for MVP0 architecture)
- [duckdb/arrow arrow_scan_ipc.cpp](https://github.com/duckdb/arrow/blob/main/src/arrow_scan_ipc.cpp) — `ArrowArrayStream` consumption pattern in DuckDB C++
- [DuckDB Arrow integration blog](https://duckdb.org/2021/12/03/duck-arrow) — `arrow_scan` built-in accepting `ArrowArrayStream*`
- [DuckDB Arrow IPC 2025](https://duckdb.org/2025/05/23/arrow-ipc-support-in-duckdb) — nanoarrow migration, 1.3 deprecation of old arrow extension
- [vortex-data/duckdb-vortex GitHub](https://github.com/vortex-data/duckdb-vortex) — CMake + C++ + Rust architecture confirmation
- [corrosion-rs/corrosion GitHub](https://github.com/corrosion-rs/corrosion) — CMake-Rust integration alternative (considered, not recommended for MVP0)

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
