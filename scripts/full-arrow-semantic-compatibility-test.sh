#!/usr/bin/env bash
# full-arrow-semantic-compatibility-test.sh - Phase 31 Arrow semantic source gate.

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

info() { echo "${YLW}[full-arrow-semantic]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

check_marker() {
    local pattern="$1"
    local file="$2"
    local label="$3"
    rg -q --fixed-strings "${pattern}" "${file}" || fail "missing ${label}: ${pattern} in ${file}"
}

echo "=== Loom Phase 31 full Arrow semantic compatibility gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking semantic artifact markers..."
check_marker "ArrowSemantic" crates/loom-source-ingress/src/lib.rs "source emission kind"
check_marker "decode_arrow_semantic_payload" crates/loom-core/src/artifact_verifier.rs "LMA1 verifier routing"
check_marker "encode_arrow_semantic_payload" crates/loom-parquet-ingress/src/source_contract.rs "Parquet semantic emission"
check_marker "encode_arrow_semantic_payload" crates/loom-lance-ingress/src/source_contract.rs "Lance semantic emission"
ok "semantic artifact markers are present"

info "Running core LMA1 and source full-schema tests..."
cargo test -p loom-core --test arrow_semantic
cargo test -p loom-parquet-ingress --test full_arrow_schema_compatibility
cargo test -p loom-lance-ingress --test full_arrow_schema_compatibility
ok "full Arrow semantic source tests"

info "Running source handoff regression tests..."
cargo test -p loom-parquet-ingress --test source_ingress_handoff
cargo test -p loom-lance-ingress --test source_ingress_handoff
ok "source handoff regressions"
