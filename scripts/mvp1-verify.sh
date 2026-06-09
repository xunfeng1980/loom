#!/usr/bin/env bash
# mvp1-verify.sh - one-command MVP1 gate.

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

info() { echo "${YLW}[mvp1-verify]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }

echo "=== Loom MVP1 release gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running inherited MVP0 release gate..."
bash scripts/mvp0-verify.sh
ok "scripts/mvp0-verify.sh"

info "Running MVP1 DuckDB source e2e gate..."
bash scripts/duckdb-source-e2e-test.sh
ok "scripts/duckdb-source-e2e-test.sh"

echo ""
echo "${GRN}=== MVP1 release gate PASSED ===${RST}"
