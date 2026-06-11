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
# CMAKE_SOURCE_DIR = <repo>/contrib/duckdb-ext   (because cmake -S contrib/duckdb-ext)
# Workspace Cargo.toml is two directories up.
# ---------------------------------------------------------------------------
set(WORKSPACE_ROOT "${CMAKE_SOURCE_DIR}/../..")
set(WORKSPACE_CARGO_TOML "${WORKSPACE_ROOT}/Cargo.toml")
set(LIBLOOM_FFI "${WORKSPACE_ROOT}/target/release/libloom_ffi.a")

# ---------------------------------------------------------------------------
# Step 1: cargo build trigger — ensures the sidecar staticlib is fresh before link.
# D-03: the staticlib must be rebuilt before the C++ extension links against it.
# Phase 101: sidecar-only mode is now the only code path.
# ---------------------------------------------------------------------------
add_custom_command(
    OUTPUT "${LIBLOOM_FFI}"
    COMMAND cargo build -p loom-ffi --release
            --manifest-path "${WORKSPACE_CARGO_TOML}"
    WORKING_DIRECTORY "${WORKSPACE_ROOT}"
    COMMENT "Building libloom_ffi.a (sidecar-only staticlib)"
)
add_custom_target(loom_ffi_build ALL
    DEPENDS "${LIBLOOM_FFI}"
)

# ---------------------------------------------------------------------------
# Step 2: The DuckDB extension shared library.
# Source: single C++ file.
# Includes: duckdb.hpp (amalgam) and loom.h (sidecar FFI header).
# Phase 101: LOOM_SIDECAR_ONLY is always defined — the only code path.
# ---------------------------------------------------------------------------
add_library(loom_loadable_extension SHARED
    "${CMAKE_SOURCE_DIR}/loom_extension.cpp"
)

add_dependencies(loom_loadable_extension loom_ffi_build)

target_include_directories(loom_loadable_extension PRIVATE
    "${CMAKE_SOURCE_DIR}/vendor/duckdb-src"
    "${WORKSPACE_ROOT}/crates/loom-ffi/include"
)
target_compile_definitions(loom_loadable_extension PRIVATE LOOM_SIDECAR_ONLY)

# Link the Rust staticlib.  Do NOT link libduckdb — DuckDB symbols are resolved
# at dlopen time from the CLI binary.
target_link_libraries(loom_loadable_extension PRIVATE
    "${LIBLOOM_FFI}"
)

# ---------------------------------------------------------------------------
# Step 3: Symbol resolution and hiding.
#
# macOS: -undefined dynamic_lookup lets DuckDB symbols resolve from the CLI at
#        runtime.  -exported_symbol restricts the export list to the named entry
#        symbol.
# Linux: -fvisibility=hidden + --exclude-libs,ALL achieves the same hiding.
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

file(WRITE "${CMAKE_BINARY_DIR}/duckdb_platform_out" "${DUCKDB_PLATFORM}")

message(STATUS "DuckDB extension platform: ${DUCKDB_PLATFORM}")

# ---------------------------------------------------------------------------
# Step 6: Footer-stamp POST_BUILD (the unsigned-load enabler).
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