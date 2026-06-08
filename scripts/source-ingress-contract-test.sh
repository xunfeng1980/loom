#!/usr/bin/env bash
# source-ingress-contract-test.sh - Phase 26 source ingress contract gate.

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

info() { echo "${YLW}[source-ingress]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/26-external-source-ingress-contract"

check_file() {
    local file="$1"
    if [ ! -f "${file}" ]; then
        fail "required artifact missing: ${file}"
    fi
}

check_marker() {
    local pattern="$1"
    local file="$2"
    local label="$3"
    rg -q --fixed-strings "${pattern}" "${file}" || fail "missing ${label}: ${pattern} in ${file}"
}

check_no_fixed_patterns() {
    local label="$1"
    shift
    local -a files=()
    while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do
        files+=("$1")
        shift
    done
    shift

    local pattern
    for pattern in "$@"; do
        set +e
        rg -n --fixed-strings "${pattern}" "${files[@]}" >/tmp/source-ingress-rg.out 2>/tmp/source-ingress-rg.err
        local status=$?
        set -e
        if [ "${status}" -eq 0 ]; then
            cat /tmp/source-ingress-rg.out >&2
            fail "${label} found forbidden marker: ${pattern}"
        fi
        if [ "${status}" -ne 1 ]; then
            cat /tmp/source-ingress-rg.err >&2 || true
            fail "${label} check failed for marker: ${pattern}"
        fi
    done
}

check_cargo_tree_clean() {
    local package="$1"
    shift
    local output
    output="$(cargo tree -p "${package}")"
    local pattern
    for pattern in "$@"; do
        if grep -E -i "${pattern}" <<<"${output}" >/tmp/source-ingress-tree.out; then
            cat /tmp/source-ingress-tree.out >&2
            fail "${package} dependency tree contains forbidden source dependency marker: ${pattern}"
        fi
    done
}

check_no_manifest_patterns() {
    local label="$1"
    shift
    local -a files=()
    while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do
        files+=("$1")
        shift
    done
    shift

    local pattern
    for pattern in "$@"; do
        local matches
        matches="$(
            awk 'BEGIN{IGNORECASE=1} /^[[:space:]]*#/ {next} /^[[:space:]]*description[[:space:]]*=/ {next} {print FILENAME ":" FNR ":" $0}' "${files[@]}" \
                | grep -F -i "${pattern}" || true
        )"
        if [ -n "${matches}" ]; then
            printf '%s\n' "${matches}" >&2
            fail "${label} found forbidden manifest marker: ${pattern}"
        fi
    done
}

echo "=== Loom Phase 26 source ingress contract gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 26 required planning artifacts..."
for file in \
    "${PHASE_DIR}/26-CONTEXT.md" \
    "${PHASE_DIR}/26-RESEARCH.md" \
    "${PHASE_DIR}/26-PATTERNS.md" \
    "${PHASE_DIR}/26-SOURCE-INGRESS-CONTRACT.md" \
    "${PHASE_DIR}/26-SOURCE-INGRESS-REPORT.md"; do
    check_file "${file}"
done

rg -q "accepted" "${PHASE_DIR}/26-SOURCE-INGRESS-CONTRACT.md" \
    || fail "contract missing accepted semantics"
rg -q "unsupported" "${PHASE_DIR}/26-SOURCE-INGRESS-CONTRACT.md" \
    || fail "contract missing unsupported semantics"
rg -q "rejected" "${PHASE_DIR}/26-SOURCE-INGRESS-CONTRACT.md" \
    || fail "contract missing rejected semantics"
rg -q "LMC1" "${PHASE_DIR}/26-SOURCE-INGRESS-CONTRACT.md" \
    || fail "contract missing LMC1 accepted emission boundary"
rg -q "Phase 27 Handoff" "${PHASE_DIR}/26-SOURCE-INGRESS-CONTRACT.md" \
    || fail "contract missing Phase 27 handoff"
rg -q "Vortex Mapping" "${PHASE_DIR}/26-SOURCE-INGRESS-REPORT.md" \
    || fail "report missing Vortex mapping"
rg -q "Adapter Obligations" "${PHASE_DIR}/26-SOURCE-INGRESS-REPORT.md" \
    || fail "report missing adapter obligations"
rg -q "Current-Phase Tradeoffs" "${PHASE_DIR}/26-SOURCE-INGRESS-REPORT.md" \
    || fail "report missing current phase tradeoffs"
rg -q "Non-Goals" "${PHASE_DIR}/26-SOURCE-INGRESS-REPORT.md" \
    || fail "report missing non-goals"
rg -q "Phase 27 Handoff" "${PHASE_DIR}/26-SOURCE-INGRESS-REPORT.md" \
    || fail "report missing Phase 27 handoff"
ok "Phase 26 contract and report artifacts are present"

info "Checking source-ingress implementation markers..."
check_marker "pub struct SourceIngressReport" crates/loom-source-ingress/src/lib.rs "SourceIngressReport"
check_marker "pub struct SourceFacts" crates/loom-source-ingress/src/lib.rs "SourceFacts"
check_marker "pub struct SourceOracleEvidence" crates/loom-source-ingress/src/lib.rs "SourceOracleEvidence"
check_marker "source_facts_from_vortex_reader_facts" crates/loom-vortex-ingress/src/source_contract.rs "Vortex facts mapping helper"
check_marker "source_report_from_vortex_reader_facts" crates/loom-vortex-ingress/src/source_contract.rs "Vortex report mapping helper"
check_marker "emit_source_ingress_lmc1_from_vortex_buffer" crates/loom-vortex-ingress/src/source_contract.rs "verifier-routed Vortex handoff helper"
ok "implementation markers are present"

info "Running focused Phase 26 contract tests..."
cargo test -p loom-source-ingress
cargo test -p loom-vortex-ingress --test source_ingress_contract
cargo test -p loom-vortex-ingress --test source_ingress_handoff
ok "focused Phase 26 contract tests"

info "Running prior reader and artifact verifier handoff smoke tests..."
cargo test -p loom-vortex-ingress --test reader_facts_contract
cargo test -p loom-vortex-ingress --test single_column_to_loom
cargo test -p loom-vortex-ingress --test table_to_loom
cargo test -p loom-core --test artifact_verifier
ok "reader and artifact verifier handoff tests"

info "Checking source dependency boundaries..."
source_dep_patterns=(
    "vort""ex"
    "fast""lanes"
    "lanc""e"
    "par""quet"
    "ice""berg"
    "m""cap"
    "z""arr"
    "Le""Robot"
    "object_""store"
    "object-""store"
)

check_cargo_tree_clean loom-core "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-ffi "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-source-ingress "${source_dep_patterns[@]}"

check_no_fixed_patterns "generic source-ingress crate" \
    crates/loom-source-ingress/Cargo.toml \
    crates/loom-source-ingress/src/lib.rs \
    crates/loom-source-ingress/tests/source_ingress_contract.rs \
    -- \
    "${source_dep_patterns[@]}" \
    "Duck""DB" \
    "mel""ior"

check_no_manifest_patterns "core/ffi/source-ingress manifests" \
    crates/loom-core/Cargo.toml \
    crates/loom-ffi/Cargo.toml \
    crates/loom-source-ingress/Cargo.toml \
    -- \
    "${source_dep_patterns[@]}"
ok "source dependency boundaries"

info "Checking DuckDB and public API surfaces for source-ingress creep..."
api_surfaces=(
    crates/loom-ffi/include/loom.h
    crates/loom-ffi/include/loom_runtime.h
    crates/loom-ffi/include/loom_duckdb_internal.h
    duckdb-ext/loom_extension.cpp
    crates/loom-cli/src/main.rs
)

source_route_markers=(
    "loom_scan_""lance"
    "loom_scan_""parquet"
    "loom_scan_""iceberg"
    "loom_scan_""mcap"
    "loom_scan_""zarr"
    "loom_scan_""lerobot"
    "loom_ingest_""lance"
    "loom_ingest_""parquet"
    "loom_source_""sql"
)

credential_markers=(
    "object_""store"
    "object-""store"
    "aws_""access_key"
    "secret_""access_key"
    "s3_""credentials"
    "credential_""mode"
    "storage_""options"
    "cloud_""credentials"
)

execution_creep_markers=(
    "predicate_""pushdown"
    "pushdown_""predicate"
    "loom_scan_with_""predicate"
    "parallel_""split"
    "split_""workers"
    "loom_scan_""split"
    "ArrowArray""Stream"
    "native_""kernel_public"
    "loom_native_""kernel"
    "source_native_""kernel"
)

check_no_fixed_patterns "route-specific source SQL/API" "${api_surfaces[@]}" -- "${source_route_markers[@]}"
check_no_fixed_patterns "object credential controls" "${api_surfaces[@]}" -- "${credential_markers[@]}"
check_no_fixed_patterns "execution surface creep" "${api_surfaces[@]}" -- "${execution_creep_markers[@]}"
ok "DuckDB and public API surfaces"

echo ""
echo "${GRN}=== Phase 26 source ingress contract gate PASSED ===${RST}"
