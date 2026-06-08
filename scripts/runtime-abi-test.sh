#!/usr/bin/env bash
# runtime-abi-test.sh - Phase 22 host runtime ABI and policy gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

if [ -t 1 ] && command -v tput >/dev/null 2>&1; then
    GRN="$(tput setaf 2)"
    YLW="$(tput setaf 3)"
    RST="$(tput sgr0)"
else
    GRN=""
    YLW=""
    RST=""
fi

info() { echo "${YLW}[runtime-abi]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }

echo "=== Loom Phase 22 runtime ABI gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running runtime ABI contract tests..."
cargo test -p loom-core --test runtime_abi_contract
ok "runtime_abi_contract"

info "Running runtime execution policy tests..."
cargo test -p loom-core --test runtime_execution_policy
ok "runtime_execution_policy"

info "Running runtime scan planning tests..."
cargo test -p loom-core --test runtime_scan_planning
ok "runtime_scan_planning"

info "Running runtime cache key tests..."
cargo test -p loom-core --test runtime_cache_key
ok "runtime_cache_key"

info "Checking runtime ABI sketch is host-neutral..."
rg -n "LoomRuntimePlan|LoomRuntimeScan|LoomRuntimeWorker|loom_runtime|ArrowArray|ArrowSchema|Phase 22" \
    crates/loom-ffi/include/loom_runtime.h >/dev/null
if rg -n "DuckDB|StarRocks|MLIR|LLVM|Vortex" crates/loom-ffi/include/loom_runtime.h >/dev/null; then
    echo "[FAIL] loom_runtime.h mentions host/backend/source-format types" >&2
    exit 1
fi
ok "loom_runtime.h host-neutral sketch"

echo ""
echo "${GRN}=== Phase 22 runtime ABI gate PASSED ===${RST}"
