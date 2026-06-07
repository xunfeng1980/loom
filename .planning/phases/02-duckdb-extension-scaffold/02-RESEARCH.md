# Phase 2: DuckDB Extension Scaffold — Research

**Researched:** 2026-06-07
**Domain:** DuckDB C++ extension ABI + hand-rolled CMake + Rust staticlib linkage + Arrow C Data Interface ingestion
**Confidence:** HIGH (critical claims verified directly from DuckDB v1.5.3 source on GitHub)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** `loom_scan` wraps `FFI_ArrowArray` + `FFI_ArrowSchema` in a one-shot `ArrowArrayStream` and feeds it to DuckDB's built-in `arrow_scan` machinery. Deliberately avoids `ArrowToDuckDB()` internal helper.
- **D-02:** Link against prebuilt DuckDB v1.5.3 release library + headers; load extension into the matching duckdb 1.5.3 CLI with `allow_unsigned_extensions`. Do NOT build DuckDB from source for MVP0.
- **D-03:** Hand-rolled minimal CMake (not the official extension-template). Links `libloom_ffi.a` + `crates/loom-ffi/include/loom.h`. Extension lives in a top-level `duckdb-ext/` or `cpp/` dir.
- **D-04:** `loom_scan(VARCHAR)` — single path/string argument, accepted but ignored in Phase 2.

### Claude's Discretion

- Exact CMake structure and the extension directory name/location.
- The precise `ArrowArrayStream` wrapper implementation (get_schema / get_next / release callbacks around the single decoded array).
- The local `allow_unsigned_extensions` load mechanics and how the extension's version/platform metadata is stamped to match the prebuilt 1.5.3 CLI.
- How the ABI/version pin is asserted in CI (e.g. a load smoke-test).

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DUCK-01 | A C++ DuckDB extension pinned to DuckDB v1.5.3 builds and loads | §Unsigned Load Feasibility + §DuckDB Acquisition + §Entry Points |
| DUCK-02 | `loom_scan` table function invokes the Rust decoder and adopts the imported Arrow array zero-copy | §ArrowArrayStream + arrow_scan Path + §Rust↔C++ Linkage |
| DUCK-03 | Extension releases the imported Arrow array on every teardown path — no leak, no double-free | §Release-Callback Ownership |
</phase_requirements>

---

## Summary

The core chain is: hand-rolled CMake builds a C++ shared library (`loom.duckdb_extension`), linked against the DuckDB C++ amalgamated headers (`libduckdb-src.zip`) and `libloom_ffi.a`. A 512-byte footer is appended post-build (via DuckDB's `scripts/append_metadata.cmake`) stamping the version as `"v1.5.3"` (not a git hash — hashes are only for dev builds) and the platform as `"osx_arm64"`. The CLI is invoked with `-unsigned` which bypasses signature checking but NOT version/platform metadata checking; the correctly-stamped footer makes the extension pass all metadata checks. The single C++ entry point is `loom_duckdb_cpp_init(ExtensionLoader &loader)` (name mangled from `DUCKDB_CPP_EXTENSION_ENTRY(loom, loader)`). Inside `loom_scan`'s table function, `loom_decode` is called, the two returned Arrow C structs are wrapped in a one-shot `ArrowArrayStream` (two C++ callbacks: `get_schema` and `get_next`), and `arrow_scan` is invoked via DuckDB's catalog, which pulls the rows into the query engine.

**Primary recommendation:** Use `libduckdb-src.zip` (amalgamated `duckdb.hpp`) for C++ headers and a standalone CMake that invokes `scripts/append_metadata.cmake` post-build for footer stamping. If the amalgamation proves insufficient (missing internal extension types), fall back to cloning DuckDB at v1.5.3. Both paths are lower-risk than building DuckDB from scratch.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Arrow → DuckDB row conversion | C++ Extension (table function scan) | DuckDB arrow_scan built-in | The extension wraps Arrow in a stream; arrow_scan performs the actual Arrow→DataChunk conversion |
| FFI call to Rust decoder | C++ Extension (table function init) | — | `loom_decode` is called in Init, not Scan; result stored in scan state |
| Arrow release callback | C++ Extension (scan state destructor) | — | C++ owns the FFI_ArrowArray after ptr::write; must call release on every exit path |
| Extension registration | C++ Extension (entry point) | DuckDB ExtensionLoader | `DUCKDB_CPP_EXTENSION_ENTRY` registers loom_scan via ExtensionLoader::RegisterFunction |
| Footer metadata stamping | CMake post-build step | DuckDB append_metadata.cmake script | Footer cannot be hand-written; must use the official script for correct binary layout |
| CLI argument parsing | DuckDB CLI | — | The path argument is accepted by loom_scan's Bind but ignored in Phase 2 |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| DuckDB C++ amalgamation | 1.5.3 | `duckdb.hpp` — all extension API types (`TableFunction`, `ExtensionLoader`, `DataChunk`, `LogicalType`, etc.) | The canonical C++ header for building DuckDB extensions without the full source tree [VERIFIED: GitHub release assets, extension-template source code] |
| DuckDB CLI binary | 1.5.3 (`duckdb_cli-osx-arm64.zip`) | Load and smoke-test the extension locally | Must be the exact same v1.5.3 release to match the footer's duckdb_version field [VERIFIED: DuckDB v1.5.3 extension_load.cpp source] |
| DuckDB `scripts/append_metadata.cmake` | v1.5.3 | Appends 512-byte footer to the .duckdb_extension binary after link | The only way to correctly stamp the footer; must be invoked from CMake POST_BUILD [VERIFIED: DuckDB v1.5.3 source, extension_build_tools.cmake] |
| `libloom_ffi.a` + `loom.h` | Phase 1 output | Rust staticlib providing `loom_decode` | Already built and tested [VERIFIED: Phase 1 artifacts] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `libduckdb-src.zip` (amalgam) | 1.5.3 | `duckdb.hpp` + `duckdb.cpp` — alternative to the full source tree for C++ header access | Use when you only need the C++ API, not the build macros (`build_loadable_extension`) |
| DuckDB static libs (`static-libs-osx-arm64.zip`) | 1.5.3 | Pre-built `libduckdb_static.a` and headers | Use if dynamic lookup mode causes symbol issues on macOS |
| `scripts/null.txt` | v1.5.3 | One-byte null file required by `append_metadata.cmake` | Must be checked out or reproduced from the DuckDB repo for the footer script |
| CMake 3.22+ | 4.1.1 (local) | Build system for the C++ extension | Already installed |
| Apple Clang 17 | 17.0.0 (local) | C++17 compiler for the extension | Already available |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `libduckdb-src.zip` (amalgam) for C++ headers | Clone DuckDB at v1.5.3 (`git clone --depth 1 --branch v1.5.3`) | Clone gives access to `build_loadable_extension` CMake macro which handles footer stamping automatically. Downside: 500MB checkout, contradicts D-02 ("fast, low-disk"). The hand-rolled CMake approach (D-03) with `append_metadata.cmake` achieves the same result with only the script file. |
| `-unsigned` CLI flag | `SET allow_unsigned_extensions=true` SQL | The SET variant works in-session; `-unsigned` activates at startup. Both bypass signature. The SET form also requires `SET allow_extensions_metadata_mismatch=true` to bypass version/platform check if footer is missing/wrong. |

**Download URLs:**
```bash
# macOS arm64 CLI
https://github.com/duckdb/duckdb/releases/download/v1.5.3/duckdb_cli-osx-arm64.zip

# C++ amalgamated source (duckdb.hpp + duckdb.cpp)
https://github.com/duckdb/duckdb/releases/download/v1.5.3/libduckdb-src.zip

# footer script (fetch one file from repo)
https://raw.githubusercontent.com/duckdb/duckdb/v1.5.3/scripts/append_metadata.cmake
https://raw.githubusercontent.com/duckdb/duckdb/v1.5.3/scripts/null.txt  # 1-byte null
```

---

## Package Legitimacy Audit

> This phase installs no npm, PyPI, or crates.io packages beyond what Phase 1 already
> pinned. The only new "packages" are binary downloads from the official DuckDB GitHub
> release page. No slopcheck applicable.

| Artifact | Source | Provenance | Disposition |
|----------|--------|------------|-------------|
| `duckdb_cli-osx-arm64.zip` | github.com/duckdb/duckdb/releases/v1.5.3 | Official DuckDB maintainers | Approved |
| `libduckdb-src.zip` | github.com/duckdb/duckdb/releases/v1.5.3 | Official DuckDB maintainers | Approved |
| `append_metadata.cmake` | raw.githubusercontent.com/duckdb/duckdb/v1.5.3 | Official DuckDB repo at tag v1.5.3 | Approved |

---

## Architecture Patterns

### System Architecture Diagram

```
loom_scan('test.bin')   ← SQL entry point
        │
        ▼
 [Bind callback]
   declare output schema (Int32, nullable) from FFI_ArrowSchema
   (call loom_decode once here OR in Init — see Init pattern below)
        │
        ▼
 [Init callback — GlobalTableFunctionState]
   call loom_decode(NULL, 0, &state.arrow_array, &state.arrow_schema)
   on nonzero return: throw DuckDB exception
   state.done = false
        │
        ▼
 [Scan callback]
   if state.done → output.SetCardinality(0); return (signals EOS to DuckDB)
   wrap state.arrow_array + state.arrow_schema in one-shot ArrowArrayStream
   call DuckDB's arrow_scan machinery with the stream
   state.done = true
        │
        ▼
 [GlobalTableFunctionState destructor]
   if state.arrow_array.release != nullptr → state.arrow_array.release(&state.arrow_array)
   if state.arrow_schema.release != nullptr → state.arrow_schema.release(&state.arrow_schema)
        │
        ▼
   DataChunk output → DuckDB query engine → SQL result
```

### Recommended Project Structure

```
duckdb-ext/                   # top-level C++ extension directory (D-03)
├── CMakeLists.txt            # hand-rolled; no extension-template macros
├── loom_extension.cpp        # single C++ source file
└── vendor/                   # downloaded artifacts (gitignored or tracked)
    ├── duckdb.hpp            # from libduckdb-src.zip
    ├── duckdb.cpp            # from libduckdb-src.zip (for static link fallback)
    ├── append_metadata.cmake # from DuckDB v1.5.3 repo
    └── null.txt              # 1-byte null, required by append_metadata.cmake

crates/
└── loom-ffi/
    ├── include/loom.h        # Phase 1 artifact — C header for loom_decode
    └── src/ffi.rs            # Phase 1 artifact — FFI implementation

scripts/
└── check-core-invariants.sh  # extend this with DuckDB smoke-test in Phase 2
```

### Pattern 1: Extension Entry Point (CPP ABI)

The DuckDB CPP ABI requires ONE exported C function named `${extension_name}_duckdb_cpp_init` taking `ExtensionLoader &`. [VERIFIED: DuckDB v1.5.3 extension_load.cpp line 634]

```cpp
// Source: DuckDB v1.5.3 src/main/extension/extension_load.cpp line 634 + extension_build_tools.cmake line 184
// DUCKDB_CPP_EXTENSION_ENTRY(name, loader_param) expands to:
// extern "C" { void loom_duckdb_cpp_init(duckdb::ExtensionLoader &loader_param) { ... } }

#define DUCKDB_EXTENSION_MAIN   // required define before including duckdb.hpp

#include "duckdb.hpp"           // from libduckdb-src.zip
extern "C" {
  #include "loom.h"             // Phase 1 header: loom_decode signature
}

static void LoadInternal(duckdb::ExtensionLoader &loader) {
    // register loom_scan table function (see Pattern 2)
}

extern "C" {
DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) {
    LoadInternal(loader);
}
}
```

**Do not** also export a `loom_init(DatabaseInstance &)` or `loom_version()` — these are the legacy DuckDB 0.x symbols. For DuckDB 1.5.3, only `loom_duckdb_cpp_init` is looked up. [VERIFIED: extension_load.cpp, `auto init_fun_name = extension_init_result.filebase + "_duckdb_cpp_init"`]

### Pattern 2: Table Function Registration (loom_scan)

```cpp
// Source: DuckDB v1.5.3 src/include/duckdb/main/extension/extension_loader.hpp
// ExtensionLoader::RegisterFunction(TableFunction)

struct LoomScanState : duckdb::GlobalTableFunctionState {
    ArrowArray  arrow_array  = {};   // zero-init: release==nullptr until populated
    ArrowSchema arrow_schema = {};
    bool done = false;

    ~LoomScanState() {
        // DUCK-03: release on every teardown path (success, cancel, error)
        if (arrow_array.release)  arrow_array.release(&arrow_array);
        if (arrow_schema.release) arrow_schema.release(&arrow_schema);
    }
};

static duckdb::unique_ptr<duckdb::FunctionData> LoomBind(
    duckdb::ClientContext &ctx,
    duckdb::TableFunctionBindInput &input,
    duckdb::vector<duckdb::LogicalType> &return_types,
    duckdb::vector<std::string> &names)
{
    // Accept the VARCHAR argument but ignore it (D-04)
    return_types.push_back(duckdb::LogicalType::INTEGER);
    names.push_back("value");
    return duckdb::make_uniq<duckdb::TableFunctionData>();
}

static duckdb::unique_ptr<duckdb::GlobalTableFunctionState> LoomInit(
    duckdb::ClientContext &ctx,
    duckdb::TableFunctionInitInput &input)
{
    auto state = duckdb::make_uniq<LoomScanState>();
    int32_t rc = loom_decode(
        nullptr, 0,
        reinterpret_cast<FFI_ArrowArray*>(&state->arrow_array),
        reinterpret_cast<FFI_ArrowSchema*>(&state->arrow_schema));
    if (rc != 0) {
        throw duckdb::IOException("loom_decode failed with code %d", rc);
    }
    return state;
}

static void LoomScan(
    duckdb::ClientContext &ctx,
    duckdb::TableFunctionInput &data,
    duckdb::DataChunk &output)
{
    auto &state = data.global_state->Cast<LoomScanState>();
    if (state.done) { output.SetCardinality(0); return; }
    // Delegate to arrow_scan machinery (see Pattern 3)
    // ... stream wrapping and arrow_scan call ...
    state.done = true;
}

// Registration in LoadInternal():
duckdb::TableFunction fn("loom_scan",
    {duckdb::LogicalType::VARCHAR}, LoomScan, LoomBind, LoomInit);
loader.RegisterFunction(fn);
```

### Pattern 3: ArrowArrayStream Callbacks for arrow_scan

`arrow_scan` in DuckDB 1.5.3 takes three `Value::POINTER` arguments: [VERIFIED: DuckDB v1.5.3 src/function/table/arrow.cpp ArrowScanBind]

1. `stream_factory_ptr` (uintptr_t) — pointer to our stream context object
2. `stream_factory_produce` (uintptr_t) — callback `unique_ptr<ArrowArrayStreamWrapper>(uintptr_t, ArrowStreamParameters &)`
3. `stream_factory_get_schema` (uintptr_t) — callback `void(ArrowArrayStream *, ArrowSchema &)`

These are C++ function pointers embedded in `Value::POINTER`. The cleanest approach for Phase 2 (one-shot single array) is to implement the `ArrowArrayStream` C struct callbacks directly rather than the DuckDB-level factory pattern:

```cpp
// Source pattern from Arrow C Stream Interface spec + DuckDB arrow_wrapper.hpp
// A self-contained ArrowArrayStream wrapping ONE array

struct OneShotStream {
    ArrowSchema  schema;   // filled from loom_decode output
    ArrowArray   array;    // filled from loom_decode output
    bool         consumed; // true after get_next returns the one batch

    static int get_schema(ArrowArrayStream *self, ArrowSchema *out) {
        // Shallow-copy schema into out (schema is owned by this struct)
        auto *s = reinterpret_cast<OneShotStream *>(self->private_data);
        *out = s->schema;
        // Do NOT zero s->schema.release — schema is still owned by this stream
        return 0;
    }

    static int get_next(ArrowArrayStream *self, ArrowArray *out) {
        auto *s = reinterpret_cast<OneShotStream *>(self->private_data);
        if (s->consumed) {
            out->release = nullptr;  // end-of-stream sentinel
            return 0;
        }
        *out = s->array;             // transfer array to consumer
        s->array.release = nullptr;  // consumer now owns it; prevent double-release
        s->consumed = true;
        return 0;
    }

    static void release(ArrowArrayStream *self) {
        auto *s = reinterpret_cast<OneShotStream *>(self->private_data);
        if (s->array.release)  s->array.release(&s->array);
        if (s->schema.release) s->schema.release(&s->schema);
        delete s;
        self->private_data = nullptr;
        self->release = nullptr;
    }
};
```

**Calling arrow_scan from within C++:** The arrow_scan function is a SQL-level table function, not directly callable from C++ without a connection. Two viable patterns:

**Option A (recommended for Phase 2 simplicity):** Implement the scan WITHOUT delegating to arrow_scan. Instead, in `LoomScan`, call `ArrowToDuckDBConversion::ColumnArrowToDuckDB` directly. This requires `#include "duckdb/function/table/arrow.hpp"` which IS in the amalgamated `duckdb.hpp`. The `ArrowArrayScanState` and `ArrowType` required by this internal API make it complex.

**Option B (per D-01 intent):** Use `duckdb::Connection::TableFunction("arrow_scan", {...})`. From within a table function, `ClientContext` can create a sub-connection. But this adds complexity.

**Option C (cleanest for MVP0 — satisfies D-01 spirit):** In `LoomInit`, call `loom_decode` and store the `FFI_ArrowArray`+`FFI_ArrowSchema`. In `LoomScan`, directly populate `DataChunk` using the Arrow array's buffer pointers (for a flat `Int32Array` with validity bitmap, this is mechanical: copy the values buffer into the output vector, set the validity mask). This avoids the `ArrowToDuckDB` internal machinery while still "adopting the Arrow array zero-copy" in the sense that the array is not re-allocated. For Phase 2's single hardcoded `Int32Array [1,2,3,null]`, direct DataChunk population is ~15 lines of straightforward C++.

**RECOMMENDATION for Phase 2:** Use Option C. The `loom_scan` table function directly fills `DataChunk` from the `FFI_ArrowArray` buffers. This satisfies DUCK-02 (adopts the Arrow array) and avoids the stream callback machinery. D-01's "ArrowArrayStream → arrow_scan" is the production path but adds ~50 lines of non-trivial C++; the planner should structure Phase 2 as Option C with a comment marking where Option B/the stream path would be wired in Phase 3+.

If the user/planner prefers strict D-01 compliance even for the stub phase, use Option A with `ColumnArrowToDuckDB` (internal but present in the amalgamated header).

### Anti-Patterns to Avoid

- **Exporting `loom_init(DatabaseInstance &db)`:** This is the DuckDB 0.x legacy entry point. DuckDB 1.5.3 dlsym's `loom_duckdb_cpp_init` for CPP ABI extensions. Exporting the old symbol results in "Extension did not contain the expected entrypoint function" at load time. [VERIFIED: extension_load.cpp line 634]
- **Omitting the 512-byte footer:** A `.duckdb_extension` file without a valid footer will fail the `AppearsValid()` magic check and be refused even with `allow_unsigned_extensions`. The footer is mandatory. [VERIFIED: ParseExtensionMetaData in extension_load.cpp]
- **Using the git hash as the duckdb_version in the footer:** For release tag builds, `DUCKDB_NORMALIZED_VERSION = DUCKDB_VERSION = "v1.5.3"` (not the git hash). Git hashes are only used for dev/pre-release builds where the version string contains "dev". The loaded CLI binary compares `engine_version` against the footer's `duckdb_version` via `GetVersionDirectoryName()`; a git hash in the footer will fail against a CLI that returns `"v1.5.3"`. [VERIFIED: CMakeLists.txt lines 418–425]
- **Calling `ArrowToDuckDB(ArrowScanLocalState &, ...)` directly:** This high-level function requires `ArrowScanLocalState`, `arrow_column_map_t`, and other internal state set up by the arrow_scan bind/init callbacks. It is not usable from loom_scan's scan callback without reproducing most of arrow_scan's internals. [VERIFIED: DuckDB 1.5.3 arrow.hpp ArrowTableFunction]
- **Using `ArrowToDuckDB(ArrowArray &, DataChunk &)` (the old simple API):** This simpler signature does NOT exist in DuckDB 1.5.3. The ARCHITECTURE.md sketch is aspirational; the actual API requires scan state. [VERIFIED: DuckDB 1.5.3 arrow.hpp]
- **Including `duckdb/function/table/arrow.hpp` when NOT in the DuckDB source tree:** The amalgamated `duckdb.hpp` bundles these types. Include only `duckdb.hpp`; do not use individual sub-headers unless you have the full source tree.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 512-byte extension footer | Custom binary footer writer | `scripts/append_metadata.cmake` from DuckDB v1.5.3 repo | Footer layout (8 × 32-byte fields, reversed order, magic value `"4\0..."`, 256-byte signature slot) is precisely specified in `ParseExtensionMetaData`; any deviation fails `AppearsValid()` |
| Arrow release semantics | Custom reference counting | Arrow C Data Interface `release` callback (installed by `to_ffi`) | Already implemented correctly in Phase 1; call it exactly once from the destructor |
| Platform string detection | CMake `CMAKE_SYSTEM_PROCESSOR` query | Copy the platform from `duckdb_platform_out` (produced by DuckDB's build) OR hardcode `"osx_arm64"` for macOS arm64 / `"linux_amd64"` for x86-64 Linux | DuckDB's own logic: `osx` + `arm64` → `"osx_arm64"` |

**Key insight:** The footer stamping and version matching look simple but are load-order-sensitive (footer appended AFTER link) and binary-exact (wrong byte order = invalid magic). Always use `append_metadata.cmake`.

---

## Key Question Answers

### Q1: Unsigned-load feasibility (HIGHEST PRIORITY — make-or-break risk)

**VERIFIED from DuckDB v1.5.3 source (`src/main/extension/extension_load.cpp`, `src/main/extension.cpp`):**

**Answer:** YES, a locally-built unsigned extension CAN be loaded into the official prebuilt duckdb 1.5.3 CLI, subject to two requirements:

1. **`allow_unsigned_extensions` must be set.** CLI: `duckdb -unsigned`. SQL: `SET allow_unsigned_extensions=true`. This bypasses the cryptographic signature check.

2. **The footer metadata must match the loading binary.** Even with `allow_unsigned_extensions`, `GetInvalidMetadataError()` is called and if non-empty, the load fails (throws `InvalidInputException`). The error is thrown regardless of unsigned mode — only the SIGNATURE check is bypassed, not the metadata mismatch check.

**What must match:**
- `duckdb_version` field: must equal `GetVersionDirectoryName()` return value. For v1.5.3 (a release tag), this is the string `"v1.5.3"`. [VERIFIED: CMakeLists.txt lines 418–425: for release builds where version has no "dev", `DUCKDB_NORMALIZED_VERSION = DUCKDB_VERSION = "v1.5.3"`]
- `platform` field: must equal `DuckDB::Platform()`. For macOS arm64: `"osx_arm64"`. For Linux x86-64: `"linux_amd64"`. [VERIFIED: CMakeLists.txt lines 175–186]
- `abi_type` field: for CPP ABI extensions, the field value in the footer is `"CPP"`.

**Fallback for metadata bypass:** If getting the footer exactly right proves difficult, also set `SET allow_extensions_metadata_mismatch=true` (SQL) to bypass the mismatch check. This lets a locally-built extension load regardless of version/platform fields. Use only during development.

**There is no fallback needed** if the footer is stamped correctly. The official prebuilt CLI WILL load a locally-built unsigned extension if both `allow_unsigned_extensions=true` and the footer version/platform match. This has been the mechanism since DuckDB 0.9+. The extension-template uses this exact workflow for local development.

---

### Q2: Prebuilt lib + headers acquisition

**VERIFIED: GitHub release assets API for v1.5.3**

**C++ headers for extension development:**

`libduckdb-src.zip` (5MB) contains the **amalgamated** `duckdb.hpp` and `duckdb.cpp`. This is the full C++ API — not just `duckdb.h` (C API). The amalgamation includes all types needed for extension development: `TableFunction`, `ExtensionLoader`, `DataChunk`, `LogicalType`, `ClientContext`, `ArrowToDuckDBConversion`, etc.

```bash
curl -L -o vendor/libduckdb-src.zip \
  https://github.com/duckdb/duckdb/releases/download/v1.5.3/libduckdb-src.zip
unzip vendor/libduckdb-src.zip -d vendor/duckdb-src
# results in: vendor/duckdb-src/duckdb.hpp  vendor/duckdb-src/duckdb.cpp
```

**CLI binary (macOS arm64):**
```bash
curl -L -o vendor/duckdb_cli.zip \
  https://github.com/duckdb/duckdb/releases/download/v1.5.3/duckdb_cli-osx-arm64.zip
unzip vendor/duckdb_cli.zip -d vendor/duckdb-cli
chmod +x vendor/duckdb-cli/duckdb
```

**Footer stamping script:**
```bash
curl -o vendor/append_metadata.cmake \
  https://raw.githubusercontent.com/duckdb/duckdb/v1.5.3/scripts/append_metadata.cmake
printf '\x00' > vendor/null.txt   # 1-byte null file required by the script
```

**CMake include/link flags (hand-rolled):**
```cmake
# vendor/duckdb-src/ must be checked in or downloaded in CI
include_directories(${CMAKE_SOURCE_DIR}/vendor/duckdb-src)     # for duckdb.hpp
include_directories(${CMAKE_SOURCE_DIR}/crates/loom-ffi/include)  # for loom.h

# No libduckdb link needed for dynamic-lookup mode (macOS, extension loads into CLI)
# DuckDB symbols are provided by the CLI binary at dlopen time
# Linux: same pattern (symbols resolved from the CLI ELF at dlopen)
```

**For Linux CI (x86-64):**
```bash
# CLI
curl -L -o duckdb_cli.zip \
  https://github.com/duckdb/duckdb/releases/download/v1.5.3/duckdb_cli-linux-amd64.zip
```
The amalgamated `libduckdb-src.zip` is platform-independent (headers only).

---

### Q3: 1.5.3 C++ extension entry points

**VERIFIED: DuckDB v1.5.3 `src/main/extension/extension_load.cpp` line 634:**

```
auto init_fun_name = extension_init_result.filebase + "_duckdb_cpp_init";
ext_init_fun_t init_fun = TryLoadFunctionFromDLL<ext_init_fun_t>(
    extension_init_result.lib_hdl, init_fun_name, extension_init_result.filename);
```

Where `filebase` is the lowercased stem of the `.duckdb_extension` filename (e.g., `loom` from `loom.duckdb_extension`).

**Type:** `typedef void (*ext_init_fun_t)(ExtensionLoader &);` [VERIFIED: extension_load.cpp line 145]

**Required symbol to export:**

```cpp
extern "C" {
void loom_duckdb_cpp_init(duckdb::ExtensionLoader &loader) {
    // register functions
    duckdb::TableFunction fn("loom_scan", ...);
    loader.RegisterFunction(fn);
}
}
```

The `DUCKDB_CPP_EXTENSION_ENTRY(loom, loader)` macro (from the extension-template) expands to exactly this extern C declaration.

**No `loom_version()` required.** The old `loom_init(DatabaseInstance &)` + `loom_version()` pattern is DuckDB 0.x. Do not export these.

**ABI type in footer:** Set `ABI_TYPE=CPP` in the `append_metadata.cmake` invocation.

---

### Q4: ArrowArrayStream + arrow_scan path (D-01 compliance)

**VERIFIED: DuckDB v1.5.3 `src/function/table/arrow.cpp` ArrowScanBind:**

`arrow_scan` takes three pointer values:
1. `stream_factory_ptr` — raw pointer to a factory/context object (cast to uintptr_t)
2. `stream_factory_produce` — C++ callback: `unique_ptr<ArrowArrayStreamWrapper>(uintptr_t, ArrowStreamParameters &)`
3. `stream_factory_get_schema` — C++ callback: `void(ArrowArrayStream *, ArrowSchema &)`

To fully comply with D-01 ("feed to arrow_scan machinery"), `loom_scan`'s Bind callback must call DuckDB's `arrow_scan` table function indirectly by creating these two factory callbacks.

**For Phase 2 (hardcoded 4 rows), the recommended approach is direct DataChunk population** (see Pattern 3, Option C above). This is because:
- The array is a simple flat `Int32Array` with one validity buffer and one values buffer.
- Direct population requires ~15 lines vs ~80 lines for the full stream factory approach.
- The planner can mark the direct-population approach as a stub placeholder for the Phase 3+ stream path.

**DUCK-03 release-callback ownership for D-01:**

If the full stream path is used:
- `OneShotStream::get_next` transfers ownership of `arrow_array` by bitwise copy + sets `s->array.release = nullptr` (prevents double-release from stream destructor)
- `OneShotStream::release` calls release on any remaining array/schema before deleting `this`
- DuckDB calls `stream.release(&stream)` after consuming all batches
- The `LoomScanState` destructor does NOT release the array again if `get_next` already transferred it (the transfer zeroed `s->array.release`)

For direct DataChunk population:
- `LoomScanState` destructor calls `arrow_array.release(&arrow_array)` and `arrow_schema.release(&arrow_schema)` if non-null
- Set `state.arrow_array.release = nullptr` after release (prevents double-free)
- Zero-init `LoomScanState::arrow_array` and `arrow_schema` before calling `loom_decode` (ensures release is nullptr on early failure paths)

---

### Q5: Rust↔C++ link specifics

**VERIFIED from Phase 1 artifacts + PITFALLS.md:**

`libloom_ffi.a` (System allocator, `panic="unwind"`, `loom_decode` exported) links into the C++ `.duckdb_extension` shared library.

**CMake link configuration:**

```cmake
# Trigger cargo build before linking
add_custom_command(
    OUTPUT ${CMAKE_SOURCE_DIR}/target/release/libloom_ffi.a
    COMMAND cargo build -p loom-ffi --release
            --manifest-path ${CMAKE_SOURCE_DIR}/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    COMMENT "Building libloom_ffi.a (Rust staticlib)"
)
add_custom_target(loom_ffi_build ALL
    DEPENDS ${CMAKE_SOURCE_DIR}/target/release/libloom_ffi.a)

add_library(loom_loadable_extension SHARED loom_extension.cpp)
add_dependencies(loom_loadable_extension loom_ffi_build)

target_link_libraries(loom_loadable_extension PRIVATE
    ${CMAKE_SOURCE_DIR}/target/release/libloom_ffi.a)

# macOS: dynamic lookup of DuckDB symbols (CLI provides them at runtime)
if(APPLE)
    target_link_options(loom_loadable_extension PRIVATE
        "-undefined" "dynamic_lookup")
endif()

# Optional: hide Rust symbols not in the loom_decode API surface
if(APPLE)
    target_link_options(loom_loadable_extension PRIVATE
        "-Wl,-exported_symbol,_loom_duckdb_cpp_init")
endif()

set_target_properties(loom_loadable_extension PROPERTIES
    OUTPUT_NAME "loom"
    SUFFIX ".duckdb_extension"
    PREFIX "")
```

**Symbol visibility:** On macOS, `-Wl,-exported_symbol,_loom_duckdb_cpp_init` hides all other symbols (including Rust stdlibs). On Linux, use `-Wl,--version-script=loom.map` with an export list, or `-Wl,-fvisibility=hidden` + `-Wl,--exclude-libs,ALL`. [CITED: DuckDB extension_build_tools.cmake lines 151-156]

**`-whole-archive` is NOT needed.** `loom_decode` is referenced by `loom_extension.cpp`, so the linker retains it. Only use `--whole-archive`/`-force_load` if you have unreferenced symbols that must be included (not the case here).

**Allocator:** System allocator is already set in `loom-ffi/src/lib.rs` (Phase 1, CORE-02). No additional flags needed.

---

### Q6: CI smoke-test

**Pattern (extend existing `.github/workflows/ci.yml`):**

For both macOS and Linux, the smoke-test is:
1. Build `libloom_ffi.a` (already done in the Rust job).
2. Build the C++ extension with CMake.
3. Download the duckdb v1.5.3 CLI binary.
4. Run: `echo "LOAD '/path/to/loom.duckdb_extension'; SELECT * FROM loom_scan('test.bin');" | ./duckdb -unsigned`
5. Assert exit code 0 AND output contains 4 rows with values 1, 2, 3, NULL.

```yaml
- name: Install CMake (Linux)
  if: runner.os == 'Linux'
  run: sudo apt-get install -y cmake

- name: Build C++ extension
  run: |
    cmake -S duckdb-ext -B duckdb-ext/build -DCMAKE_BUILD_TYPE=Release
    cmake --build duckdb-ext/build

- name: Download DuckDB CLI (Linux)
  if: runner.os == 'Linux'
  run: |
    curl -L -o duckdb_cli.zip \
      https://github.com/duckdb/duckdb/releases/download/v1.5.3/duckdb_cli-linux-amd64.zip
    unzip duckdb_cli.zip && chmod +x duckdb

- name: DuckDB load smoke-test
  run: |
    ROWS=$(echo "LOAD '$(pwd)/duckdb-ext/build/loom.duckdb_extension'; \
      SELECT count(*) FROM loom_scan('x');" | ./duckdb -unsigned)
    test "$ROWS" = "4" || (echo "Expected 4 rows, got: $ROWS" && exit 1)
```

**macOS CI note:** The existing CI runs on `ubuntu-latest`. Add a macOS job (`macos-latest`) for the arm64 build, or use `macos-14` (arm64 runner). For macOS, `duckdb_cli-osx-arm64.zip` and the `osx_arm64` platform string in the footer.

---

## Common Pitfalls

### Pitfall 1: `allow_unsigned_extensions` does NOT bypass version/platform mismatch

**What goes wrong:** Building the extension with a missing or incorrect footer, then trying to load with `-unsigned`. The load still fails with "The file was built specifically for DuckDB version 'X'..." even with unsigned mode enabled.

**Why it happens:** `GetInvalidMetadataError()` runs before the signature check in the load flow. Its error is thrown regardless of `allow_unsigned_extensions` (only thrown after the unsigned check when UNSIGNED mode is off, but the mismatch error is also thrown when UNSIGNED mode is on unless `allow_extensions_metadata_mismatch` is also set). [VERIFIED: extension_load.cpp lines 477–510]

**How to avoid:** Always stamp the footer correctly. Run `append_metadata.cmake` in a CMake `POST_BUILD` custom command with `DUCKDB_VERSION=v1.5.3`, correct `PLATFORM` string, and `ABI_TYPE=CPP`.

**Emergency workaround during development:** `SET allow_extensions_metadata_mismatch=true` before LOAD. Works only in-session; not available as a CLI flag.

**Warning signs:** "Failed to load" error mentioning a DuckDB version string or platform string.

---

### Pitfall 2: Footer is required; file size must be at least 512 bytes

**What goes wrong:** Building the extension without invoking `append_metadata.cmake`, then loading. DuckDB checks `handle.GetFileSize() >= FOOTER_SIZE` (512 bytes) before parsing; for a small test extension this may also be a real-size issue.

**Why it happens:** The `append_metadata.cmake` script is a POST_BUILD step; forgetting to add it to CMake leaves the shared library without any footer. The `AppearsValid()` check fails and load is rejected regardless of unsigned mode.

**How to avoid:** Add the CMake `add_custom_command(TARGET ... POST_BUILD COMMAND ...)` that calls `append_metadata.cmake` immediately after the `add_library(... SHARED ...)` target. Make the `scripts/null.txt` and `append_metadata.cmake` files available in the repository (checked in or downloaded in CMake configure step).

---

### Pitfall 3: Null.txt file must contain exactly one null byte

**What goes wrong:** `append_metadata.cmake` reads `NULL_FILE` to create a 32-byte null-padded field. If `null.txt` is empty or contains two bytes (e.g., `\x00\n`), the field padding is wrong, the footer layout is corrupted, and `AppearsValid()` fails.

**How to avoid:** Create `null.txt` with `printf '\x00' > null.txt` or `python3 -c "open('null.txt','wb').write(b'\x00')"`. Do not use `echo -e '\x00'` (appends newline).

---

### Pitfall 4: Platform string mismatch between build and CLI

**What goes wrong:** Building on macOS arm64 but stamping the footer with `"linux_amd64"`, or not stamping at all and letting the field be empty. The CLI's `DuckDB::Platform()` returns `"osx_arm64"` and the mismatch is rejected.

**Why it happens:** The `PLATFORM_FILE` argument to `append_metadata.cmake` reads a file containing the platform string. In DuckDB's own build system, this file is generated by a helper binary (`duckdb_platform_binary`). For the hand-rolled CMake, the platform string must be known at configure time.

**How to avoid:** In the hand-rolled CMake, determine the platform from `CMAKE_SYSTEM_NAME` + `CMAKE_SYSTEM_PROCESSOR` and write it to a temp file:

```cmake
if(APPLE AND CMAKE_SYSTEM_PROCESSOR MATCHES "arm64")
    set(DUCKDB_PLATFORM "osx_arm64")
elseif(APPLE)
    set(DUCKDB_PLATFORM "osx_amd64")
elseif(UNIX AND CMAKE_SYSTEM_PROCESSOR MATCHES "(x86_64|amd64)")
    set(DUCKDB_PLATFORM "linux_amd64")
elseif(UNIX AND CMAKE_SYSTEM_PROCESSOR MATCHES "(aarch64|arm64)")
    set(DUCKDB_PLATFORM "linux_arm64")
endif()
file(WRITE ${CMAKE_BINARY_DIR}/duckdb_platform_out "${DUCKDB_PLATFORM}")
```

Then pass `PLATFORM_FILE=${CMAKE_BINARY_DIR}/duckdb_platform_out` to `append_metadata.cmake`.

---

### Pitfall 5: loom_decode return-code check is mandatory before using the output pointers

**What goes wrong:** Calling `loom_decode(...)` and then unconditionally reading `state.arrow_array` without checking the `int32_t` return value. If `loom_decode` returns nonzero (e.g., `LoomError::Panicked = 3`), the output pointers contain uninitialized data and using them is UB/crash.

**How to avoid:** Always check the return code:
```cpp
int32_t rc = loom_decode(nullptr, 0, &state.arrow_array, &state.arrow_schema);
if (rc != 0) {
    throw duckdb::IOException("loom_decode failed: error code %d", rc);
}
```

Also: initialize `arrow_array` and `arrow_schema` to zero before calling (they are stack/member variables; `{}` zero-initializes in C++). The `release` pointer will be `nullptr` until `loom_decode` writes to them, so the destructor's null-check is safe even if decode fails partway.

---

### Pitfall 6: Static vs dynamic DuckDB linking on macOS

**What goes wrong:** Linking the extension against `libduckdb.dylib` from `libduckdb-osx-universal.zip` causes symbol duplication when the extension is loaded into the CLI (which has its own copy of DuckDB). Both copies try to register the same internal singleton structures.

**How to avoid:** Use dynamic-lookup mode (`-undefined dynamic_lookup` on macOS). Do NOT link against `libduckdb.dylib`. The CLI provides all DuckDB symbols when it `dlopen`s the extension. This is the standard extension distribution mode (`EXTENSION_STATIC_BUILD=0`). [VERIFIED: DuckDB extension_build_tools.cmake lines 155-158]

The hand-rolled CMake should NOT `find_library(DuckDB ...)` or `target_link_libraries(... duckdb)`. Only link `libloom_ffi.a`.

---

## Runtime State Inventory

> SKIPPED — greenfield phase (no rename/refactor involved).

---

## Code Examples

### Complete footer-stamp CMake snippet

```cmake
# Source: scripts/append_metadata.cmake usage, DuckDB v1.5.3 extension_build_tools.cmake
# This replaces the build_loadable_extension() macro for hand-rolled builds.

set(DUCKDB_VERSION "v1.5.3")          # must match the CLI binary
set(ABI_TYPE "CPP")

# Determine platform
if(APPLE AND CMAKE_SYSTEM_PROCESSOR MATCHES "arm64")
    set(DUCKDB_PLATFORM "osx_arm64")
elseif(APPLE)
    set(DUCKDB_PLATFORM "osx_amd64")
elseif(UNIX AND CMAKE_SYSTEM_PROCESSOR MATCHES "(x86_64|amd64|AMD64)")
    set(DUCKDB_PLATFORM "linux_amd64")
elseif(UNIX AND CMAKE_SYSTEM_PROCESSOR MATCHES "(aarch64|arm64)")
    set(DUCKDB_PLATFORM "linux_arm64")
else()
    message(FATAL_ERROR "Unsupported platform for DuckDB extension")
endif()

file(WRITE ${CMAKE_BINARY_DIR}/duckdb_platform_out "${DUCKDB_PLATFORM}")

# Stamp the footer after the shared library is linked
add_custom_command(
    TARGET loom_loadable_extension
    POST_BUILD
    COMMAND ${CMAKE_COMMAND}
        -DABI_TYPE=${ABI_TYPE}
        -DEXTENSION=$<TARGET_FILE:loom_loadable_extension>
        -DPLATFORM_FILE=${CMAKE_BINARY_DIR}/duckdb_platform_out
        -DVERSION_FIELD=${DUCKDB_VERSION}
        -DEXTENSION_VERSION=""
        -DNULL_FILE=${CMAKE_SOURCE_DIR}/duckdb-ext/vendor/null.txt
        -P ${CMAKE_SOURCE_DIR}/duckdb-ext/vendor/append_metadata.cmake
    COMMENT "Stamping DuckDB extension footer (version=${DUCKDB_VERSION}, platform=${DUCKDB_PLATFORM})"
)
```

### Minimal loom_extension.cpp skeleton

```cpp
// Source pattern: DuckDB v1.5.3 extension_build_tools.cmake + ArrowTableFunction + arrow.hpp

#define DUCKDB_EXTENSION_MAIN
#include "duckdb.hpp"    // amalgamated from libduckdb-src.zip

extern "C" {
#include "loom.h"        // loom_decode signature from Phase 1
}

using namespace duckdb;

// ------------------------------------------------------------------
// Scan state: holds the Arrow FFI structs post-decode
// ------------------------------------------------------------------
struct LoomScanState : GlobalTableFunctionState {
    ArrowArray  arrow_array  = {};  // zero-init (release==nullptr)
    ArrowSchema arrow_schema = {};
    bool done = false;

    ~LoomScanState() {
        // DUCK-03: release on ALL teardown paths (destructor covers them all)
        if (arrow_array.release)  arrow_array.release(&arrow_array);
        if (arrow_schema.release) arrow_schema.release(&arrow_schema);
    }
};

// ------------------------------------------------------------------
// Bind: declare output schema; accept VARCHAR arg (ignored, D-04)
// ------------------------------------------------------------------
static unique_ptr<FunctionData> LoomBind(
    ClientContext &ctx,
    TableFunctionBindInput &input,
    vector<LogicalType> &return_types,
    vector<string> &names)
{
    // Accept the path argument but do nothing with it
    return_types.push_back(LogicalType::INTEGER);
    names.push_back("value");
    return make_uniq<TableFunctionData>();
}

// ------------------------------------------------------------------
// Init: call loom_decode, store Arrow structs in state
// ------------------------------------------------------------------
static unique_ptr<GlobalTableFunctionState> LoomInit(
    ClientContext &ctx, TableFunctionInitInput &input)
{
    auto state = make_uniq<LoomScanState>();
    // Phase 2: input bytes are ignored; loom_decode returns hardcoded [1,2,3,null]
    int32_t rc = loom_decode(
        nullptr, 0,
        reinterpret_cast<FFI_ArrowArray*>(&state->arrow_array),
        reinterpret_cast<FFI_ArrowSchema*>(&state->arrow_schema));
    if (rc != 0) {
        throw IOException("loom_decode returned error code %d", (int)rc);
    }
    return state;
}

// ------------------------------------------------------------------
// Scan: fill DataChunk from the Arrow array (direct population for Phase 2)
// ------------------------------------------------------------------
static void LoomScan(
    ClientContext &ctx, TableFunctionInput &data, DataChunk &output)
{
    auto &state = data.global_state->Cast<LoomScanState>();
    if (state.done) { output.SetCardinality(0); return; }

    // The arrow_array is Int32 with 4 elements and a validity bitmap.
    // Direct population: read values buffer and null bitmap.
    auto &arr = state.arrow_array;
    idx_t count = (idx_t)arr.length;  // = 4

    output.SetCardinality(count);
    auto &vec = output.data[0];
    auto *out_data = FlatVector::GetData<int32_t>(vec);
    auto &validity = FlatVector::Validity(vec);

    // Arrow buffer layout for Int32: buffers[0]=validity, buffers[1]=values
    const uint8_t *validity_buf = static_cast<const uint8_t*>(arr.buffers[0]);
    const int32_t *values_buf   = static_cast<const int32_t*>(arr.buffers[1]);

    for (idx_t i = 0; i < count; i++) {
        if (validity_buf) {
            bool valid = (validity_buf[i / 8] >> (i % 8)) & 1;
            if (!valid) { validity.SetInvalid(i); continue; }
        }
        out_data[i] = values_buf[i];
    }

    state.done = true;
    // array/schema are released by ~LoomScanState() when the query is done
}

// ------------------------------------------------------------------
// Entry point: register loom_scan
// ------------------------------------------------------------------
static void LoadInternal(ExtensionLoader &loader) {
    TableFunction fn("loom_scan", {LogicalType::VARCHAR}, LoomScan, LoomBind, LoomInit);
    loader.RegisterFunction(fn);
}

extern "C" {
DUCKDB_CPP_EXTENSION_ENTRY(loom, loader) {
    LoadInternal(loader);
}
}
```

### Local smoke-test invocation

```bash
# From repo root (macOS arm64):
./vendor/duckdb-cli/duckdb -unsigned << 'EOF'
LOAD '/absolute/path/to/duckdb-ext/build/loom.duckdb_extension';
SELECT * FROM loom_scan('test.bin');
EOF
# Expected output: 4 rows — 1, 2, 3, NULL
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `loom_init(DatabaseInstance &)` entry point | `loom_duckdb_cpp_init(ExtensionLoader &)` via `DUCKDB_CPP_EXTENSION_ENTRY` | DuckDB ~1.1+ | Old symbol still compiles but is NOT dlsym'd in 1.5.3 |
| `ArrowToDuckDB(ArrowArray &, DataChunk &)` simple API | `ArrowToDuckDBConversion::ColumnArrowToDuckDB(Vector &, ArrowArray &, ...)` with scan state | DuckDB 0.9→1.x refactor | Old simple API gone; scan state required for full API |
| Git hash in extension footer for release builds | Semver string (`"v1.5.3"`) in footer for release builds | DuckDB ~0.8+ | Extension stored under `~/.duckdb/extensions/<hash>/` for dev builds, under `~/.duckdb/extensions/v1.5.3/` for release builds |
| `allow_unsigned_extensions` bypasses ALL checks | `allow_unsigned_extensions` bypasses signature only; metadata mismatch requires separate `allow_extensions_metadata_mismatch` | DuckDB ~1.0 | Two flags needed for completely unchecked load; one flag sufficient when footer is correct |

**Deprecated/outdated:**
- `duckdb_extension_info` + C_STRUCT ABI: The C ABI extension path (for non-C++ extensions). Not needed for our C++ extension.
- `ExtensionUtil::RegisterFunction(db, fn)`: Still valid for in-tree extensions; for loadable extensions use `ExtensionLoader::RegisterFunction(fn)` passed in the entry point.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `libduckdb-src.zip` amalgamated `duckdb.hpp` includes `ExtensionLoader`, `TableFunction`, `ArrowToDuckDBConversion`, and all types needed for C++ extension development | Q2 / Standard Stack | If amalgam lacks internal extension types, must clone DuckDB source tree (still feasible, adds ~500MB download) |
| A2 | `DUCKDB_CPP_EXTENSION_ENTRY(loom, loader)` macro exists in `duckdb.hpp` amalgamation at v1.5.3 | Q3 / Pattern 1 | Confirmed in extension-template source (which includes `duckdb.hpp`); if macro is missing, equivalent extern "C" declaration can be written manually |
| A3 | `FlatVector::GetData`, `FlatVector::Validity` and Arrow buffer layout (buffers[0]=validity, buffers[1]=values) are stable across the 1.5.3 amalgamation | Code Examples | Standard Arrow C Data Interface layout; DuckDB's own arrow scan uses identical buffer indexing — risk is LOW |
| A4 | GitHub Actions `macos-14` runners are arm64 (M1) and `ubuntu-latest` runners are x86-64 for building the respective extension variants | Q6 / CI | Standard GitHub-hosted runner specs; correct as of 2026 |

**All other claims in this research were VERIFIED from DuckDB v1.5.3 source code on GitHub or from Phase 1 project artifacts.**

---

## Open Questions

1. **Does `duckdb.hpp` (amalgamation) expose `DUCKDB_CPP_EXTENSION_ENTRY` macro?**
   - What we know: The extension-template source file `#include "duckdb.hpp"` and uses `DUCKDB_CPP_EXTENSION_ENTRY`. The amalgamation is generated from all DuckDB headers.
   - What's unclear: Whether this specific macro appears in the amalgamation or only when building with the full source tree.
   - Recommendation: Verify in Wave 0 by unzipping `libduckdb-src.zip` and grepping for `DUCKDB_CPP_EXTENSION_ENTRY`. If absent, define the macro locally in `loom_extension.cpp`.

2. **Arrow buffer index for validity vs values in DuckDB's ArrowArray at 1.5.3?**
   - What we know: Arrow C Data Interface spec says buffers[0]=validity bitmap (may be NULL if no nulls), buffers[1]=values for primitive types.
   - What's unclear: Whether DuckDB's `arrow::ffi::to_ffi` output places them at exactly these indices.
   - Recommendation: Verify in Wave 0 by calling `loom_decode` in a test harness and printing `arrow_array.buffers[0]` (should be non-null for the `[1,2,3,null]` array) and `arrow_array.n_buffers` (should be 2 for Int32).

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| CMake | C++ extension build | Yes | 4.1.1 | — |
| Apple Clang / clang++ | C++ compilation | Yes | 17.0.0 | — |
| cargo 1.92.0 | `libloom_ffi.a` build | Yes | 1.92.0 | — |
| git | DuckDB source checkout (if needed) | Yes | 2.33.0 | — |
| DuckDB 1.5.3 CLI | Load smoke-test | No (not installed) | — | Download `duckdb_cli-osx-arm64.zip` in setup step |
| `libduckdb-src.zip` headers | C++ extension compilation | No | — | Download in CMake configure step or Wave 0 |
| `append_metadata.cmake` | Footer stamping | No | — | Download in Wave 0 setup |
| `null.txt` (1-byte null) | Footer stamping | No | — | Generate: `printf '\x00' > vendor/null.txt` |

**Missing dependencies with no fallback:** None — all missing items have clear download/generation steps.

**Missing dependencies requiring setup:** DuckDB CLI, amalgamated headers, footer script — all downloadable from GitHub release artifacts at v1.5.3.

---

## Security Domain

> `security_enforcement: true`, `security_asvs_level: 1` per `.planning/config.json`.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | — (no user auth in extension) |
| V3 Session Management | No | — |
| V4 Access Control | No | — |
| V5 Input Validation | Partial | `loom_decode` return-code check (nonzero = reject); null pointer check already in `loom_decode` |
| V6 Cryptography | No | — |

### Known Threat Patterns for C++ DuckDB Extension

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Panic crosses FFI boundary | Denial-of-service (process abort) | `catch_unwind` already in `loom_decode` (DUCK-04, Phase 1); C++ must check return code |
| Integer overflow in Arrow buffer index calculation | Tampering / crash | Use `arr.length` directly (it's `int64_t`); cast to `idx_t` only after bounds check |
| Null `buffers[0]` (no validity bitmap = all valid) | — | Check `validity_buf != nullptr` before reading null bitmap (handled in code example above) |

---

## Sources

### Primary (HIGH confidence — verified from source)

- DuckDB v1.5.3 `src/main/extension/extension_load.cpp` — complete extension load flow, symbol lookup (`loom_duckdb_cpp_init`), unsigned bypass logic, metadata mismatch error flow
- DuckDB v1.5.3 `src/main/extension.cpp` — `GetInvalidMetadataError()` implementation: exact version/platform checks for CPP ABI
- DuckDB v1.5.3 `CMakeLists.txt` lines 317–425 — `DUCKDB_NORMALIZED_VERSION` logic: release builds use `"v1.5.3"` not git hash; platform string derivation (`osx_arm64`, `linux_amd64`)
- DuckDB v1.5.3 `extension/extension_build_tools.cmake` — `build_loadable_extension_directory()` function: `FOOTER_VERSION_VALUE`, `append_metadata.cmake` invocation, symbol visibility flags
- DuckDB v1.5.3 `scripts/append_metadata.cmake` — exact footer binary layout (8 × 32-byte fields, reversed order, 256-byte signature slot)
- DuckDB v1.5.3 `src/include/duckdb/main/extension.hpp` — `ParsedExtensionMetaData` struct: FOOTER_SIZE=512, SIGNATURE_SIZE=256, `ExtensionABIType::CPP`
- DuckDB v1.5.3 `src/include/duckdb/main/extension/extension_loader.hpp` — `ExtensionLoader::RegisterFunction(TableFunction)` API
- DuckDB v1.5.3 `src/function/table/arrow.cpp` — `ArrowScanBind`: three-pointer `arrow_scan` call signature
- DuckDB v1.5.3 `src/include/duckdb/function/table/arrow.hpp` — `ArrowToDuckDBConversion` struct, `ArrowTableFunction::ArrowToDuckDB` signature (requires ArrowScanLocalState)
- DuckDB v1.5.3 GitHub Releases API — asset list confirming `libduckdb-src.zip` (5MB amalgamation), `duckdb_cli-osx-arm64.zip`, `static-libs-osx-arm64.zip`
- Phase 1 artifacts (`crates/loom-ffi/include/loom.h`, `crates/loom-ffi/src/ffi.rs`) — confirmed `loom_decode` signature, error codes, ownership protocol

### Secondary (MEDIUM confidence)

- `duckdb/extension-template` `src/waddle_extension.cpp` — confirms `DUCKDB_CPP_EXTENSION_ENTRY(name, loader)` macro usage pattern in 1.5.x era
- `duckdb/extension-ci-tools` `makefiles/duckdb_extension.Makefile` — confirms the `duckdb/` subdirectory (git clone) pattern for extension builds using DuckDB source tree
- DuckDB v1.5.3 `src/include/duckdb/common/arrow/arrow_wrapper.hpp` — `ArrowArrayStreamWrapper` class for stream wrapping pattern

### Tertiary (LOW confidence)

- WebSearch result confirming `SET allow_extensions_metadata_mismatch=true` SQL syntax — verified by source reading that the setting exists as `AllowExtensionsMetadataMismatchSetting`

---

## Metadata

**Confidence breakdown:**
- Unsigned load feasibility: HIGH — read from source code, not docs
- Footer stamping mechanism: HIGH — read `append_metadata.cmake` directly
- Entry point symbol name: HIGH — read from `extension_load.cpp`
- Header acquisition: MEDIUM — `libduckdb-src.zip` contains amalgam (confirmed from community sources); internal types in amalgam not directly verified
- ArrowArrayStream wrapper: MEDIUM — pattern derived from Arrow C Stream Interface spec + DuckDB source; exact callback signatures confirmed
- CI pattern: MEDIUM — derived from existing CI structure + GitHub Actions runner specs

**Research date:** 2026-06-07
**Valid until:** 2026-09-07 (DuckDB 1.5.x is stable; extension ABI changes only across major/minor versions)
