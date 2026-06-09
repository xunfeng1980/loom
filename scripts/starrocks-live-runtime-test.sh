#!/usr/bin/env bash
# starrocks-live-runtime-test.sh - Phase 43 StarRocks runtime evidence gate.

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

info() { echo "${YLW}[starrocks-live]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

OUT_DIR="${REPO_ROOT}/target/loom-starrocks-live-runtime-test"
DESCRIPTOR_PATH="${OUT_DIR}/starrocks-descriptors.json"
REPORT_PATH=".planning/phases/43-starrocks-live-runtime-integration/43-STARROCKS-RUNTIME-REPORT.md"

echo "=== Loom Phase 43 StarRocks live runtime gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 43 artifacts and runtime evidence markers..."
for file in \
    ".planning/phases/43-starrocks-live-runtime-integration/43-CONTEXT.md" \
    ".planning/phases/43-starrocks-live-runtime-integration/43-01-PLAN.md" \
    ".planning/phases/43-starrocks-live-runtime-integration/43-02-PLAN.md" \
    ".planning/phases/43-starrocks-live-runtime-integration/43-01-SUMMARY.md" \
    "crates/loom-dual-query-surface/tests/starrocks_runtime_contract.rs"; do
    [ -f "${file}" ] || fail "missing Phase 43 artifact: ${file}"
done
for marker in \
    "StarRocksRuntimeEvidence" \
    "StarRocksRuntimeStatus" \
    "validate_starrocks_runtime_output" \
    "missing_starrocks_runtime_evidence" \
    "unsupported_starrocks_runtime_evidence"; do
    rg -q -F "${marker}" crates/loom-dual-query-surface \
        || fail "missing StarRocks runtime evidence marker: ${marker}"
done
ok "Phase 43 runtime evidence markers are present"

info "Running StarRocks runtime contract tests..."
cargo test -p loom-dual-query-surface --test starrocks_runtime_contract
ok "StarRocks runtime contract tests"

info "Running inherited query-surface evidence tests..."
cargo test -p loom-dual-query-surface --test query_surface_contract
cargo test -p loom-dual-query-surface --test query_surface_negative
cargo test -p loom-dual-query-surface --test duckdb_evidence
ok "query-surface contract, negative, and DuckDB evidence tests"

info "Generating accepted fixture and StarRocks descriptors..."
rm -rf "${OUT_DIR}"
cargo run -p loom-dual-query-surface --bin emit_dual_query_fixture -- "${OUT_DIR}" >/dev/null
[ -f "${DESCRIPTOR_PATH}" ] || fail "descriptor JSON was not generated: ${DESCRIPTOR_PATH}"
ok "generated descriptor JSON"

descriptor_sha="$(
    python3 - "${DESCRIPTOR_PATH}" <<'PY'
import json
import sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
print(data[0]["identity"]["artifact_sha256"])
PY
)"
[ "${#descriptor_sha}" -eq 64 ] || fail "descriptor artifact SHA-256 was malformed"

run_live_starrocks_matrix() {
    local missing=()
    for var in \
        STARROCKS_MYSQL \
        STARROCKS_HOST \
        STARROCKS_PORT \
        STARROCKS_USER \
        STARROCKS_PASSWORD \
        STARROCKS_DATABASE \
        STARROCKS_TABLE \
        STARROCKS_LOOM_ARTIFACT_SHA256; do
        if [ -z "${!var:-}" ]; then
            missing+=("${var}")
        fi
    done

    if [ "${#missing[@]}" -ne 0 ]; then
        info "live StarRocks runtime evidence missing env: ${missing[*]}"
        info "missing live runtime evidence is not accepted StarRocks runtime evidence"
        if [ "${LOOM_REQUIRE_STARROCKS_LIVE:-0}" = "1" ]; then
            fail "strict live mode requires StarRocks runtime env"
        fi
        ok "local contract mode completed without live runtime claim"
        return
    fi

    [ -x "${STARROCKS_MYSQL}" ] || fail "STARROCKS_MYSQL is not executable: ${STARROCKS_MYSQL}"
    if [ "${STARROCKS_LOOM_ARTIFACT_SHA256}" != "${descriptor_sha}" ]; then
        fail "STARROCKS_LOOM_ARTIFACT_SHA256 does not match accepted descriptor identity"
    fi

    local rows_out="${OUT_DIR}/starrocks-rows.tsv"
    local predicate_out="${OUT_DIR}/starrocks-predicate.tsv"
    local count_out="${OUT_DIR}/starrocks-count.tsv"
    local sum_out="${OUT_DIR}/starrocks-sum.tsv"
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
    ok "live StarRocks runtime matched the accepted Loom/DuckDB/oracle matrix"
}

run_live_starrocks_matrix

if [ -f "${REPORT_PATH}" ]; then
    info "Checking runtime report markers..."
    for marker in \
        "Live Runtime Evidence Status" \
        "Missing runtime is not accepted evidence" \
        "Strict Live Mode" \
        "Fail-Closed Matrix" \
        "Artifact Identity Binding"; do
        rg -q -F "${marker}" "${REPORT_PATH}" || fail "runtime report missing marker: ${marker}"
    done
    ok "runtime report markers"
fi

echo ""
echo "${GRN}=== Phase 43 StarRocks live runtime gate PASSED ===${RST}"
