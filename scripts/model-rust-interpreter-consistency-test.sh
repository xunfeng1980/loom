#!/usr/bin/env bash
# model-rust-interpreter-consistency-test.sh - Phase 39 model/Rust interpreter gate.

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

info() { echo "${YLW}[model-rust-consistency]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

info "Running L2Core reference executor tests..."
cargo test -p loom-core --test l2_core_reference_executor
ok "cargo test -p loom-core --test l2_core_reference_executor"

info "Running model/Rust interpreter trace consistency tests..."
cargo test -p loom-core --test l2_core_interpreter_consistency
ok "cargo test -p loom-core --test l2_core_interpreter_consistency"

info "Checking observer/reference separation markers..."
rg -q "Observer-only production trace subject under test" crates/loom-core/tests/l2_core_interpreter_consistency.rs \
    || fail "missing observer-only production trace subject marker"
rg -q "not call reference executor code" crates/loom-core/tests/l2_core_interpreter_consistency.rs \
    || fail "missing does-not-call-reference marker"
rg -q "differential oracle, not the production interpreter" crates/loom-core/src/l2_core_reference_executor.rs \
    || fail "missing reference oracle boundary marker"
ok "observer/reference separation markers"

echo "${GRN}=== Model/Rust interpreter consistency gate PASSED ===${RST}"
