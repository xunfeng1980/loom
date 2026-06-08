#!/usr/bin/env bash
# vortex-encoding-coverage-test.sh - Phase 21 expanded Vortex encoding coverage gate.

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

info() { echo "${YLW}[vortex-coverage]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/21-expanded-vortex-encoding-coverage"

echo "=== Loom Phase 21 expanded Vortex encoding coverage gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 21 required planning artifacts..."
for file in \
    "${PHASE_DIR}/21-COVERAGE-MATRIX.md" \
    "${PHASE_DIR}/21-COVERAGE-REPORT.md" \
    "${PHASE_DIR}/21-SUMMARY.md"; do
    if [ ! -f "${file}" ]; then
        fail "required Phase 21 artifact missing: ${file}"
    fi
done

rg -q "Coverage Matrix" "${PHASE_DIR}/21-COVERAGE-REPORT.md" \
    || fail "coverage report missing Coverage Matrix"
rg -q "Accepted Emission Matrix" "${PHASE_DIR}/21-COVERAGE-REPORT.md" \
    || fail "coverage report missing Accepted Emission Matrix"
rg -q "Unsupported and Deferred Matrix" "${PHASE_DIR}/21-COVERAGE-REPORT.md" \
    || fail "coverage report missing Unsupported and Deferred Matrix"
rg -q "Phase 22 ABI Handoff" "${PHASE_DIR}/21-COVERAGE-REPORT.md" \
    || fail "coverage report missing Phase 22 ABI Handoff"
rg -q "Phase 23 Backend Handoff" "${PHASE_DIR}/21-COVERAGE-REPORT.md" \
    || fail "coverage report missing Phase 23 Backend Handoff"
rg -q "Self-Check: PASSED" "${PHASE_DIR}/21-SUMMARY.md" \
    || fail "summary missing passing self-check"
ok "Phase 21 docs are present"

info "Checking coverage implementation markers..."
rg -q "pub struct VortexEncodingCoverage" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing VortexEncodingCoverage"
rg -q "pub enum VortexLoweringDisposition" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing VortexLoweringDisposition"
rg -q "array_encoding" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing array encoding classifier"
ok "implementation markers are present"

info "Running focused Phase 21 ingress tests..."
cargo test -p loom-vortex-ingress --test reader_facts_contract
cargo test -p loom-vortex-ingress --test nullable_primitive_coverage
cargo test -p loom-vortex-ingress --test chunked_primitive_coverage
cargo test -p loom-vortex-ingress --test dictionary_runend_coverage
cargo test -p loom-vortex-ingress --test bitpack_for_coverage
ok "focused Phase 21 ingress tests"

info "Running artifact verifier and native fail-closed handoff tests..."
cargo test -p loom-core --test artifact_verifier
cargo test -p loom-core --test production_native_kernels
ok "artifact verifier and native fail-closed tests"

info "Checking matrix markers..."
rg -qi "nullable primitive" "${PHASE_DIR}/21-COVERAGE-MATRIX.md" \
    || fail "matrix missing nullable primitive"
rg -qi "dictionary" "${PHASE_DIR}/21-COVERAGE-MATRIX.md" \
    || fail "matrix missing dictionary"
rg -qi "bitpack" "${PHASE_DIR}/21-COVERAGE-MATRIX.md" \
    || fail "matrix missing bitpack"
rg -q "fail-closed/deferred" "${PHASE_DIR}/21-COVERAGE-MATRIX.md" \
    || fail "matrix missing fail-closed/deferred"
ok "coverage matrix markers"

echo ""
echo "${GRN}=== Phase 21 expanded Vortex encoding coverage gate PASSED ===${RST}"
