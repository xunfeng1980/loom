#!/usr/bin/env bash
# mvp1-review-audit-test.sh - focused Phase 32 review marker gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

fail() {
    echo "[FAIL] $*" >&2
    exit 1
}

ok() {
    echo "[PASS] $*"
}

require_file() {
    local path="$1"
    test -f "${path}" || fail "missing ${path}"
}

require_fixed() {
    local needle="$1"
    local path="$2"
    rg -q --fixed-strings "${needle}" "${path}" || fail "${path} missing fixed marker: ${needle}"
}

require_regex() {
    local pattern="$1"
    local path="$2"
    rg -q "${pattern}" "${path}" || fail "${path} missing regex marker: ${pattern}"
}

require_order() {
    local first="$1"
    local second="$2"
    local path="$3"
    local first_line
    local second_line
    first_line="$(rg -n --fixed-strings "${first}" "${path}" | head -n1 | cut -d: -f1 || true)"
    second_line="$(rg -n --fixed-strings "${second}" "${path}" | head -n1 | cut -d: -f1 || true)"
    if [ -z "${first_line}" ] || [ -z "${second_line}" ] || [ "${first_line}" -ge "${second_line}" ]; then
        fail "${path} does not contain '${first}' before '${second}'"
    fi
}

CLAIM_LEDGER=".planning/phases/32-mvp1-architecture-and-code-review/32-CLAIM-LEDGER.md"
EVIDENCE_REVIEW=".planning/phases/32-mvp1-architecture-and-code-review/32-EXECUTION-EVIDENCE-REVIEW.md"
BOUNDARY_REVIEW=".planning/phases/32-mvp1-architecture-and-code-review/32-ARCHITECTURE-BOUNDARY-REVIEW.md"
CODE_REVIEW=".planning/phases/32-mvp1-architecture-and-code-review/32-CODE-REVIEW.md"
READINESS_REPORT=".planning/phases/32-mvp1-architecture-and-code-review/32-MVP1-RELEASE-READINESS.md"
MVP1_SCRIPT="scripts/mvp1-verify.sh"
SOURCE_E2E="scripts/duckdb-source-e2e-test.sh"
NATIVE_HARDENING="scripts/native-hardening-test.sh"
PUBLIC_HEADER="crates/loom-ffi/include/loom.h"
INTERNAL_DUCKDB_HEADER="crates/loom-ffi/include/loom_duckdb_internal.h"
CBINDGEN_CONFIG="crates/loom-ffi/cbindgen.toml"
ARROW_SEMANTIC_CODEC="crates/loom-core/src/arrow_semantic_codec.rs"

echo "=== Loom MVP1 review audit marker gate ==="

require_file "${CLAIM_LEDGER}"
require_file "${EVIDENCE_REVIEW}"
require_file "${BOUNDARY_REVIEW}"
require_file "${CODE_REVIEW}"
require_file "${READINESS_REPORT}"
require_file "${MVP1_SCRIPT}"
require_file "${SOURCE_E2E}"
require_file "${NATIVE_HARDENING}"
require_file "${PUBLIC_HEADER}"
require_file "${INTERNAL_DUCKDB_HEADER}"
require_file "${CBINDGEN_CONFIG}"
require_file "${ARROW_SEMANTIC_CODEC}"
ok "required review files exist"

require_fixed "Claim Ledger" "${CLAIM_LEDGER}"
require_fixed "Actual Status" "${CLAIM_LEDGER}"
require_fixed "Required Action" "${CLAIM_LEDGER}"
require_fixed "fallback" "${CLAIM_LEDGER}"
require_fixed "deferred" "${CLAIM_LEDGER}"
require_fixed "unsupported" "${CLAIM_LEDGER}"
ok "claim ledger markers"

require_fixed "Execution Evidence Matrix" "${EVIDENCE_REVIEW}"
require_fixed "Proves" "${EVIDENCE_REVIEW}"
require_fixed "Does Not Prove" "${EVIDENCE_REVIEW}"
require_fixed "duckdb-source-e2e" "${EVIDENCE_REVIEW}"
require_fixed "native-hardening" "${EVIDENCE_REVIEW}"
require_fixed "fallback" "${EVIDENCE_REVIEW}"
require_fixed "skip" "${EVIDENCE_REVIEW}"
require_fixed "single-column" "${EVIDENCE_REVIEW}"
require_fixed "StarRocks" "${EVIDENCE_REVIEW}"
require_fixed "LMC2" "${EVIDENCE_REVIEW}"
ok "execution evidence review markers"

require_fixed "Architecture Boundary Review" "${BOUNDARY_REVIEW}"
require_fixed 'Public `loom.h`' "${BOUNDARY_REVIEW}"
require_fixed "Internal DuckDB Header" "${BOUNDARY_REVIEW}"
require_fixed 'future `LMC2` wrapper' "${BOUNDARY_REVIEW}"
require_fixed "arrow-semantic-lowering-deferred" "${BOUNDARY_REVIEW}"
ok "architecture boundary review markers"

require_fixed "Findings" "${CODE_REVIEW}"
require_fixed "Severity" "${CODE_REVIEW}"
require_fixed "Test Gaps" "${CODE_REVIEW}"
require_fixed "No high-severity production bugs" "${CODE_REVIEW}"
ok "code review markers"

require_fixed "Go/No-Go" "${READINESS_REPORT}"
require_fixed "BLOCKING" "${READINESS_REPORT}"
require_fixed "HIGH" "${READINESS_REPORT}"
require_fixed "MEDIUM" "${READINESS_REPORT}"
require_fixed "Phase 30" "${READINESS_REPORT}"
require_fixed "Deferred" "${READINESS_REPORT}"
require_fixed "Remediation" "${READINESS_REPORT}"
require_fixed "GO for an MVP1 baseline with bounded claims" "${READINESS_REPORT}"
ok "readiness report markers"

require_order "bash scripts/mvp0-verify.sh" "bash scripts/duckdb-source-e2e-test.sh" "${MVP1_SCRIPT}"
ok "mvp1-verify runs inherited gate before source e2e"

require_fixed "assert_lma1" "${SOURCE_E2E}"
require_fixed "expected_rows=\$'7\\n-1\\n42'" "${SOURCE_E2E}"
require_fixed 'expected_agg="3,48,-1,42"' "${SOURCE_E2E}"
require_fixed "SELECT value FROM loom_scan" "${SOURCE_E2E}"
require_fixed "SELECT COUNT(*), SUM(value), MIN(value), MAX(value) FROM loom_scan" "${SOURCE_E2E}"
ok "duckdb source e2e checks LMA1 magic and SQL rows/aggregates"

require_regex "route=native-candidate" "${NATIVE_HARDENING}"
require_regex "native-execution-engine-output" "${NATIVE_HARDENING}"
require_regex "interpreter-fallback" "${NATIVE_HARDENING}"
require_regex "toolchain-skipped|toolchain-failed" "${NATIVE_HARDENING}"
require_regex "fail-closed" "${NATIVE_HARDENING}"
require_regex "cache-miss" "${NATIVE_HARDENING}"
require_regex "cache-hit" "${NATIVE_HARDENING}"
ok "native hardening route/fallback/skip markers"

core_forbidden_count="$(cargo tree -p loom-core | awk '/vortex|fastlanes|parquet|lance|iceberg/{c++} END{print c+0}')"
if [ "${core_forbidden_count}" != "0" ]; then
    fail "loom-core has forbidden source/native dependency entries: ${core_forbidden_count}"
fi
ffi_forbidden_count="$(cargo tree -p loom-ffi | awk '/vortex|fastlanes|parquet|lance|iceberg/{c++} END{print c+0}')"
if [ "${ffi_forbidden_count}" != "0" ]; then
    fail "loom-ffi has forbidden source SDK dependency entries: ${ffi_forbidden_count}"
fi
ok "core/ffi source dependency guards"

require_fixed "loom_decode" "${PUBLIC_HEADER}"
for forbidden in \
    "loom_duckdb_" \
    "LoomDuckDb" \
    "duckdb_runtime" \
    "native_preparation" \
    "ArrowArrayStream"; do
    if rg -q --fixed-strings "${forbidden}" "${PUBLIC_HEADER}"; then
        fail "public loom.h leaked internal marker: ${forbidden}"
    fi
done
ok "public loom.h excludes internal route/native symbols"

require_fixed "loom_duckdb_plan_create" "${INTERNAL_DUCKDB_HEADER}"
require_fixed "loom_duckdb_prepare_native_buffer" "${INTERNAL_DUCKDB_HEADER}"
require_fixed "This header is non-public" "${INTERNAL_DUCKDB_HEADER}"
require_fixed "LoomDuckDbPlan" "${CBINDGEN_CONFIG}"
require_fixed "loom_duckdb_plan_create" "${CBINDGEN_CONFIG}"
ok "internal DuckDB header and cbindgen exclusions"

require_fixed "LMA1_MAGIC" "${ARROW_SEMANTIC_CODEC}"
require_fixed "is_arrow_semantic_container" "${ARROW_SEMANTIC_CODEC}"
ok "LMA1 direct payload and future LMC2 wrapper markers"

echo "[PASS] MVP1 review audit marker gate"
