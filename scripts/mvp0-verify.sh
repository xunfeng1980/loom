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

info "Checking loom-ffi has no Vortex/FastLanes dependencies..."
ffi_dep_count="$(cargo tree -p loom-ffi | awk '/vortex|fastlanes/{c++} END{print c+0}')"
if [ "${ffi_dep_count}" != "0" ]; then
    fail "loom-ffi dependency guard failed: found ${ffi_dep_count} vortex/fastlanes entries"
fi
ok "loom-ffi dependency guard printed 0"

info "Checking vortex-file direct dependency is isolated to ingress crate..."
vortex_file_refs="$(rg -n 'vortex-file' Cargo.toml crates/*/Cargo.toml || true)"
unexpected_vortex_file_refs="$(printf '%s\n' "${vortex_file_refs}" | grep -v '^crates/loom-vortex-ingress/Cargo.toml:' || true)"
if [ -n "${unexpected_vortex_file_refs}" ]; then
    fail "vortex-file direct dependency found outside crates/loom-vortex-ingress: ${unexpected_vortex_file_refs}"
fi
ok "vortex-file direct dependency allowlist"

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

info "Running Phase 12 safety proof gate..."
bash scripts/safety-proof-test.sh
ok "scripts/safety-proof-test.sh"

info "Running Phase 13 full-verifier gate..."
bash scripts/full-verifier-test.sh
ok "scripts/full-verifier-test.sh"

info "Running Phase 14 native-lowering gate..."
bash scripts/native-lowering-test.sh
ok "scripts/native-lowering-test.sh"

info "Running Phase 15 real Vortex ingress gate..."
bash scripts/vortex-ingress-test.sh
ok "scripts/vortex-ingress-test.sh"

info "Running Phase 16 melior/LLVM/JIT backend gate..."
bash scripts/melior-jit-test.sh
ok "scripts/melior-jit-test.sh"

info "Running Phase 17 artifact verifier gate..."
bash scripts/artifact-verifier-test.sh
ok "scripts/artifact-verifier-test.sh"

info "Running Phase 18 complete Vortex reader gate..."
bash scripts/complete-vortex-reader-test.sh
ok "scripts/complete-vortex-reader-test.sh"

info "Running Phase 19 solver-backed verifier gate..."
bash scripts/solver-verifier-test.sh
ok "scripts/solver-verifier-test.sh"

info "Running DuckDB SQL smoke test..."
bash scripts/duckdb-smoke-test.sh
ok "scripts/duckdb-smoke-test.sh"

echo ""
echo "${GRN}=== MVP0 release gate PASSED ===${RST}"
