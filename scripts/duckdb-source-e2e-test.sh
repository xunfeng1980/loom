#!/usr/bin/env bash
# duckdb-source-e2e-test.sh - MVP1 DuckDB e2e gate for source semantic artifacts.

set -euo pipefail

DUCKDB_VERSION="v1.5.3"
REPO_ROOT="$(git rev-parse --show-toplevel)"
EXT_PATH="${REPO_ROOT}/duckdb-ext/build/loom.duckdb_extension"
CLI_CACHE_DIR="${REPO_ROOT}/duckdb-ext/vendor/duckdb-cli"
FIXTURE_DIR="${REPO_ROOT}/target/loom-duckdb-source-e2e"

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

info() { echo "${YLW}[duckdb-source-e2e]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "=== Loom MVP1 DuckDB source e2e gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Generating Parquet, Lance, and Vortex source-backed LMC2 distribution fixtures plus bounded DuckDB bridge fixtures..."
rm -rf "${FIXTURE_DIR}"
mkdir -p "${FIXTURE_DIR}"
cargo run -p loom-parquet-ingress --bin emit_duckdb_parquet_lma1_fixture -- "${FIXTURE_DIR}/parquet" >/dev/null
cargo run -p loom-lance-ingress --bin emit_duckdb_lance_lma1_fixture -- "${FIXTURE_DIR}/lance" >/dev/null
cargo run -p loom-vortex-ingress --bin emit_duckdb_vortex_lma1_fixture -- "${FIXTURE_DIR}/vortex" >/dev/null
ok "generated source-backed LMC2 fixtures and direct LMA1 DuckDB bridge fixtures in ${FIXTURE_DIR}"

assert_magic() {
    local payload="$1"
    local expected="$2"
    local magic
    magic="$(dd if="${payload}" bs=4 count=1 2>/dev/null)"
    if [ "${magic}" != "${expected}" ]; then
        fail "expected ${payload} to be a ${expected} artifact, got '${magic}'"
    fi
}

for payload in \
    "${FIXTURE_DIR}/parquet/parquet.loom" \
    "${FIXTURE_DIR}/lance/lance.loom" \
    "${FIXTURE_DIR}/vortex/vortex.loom"
do
    test -f "${payload}" || fail "missing fixture ${payload}"
    assert_magic "${payload}" "LMC2"
done
ok "all source-backed distribution fixtures are LMC2"

for payload in \
    "${FIXTURE_DIR}/parquet/parquet-duckdb-bridge-lma1.loom" \
    "${FIXTURE_DIR}/lance/lance-duckdb-bridge-lma1.loom" \
    "${FIXTURE_DIR}/vortex/vortex-duckdb-bridge-lma1.loom"
do
    test -f "${payload}" || fail "missing DuckDB bridge fixture ${payload}"
    assert_magic "${payload}" "LMA1"
done
ok "direct LMA1 DuckDB bridge fixtures are explicitly labeled"

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
test -f "${EXT_PATH}" || fail "missing extension at ${EXT_PATH}"
ok "built ${EXT_PATH}"

if [ -n "${DUCKDB_CLI:-}" ]; then
    DUCKDB_BIN="${DUCKDB_CLI}"
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

    CLI_URL="https://github.com/duckdb/duckdb/releases/download/${DUCKDB_VERSION}/${CLI_ASSET}"
    DUCKDB_BIN="${CLI_CACHE_DIR}/duckdb"
    if [ ! -x "${DUCKDB_BIN}" ]; then
        info "Downloading DuckDB ${DUCKDB_VERSION} CLI (${CLI_ASSET})..."
        mkdir -p "${CLI_CACHE_DIR}"
        TMPZIP="${CLI_CACHE_DIR}/${CLI_ASSET}"
        curl -fSL --retry 3 --retry-delay 2 -o "${TMPZIP}" "${CLI_URL}"
        unzip -o "${TMPZIP}" -d "${CLI_CACHE_DIR}"
        rm -f "${TMPZIP}"
        chmod +x "${DUCKDB_BIN}"
    fi
fi
test -x "${DUCKDB_BIN}" || fail "DuckDB CLI not executable at ${DUCKDB_BIN}"
ok "DuckDB CLI ready"

sql_to_file() {
    local sql="$1"
    local out="$2"
    "${DUCKDB_BIN}" -unsigned -c \
        "LOAD '${EXT_PATH}'; COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);" \
        >/dev/null
}

check_source_artifact() {
    local label="$1"
    local payload="$2"
    local bridge_payload="$3"
    local rows_out="${TMP_DIR}/${label}-rows.csv"
    local agg_out="${TMP_DIR}/${label}-agg.csv"
    local bridge_out="${TMP_DIR}/${label}-bridge.csv"
    local expected_rows=$'7\n-1\n42'
    local expected_agg="3,48,-1,42"

    info "Checking bounded DuckDB SQL over ${label} default LMC2 artifact..."
    sql_to_file "SELECT value FROM loom_scan('${payload}')" "${rows_out}"
    local actual_rows
    actual_rows="$(cat "${rows_out}")"
    if [ "${actual_rows}" != "${expected_rows}" ]; then
        echo "Expected rows:" >&2
        echo "${expected_rows}" >&2
        echo "Actual rows:" >&2
        echo "${actual_rows}" >&2
        fail "row mismatch for ${label}"
    fi

    sql_to_file "SELECT COUNT(*), SUM(value), MIN(value), MAX(value) FROM loom_scan('${payload}')" "${agg_out}"
    local actual_agg
    actual_agg="$(cat "${agg_out}")"
    if [ "${actual_agg}" != "${expected_agg}" ]; then
        fail "aggregate mismatch for ${label}: expected '${expected_agg}', got '${actual_agg}'"
    fi

    sql_to_file "SELECT COUNT(*), SUM(value) FROM loom_scan('${bridge_payload}')" "${bridge_out}"
    local actual_bridge
    actual_bridge="$(cat "${bridge_out}")"
    if [ "${actual_bridge}" != "3,48" ]; then
        fail "direct LMA1 bridge regression mismatch for ${label}: got '${actual_bridge}'"
    fi
    ok "${label} default LMC2 SQL matched; direct LMA1 bridge regression retained"
}

check_source_artifact \
    "parquet" \
    "${FIXTURE_DIR}/parquet/parquet.loom" \
    "${FIXTURE_DIR}/parquet/parquet-duckdb-bridge-lma1.loom"
check_source_artifact \
    "lance" \
    "${FIXTURE_DIR}/lance/lance.loom" \
    "${FIXTURE_DIR}/lance/lance-duckdb-bridge-lma1.loom"
check_source_artifact \
    "vortex" \
    "${FIXTURE_DIR}/vortex/vortex.loom" \
    "${FIXTURE_DIR}/vortex/vortex-duckdb-bridge-lma1.loom"

echo ""
echo "${GRN}=== DuckDB source e2e gate PASSED ===${RST}"
