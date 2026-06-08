#!/usr/bin/env bash
# lance-parquet-ingress-test.sh - Phase 27 Lance/Parquet ingress boundary gate.

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

info() { echo "${YLW}[lance-parquet-ingress]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

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
        rg -n --fixed-strings "${pattern}" "${files[@]}" >/tmp/lance-parquet-ingress-rg.out 2>/tmp/lance-parquet-ingress-rg.err
        local status=$?
        set -e
        if [ "${status}" -eq 0 ]; then
            cat /tmp/lance-parquet-ingress-rg.out >&2
            fail "${label} found forbidden marker: ${pattern}"
        fi
        if [ "${status}" -ne 1 ]; then
            cat /tmp/lance-parquet-ingress-rg.err >&2 || true
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
        if grep -E -i "${pattern}" <<<"${output}" >/tmp/lance-parquet-ingress-tree.out; then
            cat /tmp/lance-parquet-ingress-tree.out >&2
            fail "${package} dependency tree contains forbidden source marker: ${pattern}"
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

echo "=== Loom Phase 27 Lance/Parquet ingress boundary gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking adapter crate scaffolding..."
for file in \
    crates/loom-lance-ingress/Cargo.toml \
    crates/loom-lance-ingress/src/lib.rs \
    crates/loom-lance-ingress/tests/dependency_boundary.rs \
    crates/loom-parquet-ingress/Cargo.toml \
    crates/loom-parquet-ingress/src/lib.rs \
    crates/loom-parquet-ingress/tests/dependency_boundary.rs; do
    check_file "${file}"
done

check_marker "crates/loom-lance-ingress" Cargo.toml "Lance adapter workspace member"
check_marker "crates/loom-parquet-ingress" Cargo.toml "Parquet adapter workspace member"
check_marker "SourceIngressAcceptedArtifact" crates/loom-source-ingress/src/lib.rs "accepted artifact handoff type"
ok "adapter crates and common handoff type are present"

info "Running Phase 27 scaffold compile smoke..."
cargo check -p loom-lance-ingress -p loom-parquet-ingress
ok "adapter crates compile"

info "Running dependency boundary tests..."
cargo test -p loom-source-ingress --test source_ingress_contract
cargo test -p loom-lance-ingress --test dependency_boundary
cargo test -p loom-parquet-ingress --test dependency_boundary
ok "dependency boundary tests"

info "Checking source dependency boundaries..."
source_dep_patterns=(
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

check_no_manifest_patterns "core/ffi/source-ingress manifests" \
    crates/loom-core/Cargo.toml \
    crates/loom-ffi/Cargo.toml \
    crates/loom-source-ingress/Cargo.toml \
    -- \
    "${source_dep_patterns[@]}"

check_no_fixed_patterns "generic source-ingress crate" \
    crates/loom-source-ingress/Cargo.toml \
    crates/loom-source-ingress/src/lib.rs \
    crates/loom-source-ingress/tests/source_ingress_contract.rs \
    -- \
    "${source_dep_patterns[@]}" \
    "Duck""DB" \
    "ArrowArray""Stream"
ok "source dependency boundaries"

info "Checking public and host surfaces for Phase 27 scope creep..."
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
ok "public and host surfaces"

info "Checking main release gate is not wired in Plan 27-01..."
if rg -q --fixed-strings "lance-parquet-ingress-test.sh" scripts/mvp0-verify.sh; then
    fail "scripts/mvp0-verify.sh must not wire Phase 27 until Plan 27-05"
fi
ok "main release gate unchanged for Phase 27"

echo ""
echo "${GRN}=== Phase 27 Lance/Parquet ingress boundary gate PASSED ===${RST}"
