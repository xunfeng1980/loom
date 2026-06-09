#!/usr/bin/env bash
# vortex-semantic-compatibility-test.sh - Phase 28 semantic compatibility gate.

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

info() { echo "${YLW}[vortex-semantics]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/28-full-lance-parquet-vortex-semantic-compatibility"
REPORT="${PHASE_DIR}/28-LANCE-PARQUET-VORTEX-SEMANTIC-COMPATIBILITY-REPORT.md"

echo "=== Loom Phase 28 Lance/Parquet/Vortex semantic compatibility gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 28 planning artifacts..."
for file in \
    "${PHASE_DIR}/28-CONTEXT.md" \
    "${PHASE_DIR}/28-RESEARCH.md" \
    "${PHASE_DIR}/28-PATTERNS.md" \
    "${PHASE_DIR}/28-VALIDATION.md" \
    "${PHASE_DIR}/28-01-PLAN.md" \
    "${PHASE_DIR}/28-02-PLAN.md" \
    "${PHASE_DIR}/28-03-PLAN.md" \
    "${PHASE_DIR}/28-04-PLAN.md" \
    "${PHASE_DIR}/28-05-PLAN.md"; do
    if [ ! -f "${file}" ]; then
        fail "required Phase 28 artifact missing: ${file}"
    fi
done
ok "Phase 28 planning artifacts are present"

info "Checking semantic compatibility implementation markers..."
rg -q "pub struct VortexSemanticCompatibilityRow" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing VortexSemanticCompatibilityRow"
rg -q "pub enum VortexSemanticNativeClass" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing VortexSemanticNativeClass"
rg -q "canonical-raw-overclaim" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing canonical raw overclaim diagnostic"
rg -q "native-evidence-missing" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing native evidence diagnostic"
rg -q "nullable-validity-emission-deferred" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing nullable deferral marker"
rg -q "structured-dictionary-facts-deferred" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing dictionary structured deferral marker"
rg -q "structured-run-end-facts-deferred" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing run-end structured deferral marker"
rg -q "structured-bitpack-facts-deferred" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing bitpack structured deferral marker"
rg -q "structured-for-facts-deferred" crates/loom-vortex-ingress/src/lib.rs \
    || fail "missing FOR structured deferral marker"
ok "implementation markers are present"

info "Running focused Phase 28 semantic compatibility tests..."
cargo test -p loom-vortex-ingress --test semantic_compatibility_matrix
cargo test -p loom-vortex-ingress --test nullable_semantic_compatibility
cargo test -p loom-vortex-ingress --test structured_encoding_semantics
ok "focused Phase 28 semantic tests"

info "Re-running real Phase 21 shape coverage tests used by the matrix..."
cargo test -p loom-vortex-ingress --test nullable_primitive_coverage
cargo test -p loom-vortex-ingress --test dictionary_runend_coverage
cargo test -p loom-vortex-ingress --test bitpack_for_coverage
ok "real shape coverage tests"

info "Checking native ExecutionEngine evidence boundary..."
rg -q "native-execution-engine-output" crates/loom-ffi scripts \
    || fail "missing native ExecutionEngine output evidence marker"
if rg -n "native-raw-copy-output" crates/loom-ffi crates/loom-native-melior scripts \
    -g '!vortex-semantic-compatibility-test.sh' >/dev/null; then
    fail "native raw-copy marker is still accepted in production/native gates"
fi
ok "native evidence boundary is explicit"

if [ -f "${REPORT}" ]; then
    info "Checking Phase 28 final report..."
    for marker in \
        "Scope" \
        "Accepted Matrix" \
        "Unsupported Matrix" \
        "Rejected Matrix" \
        "Canonicalized Rows" \
        "Native Disposition" \
        "Phase 30 Tradeoff" \
        "Release Gate Evidence" \
        "Residual Risks" \
        "Phase/Milestone Handoff"; do
        rg -q "${marker}" "${REPORT}" || fail "report missing section: ${marker}"
    done
    if rg -n "StarRocks evidence complete|dual-query evidence complete" "${REPORT}" >/dev/null; then
        fail "report overclaims deferred Phase 30 evidence"
    fi
    rg -q "native-execution-engine-output" "${REPORT}" \
        || fail "report missing native ExecutionEngine evidence marker"
    ok "Phase 28 report is bounded"
else
    info "Phase 28 report not present yet; skipping final-report checks"
fi

echo ""
echo "${GRN}=== Phase 28 Lance/Parquet/Vortex semantic compatibility gate PASSED ===${RST}"
