#!/usr/bin/env bash
# iceberg-binding-test.sh - Phase 29 Iceberg binding dependency/scope guard.

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

PHASE_DIR=".planning/phases/29-iceberg-ref-table-binding"
REPORT="${PHASE_DIR}/29-ICEBERG-BINDING-REPORT.md"

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
    rg -q --fixed-strings "${pattern}" "${file}" \
        || fail "missing ${label}: ${pattern} in ${file}"
}

non_comment_lines() {
    awk '
        /^[[:space:]]*#/ { next }
        /^[[:space:]]*\/\// { next }
        /^[[:space:]]*$/ { next }
        { print FILENAME ":" FNR ":" $0 }
    ' "$@"
}

check_required_code_patterns() {
    local label="$1"
    shift
    local -a files=()
    while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do
        files+=("$1")
        shift
    done
    shift

    local pattern matches
    for pattern in "$@"; do
        matches="$(non_comment_lines "${files[@]}" | grep -F "${pattern}" || true)"
        if [ -z "${matches}" ]; then
            fail "${label} missing required marker: ${pattern}"
        fi
    done
}

check_no_code_patterns() {
    local label="$1"
    shift
    local -a files=()
    while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do
        files+=("$1")
        shift
    done
    shift

    local pattern matches
    for pattern in "$@"; do
        matches="$(non_comment_lines "${files[@]}" | grep -F "${pattern}" || true)"
        if [ -n "${matches}" ]; then
            printf '%s\n' "${matches}" >&2
            fail "${label} found forbidden marker: ${pattern}"
        fi
    done
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

check_no_duplicate_arrow_parquet_families() {
    local output duplicate_headers
    output="$(cargo tree -d)"
    duplicate_headers="$(
        awk '/^[A-Za-z0-9_-]+ v[0-9]/{print}' <<<"${output}" \
            | rg '^(arrow|arrow-array|arrow-schema|arrow-data|parquet) v' || true
    )"
    if [ -n "${duplicate_headers}" ]; then
        printf '%s\n' "${duplicate_headers}" >&2
        fail "duplicate Arrow/Parquet dependency family detected"
    fi
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

check_binding_report_language() {
    python3 - "${REPORT}" <<'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text()
claim_terms = (
    "manifest-only",
    "sidecar-only",
    "metadata-only",
    "verifier-status-only",
    "source-evidence-only",
    "oracle-accepted-flag-only",
)
success_terms = re.compile(r"\b(success|successful|passing|accepted|proof|evidence)\b", re.I)
negating_terms = re.compile(
    r"\b(no|not|never|cannot|must not|without|fail-closed|failing|rejected|unsupported|descriptive only|not proof)\b",
    re.I,
)

bad = []
for lineno, line in enumerate(text.splitlines(), 1):
    lowered = line.lower()
    if not any(term in lowered for term in claim_terms):
        continue
    if not success_terms.search(line):
        continue
    if negating_terms.search(line):
        continue
    bad.append((lineno, line))

if bad:
    for lineno, line in bad:
        print(f"{lineno}: {line}", file=sys.stderr)
    raise SystemExit(
        "report treats metadata-only, manifest-only, sidecar-only, verifier-status-only, "
        "source-evidence-only, or oracle-accepted-flag-only claims as successful proof"
    )
PY
}

echo "=== Loom Phase 29 Iceberg binding dependency/scope guard ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking adapter crate scaffold..."
check_file "${PHASE_DIR}/29-CONTEXT.md"
check_file "${PHASE_DIR}/29-RESEARCH.md"
check_file "${PHASE_DIR}/29-PATTERNS.md"
check_file "${PHASE_DIR}/29-01-SUMMARY.md"
check_file "${PHASE_DIR}/29-02-SUMMARY.md"
check_file "${PHASE_DIR}/29-03-SUMMARY.md"
check_file "${PHASE_DIR}/29-04-SUMMARY.md"
check_file "${REPORT}"
check_file "crates/loom-iceberg-binding/Cargo.toml"
check_file "crates/loom-iceberg-binding/src/lib.rs"
check_file "crates/loom-iceberg-binding/src/binding_contract.rs"
check_file "crates/loom-iceberg-binding/tests/dependency_boundary.rs"
check_file "crates/loom-iceberg-binding/tests/binding_contract.rs"
check_file "crates/loom-iceberg-binding/tests/binding_handoff.rs"
check_file "crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-metadata.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-source-evidence.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/mismatch-schema-sidecar.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/mismatch-snapshot-sidecar.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/manifest-only-sidecar.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/forged-oracle-evidence.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/unsupported-remote-metadata.json"
check_file "crates/loom-iceberg-binding/tests/fixtures/local/rejected-missing-identity.json"
rg -q --fixed-strings '"crates/loom-iceberg-binding"' Cargo.toml \
    || fail "workspace member missing: crates/loom-iceberg-binding"
ok "adapter crate scaffold and local parser fixtures"

info "Checking Phase 29 binding report evidence markers..."
for marker in \
    "Executive Summary" \
    "Implemented Artifacts" \
    "Binding Schema" \
    "Accepted Unsupported Rejected Matrix" \
    "Mismatch Fail-Closed Matrix" \
    "Source Evidence" \
    "Verifier Evidence" \
    "Oracle Evidence" \
    "Dependency and API Boundary" \
    "Current-Phase Tradeoffs" \
    "Non-Goals" \
    "Release Gate Evidence" \
    "Phase 29 Handoff"; do
    check_marker "${marker}" "${REPORT}" "report section marker"
done
check_marker "accepted-table-source-evidence.json" "${REPORT}" "accepted evidence fixture"
check_marker "stale-source-evidence.json" "${REPORT}" "stale source evidence fixture"
check_marker "forged-oracle-evidence.json" "${REPORT}" "forged oracle evidence fixture"
check_marker "does not add the official \`iceberg\` crate by default" "${REPORT}" "no-default-iceberg decision"
check_marker "Current-Phase Tradeoffs" "${REPORT}" "current tradeoffs section"
check_binding_report_language
ok "Phase 29 binding report evidence is present"

info "Running focused adapter dependency and contract tests..."
cargo test -p loom-iceberg-binding --test dependency_boundary
cargo test -p loom-iceberg-binding --test binding_contract
cargo test -p loom-iceberg-binding --test binding_handoff
cargo test -p loom-iceberg-binding --test mismatch_fail_closed
cargo test -p loom-core --test artifact_verifier
cargo check -p loom-iceberg-binding
ok "focused adapter tests"

info "Checking SDK and JSON dependency placement..."
check_direct_iceberg_sdk_deps
check_serde_json_placement
check_no_duplicate_arrow_parquet_families

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

info "Checking source-neutral and public surfaces for Phase 29 scope creep..."
check_no_fixed_patterns "generic source-ingress crate" \
    crates/loom-source-ingress/Cargo.toml \
    crates/loom-source-ingress/src/lib.rs \
    crates/loom-source-ingress/tests/source_ingress_contract.rs \
    -- \
    "ice""berg" \
    "Ice""berg"

info "Checking accepted binding evidence and verifier markers..."
binding_contract_src="crates/loom-iceberg-binding/src/binding_contract.rs"
rg -q 'verify_artifact\(' "${binding_contract_src}" \
    || fail "accepted binding production path must call verify_artifact"
rg -q 'sha256_bytes\(&artifact_bytes\)' "${binding_contract_src}" \
    || fail "accepted binding production path must recompute artifact SHA-256"
rg -q 'let decoded_values_sha256 = decoded_values_sha256\(' "${binding_contract_src}" \
    || fail "accepted binding production path must validate decoded values digest"
rg -q 'append_int32_array_digest_lines' "${binding_contract_src}" \
    || fail "accepted binding production path must canonicalize decoded Int32 values"
rg -q 'resolve_local_sidecar_path\(' "${binding_contract_src}" \
    || fail "accepted binding production path must confine sidecar-controlled paths"
rg -q 'resolve_local_evidence_path\(' "${binding_contract_src}" \
    || fail "accepted binding production path must confine evidence-controlled paths"
rg -q 'fs::read\(&source_path\)' "${binding_contract_src}" \
    || fail "accepted binding production path must read and hash local source evidence bytes"
rg -q 'source evidence SHA-256 does not match local source bytes' "${binding_contract_src}" \
    || fail "accepted binding production path must diagnose source hash mismatch"

check_required_code_patterns "mismatch fail-closed matrix" \
    crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs \
    crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json \
    crates/loom-iceberg-binding/tests/fixtures/local/forged-oracle-evidence.json \
    -- \
    "schema_snapshot_table_and_artifact_mismatches_return_no_bytes" \
    "verifier_status_rejected_bytes_and_missing_evidence_return_no_bytes" \
    "stale_source_and_forged_oracle_evidence_flags_return_no_bytes" \
    "manifest_only_remote_credentials_and_public_route_scope_fail_closed" \
    "assert_no_accepted_bytes" \
    "source evidence SHA-256 does not match local source bytes" \
    "decoded-row fixture values SHA-256 does not match verified Loom artifact values" \
    "stale-source-evidence" \
    "forged-oracle-evidence" \
    "manifest-only"

check_required_code_patterns "manifest-only negative coverage" \
    crates/loom-iceberg-binding/tests/binding_handoff.rs \
    crates/loom-iceberg-binding/tests/mismatch_fail_closed.rs \
    -- \
    "sidecar_hash_or_mutated_artifact_bytes_cannot_force_acceptance" \
    "manifest_list_location" \
    "manifest-only"

check_required_code_patterns "concrete source/oracle evidence fixture" \
    crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-source-evidence.json \
    crates/loom-iceberg-binding/tests/fixtures/local/accepted-table-loom-binding.json \
    crates/loom-iceberg-binding/tests/fixtures/local/stale-source-evidence.json \
    crates/loom-iceberg-binding/tests/fixtures/local/forged-oracle-evidence.json \
    -- \
    "accepted-table-source-evidence" \
    "row_count" \
    "table_uuid" \
    "schema_id" \
    "snapshot_id" \
    "artifact_sha256" \
    "source/demo-events.parquet" \
    "values_sha256" \
    "sha256"

check_no_code_patterns "query-engine route evidence" \
    crates/loom-iceberg-binding/src/binding_contract.rs \
    crates/loom-iceberg-binding/tests/binding_handoff.rs \
    -- \
    "loom_scan_iceberg" \
    "duckdb" \
    "StarRocks" \
    "starrocks"
ok "accepted binding evidence markers"

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

echo ""
echo "${GRN}=== Phase 29 Iceberg binding dependency/scope guard PASSED ===${RST}"
