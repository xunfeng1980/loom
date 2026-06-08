#!/usr/bin/env bash
# melior-jit-test.sh - Phase 16 melior/LLVM/JIT backend gate.

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

info() { echo "${YLW}[melior-jit]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
skip() { echo "${YLW}[SKIP]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

. "${REPO_ROOT}/scripts/toolchain-common.sh"

echo "=== Loom Phase 16 melior/LLVM/JIT backend gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running default loom-native-melior tests..."
cargo test -p loom-native-melior
ok "cargo test -p loom-native-melior"

info "Running focused builder, pipeline, and JIT boundary tests..."
cargo test -p loom-native-melior builder
cargo test -p loom-native-melior pipeline
cargo test -p loom-native-melior jit
ok "builder, pipeline, and JIT boundary tests"

info "Checking managed MLIR/LLVM toolchain..."
set +e
llvm_bin_dir="$(toolchain_llvm_bin_dir)"
tool_status=$?
set -e
if [ "${tool_status}" -eq 2 ]; then
    skip "feature-enabled melior evidence skipped by explicit LOOM_ALLOW_NATIVE_TOOL_SKIP=1"
    echo ""
    echo "${GRN}=== Phase 16 melior/LLVM/JIT backend gate PASSED WITH SKIP ===${RST}"
    exit 0
elif [ "${tool_status}" -ne 0 ]; then
    fail "managed MLIR/LLVM toolchain is unavailable or incompatible"
fi

ok "compatible LLVM/MLIR ${LOOM_EXPECTED_MLIR_MAJOR:-22} toolchain detected"
export PATH="${llvm_bin_dir}:${PATH}"

info "Running feature-enabled melior jit equivalence tests..."
cargo test -p loom-native-melior --features melior jit
ok "cargo test -p loom-native-melior --features melior jit"

echo ""
echo "${GRN}=== Phase 16 melior/LLVM/JIT backend gate PASSED ===${RST}"
