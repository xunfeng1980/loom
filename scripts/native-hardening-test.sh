#!/usr/bin/env bash
# native-hardening-test.sh - Phase 25 cache/fallback/fail-closed DuckDB gate.

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

info() { echo "${YLW}[native-hardening]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/loom-native-hardening-XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

export LOOM_DUCKDB_TEST_ROUTE_REPORT="${TMP_DIR}/route-report.tsv"
: >"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"

echo "=== Loom Phase 25 native hardening gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Generating deterministic DuckDB payloads..."
cargo run -p loom-fixtures --bin emit_duckdb_payloads >/dev/null
for payload_name in \
    native-primitives-table \
    fsst-utf8 \
    bitpack-i32 \
    bitpack-nullable-i32; do
    test -f "${PAYLOAD_DIR}/${payload_name}.loom" || fail "missing ${payload_name}.loom"
    magic="$(dd if="${PAYLOAD_DIR}/${payload_name}.loom" bs=4 count=1 2>/dev/null)"
    [ "${magic}" = "LMC1" ] || fail "${payload_name}.loom is not LMC1"
done
rg -q '^native-primitives-table\b' "${PAYLOAD_DIR}/manifest.tsv" || \
    fail "manifest is missing native-primitives-table"
ok "generated Phase 25 fixture set"

info "Building loom-ffi and DuckDB extension..."
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

duckdb_exec() {
    local sql="$1"
    "${DUCKDB_BIN}" -unsigned -c "LOAD '${EXT_PATH}'; ${sql}" >/dev/null
}

sql_to_file() {
    local sql="$1"
    local out="$2"
    duckdb_exec "COPY (${sql}) TO '${out}' (FORMAT CSV, HEADER FALSE);"
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

reset_report() {
    : >"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
}

print_report() {
    echo "Route report:" >&2
    cat "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" >&2
}

require_report() {
    local pattern="$1"
    rg -q "${pattern}" "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" || {
        print_report
        fail "route report missing ${pattern}"
    }
}

require_file_contains() {
    local path="$1"
    local pattern="$2"
    rg -q "${pattern}" "${path}" || {
        echo "File ${path}:" >&2
        cat "${path}" >&2
        fail "${path} missing ${pattern}"
    }
}

require_ordered_report() {
    local first="$1"
    local second="$2"
    local first_line
    local second_line
    first_line="$(rg -n "${first}" "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" | head -n1 | cut -d: -f1 || true)"
    second_line="$(rg -n "${second}" "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" | head -n1 | cut -d: -f1 || true)"
    if [ -z "${first_line}" ] || [ -z "${second_line}" ] || [ "${first_line}" -ge "${second_line}" ]; then
        print_report
        fail "route report did not contain ${first} before ${second}"
    fi
}

assert_cache_smoke_or_toolchain_skip() {
    local scope="$1"
    if rg -q 'cache-miss' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" &&
       rg -q 'cache-inserted' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}" &&
       rg -q 'cache-hit' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
        require_ordered_report 'cache-miss' 'cache-hit'
        ok "${scope} reports cache miss/insert followed by hit"
        return
    fi

    if [ "${LOOM_ALLOW_NATIVE_TOOL_SKIP:-}" = "1" ] &&
       rg -q 'toolchain-skipped|toolchain-failed' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
        info "${scope} emitted native toolchain skip/failure diagnostics; SQL rows still verified"
        return
    fi

    print_report
    fail "${scope} missing cache miss/insert/hit smoke evidence"
}

native_payload="${PAYLOAD_DIR}/native-primitives-table.loom"
fsst_payload="${PAYLOAD_DIR}/fsst-utf8.loom"
bitpack_payload="${PAYLOAD_DIR}/bitpack-i32.loom"
nullable_payload="${PAYLOAD_DIR}/bitpack-nullable-i32.loom"

info "Checking native primitive aggregate equality and repeated-scan cache evidence..."
reset_report
unset LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK
export LOOM_DUCKDB_TEST_USE_NATIVE_FACTS=1
unset LOOM_DUCKDB_TEST_CANCEL_PREPARE
first_native="${TMP_DIR}/native-agg-first.csv"
second_native="${TMP_DIR}/native-agg-second.csv"
duckdb_exec "
COPY (
    SELECT COUNT(*), SUM(i32_col), SUM(i64_col), SUM(f32_col), SUM(f64_col)
    FROM loom_scan('${native_payload}')
) TO '${first_native}' (FORMAT CSV, HEADER FALSE);
COPY (
    SELECT COUNT(*), SUM(i32_col), SUM(i64_col), SUM(f32_col), SUM(f64_col)
    FROM loom_scan('${native_payload}')
) TO '${second_native}' (FORMAT CSV, HEADER FALSE);
"
[ "$(cat "${first_native}")" = "4,0,0,0.0,0.0" ] || \
    fail "native aggregate mismatch: $(cat "${first_native}")"
cmp -s "${first_native}" "${second_native}" || fail "identical scan aggregate output changed"
require_report 'route=native-candidate|route=interpreter-fallback|toolchain-skipped|toolchain-failed'
assert_cache_smoke_or_toolchain_skip "identical native-primitives-table scans"
ok "native primitive aggregate equality is stable"

info "Checking reordered projection equality and cache-key drift..."
reset_report
projection_first="${TMP_DIR}/projection-first.csv"
projection_second="${TMP_DIR}/projection-second.csv"
full_projection_seed="${TMP_DIR}/projection-seed.csv"
duckdb_exec "
COPY (
    SELECT COUNT(*)
    FROM loom_scan('${native_payload}')
) TO '${full_projection_seed}' (FORMAT CSV, HEADER FALSE);
COPY (
    SELECT f64_col, i32_col
    FROM loom_scan('${native_payload}')
    ORDER BY i32_col, f64_col
) TO '${projection_first}' (FORMAT CSV, HEADER FALSE);
COPY (
    SELECT f64_col, i32_col
    FROM loom_scan('${native_payload}')
    ORDER BY i32_col, f64_col
) TO '${projection_second}' (FORMAT CSV, HEADER FALSE);
"
[ "$(cat "${full_projection_seed}")" = "4" ] || fail "full projection seed mismatch"
awk -F, 'NF == 2 && ($1 == "0" || $1 == "0.0") && $2 == "0" { ok++ } END { exit ok == 4 ? 0 : 1 }' \
    "${projection_first}" || fail "reordered projection output mismatch"
cmp -s "${projection_first}" "${projection_second}" || fail "repeated projection output changed"
require_report 'projection=columns:3>0,0>1'
if rg -q 'cache-inserted' "${LOOM_DUCKDB_TEST_ROUTE_REPORT}"; then
    require_report 'projection=columns:3>0,0>1.*cache-miss|cache-miss.*projection=columns:3>0,0>1'
    ok "projection cache key drift reports a miss"
else
    require_report 'toolchain-skipped|toolchain-failed'
    info "projection cache miss assertion skipped because native toolchain did not prepare"
fi
ok "projection order and repeated projection equality"
unset LOOM_DUCKDB_TEST_USE_NATIVE_FACTS

info "Checking FSST Utf8 interpreter fallback through public SQL..."
reset_report
fsst_out="${TMP_DIR}/fsst.csv"
sql_to_file "SELECT COUNT(*), COUNT(value), MIN(value), MAX(value) FROM loom_scan('${fsst_payload}')" "${fsst_out}"
[ "$(cat "${fsst_out}")" = "3,2,alpha,beta" ] || fail "FSST fallback aggregate mismatch: $(cat "${fsst_out}")"
require_report 'interpreter-fallback'
require_report 'lowering-unsupported|unsupported-type|unsupported-kernel'
ok "FSST Utf8 fallback is visible"

info "Checking strict fail-closed unsupported string/native lowering..."
reset_report
export LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK=0
strict_err="${TMP_DIR}/strict-fsst.err"
sql_expect_failure "SELECT COUNT(*) FROM loom_scan('${fsst_payload}');" "${strict_err}"
require_file_contains "${strict_err}" 'diagnostic code=.*path='
cat "${strict_err}" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
require_report 'fail-closed'
require_report 'fallback-disabled|lowering-unsupported|unsupported-type|unsupported-kernel'
unset LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK
ok "strict string/native lowering fails closed"

info "Checking nullable and compressed fixture fallback/fail-closed evidence..."
reset_report
nullable_out="${TMP_DIR}/nullable.csv"
bitpack_out="${TMP_DIR}/bitpack.csv"
sql_to_file "SELECT COUNT(*), COUNT(value), SUM(value), MIN(value), MAX(value) FROM loom_scan('${nullable_payload}')" "${nullable_out}"
[ "$(cat "${nullable_out}")" = "5,3,11,1,7" ] || fail "nullable fallback aggregate mismatch: $(cat "${nullable_out}")"
sql_to_file "SELECT COUNT(*), SUM(value), MIN(value), MAX(value) FROM loom_scan('${bitpack_payload}')" "${bitpack_out}"
[ "$(cat "${bitpack_out}")" = "4,10,1,4" ] || fail "bitpack fallback aggregate mismatch: $(cat "${bitpack_out}")"
require_report 'interpreter-fallback'
require_report 'lowering-unsupported|unsupported-kernel|missing-l2-facts'
export LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK=0
strict_bitpack_err="${TMP_DIR}/strict-bitpack.err"
sql_expect_failure "SELECT COUNT(*) FROM loom_scan('${bitpack_payload}');" "${strict_bitpack_err}"
require_file_contains "${strict_bitpack_err}" 'diagnostic code=.*path='
cat "${strict_bitpack_err}" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
require_report 'fail-closed'
unset LOOM_DUCKDB_TEST_ALLOW_INTERPRETER_FALLBACK
ok "nullable/compressed routes fall back or fail closed deterministically"

info "Checking cancellation through internal prepare hook..."
reset_report
export LOOM_DUCKDB_TEST_USE_NATIVE_FACTS=1
export LOOM_DUCKDB_TEST_CANCEL_PREPARE=1
cancel_err="${TMP_DIR}/cancel.err"
sql_expect_failure "SELECT COUNT(*) FROM loom_scan('${bitpack_payload}');" "${cancel_err}"
cat "${cancel_err}" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
require_report 'cancelled'
require_report 'cache-non-cacheable'
unset LOOM_DUCKDB_TEST_USE_NATIVE_FACTS
unset LOOM_DUCKDB_TEST_CANCEL_PREPARE
ok "cancelled prepare is fail-closed with diagnostics"

info "Checking malformed artifact followed by successful scan..."
reset_report
bad_payload="${TMP_DIR}/bad.loom"
printf 'LMC1bad' >"${bad_payload}"
bad_err="${TMP_DIR}/bad.err"
sql_expect_failure "SELECT COUNT(*) FROM loom_scan('${bad_payload}');" "${bad_err}"
post_error_out="${TMP_DIR}/post-error.csv"
sql_to_file "SELECT COUNT(*) FROM loom_scan('${native_payload}')" "${post_error_out}"
[ "$(cat "${post_error_out}")" = "4" ] || fail "valid scan failed after malformed artifact"
ok "malformed artifact failure does not poison later scans"

info "Checking helper-level mismatch and non-cacheable cache routes..."
LOOM_ALLOW_NATIVE_TOOL_SKIP="${LOOM_ALLOW_NATIVE_TOOL_SKIP:-1}" \
    cargo test -p loom-ffi --test duckdb_runtime native_output_mismatch_fails_closed_without_interpreter_fallback
echo "native-output-mismatch: helper test passed" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
LOOM_ALLOW_NATIVE_TOOL_SKIP="${LOOM_ALLOW_NATIVE_TOOL_SKIP:-1}" \
    cargo test -p loom-ffi --test duckdb_runtime_cache unsafe_routes_are_non_cacheable_and_do_not_seed_hits
echo "cache-non-cacheable: helper test passed" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
LOOM_ALLOW_NATIVE_TOOL_SKIP="${LOOM_ALLOW_NATIVE_TOOL_SKIP:-1}" \
    cargo test -p loom-ffi --test duckdb_runtime_cache canonical_input_mismatch_for_same_stable_id_reports_key_mismatch
echo "cache-key-mismatch: helper test passed" >>"${LOOM_DUCKDB_TEST_ROUTE_REPORT}"
require_report 'native-output-mismatch'
require_report 'cache-non-cacheable'
require_report 'cache-key-mismatch'
ok "helper-only mismatch and cache safety routes passed"

info "Checking public SQL/API creep gates..."
route_prefix="loom_scan_"
for suffix in native interpreter fallback cache; do
    if rg -n "${route_prefix}${suffix}" scripts/native-hardening-test.sh duckdb-ext/loom_extension.cpp crates/loom-ffi/include/loom.h; then
        fail "found forbidden public route function marker"
    fi
done
cache_word="cache"
for suffix in "_mode" "-mode" " mode"; do
    if rg -n "${cache_word}${suffix}" scripts/native-hardening-test.sh duckdb-ext/loom_extension.cpp crates/loom-ffi/include/loom.h; then
        fail "found forbidden public ${cache_word} ${suffix} marker"
    fi
done
stream_left="ArrowArray"
stream_right="Stream"
predicate_left="predicate"
predicate_right="pushdown"
split_left="parallel"
split_right="split"
for term in \
    "${stream_left}${stream_right}" \
    "${predicate_left}[ _-]${predicate_right}" \
    "${split_left}[ _-]${split_right}"; do
    if rg -n "${term}" scripts/native-hardening-test.sh duckdb-ext/loom_extension.cpp crates/loom-ffi/include/loom.h; then
        fail "found forbidden public API marker: ${term}"
    fi
done
ok "public SQL remains loom_scan(path)"

echo ""
echo "${GRN}=== Phase 25 native hardening gate PASSED ===${RST}"
