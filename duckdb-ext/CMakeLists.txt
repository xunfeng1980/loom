cmake_minimum_required(VERSION 3.22)
project(loom_duckdb_extension CXX)

# ---------------------------------------------------------------------------
# C++17 required (duckdb.hpp amalgamation uses C++17 features)
# ---------------------------------------------------------------------------
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ---------------------------------------------------------------------------
# Paths
#
# CMAKE_SOURCE_DIR = <repo>/duckdb-ext   (because cmake -S duckdb-ext)
# Workspace Cargo.toml is one directory up.
# ---------------------------------------------------------------------------
set(WORKSPACE_ROOT "${CMAKE_SOURCE_DIR}/..")
set(WORKSPACE_CARGO_TOML "${WORKSPACE_ROOT}/Cargo.toml")
set(LIBLOOM_FFI "${WORKSPACE_ROOT}/target/release/libloom_ffi.a")

# ---------------------------------------------------------------------------
# Step 1: cargo build trigger — ensures libloom_ffi.a is fresh before link.
# D-03: the staticlib must be rebuilt before the C++ extension links against it.
# ---------------------------------------------------------------------------
add_custom_command(
    OUTPUT "${LIBLOOM_FFI}"
    COMMAND cargo build -p loom-ffi --release
            --manifest-path "${WORKSPACE_CARGO_TOML}"
    WORKING_DIRECTORY "${WORKSPACE_ROOT}"
    COMMENT "Building libloom_ffi.a (Rust staticlib, cargo build -p loom-ffi --release)"
)
add_custom_target(loom_ffi_build ALL
    DEPENDS "${LIBLOOM_FFI}"
)

# ---------------------------------------------------------------------------
# Step 2: The DuckDB extension shared library.
# Source: single C++ file authored in Plan 01.
# Includes: duckdb.hpp (amalgam) and loom.h (FFI header from Phase 1).
# ---------------------------------------------------------------------------
add_library(loom_loadable_extension SHARED
    "${CMAKE_SOURCE_DIR}/loom_extension.cpp"
)

# D-03: extension must depend on the freshly-built staticlib.
add_dependencies(loom_loadable_extension loom_ffi_build)

# Include directories:
#   - vendor/duckdb-src  → duckdb.hpp (DuckDB v1.5.3 amalgamated header)
#   - ../crates/loom-ffi/include → loom.h (Phase 1 FFI header)
target_include_directories(loom_loadable_extension PRIVATE
    "${CMAKE_SOURCE_DIR}/vendor/duckdb-src"
    "${WORKSPACE_ROOT}/crates/loom-ffi/include"
)

# Link the Rust staticlib.  Do NOT link libduckdb — DuckDB symbols are resolved
# at dlopen time from the CLI binary (PITFALLS P6; RESEARCH Q5).
target_link_libraries(loom_loadable_extension PRIVATE
    "${LIBLOOM_FFI}"
)

# ---------------------------------------------------------------------------
# Step 3: Symbol resolution and hiding.
#
# macOS: -undefined dynamic_lookup lets DuckDB symbols resolve from the CLI at
#        runtime (PITFALLS P6).  -exported_symbol hides all non-API symbols
#        exported from the Rust staticlib so no rogue allocator symbols leak
#        into the extension's export table.
#
# Linux: -fvisibility=hidden + --exclude-libs,ALL achieves the same hiding.
#        DuckDB symbols resolve from the CLI ELF at dlopen (no -lduckdb needed).
# ---------------------------------------------------------------------------
if(APPLE)
    target_link_options(loom_loadable_extension PRIVATE
        "-undefined" "dynamic_lookup"
        "-Wl,-exported_symbol,_loom_duckdb_cpp_init"
    )
elseif(UNIX)
    target_compile_options(loom_loadable_extension PRIVATE
        "-fvisibility=hidden"
    )
    target_link_options(loom_loadable_extension PRIVATE
        "-Wl,--exclude-libs,ALL"
    )
endif()

# ---------------------------------------------------------------------------
# Step 4: Output naming.
# The file must be "loom.duckdb_extension" so that DuckDB dlsym's the entry
# symbol "loom_duckdb_cpp_init" (filebase = "loom").
# ---------------------------------------------------------------------------
set_target_properties(loom_loadable_extension PROPERTIES
    OUTPUT_NAME "loom"
    SUFFIX ".duckdb_extension"
    PREFIX ""
)

# ---------------------------------------------------------------------------
# Step 5: Platform detection for the footer stamp.
# DuckDB's platform string is derived from OS + CPU architecture.
# PITFALLS Pitfall 4 — wrong platform string causes a mismatch error even
# with -unsigned; only the signature check is bypassed, not metadata.
# ---------------------------------------------------------------------------
if(APPLE AND CMAKE_SYSTEM_PROCESSOR MATCHES "arm64")
    set(DUCKDB_PLATFORM "osx_arm64")
elseif(APPLE)
    set(DUCKDB_PLATFORM "osx_amd64")
elseif(UNIX AND CMAKE_SYSTEM_PROCESSOR MATCHES "(x86_64|amd64|AMD64)")
    set(DUCKDB_PLATFORM "linux_amd64")
elseif(UNIX AND CMAKE_SYSTEM_PROCESSOR MATCHES "(aarch64|arm64)")
    set(DUCKDB_PLATFORM "linux_arm64")
else()
    message(FATAL_ERROR "Unsupported platform for DuckDB extension: ${CMAKE_SYSTEM_NAME}/${CMAKE_SYSTEM_PROCESSOR}")
endif()

# Write the platform string to a file so append_metadata.cmake can read it.
file(WRITE "${CMAKE_BINARY_DIR}/duckdb_platform_out" "${DUCKDB_PLATFORM}")

message(STATUS "DuckDB extension platform: ${DUCKDB_PLATFORM}")

# ---------------------------------------------------------------------------
# Step 6: Footer-stamp POST_BUILD (the unsigned-load enabler — DUCK-01).
#
# append_metadata.cmake appends a 512-byte footer to the .duckdb_extension
# binary with fields: duckdb_version=v1.5.3, platform=<host>, abi_type=CPP.
# Without this step, DuckDB rejects the extension even with -unsigned
# (AppearsValid() checks for the 512-byte footer regardless of unsigned mode).
#
# IMPORTANT PATH NOTE: CMAKE_SOURCE_DIR = <repo>/duckdb-ext (because cmake -S
# duckdb-ext is the configure command used by Task 2 / CI). The correct paths
# are ${CMAKE_SOURCE_DIR}/vendor/null.txt and .../vendor/append_metadata.cmake
# — NOT the double-nested "duckdb-ext/vendor/..." that appears in the RESEARCH
# §Code Examples snippet (that snippet assumes CMAKE_SOURCE_DIR = <repo>,
# which is WRONG for this layout). Using the double-nested path causes
# append_metadata.cmake to read a missing NULL_FILE, silently corrupting the
# footer padding so the unsigned LOAD later fails AppearsValid().
# ---------------------------------------------------------------------------
add_custom_command(
    TARGET loom_loadable_extension
    POST_BUILD
    COMMAND ${CMAKE_COMMAND}
        -DABI_TYPE=CPP
        -DEXTENSION=$<TARGET_FILE:loom_loadable_extension>
        -DPLATFORM_FILE=${CMAKE_BINARY_DIR}/duckdb_platform_out
        -DVERSION_FIELD=v1.5.3
        -DEXTENSION_VERSION=""
        -DNULL_FILE=${CMAKE_SOURCE_DIR}/vendor/null.txt
        -P ${CMAKE_SOURCE_DIR}/vendor/append_metadata.cmake
    COMMENT "Stamping DuckDB extension footer (version=v1.5.3, platform=${DUCKDB_PLATFORM}, abi=CPP)"
)
