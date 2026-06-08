#!/usr/bin/env bash
# mvp0-verify.sh - one-command release gate for the Loom MVP0 baseline.

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

info() { echo "${YLW}[mvp0-verify]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

echo "=== Loom MVP0 release gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running workspace tests..."
cargo test --workspace
ok "cargo test --workspace"

info "Checking loom-core has no Vortex/FastLanes dependencies..."
dep_count="$(cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}')"
if [ "${dep_count}" != "0" ]; then
    fail "loom-core dependency guard failed: found ${dep_count} vortex/fastlanes entries"
fi
ok "loom-core dependency guard printed 0"

info "Checking loom-fixtures does not use file-backed Vortex APIs..."
set +e
rg -n 'vortex_file|vortex-file|\.vortex|VortexFile|from_path|read_file' crates/loom-fixtures
rg_status=$?
set -e
if [ "${rg_status}" -eq 0 ]; then
    fail "forbidden file-backed Vortex API references found in crates/loom-fixtures"
elif [ "${rg_status}" -eq 1 ]; then
ok "fixture hygiene grep found no forbidden file-backed Vortex APIs"
else
    fail "fixture hygiene grep failed with rg status ${rg_status}"
fi

info "Running verifier negative descriptor gate..."
bash scripts/verifier-negative-test.sh
ok "scripts/verifier-negative-test.sh"

info "Running container negative gate..."
bash scripts/container-negative-test.sh
ok "scripts/container-negative-test.sh"

info "Running DuckDB SQL smoke test..."
bash scripts/duckdb-smoke-test.sh
ok "scripts/duckdb-smoke-test.sh"

echo ""
echo "${GRN}=== MVP0 release gate PASSED ===${RST}"
