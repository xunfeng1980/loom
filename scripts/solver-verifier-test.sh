#!/usr/bin/env bash
# solver-verifier-test.sh - Phase 19 solver-backed verifier gate.

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

info() { echo "${YLW}[solver-verifier]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
skip() { echo "${YLW}[SKIP]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

STRICT="${LOOM_REQUIRE_SOLVER:-0}"

echo "=== Loom Phase 19 solver-backed verifier gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running core solver contract tests..."
cargo test -p loom-core --test solver_contract
ok "cargo test -p loom-core --test solver_contract"

info "Running deterministic SMT-LIB emitter tests..."
cargo test -p loom-core --test smtlib_emitter
ok "cargo test -p loom-core --test smtlib_emitter"

info "Running solver backend crate tests..."
cargo test -p loom-solver-smt
ok "cargo test -p loom-solver-smt"

info "Checking Bitwuzla discovery..."
if ! command -v bitwuzla >/dev/null 2>&1; then
    if [ "${STRICT}" = "1" ]; then
        fail "bitwuzla unavailable; strict solver evidence required by LOOM_REQUIRE_SOLVER=1"
    fi
    skip "bitwuzla unavailable; strict solver evidence skipped"
    echo ""
    echo "${GRN}=== Phase 19 solver-backed verifier gate PASSED WITH SKIP ===${RST}"
    exit 0
fi

bitwuzla_path="$(command -v bitwuzla)"
bitwuzla_version="$(bitwuzla --version 2>&1 | head -n 1)"
ok "bitwuzla detected at ${bitwuzla_path}: ${bitwuzla_version}"

info "Running focused Bitwuzla execution tests..."
LOOM_REQUIRE_SOLVER=1 cargo test -p loom-solver-smt bitwuzla
ok "Bitwuzla execution tests"

echo ""
echo "${GRN}=== Phase 19 solver-backed verifier gate PASSED ===${RST}"
