#!/usr/bin/env bash
# duckdb-lmc2-sql-surface-test.sh - DuckDB SQL gate for default LMC2(LMA1).

set -euo pipefail

DUCKDB_VERSION="v1.5.3"
REPO_ROOT="$(git rev-parse --show-toplevel)"
EXT_PATH="${REPO_ROOT}/duckdb-ext/build/loom.duckdb_extension"
CLI_CACHE_DIR="${REPO_ROOT}/duckdb-ext/vendor/duckdb-cli"
FIXTURE_DIR="${REPO_ROOT}/target/loom-duckdb-lmc2-sql"

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

info() { echo "${YLW}[duckdb-lmc2-sql]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "=== Loom Phase 34 DuckDB LMC2 SQL surface gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

assert_magic() {
    local payload="$1"
    local expected="$2"
    local magic
    magic="$(dd if="${payload}" bs=4 count=1 2>/dev/null)"
    if [ "${magic}" != "${expected}" ]; then
        fail "expected ${payload} to start with ${expected}, got '${magic}'"
    fi
}

info "Generating multi-column primitive/nullable LMC2 fixture..."
rm -rf "${FIXTURE_DIR}"
cargo run -p loom-fixtures --bin emit_arrow_semantic_lmc2_sql_fixture -- "${FIXTURE_DIR}" >/dev/null
LMC2_PAYLOAD="${FIXTURE_DIR}/multi-column-lmc2.loom"
LMA1_PAYLOAD="${FIXTURE_DIR}/multi-column-direct-lma1.loom"
NATIVE_LMC2_PAYLOAD="${FIXTURE_DIR}/native-primitives-lmc2.loom"
NATIVE_LMA1_PAYLOAD="${FIXTURE_DIR}/native-primitives-direct-lma1.loom"
LOGICAL_PAYLOAD="${FIXTURE_DIR}/logical-date32-lmc2.loom"
NESTED_PAYLOAD="${FIXTURE_DIR}/nested-struct-lmc2.loom"
test -f "${LMC2_PAYLOAD}" || fail "missing ${LMC2_PAYLOAD}"
test -f "${LMA1_PAYLOAD}" || fail "missing ${LMA1_PAYLOAD}"
test -f "${NATIVE_LMC2_PAYLOAD}" || fail "missing ${NATIVE_LMC2_PAYLOAD}"
test -f "${NATIVE_LMA1_PAYLOAD}" || fail "missing ${NATIVE_LMA1_PAYLOAD}"
test -f "${LOGICAL_PAYLOAD}" || fail "missing ${LOGICAL_PAYLOAD}"
test -f "${NESTED_PAYLOAD}" || fail "missing ${NESTED_PAYLOAD}"
assert_magic "${LMC2_PAYLOAD}" "LMC2"
assert_magic "${LMA1_PAYLOAD}" "LMA1"
assert_magic "${NATIVE_LMC2_PAYLOAD}" "LMC2"
assert_magic "${NATIVE_LMA1_PAYLOAD}" "LMA1"
assert_magic "${LOGICAL_PAYLOAD}" "LMC2"
assert_magic "${NESTED_PAYLOAD}" "LMC2"
ok "generated default LMC2, direct LMA1 regression, logical, and nested fixtures"

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

export LOOM_DUCKDB_TEST_ROUTE_REPORT="${TMP_DIR}/route-report.tsv"
: >"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"

sql_to_file() {
    local sql="$1"
    local out="$2"
    "${DUCKDB_BIN}" -unsigned -c \
        "LOAD '${EXT_PATH}'; COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);" \
        >/dev/null
}

assert_query() {
    local name="$1"
    local sql="$2"
    local expected="$3"
    local out="${TMP_DIR}/${name}.csv"

    info "Checking ${name}..."
    sql_to_file "${sql}" "${out}"
    local actual
    actual="$(cat "${out}")"
    if [ "${actual}" != "${expected}" ]; then
        echo "Expected:" >&2
        echo "${expected}" >&2
        echo "Actual:" >&2
        echo "${actual}" >&2
        fail "query mismatch for ${name}"
    fi
    ok "${name}"
}

assert_query_fails() {
    local name="$1"
    local sql="$2"
    local expected="$3"
    local out="${TMP_DIR}/${name}.out"
    local err="${TMP_DIR}/${name}.err"

    info "Checking ${name}..."
    if "${DUCKDB_BIN}" -unsigned -c "LOAD '${EXT_PATH}'; ${sql}" >"${out}" 2>"${err}"; then
        cat "${out}" >&2
        fail "expected ${name} to fail"
    fi
    if ! grep -q "${expected}" "${err}"; then
        echo "Expected diagnostic substring: ${expected}" >&2
        echo "Actual stderr:" >&2
        cat "${err}" >&2
        fail "diagnostic mismatch for ${name}"
    fi
    ok "${name}"
}

assert_query \
    "native-primitives-lmc2-default-route" \
    "SELECT * FROM loom_scan('${NATIVE_LMC2_PAYLOAD}')" \
    $'1,true,10,1.5\n2,,20,2.5\n3,false,30,3.5\n4,true,40,4.5\n5,false,50,5.5'
if ! rg -q 'route=native-candidate' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" ||
   ! rg -q 'native-arrow-semantic-codegen-output' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
    echo "Route report:" >&2
    cat "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" >&2
    fail "native primitive LMC2 did not use the default production native route"
fi
ok "native-primitives-lmc2-default-route uses production native route"

assert_query \
    "default-lmc2-project-filter" \
    "SELECT id, COALESCE(label, 'NULL') FROM loom_scan('${LMC2_PAYLOAD}') WHERE id >= 3 ORDER BY id" \
    $'3,gamma\n4,delta\n5,NULL'

assert_query \
    "default-lmc2-nullable-bool" \
    "SELECT COALESCE(CAST(flag AS VARCHAR), 'NULL') FROM loom_scan('${LMC2_PAYLOAD}') ORDER BY id" \
    $'true\nNULL\nfalse\ntrue\nfalse'

assert_query \
    "default-lmc2-aggregate" \
    "SELECT COUNT(*), COUNT(label), SUM(id), SUM(amount), MIN(ratio), MAX(ratio) FROM loom_scan('${LMC2_PAYLOAD}')" \
    "5,3,15,150,1.5,5.5"

assert_query \
    "direct-lma1-regression-bridge" \
    "SELECT COUNT(*), SUM(id), COUNT(label) FROM loom_scan('${LMA1_PAYLOAD}')" \
    "5,15,3"

assert_query_fails \
    "logical-date32-unsupported" \
    "SELECT * FROM loom_scan('${LOGICAL_PAYLOAD}');" \
    "unsupported Arrow semantic schema format"

assert_query_fails \
    "nested-struct-unsupported" \
    "SELECT * FROM loom_scan('${NESTED_PAYLOAD}');" \
    "unsupported Arrow semantic schema format"

echo ""
echo "${GRN}=== DuckDB LMC2 SQL surface gate PASSED ===${RST}"
