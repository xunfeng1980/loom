#!/usr/bin/env bash
# production-backend-test.sh - Phase 23 production native backend gate.

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

info() { echo "${YLW}[production-backend]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
skip() { echo "${YLW}[SKIP]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

. "${REPO_ROOT}/scripts/toolchain-common.sh"

echo "=== Loom Phase 23 production native backend gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running production backend contract tests..."
cargo test -p loom-native-melior --test production_backend_contract
ok "production backend contract"

info "Running decode dialect manifest drift tests..."
cargo test -p loom-native-melior --test decode_dialect_manifest
ok "decode dialect manifest"

info "Running production backend pipeline tests..."
cargo test -p loom-native-melior --test production_backend_pipeline
ok "production backend pipeline"

info "Running production backend JIT seed tests..."
cargo test -p loom-native-melior --test production_backend_jit
ok "production backend JIT seed"

info "Checking managed MLIR/LLVM toolchain for strict ODS validation..."
set +e
llvm_bin_dir="$(toolchain_llvm_bin_dir)"
tool_status=$?
set -e
if [ "${tool_status}" -eq 2 ]; then
    skip "strict ODS validation skipped by explicit LOOM_ALLOW_NATIVE_TOOL_SKIP=1"
    echo ""
    echo "${GRN}=== Phase 23 production native backend gate PASSED WITH SKIP ===${RST}"
    exit 0
elif [ "${tool_status}" -ne 0 ]; then
    fail "managed MLIR/LLVM toolchain is unavailable or incompatible"
fi

export PATH="${llvm_bin_dir}:${PATH}"
mlir_tblgen="$(toolchain_find_tool mlir-tblgen || true)"
if [ -z "${mlir_tblgen}" ]; then
    fail "mlir-tblgen is required for strict ODS validation. Run: mise run external-tools"
fi

llvm_include_dir="$(llvm-config --includedir)"
ods_include_dir="${REPO_ROOT}/crates/loom-native-melior/mlir/include"
ods_ops="${ods_include_dir}/LoomDecode/LoomDecodeOps.td"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/loom-ods-XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT

info "Running mlir-tblgen over LoomDecodeOps.td..."
"${mlir_tblgen}" \
    -I "${ods_include_dir}" \
    -I "${llvm_include_dir}" \
    -gen-op-decls \
    "${ods_ops}" \
    -o "${tmp_dir}/LoomDecodeOps.h.inc"
ok "strict ODS TableGen validation"

echo ""
echo "${GRN}=== Phase 23 production native backend gate PASSED ===${RST}"
