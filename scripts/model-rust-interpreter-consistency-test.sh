#!/usr/bin/env bash
# model-rust-interpreter-consistency-test.sh - Phase 40+ K spec-oracle / native consistency gate.
#
# Replaces the deleted Phase 39 Rust ReferenceExecutor gate.
# Trust root: K is the specification oracle; native execution is validated
# against K inside cargo test.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

# Ensure K tools are on PATH (nix profile or system).
if [ -f /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]; then
    . /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
fi

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

info "Running K spec-oracle vs native trace consistency tests..."
# Strict by default: krun absence → hard fail.
# For local development without K installed, set LOOM_ALLOW_K_ORACLE_SKIP=1
cargo test -p loom-core --test native_arrow_semantic
ok "cargo test -p loom-core --test native_arrow_semantic"

info "Checking K harness integration markers..."
rg -q "kloom_trace_for_program" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing kloom_trace_for_program integration"
rg -q "kloom_harness" crates/loom-core/src/lib.rs \
    || fail "missing kloom_harness module"
rg -q "K spec-oracle" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing K spec-oracle marker"
ok "K harness integration markers"

echo "${GRN}=== Model/native consistency gate PASSED ===${RST}"
