#!/usr/bin/env bash
# iceberg-binding-test.sh - Phase 28 Iceberg binding dependency/scope guard.

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

info() { echo "${YLW}[iceberg-binding]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

check_file() {
    local file="$1"
    if [ ! -f "${file}" ]; then
        fail "required artifact missing: ${file}"
    fi
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
        rg -n --fixed-strings "${pattern}" "${files[@]}" >/tmp/iceberg-binding-rg.out 2>/tmp/iceberg-binding-rg.err
        local status=$?
        set -e
        if [ "${status}" -eq 0 ]; then
            cat /tmp/iceberg-binding-rg.out >&2
            fail "${label} found forbidden marker: ${pattern}"
        fi
        if [ "${status}" -ne 1 ]; then
            cat /tmp/iceberg-binding-rg.err >&2 || true
            fail "${label} check failed for marker: ${pattern}"
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

check_cargo_tree_clean() {
    local package="$1"
    shift
    if ! cargo metadata --no-deps --format-version 1 | grep -F "\"name\":\"${package}\"" >/dev/null; then
        info "Skipping ${package} dependency tree check; package is not present"
        return
    fi
    local output
    output="$(cargo tree -p "${package}")"
    local pattern
    for pattern in "$@"; do
        if grep -E -i "${pattern}" <<<"${output}" >/tmp/iceberg-binding-tree.out; then
            cat /tmp/iceberg-binding-tree.out >&2
            fail "${package} dependency tree contains forbidden marker: ${pattern}"
        fi
    done
}

check_direct_iceberg_sdk_deps() {
    local sdk_name refs
    sdk_name="ice""berg"
    refs="$(
        rg -n "^[[:space:]]*(${sdk_name}[[:space:]]*=|[A-Za-z0-9_-]+[[:space:]]*=.*package[[:space:]]*=[[:space:]]*\"${sdk_name}\")" \
            Cargo.toml crates/*/Cargo.toml || true
    )"
    if [ -n "${refs}" ]; then
        printf '%s\n' "${refs}" >&2
        fail "official Iceberg SDK dependency must not be present by default"
    fi
}

check_serde_json_placement() {
    local json_name refs unexpected
    json_name="serde_""json"
    rg -q --fixed-strings "${json_name} = { version = \"=1.0.150\" }" Cargo.toml \
        || fail "workspace serde_json exact pin is missing"
    refs="$(
        rg -n "^[[:space:]]*${json_name}[[:space:]]*=" crates/*/Cargo.toml || true
    )"
    unexpected="$(
        printf '%s\n' "${refs}" \
            | grep -v '^$' \
            | grep -v '^crates/loom-iceberg-binding/Cargo.toml:' || true
    )"
    if [ -n "${unexpected}" ]; then
        printf '%s\n' "${unexpected}" >&2
        fail "serde_json direct adapter dependency must stay local to loom-iceberg-binding"
    fi
}

echo "=== Loom Phase 28 Iceberg binding dependency/scope guard ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking adapter crate scaffold..."
check_file "crates/loom-iceberg-binding/Cargo.toml"
check_file "crates/loom-iceberg-binding/src/lib.rs"
check_file "crates/loom-iceberg-binding/src/binding_contract.rs"
check_file "crates/loom-iceberg-binding/tests/dependency_boundary.rs"
rg -q --fixed-strings '"crates/loom-iceberg-binding"' Cargo.toml \
    || fail "workspace member missing: crates/loom-iceberg-binding"
ok "adapter crate scaffold"

info "Running focused adapter dependency and contract tests..."
cargo test -p loom-iceberg-binding --test dependency_boundary
cargo test -p loom-iceberg-binding --test binding_contract
cargo check -p loom-iceberg-binding
ok "focused adapter tests"

info "Checking SDK and JSON dependency placement..."
check_direct_iceberg_sdk_deps
check_serde_json_placement

source_dep_patterns=(
    "ice""berg"
    "object_""store"
    "object-""store"
    "opendal"
    "duck""db"
    "lanc""e"
    "par""quet"
)

check_cargo_tree_clean loom-core "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-ffi "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-source-ingress "${source_dep_patterns[@]}"
check_cargo_tree_clean loom-cli "${source_dep_patterns[@]}"

check_no_manifest_patterns "core/ffi/source-ingress/cli manifests" \
    crates/loom-core/Cargo.toml \
    crates/loom-ffi/Cargo.toml \
    crates/loom-source-ingress/Cargo.toml \
    crates/loom-cli/Cargo.toml \
    -- \
    "${source_dep_patterns[@]}"
ok "dependency placement"

info "Checking source-neutral and public surfaces for Phase 28 scope creep..."
check_no_fixed_patterns "generic source-ingress crate" \
    crates/loom-source-ingress/Cargo.toml \
    crates/loom-source-ingress/src/lib.rs \
    crates/loom-source-ingress/tests/source_ingress_contract.rs \
    -- \
    "ice""berg" \
    "Ice""berg"

api_surfaces=(
    crates/loom-ffi/include/loom.h
    crates/loom-ffi/include/loom_runtime.h
    crates/loom-ffi/include/loom_duckdb_internal.h
    duckdb-ext/loom_extension.cpp
    crates/loom-cli/src/main.rs
)

route_markers=(
    "loom_scan_""iceberg"
    "loom_ingest_""iceberg"
    "iceberg_""catalog"
    "iceberg_""rest"
    "Star""Rocks"
    "star""rocks"
)

credential_markers=(
    "ware""house"
    "object_""store"
    "object-""store"
    "aws_""access_key"
    "secret_""access_key"
    "s3_""credentials"
    "credential_""mode"
    "storage_""options"
    "cloud_""credentials"
)

mutation_markers=(
    "branch ""mutation"
    "tag ""mutation"
)

check_no_fixed_patterns "route-specific Iceberg SQL/API" "${api_surfaces[@]}" -- "${route_markers[@]}"
check_no_fixed_patterns "object-store/catalog credential controls" "${api_surfaces[@]}" -- "${credential_markers[@]}"
check_no_fixed_patterns "branch/tag mutation controls" "${api_surfaces[@]}" -- "${mutation_markers[@]}"
ok "public, host, and CLI surfaces"

info "Checking focused gate remains unwired from main release gate..."
set +e
rg -q --fixed-strings "iceberg-binding-test.sh" scripts/mvp0-verify.sh
gate_status=$?
set -e
if [ "${gate_status}" -eq 0 ]; then
    fail "Plan 28-01 must not wire scripts/iceberg-binding-test.sh into scripts/mvp0-verify.sh"
fi
if [ "${gate_status}" -ne 1 ]; then
    fail "mvp0-verify unwired check failed with rg status ${gate_status}"
fi
ok "focused gate is unwired"

echo ""
echo "${GRN}=== Phase 28 Iceberg binding dependency/scope guard PASSED ===${RST}"
