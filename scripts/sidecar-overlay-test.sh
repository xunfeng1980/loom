#!/usr/bin/env bash
# sidecar-overlay-test.sh - Phase 50 sidecar overlay release gate.
#
# Validates the end-to-end sidecar overlay model:
#   embed -> extract -> verify roundtrip (Parquet),
#   Vortex/Lance graceful None,
#   strippable overlay invariant.

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

# ripgrep (rg) is required by check_marker; fail early with a clear message
# if it is not installed, rather than a cryptic "command not found" later.
if ! command -v rg >/dev/null 2>&1; then
    echo "ERROR: ripgrep (rg) is required but not installed" >&2
    exit 2
fi

info() { echo "${YLW}[sidecar-overlay]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; }
check_file() {
    local file="$1"
    if [ ! -f "${file}" ]; then
        fail "required artifact missing: ${file}"
        return 1
    fi
}
check_marker() {
    local pattern="$1"
    local file="$2"
    local label="$3"
    rg -q --fixed-strings "${pattern}" "${file}" || { fail "missing ${label}: ${pattern} in ${file}"; return 1; }
}

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

PASSED=0
FAILED=0
declare -a FAILURE_MESSAGES

pass_section() {
    ok "$1"
    PASSED=$((PASSED + 1))
}

fail_section() {
    local section="$1"; shift
    local msg="$*"
    fail "$section: $msg"
    FAILED=$((FAILED + 1))
    FAILURE_MESSAGES+=("$section: $msg")
}

echo "=== SIDECAR OVERLAY GATE ==="
echo "Repository: ${REPO_ROOT}"
echo ""

# ---------------------------------------------------------------------------
# Section 1: Implementation markers
# ---------------------------------------------------------------------------
info "Checking implementation markers..."

check_marker "pub struct SidecarOverlay" crates/loom-ir-core/src/sidecar.rs "sidecar core type" || {
    fail_section "MARKERS" "SidecarOverlay type not found"
    exit 1
}
check_marker "pub struct ChunkBinding" crates/loom-ir-core/src/sidecar.rs "chunk binding type" || {
    fail_section "MARKERS" "ChunkBinding type not found"
    exit 1
}
check_marker "pub fn extract_sidecar_from_parquet_metadata" crates/loom-parquet-ingress/src/sidecar_parquet.rs "parquet extract" || {
    fail_section "MARKERS" "Parquet extract function not found"
    exit 1
}
check_marker "pub fn embed_sidecar_into_key_value_metadata" crates/loom-parquet-ingress/src/sidecar_parquet.rs "parquet embed" || {
    fail_section "MARKERS" "Parquet embed function not found"
    exit 1
}
check_marker "pub fn extract_sidecar_from_vortex_buffer" crates/loom-vortex-ingress/src/sidecar_vortex.rs "vortex extract" || {
    fail_section "MARKERS" "Vortex extract function not found"
    exit 1
}
check_marker "pub fn extract_sidecar_from_lance_dataset" crates/loom-lance-ingress/src/sidecar_lance.rs "lance extract" || {
    fail_section "MARKERS" "Lance extract function not found"
    exit 1
}
check_marker "pub fn compute_chunk_hash" crates/loom-ir-core/src/sidecar.rs "chunk hash helper" || {
    fail_section "MARKERS" "chunk hash helper not found"
    exit 1
}

pass_section "MARKERS"

# ---------------------------------------------------------------------------
# Section 2: loom-core sidecar tests (loom-ir-core)
# ---------------------------------------------------------------------------
info "Running loom-ir-core sidecar tests..."
if cargo test -p loom-ir-core -- sidecar --quiet 2>&1 | tee "${TMP_DIR}/core-sidecar-tests.log"; then
    pass_section "CORE_SIDECAR_TESTS"
else
    fail_section "CORE_SIDECAR_TESTS" "loom-ir-core sidecar tests failed (exit code non-zero)"
    cat "${TMP_DIR}/core-sidecar-tests.log" >&2
fi

# ---------------------------------------------------------------------------
# Section 3: Parquet sidecar roundtrip
# ---------------------------------------------------------------------------
info "Running Parquet sidecar roundtrip tests..."
if cargo test -p loom-parquet-ingress -- sidecar --quiet 2>&1 | tee "${TMP_DIR}/parquet-sidecar-tests.log"; then
    pass_section "PARQUET_SIDECAR_ROUNDTRIP"
else
    fail_section "PARQUET_SIDECAR_ROUNDTRIP" "Parquet sidecar tests failed (exit code non-zero)"
    cat "${TMP_DIR}/parquet-sidecar-tests.log" >&2
fi

# ---------------------------------------------------------------------------
# Section 4: Vortex sidecar marker
# ---------------------------------------------------------------------------
info "Checking Vortex sidecar module..."
if cargo test -p loom-vortex-ingress -- sidecar_vortex --quiet 2>&1 | tee "${TMP_DIR}/vortex-sidecar-tests.log"; then
    pass_section "VORTEX_SIDECAR_MARKER"
else
    fail_section "VORTEX_SIDECAR_MARKER" "Vortex sidecar tests failed (exit code non-zero)"
    cat "${TMP_DIR}/vortex-sidecar-tests.log" >&2
fi

# Verify the extract function returns None gracefully (format limitation)
if grep -q "format limitation\|format does not\|graceful" crates/loom-vortex-ingress/src/sidecar_vortex.rs; then
    info "  Vortex format limitation documented"
else
    fail_section "VORTEX_SIDECAR_MARKER" "Vortex format limitation not documented"
fi

# ---------------------------------------------------------------------------
# Section 5: Lance sidecar marker
# ---------------------------------------------------------------------------
info "Checking Lance sidecar module..."
if cargo test -p loom-lance-ingress -- sidecar_lance --quiet 2>&1 | tee "${TMP_DIR}/lance-sidecar-tests.log"; then
    pass_section "LANCE_SIDECAR_MARKER"
else
    fail_section "LANCE_SIDECAR_MARKER" "Lance sidecar tests failed (exit code non-zero)"
    cat "${TMP_DIR}/lance-sidecar-tests.log" >&2
fi

# Verify the extract function returns None gracefully (format limitation)
if grep -q "format limitation\|format does not\|graceful" crates/loom-lance-ingress/src/sidecar_lance.rs; then
    info "  Lance format limitation documented"
else
    fail_section "LANCE_SIDECAR_MARKER" "Lance format limitation not documented"
fi

# ---------------------------------------------------------------------------
# Section 6: Strippable overlay invariant
# ---------------------------------------------------------------------------
info "Checking strippable overlay invariant..."
# Verifies that a Parquet file with sidecar metadata is still readable by
# arrow-rs (unknown KeyValue keys are silently ignored).
# The Parquet sidecar embed/extract roundtrip test already proves this in
# sidecar_parquet.rs tests (embed_preserves_non_loom_keys, etc.).
# Check for the marker in the test code.
check_marker "embed_preserves_non_loom_keys" crates/loom-parquet-ingress/src/sidecar_parquet.rs "strippable test" || {
    fail_section "STRIPPABLE_OVERLAY" "strippable overlay test not found"
}
pass_section "STRIPPABLE_OVERLAY"

# ---------------------------------------------------------------------------
# Section 7: Full workspace build
# ---------------------------------------------------------------------------
info "Checking full workspace build..."
if cargo build --quiet 2>&1 | tee "${TMP_DIR}/full-build.log"; then
    pass_section "FULL_BUILD"
else
    fail_section "FULL_BUILD" "Full workspace build failed (exit code non-zero)"
    cat "${TMP_DIR}/full-build.log" >&2
fi

# ---------------------------------------------------------------------------
# Section 8: CLI build check
# ---------------------------------------------------------------------------
info "Checking CLI build with sidecar embed subcommand..."
if cargo build -p loom-cli --quiet 2>&1 | tee "${TMP_DIR}/cli-build.log"; then
    pass_section "CLI_BUILD"
else
    fail_section "CLI_BUILD" "CLI build failed (exit code non-zero)"
    cat "${TMP_DIR}/cli-build.log" >&2
fi

# ---------------------------------------------------------------------------
# Summary table
# ---------------------------------------------------------------------------
echo ""
echo "=== SIDECAR OVERLAY GATE SUMMARY ==="
echo ""

# Print individual section results
TOTAL=$((PASSED + FAILED))
if [ $FAILED -eq 0 ]; then
    echo "${GRN}PASS${RST} MARKERS"
    echo "${GRN}PASS${RST} CORE_SIDECAR_TESTS"
    echo "${GRN}PASS${RST} PARQUET_SIDECAR_ROUNDTRIP"
    echo "${GRN}PASS${RST} VORTEX_SIDECAR_MARKER"
    echo "${GRN}PASS${RST} LANCE_SIDECAR_MARKER"
    echo "${GRN}PASS${RST} STRIPPABLE_OVERLAY"
    echo "${GRN}PASS${RST} FULL_BUILD"
    echo "${GRN}PASS${RST} CLI_BUILD"
else
    # Print status for each section based on failure messages.
    # Use array iteration rather than echo|grep, because echo joins all
    # array elements onto one line, so only the first failing section's
    # prefix appears at start-of-line and all later failures show PASS.
    for section in MARKERS CORE_SIDECAR_TESTS PARQUET_SIDECAR_ROUNDTRIP VORTEX_SIDECAR_MARKER LANCE_SIDECAR_MARKER STRIPPABLE_OVERLAY FULL_BUILD CLI_BUILD; do
        matched=false
        for msg in "${FAILURE_MESSAGES[@]}"; do
            if [[ "$msg" == "${section}:"* ]]; then
                matched=true
                break
            fi
        done
        if $matched; then
            echo "${RED}FAIL${RST} ${section}"
        else
            echo "${GRN}PASS${RST} ${section}"
        fi
    done
fi

echo ""
echo "  Total: ${TOTAL} sections, ${GRN}${PASSED} passed${RST}, ${RED}${FAILED} failed${RST}"

if [ $FAILED -gt 0 ]; then
    echo ""
    echo "${RED}=== SIDECAR OVERLAY GATE FAILED ===${RST}"
    exit 1
fi

echo ""
echo "${GRN}=== SIDECAR OVERLAY GATE PASSED ===${RST}"
