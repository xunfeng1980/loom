#!/usr/bin/env bash
# mvp2-verify.sh - one-command MVP2 coverage gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

if [ -t 1 ] && command -v tput >/dev/null 2>&1; then
    GRN="$(tput setaf 2)"
    YLW="$(tput setaf 3)"
    RST="$(tput sgr0)"
else
    GRN=""
    YLW=""
    RST=""
fi

info() { echo "${YLW}[mvp2-verify]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }

echo "=== Loom MVP2 local coverage gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running inherited MVP1 release gate..."
bash scripts/mvp1-verify.sh
ok "scripts/mvp1-verify.sh"

info "Running Phase 42 verified/native coverage expansion gate..."
bash scripts/verified-native-coverage-expansion-test.sh
ok "scripts/verified-native-coverage-expansion-test.sh"

info "Phase 43 StarRocks local/strict runtime integration gate skipped (suspended, see 43-SUSPENSION-NOTE.md)"
ok "Phase 43 skipped — suspended pending live StarRocks runtime"

info "Running Phase 43.1 production native codegen realization gate..."
bash scripts/production-native-codegen-realization-test.sh
ok "scripts/production-native-codegen-realization-test.sh"

info "Running Phase 43.2 production native codegen stabilization gate..."
bash scripts/production-native-codegen-stabilization-test.sh
ok "scripts/production-native-codegen-stabilization-test.sh"

echo ""
echo "${GRN}=== MVP2 local coverage gate PASSED ===${RST}"
echo "Note: live StarRocks runtime evidence is required only when LOOM_REQUIRE_STARROCKS_LIVE=1 and remains a pre-GA blocker otherwise."
