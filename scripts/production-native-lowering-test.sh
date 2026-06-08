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

EXPECTED_MLIR_MAJOR="22"
STRICT="${LOOM_REQUIRE_PRODUCTION_NATIVE:-0}"

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

echo "=== Loom Phase 20 production native-lowering gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running production native-lowering core tests..."
cargo test -p loom-core --test production_native_lowering
cargo test -p loom-core --test decode_dialect
cargo test -p loom-core --test arrow_buffer_lowering
cargo test -p loom-core --test production_native_kernels
ok "Phase 20 loom-core tests"

info "Running skip-aware production MLIR validation tests..."
cargo test -p loom-native-melior --test production_pipeline
ok "loom-native-melior production_pipeline"

info "Checking optional MLIR/LLVM toolchain for strict production validation..."
llvm_config="$(find_tool llvm-config || true)"
mlir_opt="$(find_tool mlir-opt || true)"
mlir_translate="$(find_tool mlir-translate || true)"
lli="$(find_tool lli || true)"

if [ -z "${llvm_config}" ] || [ -z "${mlir_opt}" ] || [ -z "${mlir_translate}" ] || [ -z "${lli}" ]; then
    strict_or_skip "compatible LLVM/MLIR ${EXPECTED_MLIR_MAJOR} toolchain unavailable; production MLIR validation evidence skipped"
    echo ""
    echo "${GRN}=== Phase 20 production native-lowering gate PASSED WITH SKIP ===${RST}"
    exit 0
fi

llvm_version="$("${llvm_config}" --version)"
llvm_major="$(printf '%s\n' "${llvm_version}" | major_version)"
if [ "${llvm_major}" != "${EXPECTED_MLIR_MAJOR}" ]; then
    strict_or_skip "detected LLVM/MLIR major ${llvm_major}, expected ${EXPECTED_MLIR_MAJOR}; production MLIR validation evidence skipped"
    echo ""
    echo "${GRN}=== Phase 20 production native-lowering gate PASSED WITH SKIP ===${RST}"
    exit 0
fi

ok "compatible LLVM/MLIR ${EXPECTED_MLIR_MAJOR} toolchain detected"
export PATH="$(dirname "${llvm_config}"):${PATH}"

info "Running strict production MLIR validation tests..."
cargo test -p loom-native-melior --test production_pipeline
ok "strict production MLIR validation"

echo ""
echo "${GRN}=== Phase 20 production native-lowering gate PASSED ===${RST}"
