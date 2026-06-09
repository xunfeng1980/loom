#!/usr/bin/env bash
# dual-query-surface-test.sh - Phase 29 focused DuckDB executable evidence gate.

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

echo "=== Loom Phase 29 DuckDB executable evidence gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 29 planning artifacts for DuckDB slice..."
check_file ".planning/phases/29-starrocks-duckdb-dual-query-surface/29-CONTEXT.md"
check_file ".planning/phases/29-starrocks-duckdb-dual-query-surface/29-RESEARCH.md"
check_file ".planning/phases/29-starrocks-duckdb-dual-query-surface/29-PATTERNS.md"
check_file ".planning/phases/29-starrocks-duckdb-dual-query-surface/29-01-PLAN.md"
check_file ".planning/phases/29-starrocks-duckdb-dual-query-surface/29-02-PLAN.md"
check_file ".planning/phases/29-starrocks-duckdb-dual-query-surface/29-03-PLAN.md"
ok "Phase 29 DuckDB-slice artifacts are present"

info "Running adapter-local Rust tests..."
cargo test -p loom-dual-query-surface --test dependency_boundary
cargo test -p loom-dual-query-surface --test query_surface_contract
cargo test -p loom-dual-query-surface --test duckdb_evidence
ok "Rust evidence tests passed"

info "Generating accepted Phase 28 binding fixture..."
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

echo ""
echo "${GRN}=== Phase 29 DuckDB executable evidence PASSED ===${RST}"
echo "  Artifact: ${artifact_path}"
echo "  Descriptor: ${descriptor_path}"
echo "  DuckDB expected: ${expected_path}"
