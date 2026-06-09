#!/usr/bin/env bash
# dual-query-surface-test.sh - Phase 30 focused query-surface evidence gate.

set -euo pipefail

DUCKDB_VERSION="v1.5.3"
REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

EXT_PATH="${REPO_ROOT}/duckdb-ext/build/loom.duckdb_extension"
CLI_CACHE_DIR="${REPO_ROOT}/duckdb-ext/vendor/duckdb-cli"
OUT_DIR="${REPO_ROOT}/target/loom-dual-query-surface-test"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

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

info() { echo "${YLW}[dual-query]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

check_file() {
    local file="$1"
    [ -f "${file}" ] || fail "required file missing: ${file}"
}

echo "=== Loom Phase 30 DuckDB executable evidence gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 30 planning artifacts..."
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-CONTEXT.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-RESEARCH.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-PATTERNS.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-01-PLAN.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-02-PLAN.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-03-PLAN.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-04-PLAN.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-01-SUMMARY.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-02-SUMMARY.md"
check_file ".planning/phases/30-starrocks-duckdb-dual-query-surface/30-03-SUMMARY.md"
ok "Phase 30 artifacts are present"

info "Running adapter-local Rust tests..."
cargo test -p loom-dual-query-surface --test dependency_boundary
cargo test -p loom-dual-query-surface --test query_surface_contract
cargo test -p loom-dual-query-surface --test duckdb_evidence
cargo test -p loom-dual-query-surface --test query_surface_negative
ok "Rust evidence tests passed"

info "Generating accepted Phase 29 binding fixture..."
fixture_output="$(cargo run -q -p loom-dual-query-surface --bin emit_dual_query_fixture -- "${OUT_DIR}")"
artifact_path="$(printf '%s\n' "${fixture_output}" | awk -F= '/^ARTIFACT_PATH=/{print $2}')"
descriptor_path="$(printf '%s\n' "${fixture_output}" | awk -F= '/^DESCRIPTOR_PATH=/{print $2}')"
expected_path="$(printf '%s\n' "${fixture_output}" | awk -F= '/^DUCKDB_EXPECTED_PATH=/{print $2}')"
check_file "${artifact_path}"
check_file "${descriptor_path}"
check_file "${expected_path}"
magic="$(dd if="${artifact_path}" bs=4 count=1 2>/dev/null)"
[ "${magic}" = "LMC1" ] || fail "expected generated artifact to be LMC1, got ${magic}"
ok "Generated accepted fixture and descriptor files"

info "Checking descriptor JSON evidence markers..."
rg -q '"status": "accepted"' "${descriptor_path}" || fail "descriptor JSON missing accepted status"
rg -q '"query_kind": "ordered-rows"' "${descriptor_path}" || fail "descriptor JSON missing ordered rows descriptor"
rg -q '"query_kind": "predicate-id-gte-zero"' "${descriptor_path}" || fail "descriptor JSON missing predicate descriptor"
rg -q '"query_kind": "count"' "${descriptor_path}" || fail "descriptor JSON missing count descriptor"
rg -q '"query_kind": "sum"' "${descriptor_path}" || fail "descriptor JSON missing sum descriptor"
rg -q '"sql": "SELECT id FROM `demo`.`events` ORDER BY id"' "${descriptor_path}" || fail "descriptor JSON missing StarRocks-compatible ordered SQL"
rg -q '"expected_scalar": 48' "${descriptor_path}" || fail "descriptor JSON missing sum scalar"
ok "Descriptor JSON captures bounded StarRocks-compatible matrix"

info "Checking public surface and scope guards..."
for file in \
    "crates/loom-ffi/include/loom.h" \
    "crates/loom-ffi/include/loom_runtime.h" \
    "duckdb-ext/loom_extension.cpp" \
    "crates/loom-cli/src/main.rs"; do
    if awk '!/^[[:space:]]*(\/\/|#|\*|\/\*|\*\/)/ { print }' "${file}" \
        | rg -q 'loom_scan_starrocks|loom_starrocks_query|starrocks_catalog|starrocks_credential|CREATE EXTERNAL TABLE|aws_access_key|secret_access_key'; then
        fail "public surface leaked Phase 30 runtime/API marker: ${file}"
    fi
done
ok "public surfaces exclude StarRocks runtime/API creep"

info "Building loom.duckdb_extension..."
cargo build -p loom-ffi --release
rm -f "${EXT_PATH}"
cmake_out="${TMP_DIR}/cmake-configure.log"
if ! cmake -S "${REPO_ROOT}/duckdb-ext" \
          -B "${REPO_ROOT}/duckdb-ext/build" \
          -DCMAKE_BUILD_TYPE=Release \
          >"${cmake_out}" 2>&1; then
    cat "${cmake_out}" >&2
    fail "CMake configure failed"
fi
grep -v '^--' "${cmake_out}" || true
cmake --build "${REPO_ROOT}/duckdb-ext/build" 2>&1
check_file "${EXT_PATH}"
ok "Built ${EXT_PATH}"

if [ -n "${DUCKDB_CLI:-}" ]; then
    DUCKDB_BIN="${DUCKDB_CLI}"
    info "Using pre-set DUCKDB_CLI=${DUCKDB_BIN}"
else
    OS="$(uname -s)"
    ARCH="$(uname -m)"
    if [ "${OS}" = "Darwin" ] && [ "${ARCH}" = "arm64" ]; then
        CLI_ASSET="duckdb_cli-osx-arm64.zip"
    elif [ "${OS}" = "Darwin" ]; then
        CLI_ASSET="duckdb_cli-osx-amd64.zip"
    elif [ "${OS}" = "Linux" ] && [ "${ARCH}" = "x86_64" ]; then
        CLI_ASSET="duckdb_cli-linux-amd64.zip"
    elif [ "${OS}" = "Linux" ] && [[ "${ARCH}" =~ ^(aarch64|arm64)$ ]]; then
        CLI_ASSET="duckdb_cli-linux-arm64.zip"
    else
        fail "Unsupported platform for DuckDB CLI download: ${OS}/${ARCH}"
    fi

    DUCKDB_BIN="${CLI_CACHE_DIR}/duckdb"
    if [ ! -x "${DUCKDB_BIN}" ]; then
        mkdir -p "${CLI_CACHE_DIR}"
        TMPZIP="${CLI_CACHE_DIR}/${CLI_ASSET}"
        CLI_URL="https://github.com/duckdb/duckdb/releases/download/${DUCKDB_VERSION}/${CLI_ASSET}"
        info "Downloading DuckDB ${DUCKDB_VERSION} CLI (${CLI_ASSET})..."
        curl -fSL --retry 3 --retry-delay 2 -o "${TMPZIP}" "${CLI_URL}"
        unzip -o "${TMPZIP}" -d "${CLI_CACHE_DIR}"
        rm -f "${TMPZIP}"
        chmod +x "${DUCKDB_BIN}"
    fi
fi
[ -x "${DUCKDB_BIN}" ] || fail "DuckDB CLI not executable: ${DUCKDB_BIN}"
ok "DuckDB CLI ready"

sql_to_file() {
    local sql="$1"
    local out="$2"
    "${DUCKDB_BIN}" -unsigned -c \
        "LOAD '${EXT_PATH}'; COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);" \
        >/dev/null
}

assert_query() {
    local label="$1"
    local sql="$2"
    local expected="$3"
    local out="${TMP_DIR}/${label}.csv"
    info "DuckDB ${label}: ${sql}"
    sql_to_file "${sql}" "${out}"
    local actual
    actual="$(cat "${out}")"
    if [ "${actual}" != "${expected}" ]; then
        echo "Expected:" >&2
        echo "${expected}" >&2
        echo "Actual:" >&2
        echo "${actual}" >&2
        fail "DuckDB result mismatch for ${label}"
    fi
    ok "DuckDB ${label} matched"
}

assert_query "ordered_rows" "SELECT id FROM loom_scan('${artifact_path}') ORDER BY id" $'-1\n7\n42'
assert_query "predicate" "SELECT id FROM loom_scan('${artifact_path}') WHERE id >= 0 ORDER BY id" $'7\n42'
assert_query "count" "SELECT COUNT(*) FROM loom_scan('${artifact_path}')" "3"
assert_query "sum" "SELECT SUM(id) FROM loom_scan('${artifact_path}')" "48"

run_optional_starrocks_runtime_smoke() {
    if [ "${LOOM_STARROCKS_RUNTIME_SMOKE:-0}" != "1" ]; then
        info "optional StarRocks runtime smoke skipped by default"
        info "skipped StarRocks runtime smoke is not accepted StarRocks runtime evidence"
        ok "optional StarRocks runtime smoke is non-canonical"
        return
    fi

    local missing=()
    for var in \
        STARROCKS_MYSQL \
        STARROCKS_HOST \
        STARROCKS_PORT \
        STARROCKS_USER \
        STARROCKS_PASSWORD \
        STARROCKS_DATABASE \
        STARROCKS_TABLE; do
        if [ -z "${!var:-}" ]; then
            missing+=("${var}")
        fi
    done
    if [ "${#missing[@]}" -ne 0 ]; then
        fail "LOOM_STARROCKS_RUNTIME_SMOKE=1 requires env: ${missing[*]}"
    fi
    [ -x "${STARROCKS_MYSQL}" ] || fail "STARROCKS_MYSQL is not executable: ${STARROCKS_MYSQL}"

    local rows_out="${TMP_DIR}/starrocks-rows.tsv"
    local predicate_out="${TMP_DIR}/starrocks-predicate.tsv"
    local count_out="${TMP_DIR}/starrocks-count.tsv"
    local sum_out="${TMP_DIR}/starrocks-sum.tsv"
    local mysql_common=(
        -h "${STARROCKS_HOST}"
        -P "${STARROCKS_PORT}"
        -u "${STARROCKS_USER}"
        "-p${STARROCKS_PASSWORD}"
        -D "${STARROCKS_DATABASE}"
        -N
        -B
    )
    "${STARROCKS_MYSQL}" "${mysql_common[@]}" \
        -e "SELECT id FROM ${STARROCKS_TABLE} ORDER BY id" >"${rows_out}"
    "${STARROCKS_MYSQL}" "${mysql_common[@]}" \
        -e "SELECT id FROM ${STARROCKS_TABLE} WHERE id >= 0 ORDER BY id" >"${predicate_out}"
    "${STARROCKS_MYSQL}" "${mysql_common[@]}" \
        -e "SELECT COUNT(*) FROM ${STARROCKS_TABLE}" >"${count_out}"
    "${STARROCKS_MYSQL}" "${mysql_common[@]}" \
        -e "SELECT SUM(id) FROM ${STARROCKS_TABLE}" >"${sum_out}"

    [ "$(cat "${rows_out}")" = $'-1\n7\n42' ] || fail "StarRocks runtime ordered rows mismatch"
    [ "$(cat "${predicate_out}")" = $'7\n42' ] || fail "StarRocks runtime predicate rows mismatch"
    [ "$(cat "${count_out}")" = "3" ] || fail "StarRocks runtime count mismatch"
    [ "$(cat "${sum_out}")" = "48" ] || fail "StarRocks runtime sum mismatch"
    ok "supplemental StarRocks runtime smoke matched bounded matrix"
    info "supplemental StarRocks runtime smoke is not the deterministic acceptance root"
}

run_optional_starrocks_runtime_smoke

REPORT_PATH=".planning/phases/30-starrocks-duckdb-dual-query-surface/30-DUAL-QUERY-SURFACE-REPORT.md"
if [ -f "${REPORT_PATH}" ]; then
    info "Checking final report markers..."
    rg -q "DuckDB Executable Evidence" "${REPORT_PATH}" || fail "final report missing DuckDB evidence"
    rg -q "StarRocks-Compatible Descriptor Evidence" "${REPORT_PATH}" || fail "final report missing descriptor evidence"
    rg -q "Optional StarRocks Runtime Smoke" "${REPORT_PATH}" || fail "final report missing optional runtime smoke"
    rg -q "Release Gate Evidence" "${REPORT_PATH}" || fail "final report missing release gate evidence"
    rg -q "Current-Phase Tradeoffs" "${REPORT_PATH}" || fail "final report missing tradeoffs"
    ok "final report markers"
fi

echo ""
echo "${GRN}=== Phase 30 focused query-surface evidence PASSED ===${RST}"
echo "  Artifact: ${artifact_path}"
echo "  Descriptor: ${descriptor_path}"
echo "  DuckDB expected: ${expected_path}"
