#!/usr/bin/env bash
# duckdb-smoke-test.sh — MVP0 SQL acceptance gate for the Loom DuckDB extension.

set -euo pipefail

DUCKDB_VERSION="v1.5.3"
REPO_ROOT="$(git rev-parse --show-toplevel)"
EXT_PATH="${REPO_ROOT}/duckdb-ext/build/loom.duckdb_extension"
CLI_CACHE_DIR="${REPO_ROOT}/duckdb-ext/vendor/duckdb-cli"
PAYLOAD_DIR="${REPO_ROOT}/target/loom-duckdb-fixtures"

if [ -t 1 ] && command -v tput &>/dev/null; then
    RED=$(tput setaf 1)
    GRN=$(tput setaf 2)
    YLW=$(tput setaf 3)
    RST=$(tput sgr0)
else
    RED=''
    GRN=''
    YLW=''
    RST=''
fi

info()  { echo "${YLW}[smoke-test]${RST} $*"; }
ok()    { echo "${GRN}[PASS]${RST} $*"; }
fail()  { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "=== Loom DuckDB MVP0 SQL smoke-test ==="
echo ""

info "Generating deterministic Loom payloads..."
cargo run -p loom-fixtures --bin emit_duckdb_payloads >/dev/null
test -f "${PAYLOAD_DIR}/bitpack-i32.loom"
test -f "${PAYLOAD_DIR}/for-i32.loom"
test -f "${PAYLOAD_DIR}/dict-i32.loom"
test -f "${PAYLOAD_DIR}/rle-i32.loom"
test -f "${PAYLOAD_DIR}/fsst-utf8.loom"
test -f "${PAYLOAD_DIR}/dict-fsst-utf8.loom"
test -f "${PAYLOAD_DIR}/alp-f32.loom"
test -f "${PAYLOAD_DIR}/alp-f64.loom"
test -f "${PAYLOAD_DIR}/mixed-table.loom"
ok "Generated payloads in ${PAYLOAD_DIR}"

assert_lmc1() {
    local name="$1"
    local payload="${PAYLOAD_DIR}/${name}.loom"
    local magic
    magic="$(dd if="${payload}" bs=4 count=1 2>/dev/null)"
    if [ "${magic}" != "LMC1" ]; then
        fail "expected ${name}.loom to be an LMC1 container, got '${magic}'"
    fi
}

for payload_name in \
    bitpack-i32 \
    for-i32 \
    dict-i32 \
    rle-i32 \
    fsst-utf8 \
    dict-fsst-utf8 \
    alp-f32 \
    alp-f64 \
    mixed-table
do
    assert_lmc1 "${payload_name}"
done
ok "Generated smoke fixtures are LMC1 containers"

info "Building loom.duckdb_extension..."
cargo build -p loom-ffi --release
rm -f "${EXT_PATH}"
cmake -S "${REPO_ROOT}/duckdb-ext" \
      -B "${REPO_ROOT}/duckdb-ext/build" \
      -DCMAKE_BUILD_TYPE=Release \
      2>&1 | grep -v '^--' || true
cmake --build "${REPO_ROOT}/duckdb-ext/build" 2>&1

if [ ! -f "${EXT_PATH}" ]; then
    fail "Build succeeded but loom.duckdb_extension not found at: ${EXT_PATH}"
fi

EXT_SIZE=$(wc -c < "${EXT_PATH}" | tr -d ' ')
if [ "${EXT_SIZE}" -lt 512 ]; then
    fail "Extension file is only ${EXT_SIZE} bytes — footer stamp missing"
fi
ok "Built ${EXT_PATH} (${EXT_SIZE} bytes)"

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

    CLI_URL="https://github.com/duckdb/duckdb/releases/download/${DUCKDB_VERSION}/${CLI_ASSET}"
    DUCKDB_BIN="${CLI_CACHE_DIR}/duckdb"
    if [ -x "${DUCKDB_BIN}" ]; then
        info "DuckDB CLI already cached at ${DUCKDB_BIN}"
    else
        info "Downloading DuckDB ${DUCKDB_VERSION} CLI (${CLI_ASSET})..."
        mkdir -p "${CLI_CACHE_DIR}"
        TMPZIP="${CLI_CACHE_DIR}/${CLI_ASSET}"
        curl -fSL --retry 3 --retry-delay 2 -o "${TMPZIP}" "${CLI_URL}"
        unzip -o "${TMPZIP}" -d "${CLI_CACHE_DIR}"
        rm -f "${TMPZIP}"
        chmod +x "${DUCKDB_BIN}"
        ok "Downloaded ${DUCKDB_BIN}"
    fi
fi

if [ ! -x "${DUCKDB_BIN}" ]; then
    fail "DuckDB CLI not executable at: ${DUCKDB_BIN}"
fi
info "Using DuckDB CLI: ${DUCKDB_BIN}"

sql_to_file() {
    local sql="$1"
    local out="$2"
    "${DUCKDB_BIN}" -unsigned -c \
        "LOAD '${EXT_PATH}'; COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);" \
        >/dev/null
}

check_rows() {
    local name="$1"
    local expected="$2"
    local payload="${PAYLOAD_DIR}/${name}.loom"
    local out="${TMP_DIR}/${name}-rows.csv"

    info "SELECT rows for ${name}..."
    sql_to_file "SELECT COALESCE(CAST(value AS VARCHAR), 'NULL') FROM loom_scan('${payload}')" "${out}"
    local actual
    actual="$(cat "${out}")"
    if [ "${actual}" != "${expected}" ]; then
        echo "Expected rows:" >&2
        echo "${expected}" >&2
        echo "Actual rows:" >&2
        echo "${actual}" >&2
        fail "row mismatch for ${name}"
    fi
    ok "SELECT * FROM loom_scan('${name}') matched"
}

check_numeric_aggregate() {
    local name="$1"
    local expected="$2"
    local payload="${PAYLOAD_DIR}/${name}.loom"
    local out="${TMP_DIR}/${name}-agg.csv"

    info "SELECT COUNT/SUM for ${name}..."
    sql_to_file "SELECT COUNT(*), SUM(value) FROM loom_scan('${payload}')" "${out}"
    local actual
    actual="$(cat "${out}")"
    if [ "${actual}" != "${expected}" ]; then
        fail "aggregate mismatch for ${name}: expected '${expected}', got '${actual}'"
    fi
    ok "SELECT COUNT(*), SUM(value) for ${name} matched"
}

check_string_aggregate() {
    local name="$1"
    local expected="$2"
    local payload="${PAYLOAD_DIR}/${name}.loom"
    local out="${TMP_DIR}/${name}-agg.csv"

    info "SELECT COUNT/MIN/MAX for ${name}..."
    sql_to_file "SELECT COUNT(*), COUNT(value), MIN(value), MAX(value) FROM loom_scan('${payload}')" "${out}"
    local actual
    actual="$(cat "${out}")"
    if [ "${actual}" != "${expected}" ]; then
        fail "aggregate mismatch for ${name}: expected '${expected}', got '${actual}'"
    fi
    ok "SELECT COUNT/MIN/MAX for ${name} matched"
}

check_float_aggregate() {
    local name="$1"
    local expected="$2"
    local payload="${PAYLOAD_DIR}/${name}.loom"
    local out="${TMP_DIR}/${name}-agg.csv"

    info "SELECT COUNT/SUM/MIN/MAX for ${name}..."
    sql_to_file "SELECT COUNT(*), COUNT(value), SUM(value), MIN(value), MAX(value) FROM loom_scan('${payload}')" "${out}"
    local actual
    actual="$(cat "${out}")"
    if [ "${actual}" != "${expected}" ]; then
        fail "aggregate mismatch for ${name}: expected '${expected}', got '${actual}'"
    fi
    ok "SELECT COUNT/SUM/MIN/MAX for ${name} matched"
}

check_mixed_table() {
    local payload="${PAYLOAD_DIR}/mixed-table.loom"
    local rows_out="${TMP_DIR}/mixed-table-rows.csv"
    local agg_out="${TMP_DIR}/mixed-table-agg.csv"
    local expected_rows=$'1,true,alpha\n2,false,NULL\n3,true,beta\n4,true,gamma\n5,false,delta'
    local expected_agg="5,15,4,8"

    info "SELECT rows for mixed-table..."
    sql_to_file "SELECT id, CAST(flag AS VARCHAR), COALESCE(label, 'NULL') FROM loom_scan('${payload}')" "${rows_out}"
    local actual_rows
    actual_rows="$(cat "${rows_out}")"
    if [ "${actual_rows}" != "${expected_rows}" ]; then
        echo "Expected rows:" >&2
        echo "${expected_rows}" >&2
        echo "Actual rows:" >&2
        echo "${actual_rows}" >&2
        fail "row mismatch for mixed-table"
    fi
    ok "SELECT id, flag, label FROM loom_scan('mixed-table') matched"

    info "SELECT multi-column aggregates for mixed-table..."
    sql_to_file "SELECT COUNT(*), SUM(id), COUNT(label), SUM(CASE WHEN flag THEN id ELSE 0 END) FROM loom_scan('${payload}')" "${agg_out}"
    local actual_agg
    actual_agg="$(cat "${agg_out}")"
    if [ "${actual_agg}" != "${expected_agg}" ]; then
        fail "aggregate mismatch for mixed-table: expected '${expected_agg}', got '${actual_agg}'"
    fi
    ok "SELECT COUNT/SUM/COUNT(label)/filtered SUM for mixed-table matched"
}

check_rows "bitpack-i32" $'1\n2\n3\n4'
check_numeric_aggregate "bitpack-i32" "4,10"

check_rows "for-i32" $'10\n11\n12'
check_numeric_aggregate "for-i32" "3,33"

check_rows "dict-i32" $'30\n10\n20\n30'
check_numeric_aggregate "dict-i32" "4,90"

check_rows "rle-i32" $'1\n1\n2\n2\n2\n3'
check_numeric_aggregate "rle-i32" "6,11"

check_rows "fsst-utf8" $'alpha\nNULL\nbeta'
check_string_aggregate "fsst-utf8" "3,2,alpha,beta"

check_rows "dict-fsst-utf8" $'beta\nalpha\ngamma\nbeta'
check_string_aggregate "dict-fsst-utf8" "4,4,alpha,gamma"

check_rows "alp-f32" $'1.25\n-2.5\n0.0\n1.25\nNULL'
check_float_aggregate "alp-f32" "5,4,0.0,-2.5,1.25"

check_rows "alp-f64" $'10.125\n-3.5\n0.0\nNULL\n10.125'
check_float_aggregate "alp-f64" "5,4,16.75,-3.5,10.125"

check_mixed_table

echo ""
echo "${GRN}=== Smoke-test PASSED ===${RST}"
echo "  Extension: ${EXT_PATH}"
echo "  DuckDB CLI: ${DUCKDB_BIN} (${DUCKDB_VERSION})"
echo "  Covered: bitpack-i32, for-i32, dict-i32, rle-i32, fsst-utf8, dict-fsst-utf8, alp-f32, alp-f64, mixed-table"
echo ""
