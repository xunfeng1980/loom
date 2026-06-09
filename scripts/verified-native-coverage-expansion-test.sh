#!/usr/bin/env bash
# verified-native-coverage-expansion-test.sh - Phase 42 coverage matrix gate.

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

info() { echo "${YLW}[verified-native-coverage]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

MATRIX=".planning/phases/42-verified-native-coverage-expansion/42-COVERAGE-MATRIX.md"

echo "=== Loom Phase 42 verified/native coverage expansion gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 42 artifacts and markers..."
for file in \
    "${MATRIX}" \
    ".planning/phases/42-verified-native-coverage-expansion/42-01-SUMMARY.md" \
    ".planning/phases/42-verified-native-coverage-expansion/42-02-SUMMARY.md" \
    "crates/loom-vortex-ingress/tests/phase42_vortex_coverage_matrix.rs" \
    "crates/loom-parquet-ingress/tests/phase42_source_schema_matrix.rs" \
    "crates/loom-lance-ingress/tests/phase42_source_schema_matrix.rs"; do
    [ -f "${file}" ] || fail "missing Phase 42 artifact: ${file}"
done
for marker in \
    "phase42_vortex_verified_native_coverage_report" \
    "SourceVerifiedNativeCoverageRow" \
    "SourceVerifiedNativeDisposition" \
    "native-evidence-missing" \
    "verified-lineage-evidence-missing"; do
    rg -q -F "${marker}" crates/loom-vortex-ingress crates/loom-source-ingress \
        || fail "missing Phase 42 implementation marker: ${marker}"
done
for marker in \
    "native-supported" \
    "interpreter-only" \
    "canonicalized bridge" \
    "fail-closed/deferred" \
    "vortex-lmc2-fixed-width-primitive" \
    "parquet-nullable-i32" \
    "lance-nullable-i32"; do
    rg -q -F "${marker}" "${MATRIX}" \
        || fail "coverage matrix missing marker: ${marker}"
done
if rg -n "toolchain-skip|native-toolchain-skip" "${MATRIX}"; then
    fail "Phase 42 matrix must not infer native support from toolchain skip markers"
fi
ok "Phase 42 markers are present"

info "Running Vortex Phase 42 matrix tests..."
cargo test -p loom-vortex-ingress --test phase42_vortex_coverage_matrix
ok "Vortex Phase 42 matrix"

info "Running Parquet Phase 42 matrix tests..."
cargo test -p loom-parquet-ingress --test phase42_source_schema_matrix
ok "Parquet Phase 42 matrix"

info "Running Lance Phase 42 matrix tests..."
cargo test -p loom-lance-ingress --test phase42_source_schema_matrix
ok "Lance Phase 42 matrix"

info "Running full Arrow semantic compatibility gate..."
bash scripts/full-arrow-semantic-compatibility-test.sh
ok "full Arrow semantic compatibility"

info "Running verified-lineage gate..."
bash scripts/verified-lineage-test.sh
ok "verified-lineage gate"

echo ""
echo "${GRN}=== Phase 42 verified/native coverage expansion gate PASSED ===${RST}"
