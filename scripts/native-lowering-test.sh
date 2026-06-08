#!/usr/bin/env bash
# native-lowering-test.sh - Phase 14 verifier-gated textual MLIR lowering gate.

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

info() { echo "${YLW}[native-lowering]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
skip() { echo "${YLW}[SKIP]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

find_mlir_opt() {
    if command -v mlir-opt >/dev/null 2>&1; then
        command -v mlir-opt
        return 0
    fi
    for candidate in \
        /opt/homebrew/opt/llvm/bin/mlir-opt \
        /usr/local/opt/llvm/bin/mlir-opt; do
        if [ -x "${candidate}" ]; then
            echo "${candidate}"
            return 0
        fi
    done
    return 1
}

PHASE_DIR=".planning/phases/14-mlir-native-lowering-spike"
CONTRACT="${PHASE_DIR}/14-LOWERING-CONTRACT.md"

echo "=== Loom Phase 14 native-lowering gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 14 lowering docs..."
for file in \
    "${PHASE_DIR}/14-RESEARCH.md" \
    "${PHASE_DIR}/14-CONTEXT.md" \
    "${CONTRACT}" \
    "${PHASE_DIR}/14-01-PLAN.md" \
    "${PHASE_DIR}/14-02-PLAN.md" \
    "${PHASE_DIR}/14-03-PLAN.md"; do
    if [ ! -f "${file}" ]; then
        fail "required native-lowering artifact missing: ${file}"
    fi
done
ok "required Phase 14 artifacts exist"

info "Checking LOWER requirement IDs..."
for id in LOWER-01 LOWER-02 LOWER-03 LOWER-04; do
    rg -q "${id}" .planning/REQUIREMENTS.md .planning/ROADMAP.md "${PHASE_DIR}" \
        || fail "missing ${id} in Phase 14 planning docs"
done
ok "LOWER-01..LOWER-04 are present"

info "Checking textual lowering contract markers..."
rg -q "verify_l2_core" "${CONTRACT}" \
    || fail "contract missing verifier precondition"
rg -q "func" "${CONTRACT}" \
    || fail "contract missing func dialect"
rg -q "arith" "${CONTRACT}" \
    || fail "contract missing arith dialect"
rg -q "scf" "${CONTRACT}" \
    || fail "contract missing scf dialect"
rg -q "memref" "${CONTRACT}" \
    || fail "contract missing memref dialect"
ok "contract markers are present"

info "Running focused Rust native_lowering tests..."
cargo test -p loom-core native_lowering
ok "cargo test -p loom-core native_lowering"

if mlir_opt="$(find_mlir_opt)"; then
    info "Running optional mlir-opt parse validation..."
    tmp_mlir="$(mktemp "${TMPDIR:-/tmp}/loom-native-lowering.XXXXXX.mlir")"
    trap 'rm -f "${tmp_mlir}"' EXIT
    cat >"${tmp_mlir}" <<'MLIR'
module {
  func.func @loom_l2core_copy_i32(%input: memref<?xi32>, %output: memref<?xi32>, %rows: index) {
    %c0 = arith.constant 0 : index
    %c1 = arith.constant 1 : index
    scf.for %i = %c0 to %rows step %c1 {
      %v = memref.load %input[%i] : memref<?xi32>
      memref.store %v, %output[%i] : memref<?xi32>
    }
    return
  }
}
MLIR
    "${mlir_opt}" "${tmp_mlir}" >/dev/null
    ok "optional mlir-opt textual MLIR validation"
else
    skip "mlir-opt not installed; optional textual MLIR validation skipped"
fi

echo ""
echo "${GRN}=== Phase 14 native-lowering gate PASSED ===${RST}"
