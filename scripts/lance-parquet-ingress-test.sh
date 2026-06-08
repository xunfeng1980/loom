#!/usr/bin/env bash
# lance-parquet-ingress-test.sh - Phase 27 Lance/Parquet closeout gate.

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

PHASE_DIR=".planning/phases/27-lance-parquet-archival-readability-dataset-ingress"
REPORT="${PHASE_DIR}/27-ARCHIVAL-READABILITY-REPORT.md"

check_file() {
    local file="$1"
    if [ ! -f "${file}" ]; then
        fail "required artifact missing: ${file}"
    fi
}

check_dir() {
    local dir="$1"
    if [ ! -d "${dir}" ]; then
        fail "required directory artifact missing: ${dir}"
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

check_direct_source_deps() {
    local refs unexpected
    refs="$(rg -n '^[[:space:]]*(lance|parquet)[[:space:]]*=' Cargo.toml crates/*/Cargo.toml || true)"
    unexpected="$(
        printf '%s\n' "${refs}" \
            | grep -v '^$' \
            | grep -v '^Cargo.toml:' \
            | grep -v '^crates/loom-lance-ingress/Cargo.toml:' \
            | grep -v '^crates/loom-parquet-ingress/Cargo.toml:' || true
    )"
    if [ -n "${unexpected}" ]; then
        printf '%s\n' "${unexpected}" >&2
        fail "direct Lance/Parquet dependencies must stay in workspace pins and adapter manifests"
    fi
}

check_legacy_report_language() {
    python3 - "${REPORT}" <<'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text()
bad = []
for lineno, line in enumerate(text.splitlines(), 1):
    lowered = line.lower()
    if not re.search(r"(manifest-only|record-only|deterministic record)", lowered):
        continue
    if not re.search(r"(success|successful|passing|accepted|proof|evidence)", lowered):
        continue
    if re.search(r"\b(no|not|never|cannot|must not|without actual|substitute|failing|fail)\b", lowered):
        continue
    bad.append((lineno, line))

if bad:
    for lineno, line in bad:
        print(f"{lineno}: {line}", file=sys.stderr)
    raise SystemExit("report treats manifest-only or record-only legacy evidence as successful")
PY
}

echo "=== Loom Phase 27 Lance/Parquet ingress closeout gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 27 planning and report artifacts..."
for file in \
    "${PHASE_DIR}/27-CONTEXT.md" \
    "${PHASE_DIR}/27-RESEARCH.md" \
    "${PHASE_DIR}/27-PATTERNS.md" \
    "${PHASE_DIR}/27-01-SUMMARY.md" \
    "${PHASE_DIR}/27-02-SUMMARY.md" \
    "${PHASE_DIR}/27-03-SUMMARY.md" \
    "${PHASE_DIR}/27-04-SUMMARY.md" \
    "${REPORT}"; do
    check_file "${file}"
done

for marker in \
    "Supported Slice" \
    "Unsupported and Rejected Matrix" \
    "Current-Version Evidence" \
    "Actual Older-Version Fixtures" \
    "Legacy Readability" \
    "Verifier Evidence" \
    "Oracle Evidence" \
    "Dependency and API Boundary" \
    "Current-Phase Tradeoffs" \
    "Tradeoffs" \
    "Non-Goals" \
    "Phase 28 Handoff"; do
    check_marker "${marker}" "${REPORT}" "report section marker"
done
check_legacy_report_language
ok "Phase 27 planning/report evidence is present"

info "Checking actual older-version fixtures and paired Loom artifacts..."
check_file "crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.parquet"
check_file "crates/loom-parquet-ingress/tests/fixtures/legacy/legacy-v1.loom"
check_file "crates/loom-parquet-ingress/tests/fixtures/legacy/MANIFEST.md"
check_dir "crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.lance"
check_file "crates/loom-lance-ingress/tests/fixtures/legacy/legacy-v1.loom"
check_file "crates/loom-lance-ingress/tests/fixtures/legacy/MANIFEST.md"
check_marker "generator_version: 57.0.0" "crates/loom-parquet-ingress/tests/fixtures/legacy/MANIFEST.md" "older Parquet generator version"
check_marker "generator_version: 6.0.0" "crates/loom-lance-ingress/tests/fixtures/legacy/MANIFEST.md" "older Lance generator version"
check_marker "not a manifest-only record" "crates/loom-parquet-ingress/tests/fixtures/legacy/MANIFEST.md" "Parquet actual fixture statement"
check_marker "not a manifest-only record" "crates/loom-lance-ingress/tests/fixtures/legacy/MANIFEST.md" "Lance actual fixture statement"
ok "actual older-version fixtures and paired Loom artifacts"

info "Running focused Phase 27 adapter and verifier tests..."
cargo test -p loom-source-ingress
cargo test -p loom-parquet-ingress --test dependency_boundary
cargo test -p loom-parquet-ingress --test source_ingress_contract
cargo test -p loom-parquet-ingress --test source_ingress_handoff
cargo test -p loom-parquet-ingress --test legacy_readability
cargo test -p loom-lance-ingress --test dependency_boundary
cargo test -p loom-lance-ingress --test source_ingress_contract
cargo test -p loom-lance-ingress --test source_ingress_handoff
cargo test -p loom-lance-ingress --test legacy_readability
cargo test -p loom-core --test artifact_verifier
ok "focused Phase 27 tests"

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

check_direct_source_deps
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

info "Checking public, host, and CLI surfaces for Phase 27 scope creep..."
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

source_embedding_markers=(
    "manifest_""embedding"
    "embed_""manifest"
    "footer_""embedding"
    "embed_""footer"
    "loom_""footer"
    "loom_""manifest"
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
check_no_fixed_patterns "manifest/footer embedding controls" "${api_surfaces[@]}" -- "${source_embedding_markers[@]}"
check_no_fixed_patterns "object credential controls" "${api_surfaces[@]}" -- "${credential_markers[@]}"
check_no_fixed_patterns "execution surface creep" "${api_surfaces[@]}" -- "${execution_creep_markers[@]}"
ok "public, host, and CLI surfaces"

echo ""
echo "${GRN}=== Phase 27 Lance/Parquet ingress closeout gate PASSED ===${RST}"
