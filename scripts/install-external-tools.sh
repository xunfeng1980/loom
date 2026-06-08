#!/usr/bin/env bash
# install-external-tools.sh - install/check managed non-Rust verifier/backend tools.

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

info() { echo "${YLW}[external-tools]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

EXPECTED_MLIR_MAJOR="${LOOM_EXPECTED_MLIR_MAJOR:-22}"

echo "=== Loom external tool installer ==="
echo "Repository: ${REPO_ROOT}"
echo "Expected LLVM/MLIR major: ${EXPECTED_MLIR_MAJOR}"
echo ""

if ! command -v brew >/dev/null 2>&1; then
    fail "Homebrew is required to install LLVM/MLIR and Bitwuzla on this platform"
fi

ensure_formula() {
    local formula="$1"
    if brew list --versions "${formula}" >/dev/null 2>&1; then
        ok "${formula} already installed: $(brew list --versions "${formula}")"
    else
        info "Installing ${formula}..."
        brew install "${formula}"
        ok "brew install ${formula}"
    fi
}

ensure_formula llvm
ensure_formula bitwuzla

LLVM_PREFIX="$(brew --prefix llvm)"
LLVM_BIN="${LLVM_PREFIX}/bin"
export PATH="${LLVM_BIN}:${PATH}"

for tool in llvm-config mlir-opt mlir-translate lli; do
    if [ ! -x "${LLVM_BIN}/${tool}" ]; then
        fail "${tool} missing from ${LLVM_BIN}; reinstall Homebrew llvm"
    fi
done

llvm_version="$("${LLVM_BIN}/llvm-config" --version)"
llvm_major="$(printf '%s\n' "${llvm_version}" | sed -E 's/[^0-9]*([0-9]+).*/\1/')"
if [ "${llvm_major}" != "${EXPECTED_MLIR_MAJOR}" ]; then
    fail "detected LLVM/MLIR major ${llvm_major}, expected ${EXPECTED_MLIR_MAJOR}"
fi
ok "LLVM/MLIR ${llvm_version} tools available at ${LLVM_BIN}"

if ! command -v bitwuzla >/dev/null 2>&1; then
    fail "bitwuzla missing after brew install"
fi
ok "Bitwuzla available: $(command -v bitwuzla) ($(bitwuzla --version 2>&1 | head -n 1))"

if command -v z3 >/dev/null 2>&1; then
    ok "Z3 declared-backend binary visible: $(command -v z3) ($(z3 --version 2>&1 | head -n 1))"
else
    info "Z3 is not required for Phase 19 execution; install when the Z3 adapter becomes active"
fi

echo ""
echo "${GRN}=== External tools installed ===${RST}"
