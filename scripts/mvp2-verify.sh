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

echo "=== Loom MVP2 release gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running inherited MVP1 release gate..."
bash scripts/mvp1-verify.sh
ok "scripts/mvp1-verify.sh"

info "Running Phase 42 verified/native coverage expansion gate..."
bash scripts/verified-native-coverage-expansion-test.sh
ok "scripts/verified-native-coverage-expansion-test.sh"

echo ""
echo "${GRN}=== MVP2 release gate PASSED ===${RST}"
