#!/usr/bin/env bash
# artifact-verifier-test.sh - Phase 17 unified artifact verifier gate.

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

info() { echo "${YLW}[artifact-verifier]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/17-unified-artifact-verification-pipeline"
CONTRACT="${PHASE_DIR}/17-ARTIFACT-VERIFIER-CONTRACT.md"

echo "=== Loom Phase 17 artifact verifier gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 17 planning artifacts..."
for file in \
    "${PHASE_DIR}/17-RESEARCH.md" \
    "${CONTRACT}" \
    "${PHASE_DIR}/17-01-PLAN.md" \
    "${PHASE_DIR}/17-02-PLAN.md" \
    "${PHASE_DIR}/17-03-PLAN.md" \
    "${PHASE_DIR}/17-04-PLAN.md"; do
    if [ ! -f "${file}" ]; then
        fail "required artifact verifier planning artifact missing: ${file}"
    fi
done
rg -q "Facts Trust Boundary" "${CONTRACT}" \
    || fail "contract missing facts trust boundary"
rg -q "Constraint Discharge Status" "${CONTRACT}" \
    || fail "contract missing constraint discharge status"
rg -q "Lowering Readiness" "${CONTRACT}" \
    || fail "contract missing lowering readiness"
ok "required Phase 17 artifacts exist"

info "Running focused artifact verifier tests..."
cargo test -p loom-core --test artifact_verifier
ok "cargo test -p loom-core --test artifact_verifier"

info "Checking CLI exposes verify-artifact..."
cargo run --bin loom -- --help | rg -q "verify-artifact" \
    || fail "loom help does not expose verify-artifact"
ok "loom help exposes verify-artifact"

info "Generating deterministic Loom fixtures..."
cargo run -q -p loom-fixtures --bin emit_duckdb_payloads >/dev/null
test -f target/loom-duckdb-fixtures/bitpack-i32.loom \
    || fail "missing generated LMC1 fixture"
ok "deterministic Loom fixtures generated"

info "Checking CLI accepted artifact report..."
accepted_output="$(cargo run -q --bin loom -- verify-artifact target/loom-duckdb-fixtures/bitpack-i32.loom)"
grep -q "artifact_verification: pass" <<<"${accepted_output}" \
    || fail "accepted artifact missing pass status"
grep -q "facts: present" <<<"${accepted_output}" \
    || fail "accepted artifact missing facts"
grep -q "payload_kind: LMP1 layout" <<<"${accepted_output}" \
    || fail "accepted artifact missing payload kind"
grep -q "lowering_ready: false" <<<"${accepted_output}" \
    || fail "accepted artifact without L2Core should not be lowering-ready"
ok "CLI accepted artifact report"

info "Checking CLI malformed artifact rejection..."
tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT
printf 'LMC1' >"${tmpdir}/truncated.loom"
set +e
rejected_output="$(cargo run -q --bin loom -- verify-artifact "${tmpdir}/truncated.loom" 2>&1)"
rejected_rc=$?
set -e
if [ "${rejected_rc}" -eq 0 ]; then
    echo "${rejected_output}" >&2
    fail "malformed artifact unexpectedly passed"
fi
grep -q "artifact_verification: fail" <<<"${rejected_output}" \
    || fail "malformed artifact missing fail status"
grep -q "diagnostic: stage=container code=container-shape" <<<"${rejected_output}" \
    || fail "malformed artifact missing container diagnostic"
ok "CLI malformed artifact rejection"

echo ""
echo "${GRN}=== Phase 17 artifact verifier gate PASSED ===${RST}"
