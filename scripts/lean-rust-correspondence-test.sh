#!/usr/bin/env bash
# lean-rust-correspondence-test.sh - Phase 37 Lean/Rust verifier correspondence gate.

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

info() { echo "${YLW}[lean-rust-correspondence]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

export PATH="${REPO_ROOT}/.tools/bin:${HOME}/.elan/bin:${PATH}"

if ! command -v lean >/dev/null 2>&1; then
    fail "lean is required. Run: mise run formal-tools"
fi

RUST_REPORT="$(mktemp)"
LEAN_STDOUT="$(mktemp)"
LEAN_REPORT="$(mktemp)"
trap 'rm -f "${RUST_REPORT}" "${LEAN_STDOUT}" "${LEAN_REPORT}"' EXIT

info "Running Rust verifier correspondence corpus..."
LOOM_WRITE_CORRESPONDENCE_REPORT="${RUST_REPORT}" \
    cargo test -p loom-core --test full_verifier lean_rust_correspondence_matrix_matches_expected
ok "Rust correspondence corpus"

info "Running Lean verifier correspondence corpus..."
lean formal/lean/LoomCore.lean >"${LEAN_STDOUT}"
rg '^correspondence:' "${LEAN_STDOUT}" >"${LEAN_REPORT}" \
    || fail "Lean correspondence report emitted no correspondence rows"
ok "Lean correspondence corpus"

info "Comparing Lean and Rust classifications..."
diff -u "${RUST_REPORT}" "${LEAN_REPORT}" \
    || fail "Lean/Rust verifier correspondence divergence"
ok "Lean/Rust accept/reject and reject-code classifications match"

for code in \
    missing-input-capability \
    missing-output-builder \
    invalid-loop-bounds \
    non-monotone-cursor-loop \
    resource-budget-exceeded; do
    rg -q ":${code}$" "${RUST_REPORT}" \
        || fail "required reject code missing from correspondence corpus: ${code}"
done
ok "required reject-code coverage"

echo "${GRN}=== Lean/Rust verifier correspondence gate PASSED ===${RST}"
