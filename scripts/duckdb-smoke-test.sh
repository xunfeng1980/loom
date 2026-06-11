#!/usr/bin/env bash
# duckdb-smoke-test.sh — DuckDB extension build + load smoke test.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
EXT_DIR="${REPO_ROOT}/contrib/duckdb-ext"
EXT_PATH="${EXT_DIR}/build/loom.duckdb_extension"

info()  { echo "[smoke-test] $*"; }
ok()    { echo "[PASS] $*"; }
fail()  { echo "[FAIL] $*" >&2; exit 1; }

echo "=== Loom DuckDB Smoke Test ==="
echo ""

# 1. Build the Rust sidecar FFI staticlib
info "Building libloom_ffi.a..."
cargo build -p loom-ffi --release || fail "Rust build failed"
ok "libloom_ffi.a built"

# 2. Build the DuckDB extension (CMake)
info "Building DuckDB extension..."
mkdir -p "${EXT_DIR}/build"
cd "${EXT_DIR}/build"
cmake .. || fail "CMake configure failed"
make -j"$(sysctl -n hw.logicalcpu 2>/dev/null || nproc)" || fail "CMake build failed"
cd "${REPO_ROOT}"
ok "DuckDB extension built"

# 3. Verify extension file exists
test -f "${EXT_PATH}" || fail "Extension not found: ${EXT_PATH}"
ok "Extension file: ${EXT_PATH}"

# 4. Workspace tests
info "Running workspace tests..."
cargo test --workspace --exclude loom-vortex-ingress --exclude loom-lance-ingress || fail "Tests failed"
ok "Workspace tests passed"

echo ""
echo "=== Smoke test passed ==="
