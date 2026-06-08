#!/usr/bin/env bash
# production-native-lowering-test.sh - Phase 20 production native-lowering gate.

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

info() { echo "${YLW}[production-native]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
skip() { echo "${YLW}[SKIP]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

. "${REPO_ROOT}/scripts/toolchain-common.sh"

echo "=== Loom Phase 20 production native-lowering gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running production native-lowering core tests..."
cargo test -p loom-core --test production_native_lowering
cargo test -p loom-core --test decode_dialect
cargo test -p loom-core --test arrow_buffer_lowering
cargo test -p loom-core --test production_native_kernels
ok "Phase 20 loom-core tests"

info "Running production MLIR validation tests..."
cargo test -p loom-native-melior --test production_pipeline
ok "loom-native-melior production_pipeline"

info "Checking managed MLIR/LLVM toolchain for production validation..."
set +e
llvm_bin_dir="$(toolchain_llvm_bin_dir)"
tool_status=$?
set -e
if [ "${tool_status}" -eq 2 ]; then
    skip "production MLIR validation skipped by explicit LOOM_ALLOW_NATIVE_TOOL_SKIP=1"
    echo ""
    echo "${GRN}=== Phase 20 production native-lowering gate PASSED WITH SKIP ===${RST}"
    exit 0
elif [ "${tool_status}" -ne 0 ]; then
    fail "managed MLIR/LLVM toolchain is unavailable or incompatible"
fi

ok "compatible LLVM/MLIR ${LOOM_EXPECTED_MLIR_MAJOR:-22} toolchain detected"
export PATH="${llvm_bin_dir}:${PATH}"

info "Running strict production MLIR validation tests..."
cargo test -p loom-native-melior --test production_pipeline
ok "strict production MLIR validation"

echo ""
echo "${GRN}=== Phase 20 production native-lowering gate PASSED ===${RST}"
