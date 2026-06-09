#!/usr/bin/env bash
# lmc2-arrow-semantic-container-test.sh - Phase 33 LMC2 wrapper gate.

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

info() { echo "${YLW}[lmc2-arrow-semantic]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

check_marker() {
    local pattern="$1"
    local file="$2"
    local label="$3"
    rg -q --fixed-strings "${pattern}" "${file}" || fail "missing ${label}: ${pattern} in ${file}"
}

echo "=== Loom Phase 33 LMC2 Arrow semantic wrapper gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

info "Checking LMC2 wrapper and verifier markers..."
check_marker "LMC2" crates/loom-core/src/arrow_semantic_codec.rs "LMC2 codec marker"
check_marker "is_arrow_semantic_container" crates/loom-core/src/arrow_semantic_codec.rs "LMC2 marker helper"
check_marker "verify_arrow_semantic_container_artifact" crates/loom-core/src/artifact_verifier.rs "LMC2 verifier routing"
check_marker "ArtifactVerificationFacts::new(\"LMC2\")" crates/loom-core/src/artifact_verifier.rs "LMC2 verifier facts"
check_marker "LMC2(LMA1)" ingress/loom-source-ingress/src/lib.rs "source emission display"
check_marker "LMC2-wrapped LMA1 semantic emission" ingress/loom-parquet-ingress/src/source_contract.rs "Parquet LMC2 report wording"
check_marker "LMC2-wrapped LMA1 semantic emission" ingress/loom-lance-ingress/src/source_contract.rs "Lance LMC2 report wording"
check_marker "LMC2-wrapped LMA1 semantic emission" ingress/loom-vortex-ingress/src/source_contract.rs "Vortex LMC2 report wording"
ok "LMC2 markers are present"

info "Running core wrapper and artifact verifier tests..."
cargo test -p loom-core --test arrow_semantic
cargo test -p loom-core --test artifact_verifier
ok "core LMC2 wrapper tests"

info "Checking CLI artifact verification visibility for LMC2..."
cargo run -q -p loom-fixtures --bin emit_arrow_semantic_lmc2_sql_fixture -- "${TMP_DIR}" >/dev/null
cli_output="$(cargo run -q -p loom-cli -- verify-artifact "${TMP_DIR}/logical-date32-lmc2.loom")"
grep -q "artifact: LMC2" <<<"${cli_output}" || fail "CLI output did not identify artifact: LMC2"
grep -q "payload: Arrow semantic payload" <<<"${cli_output}" || fail "CLI output did not identify Arrow semantic payload"
grep -q "container_version: 1" <<<"${cli_output}" || fail "CLI output did not identify LMC2 version"
grep -q "row_count_bound: 3" <<<"${cli_output}" || fail "CLI output did not identify row count"
if grep -qi "native ready" <<<"${cli_output}"; then
    fail "CLI output must not claim native ready for LMC2 Arrow semantic artifacts"
fi
ok "CLI output identifies LMC2 wrapper facts without native overclaim"

info "Running source handoff tests for default LMC2 emission..."
cargo test -p loom-parquet-ingress --test source_ingress_handoff
cargo test -p loom-lance-ingress --test source_ingress_handoff
cargo test -p loom-vortex-ingress --test source_ingress_handoff
ok "source adapters emit verifier-accepted LMC2 wrappers"

echo ""
echo "${GRN}=== LMC2 Arrow semantic wrapper gate PASSED ===${RST}"
