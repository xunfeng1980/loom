#!/usr/bin/env bash
# full-verifier-test.sh - Phase 13 full Loom verifier foundation gate.

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

info() { echo "${YLW}[full-verifier]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
skip() { echo "${YLW}[SKIP]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/13-full-loom-verifier"
SPEC="${PHASE_DIR}/13-VERIFIER-SPEC.md"
OBLIGATIONS="${PHASE_DIR}/13-PROOF-OBLIGATIONS.md"
LEAN_FILE="formal/lean/LoomCore.lean"
TLA_FILE="specs/tla/LoomVerifierPipeline.tla"
TLA_CFG="specs/tla/LoomVerifierPipeline.cfg"

echo "=== Loom Phase 13 full-verifier gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 13 verifier documents and formal artifacts..."
for file in "${SPEC}" "${OBLIGATIONS}" "${LEAN_FILE}" "${TLA_FILE}" "${TLA_CFG}"; do
    if [ ! -f "${file}" ]; then
        fail "required verifier artifact missing: ${file}"
    fi
done
ok "required verifier artifacts exist"

info "Checking VERIFIER requirement IDs..."
for id in VERIFIER-01 VERIFIER-02 VERIFIER-03 VERIFIER-04 VERIFIER-05 VERIFIER-06 VERIFIER-07 VERIFIER-08 VERIFIER-09 VERIFIER-10; do
    rg -q "${id}" "${OBLIGATIONS}" || fail "missing ${id} in ${OBLIGATIONS}"
done
ok "all VERIFIER-01..VERIFIER-10 IDs are present"

info "Checking formal scaffold names..."
rg -q "accepted_program_safe" "${LEAN_FILE}" \
    || fail "Lean scaffold missing accepted_program_safe"
rg -q "builder_events_well_formed" "${LEAN_FILE}" \
    || fail "Lean scaffold missing builder_events_well_formed"
rg -q "LoweredImpliesVerified" "${TLA_FILE}" \
    || fail "TLA model missing LoweredImpliesVerified"
rg -q "LoweredImpliesVerified" "${TLA_CFG}" \
    || fail "TLA cfg missing LoweredImpliesVerified invariant"
ok "formal scaffold names are present"

if [ -f crates/loom-core/tests/l2_core_model.rs ]; then
    info "Running L2Core model tests..."
    cargo test -p loom-core --test l2_core_model
    ok "cargo test -p loom-core --test l2_core_model"
fi

if [ -f crates/loom-core/tests/full_verifier.rs ]; then
    info "Running executable full verifier tests..."
    cargo test -p loom-core --test full_verifier
    ok "cargo test -p loom-core --test full_verifier"
fi

info "Checking CLI full-verifier sample..."
cargo run --bin loom -- --help | rg -q "verify-l2core" \
    || fail "loom help does not expose verify-l2core"
cargo run --bin loom -- verify-l2core --sample >/dev/null
ok "loom verify-l2core --sample"

if command -v lean >/dev/null 2>&1; then
    info "Running Lean scaffold check..."
    lean "${LEAN_FILE}"
    ok "lean ${LEAN_FILE}"
else
    skip "lean not installed; syntax check skipped"
fi

if command -v tlc >/dev/null 2>&1; then
    info "Running TLC lifecycle model check..."
    tlc -config "${TLA_CFG}" "${TLA_FILE}"
    ok "tlc ${TLA_FILE}"
else
    skip "tlc not installed; TLA model execution skipped"
fi

echo ""
echo "${GRN}=== Phase 13 full-verifier gate PASSED ===${RST}"

