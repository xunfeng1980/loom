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

EXPECTED_MLIR_MAJOR="22"
STRICT="${LOOM_REQUIRE_MELIOR_JIT:-0}"

find_tool() {
    local name="$1"
    if command -v "${name}" >/dev/null 2>&1; then
        command -v "${name}"
        return 0
    fi
    for candidate in \
        "/opt/homebrew/opt/llvm/bin/${name}" \
        "/usr/local/opt/llvm/bin/${name}"; do
        if [ -x "${candidate}" ]; then
            echo "${candidate}"
            return 0
        fi
    done
    return 1
}

major_version() {
    sed -E 's/[^0-9]*([0-9]+).*/\1/'
}

strict_or_skip() {
    local message="$1"
    if [ "${STRICT}" = "1" ]; then
        fail "${message}"
    fi
    skip "${message}"
}

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

info "Checking optional MLIR/LLVM toolchain..."
llvm_config="$(find_tool llvm-config || true)"
mlir_opt="$(find_tool mlir-opt || true)"
mlir_translate="$(find_tool mlir-translate || true)"
lli="$(find_tool lli || true)"

if [ -z "${llvm_config}" ] || [ -z "${mlir_opt}" ] || [ -z "${mlir_translate}" ] || [ -z "${lli}" ]; then
    strict_or_skip "compatible LLVM/MLIR ${EXPECTED_MLIR_MAJOR} toolchain unavailable; optional feature-enabled melior evidence skipped"
    echo ""
    echo "${GRN}=== Phase 16 melior/LLVM/JIT backend gate PASSED WITH SKIP ===${RST}"
    exit 0
fi

llvm_version="$("${llvm_config}" --version)"
llvm_major="$(printf '%s\n' "${llvm_version}" | major_version)"
if [ "${llvm_major}" != "${EXPECTED_MLIR_MAJOR}" ]; then
    strict_or_skip "detected LLVM/MLIR major ${llvm_major}, expected ${EXPECTED_MLIR_MAJOR}; optional feature-enabled melior evidence skipped"
    echo ""
    echo "${GRN}=== Phase 16 melior/LLVM/JIT backend gate PASSED WITH SKIP ===${RST}"
    exit 0
fi

ok "compatible LLVM/MLIR ${EXPECTED_MLIR_MAJOR} toolchain detected"

info "Running feature-enabled melior jit equivalence tests..."
cargo test -p loom-native-melior --features melior jit
ok "cargo test -p loom-native-melior --features melior jit"

echo ""
echo "${GRN}=== Phase 16 melior/LLVM/JIT backend gate PASSED ===${RST}"
