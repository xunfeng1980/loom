#!/usr/bin/env bash
# native-hardening-test.sh - Phase 25 DuckDB native/fallback hardening gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

if [ -t 1 ] && command -v tput >/dev/null 2>&1; then
    GRN="$(tput setaf 2)"
    YLW="$(tput setaf 3)"
    RED="$(tput setaf 1)"
    RST="$(tput sgr0)"
else
    GRN=""
    YLW=""
    RED=""
    RST=""
fi

info() { echo "${YLW}[native-hardening]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

echo "=== Loom Phase 25 native hardening gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

if rg -n "LOOM_DUCKDB_TEST_USE_NATIVE_FACTS|test-native-facts|duckdb_runtime_clear_native_preparation_cache_for_test|duckdb_runtime_corrupt_cached_canonical_input_for_test" \
    duckdb-ext crates/loom-ffi scripts/duckdb-native-integration-test.sh; then
    fail "DuckDB native path still references removed LMC1 raw-copy/test-facts cache controls"
fi
ok "removed LMC1 raw-copy native controls are absent"

info "Running route-aware DuckDB SQL integration gate..."
bash scripts/duckdb-native-integration-test.sh
ok "DuckDB native SQL integration gate passed"

info "Running FFI route, native-buffer, and non-cacheable hardening tests..."
cargo test -p loom-ffi --test duckdb_runtime
cargo test -p loom-ffi --test duckdb_runtime_ffi
cargo test -p loom-ffi --test duckdb_runtime_cache
ok "FFI native route hardening tests passed"

info "Running production Arrow semantic route regression tests..."
cargo test -p loom-core --test native_arrow_semantic_codegen_stability
cargo test -p loom-native-melior --features melior --test production_arrow_semantic_route
ok "production route regressions passed"

info "Checking public SQL/API creep gates..."
route_prefix="loom_scan_"
for suffix in native interpreter fallback cache; do
    if rg -n "${route_prefix}${suffix}" scripts/native-hardening-test.sh duckdb-ext/loom_extension.cpp crates/loom-ffi/include/loom.h; then
        fail "found forbidden public route function marker"
    fi
done

echo ""
echo "${GRN}=== Phase 25 native hardening gate PASSED ===${RST}"
