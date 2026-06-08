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

ALLOW_SOLVER_SKIP="${LOOM_ALLOW_SOLVER_SKIP:-0}"

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
    if [ "${ALLOW_SOLVER_SKIP}" != "1" ]; then
        fail "bitwuzla unavailable; solver evidence is required. Run: mise run external-tools"
    fi
    skip "bitwuzla unavailable; solver evidence skipped by explicit LOOM_ALLOW_SOLVER_SKIP=1"
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

info "Checking CLI solver-backed artifact report..."
cargo run -q -p loom-fixtures --bin emit_duckdb_payloads >/dev/null
cli_output="$(LOOM_REQUIRE_SOLVER=1 cargo run -q --bin loom -- verify-artifact --solver-bitwuzla --l2core-sample target/loom-duckdb-fixtures/bitpack-i32.loom)"
grep -q "artifact_verification_mode: solver-backed" <<<"${cli_output}" \
    || fail "CLI solver-backed mode missing"
grep -q "solver_primary_backend: bitwuzla" <<<"${cli_output}" \
    || fail "CLI missing Bitwuzla primary backend"
grep -q "solver_backend: bitwuzla" <<<"${cli_output}" \
    || fail "CLI solver report missing Bitwuzla backend"
grep -q "constraint_status: discharged" <<<"${cli_output}" \
    || fail "CLI solver report did not discharge constraints"
grep -q "production_discharge_ready: true" <<<"${cli_output}" \
    || fail "CLI did not expose production discharge readiness"
ok "CLI solver-backed artifact report"

echo ""
echo "${GRN}=== Phase 19 solver-backed verifier gate PASSED ===${RST}"
