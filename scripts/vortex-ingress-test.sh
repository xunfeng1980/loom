#!/usr/bin/env bash
# vortex-ingress-test.sh - Phase 15 real Vortex ingress boundary gate.

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

info() { echo "${YLW}[vortex-ingress]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/15-real-vortex-file-container-ingress"
CONTRACT="${PHASE_DIR}/15-INGRESS-CONTRACT.md"

echo "=== Loom Phase 15 real Vortex ingress gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 15 ingress docs..."
for file in \
    "${PHASE_DIR}/15-RESEARCH.md" \
    "${PHASE_DIR}/15-CONTEXT.md" \
    "${CONTRACT}"; do
    if [ ! -f "${file}" ]; then
        fail "required ingress artifact missing: ${file}"
    fi
done
rg -q "Dependency Boundary" "${CONTRACT}" || fail "contract missing dependency boundary"
rg -q "Stable Diagnostics" "${CONTRACT}" || fail "contract missing stable diagnostics"
rg -q "fail closed|fail-closed" "${CONTRACT}" "${PHASE_DIR}/15-RESEARCH.md" \
    || fail "Phase 15 docs must mention fail-closed ingress"
ok "required Phase 15 artifacts exist"

info "Checking scoped dependency guard markers..."
rg -q "vortex-file direct dependency allowlist" scripts/check-core-invariants.sh \
    || fail "check-core-invariants missing vortex-file allowlist"
rg -q "loom-ffi has no vortex dependency" scripts/check-core-invariants.sh \
    || fail "check-core-invariants missing loom-ffi dependency guard"
rg -q "vortex-file direct dependency is isolated to ingress crate" scripts/mvp0-verify.sh \
    || fail "mvp0-verify missing vortex-file allowlist"
ok "dependency guard markers are present"

info "Running focused ingress fact tests..."
cargo test -p loom-vortex-ingress ingress_facts
ok "cargo test -p loom-vortex-ingress ingress_facts"

info "Running real file to Loom roundtrip tests..."
cargo test -p loom-vortex-ingress real_file_to_loom
ok "cargo test -p loom-vortex-ingress real_file_to_loom"

info "Generating deterministic ingress fixtures..."
cargo run -p loom-vortex-ingress --bin emit_vortex_ingress_fixtures
test -f fixtures/vortex/int32-flat.vortex || fail "missing generated Vortex fixture"
test -f fixtures/loom/int32-flat.loom || fail "missing generated Loom fixture"
ok "deterministic fixtures generated"

echo ""
echo "${GRN}=== Phase 15 real Vortex ingress gate PASSED ===${RST}"
