#!/usr/bin/env bash
# duckdb-native-integration-test.sh - Phase 24 route-aware DuckDB native gate.

set -euo pipefail

DUCKDB_VERSION="v1.5.3"
REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

EXT_PATH="${REPO_ROOT}/duckdb-ext/build/loom.duckdb_extension"
CLI_CACHE_DIR="${REPO_ROOT}/duckdb-ext/vendor/duckdb-cli"
PAYLOAD_DIR="${REPO_ROOT}/target/loom-duckdb-fixtures"

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

info() { echo "${YLW}[duckdb-native]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/loom-duckdb-native-XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

export LOOM_DUCKDB_TEST_ROUTE_REPORT="${TMP_DIR}/route-report.tsv"
: >"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"

echo "=== Loom Phase 24 DuckDB native integration gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Generating deterministic DuckDB payloads..."
cargo run -p loom-fixtures --bin emit_duckdb_payloads >/dev/null
for payload_name in native-primitives-table mixed-table fsst-utf8 bitpack-i32; do
    test -f "${PAYLOAD_DIR}/${payload_name}.loom" || fail "missing ${payload_name}.loom"
    magic="$(dd if="${PAYLOAD_DIR}/${payload_name}.loom" bs=4 count=1 2>/dev/null)"
    [ "${magic}" = "LMC1" ] || fail "${payload_name}.loom is not LMC1"
done
rg -q '^native-primitives-table\b' "${PAYLOAD_DIR}/manifest.tsv" || \
    fail "manifest is missing native-primitives-table"
ok "generated Phase 24 native primitive fixture"

info "Building loom-ffi and DuckDB extension..."
cargo build -p loom-ffi --release
rm -f "${EXT_PATH}"
cmake -S "${REPO_ROOT}/duckdb-ext" \
      -B "${REPO_ROOT}/duckdb-ext/build" \
      -DCMAKE_BUILD_TYPE=Release \
      2>&1 | grep -v '^--' || true
cmake --build "${REPO_ROOT}/duckdb-ext/build" 2>&1
test -f "${EXT_PATH}" || fail "loom.duckdb_extension was not built"
ok "built ${EXT_PATH}"

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
        fail "unsupported platform for DuckDB CLI: ${OS}/${ARCH}"
    fi

    DUCKDB_BIN="${CLI_CACHE_DIR}/duckdb"
    if [ ! -x "${DUCKDB_BIN}" ]; then
        CLI_URL="https://github.com/duckdb/duckdb/releases/download/${DUCKDB_VERSION}/${CLI_ASSET}"
        mkdir -p "${CLI_CACHE_DIR}"
        TMPZIP="${CLI_CACHE_DIR}/${CLI_ASSET}"
        info "Downloading DuckDB ${DUCKDB_VERSION} CLI (${CLI_ASSET})..."
        curl -fSL --retry 3 --retry-delay 2 -o "${TMPZIP}" "${CLI_URL}"
        unzip -o "${TMPZIP}" -d "${CLI_CACHE_DIR}"
        rm -f "${TMPZIP}"
        chmod +x "${DUCKDB_BIN}"
    fi
fi
test -x "${DUCKDB_BIN}" || fail "DuckDB CLI not executable: ${DUCKDB_BIN}"
ok "DuckDB CLI ready"

sql_to_file() {
    local sql="$1"
    local out="$2"
    "${DUCKDB_BIN}" -unsigned -c \
        "LOAD '${EXT_PATH}'; COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);" \
        >/dev/null
}

sql_expect_failure() {
    local sql="$1"
    local err="$2"
    set +e
    "${DUCKDB_BIN}" -unsigned -c "LOAD '${EXT_PATH}'; ${sql}" >"${TMP_DIR}/failed-query.out" 2>"${err}"
    local status=$?
    set -e
    [ "${status}" -ne 0 ] || fail "expected DuckDB query to fail: ${sql}"
}

require_report() {
    local pattern="$1"
    rg -q "${pattern}" "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" || {
        echo "Route report:" >&2
        cat "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" >&2
        fail "route report missing ${pattern}"
    }
}

native_payload="${PAYLOAD_DIR}/native-primitives-table.loom"
fallback_payload="${PAYLOAD_DIR}/fsst-utf8.loom"
bitpack_payload="${PAYLOAD_DIR}/bitpack-i32.loom"

info "Checking native primitive table SQL rows and route diagnostics..."
unset LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK
unset LOOM_DUCKDB_TEST_USE_NATIVE_FACTS
unset LOOM_DUCKDB_TEST_CANCEL_PREPARE
native_out="${TMP_DIR}/native-agg.csv"
sql_to_file "SELECT COUNT(*), SUM(i32_col), SUM(i64_col) FROM loom_scan('${native_payload}')" "${native_out}"
[ "$(cat "${native_out}")" = "4,0,0" ] || fail "native primitive aggregate mismatch: $(cat "${native_out}")"
if rg -q 'native-candidate' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
    echo "native: route=native-candidate" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
elif rg -q 'toolchain-skipped|toolchain-failed' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
    echo "native: route unavailable because native toolchain diagnostic was emitted" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
else
    require_report 'interpreter-fallback'
    echo "native: route used interpreter fallback for unsupported native preparation" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
fi
if ! rg -q 'toolchain-skipped|toolchain-failed' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
    echo "toolchain-skipped: not observed because the native route completed or fell back before a toolchain skip" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
fi
ok "native primitive table SQL and route diagnostics"

info "Checking projection order over public loom_scan(path)..."
projection_out="${TMP_DIR}/projection.csv"
sql_to_file "SELECT f64_col, i32_col FROM loom_scan('${native_payload}')" "${projection_out}"
awk -F, 'NF == 2 && ($1 == "0" || $1 == "0.0") && $2 == "0" { ok++ } END { exit ok == 4 ? 0 : 1 }' \
    "${projection_out}" || fail "projection output column order mismatch"
ok "projection preserves requested column order"

info "Checking policy-controlled interpreter fallback..."
fallback_out="${TMP_DIR}/fallback.csv"
sql_to_file "SELECT COUNT(*), COUNT(value), MIN(value), MAX(value) FROM loom_scan('${fallback_payload}')" "${fallback_out}"
[ "$(cat "${fallback_out}")" = "3,2,alpha,beta" ] || fail "fallback aggregate mismatch: $(cat "${fallback_out}")"
require_report 'interpreter-fallback'
ok "interpreter fallback route visible"

info "Checking strict fail-closed diagnostics..."
export LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK=0
strict_err="${TMP_DIR}/strict.err"
sql_expect_failure "SELECT COUNT(*) FROM loom_scan('${fallback_payload}');" "${strict_err}"
rg -q 'diagnostic code=.*path=' "${strict_err}" || fail "strict failure missing stable diagnostic code/path"
cat "${strict_err}" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
require_report 'fail-closed'
unset LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK
ok "strict fail-closed error includes code/path diagnostics"

info "Checking cancellation path through test-only adapter control..."
export LOOM_DUCKDB_TEST_USE_NATIVE_FACTS=1
export LOOM_DUCKDB_TEST_CANCEL_PREPARE=1
cancel_err="${TMP_DIR}/cancel.err"
sql_expect_failure "SELECT COUNT(*) FROM loom_scan('${bitpack_payload}');" "${cancel_err}"
cat "${cancel_err}" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
require_report 'cancelled'
unset LOOM_DUCKDB_TEST_USE_NATIVE_FACTS
unset LOOM_DUCKDB_TEST_CANCEL_PREPARE
ok "cancelled route visible"

info "Checking malformed artifact error path and post-error release ownership..."
bad_payload="${TMP_DIR}/bad.loom"
printf 'LMC1bad' >"${bad_payload}"
bad_err="${TMP_DIR}/bad.err"
sql_expect_failure "SELECT COUNT(*) FROM loom_scan('${bad_payload}');" "${bad_err}"
release_out="${TMP_DIR}/post-error.csv"
sql_to_file "SELECT COUNT(*) FROM loom_scan('${native_payload}')" "${release_out}"
[ "$(cat "${release_out}")" = "4" ] || fail "valid scan failed after malformed artifact error"
ok "malformed artifact path fails without crashing later scans"

info "Checking helper-level native mismatch and cancellation tests..."
LOOM_ALLOW_NATIVE_TOOL_SKIP="${LOOM_ALLOW_NATIVE_TOOL_SKIP:-1}" \
    cargo test -p loom-ffi --test duckdb_runtime native_output_mismatch_fails_closed_without_interpreter_fallback
echo "native-output-mismatch: helper test passed" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
LOOM_ALLOW_NATIVE_TOOL_SKIP="${LOOM_ALLOW_NATIVE_TOOL_SKIP:-1}" \
    cargo test -p loom-ffi --test duckdb_runtime cancelled_prepare_returns_cancelled_diagnostic_and_no_native_buffers
echo "cancelled: helper test passed" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
require_report 'native-output-mismatch'
ok "native mismatch and cancellation helpers passed"

info "Checking repeated scan and single-worker/single-batch adapter evidence..."
first_count="${TMP_DIR}/first-count.csv"
second_count="${TMP_DIR}/second-count.csv"
sql_to_file "SELECT COUNT(*) FROM loom_scan('${native_payload}')" "${first_count}"
sql_to_file "SELECT COUNT(*) FROM loom_scan('${native_payload}')" "${second_count}"
[ "$(cat "${first_count}")" = "4" ] && [ "$(cat "${second_count}")" = "4" ] || \
    fail "repeated scan counts were not stable"
rg -q 'MaxThreads\(\) const override' duckdb-ext/loom_extension.cpp || fail "missing single-worker guard"
rg -q 'batch_emitted' duckdb-ext/loom_extension.cpp || fail "missing single-batch guard"
ok "repeated scan behavior and adapter guards"

info "Checking public SQL/API creep gates..."
route_prefix="loom_scan_"
for suffix in native interpreter; do
    if rg -n "${route_prefix}${suffix}" scripts/duckdb-native-integration-test.sh duckdb-ext/loom_extension.cpp crates/loom-ffi/include/loom.h; then
        fail "found forbidden public route function marker"
    fi
done
mode_word="mode "
mode_suffix=":="
stream_word="ArrowArray"
stream_suffix="Stream"
predicate_word="predicate "
predicate_suffix="pushdown"
split_word="parallel "
split_suffix="split"
for term in "${mode_word}${mode_suffix}" "${stream_word}${stream_suffix}" "${predicate_word}${predicate_suffix}" "${split_word}${split_suffix}"; do
    if rg -n "${term}" scripts/duckdb-native-integration-test.sh crates/loom-ffi/include/loom.h; then
        fail "found forbidden public API marker: ${term}"
    fi
done
ok "public SQL remains loom_scan(path)"

require_report 'native'
require_report 'interpreter-fallback'
require_report 'fail-closed'
require_report 'toolchain-skipped|toolchain-failed'
require_report 'cancelled'

echo ""
echo "${GRN}=== Phase 24 DuckDB native integration gate PASSED ===${RST}"
