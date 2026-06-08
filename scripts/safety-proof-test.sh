#!/usr/bin/env bash
# safety-proof-test.sh - Phase 12 safety proof gate.

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

info() { echo "${YLW}[safety-proof]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/12-formal-verifier-safety-proof-mvp"
CONTRACT="${PHASE_DIR}/12-SAFETY-CONTRACT.md"
OBLIGATIONS="${PHASE_DIR}/12-PROOF-OBLIGATIONS.md"

echo "=== Loom Phase 12 safety proof gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking proof documents..."
for file in "${CONTRACT}" "${OBLIGATIONS}"; do
    if [ ! -f "${file}" ]; then
        fail "required proof document missing: ${file}"
    fi
done
ok "required proof documents exist"

info "Checking proof obligation IDs..."
for id in OBL-12-01 OBL-12-02 OBL-12-03 OBL-12-04 OBL-12-05 OBL-12-06 OBL-12-07 OBL-12-08 OBL-12-09; do
    rg -q "${id}" "${OBLIGATIONS}" || fail "missing ${id} in ${OBLIGATIONS}"
done
ok "all OBL-12-01..OBL-12-09 IDs are present"

info "Checking static unsafe and panic-boundary invariants..."
rg -q '#!\[forbid\(unsafe_code\)\]' crates/loom-core/src/lib.rs \
    || fail "loom-core does not forbid unsafe code"
rg -q 'catch_unwind' crates/loom-ffi/src/ffi.rs \
    || fail "loom-ffi does not contain catch_unwind"
ok "loom-core forbids unsafe and loom-ffi contains catch_unwind"

info "Running focused core safety contract tests..."
cargo test -p loom-core --test safety_contract
ok "cargo test -p loom-core --test safety_contract"

info "Running focused FFI safety contract tests..."
cargo test -p loom-ffi ffi_contract
ok "cargo test -p loom-ffi ffi_contract"

info "Running verifier negative descriptor gate..."
bash scripts/verifier-negative-test.sh
ok "scripts/verifier-negative-test.sh"

info "Running container negative gate..."
bash scripts/container-negative-test.sh
ok "scripts/container-negative-test.sh"

echo ""
echo "${GRN}=== Phase 12 safety proof gate PASSED ===${RST}"
