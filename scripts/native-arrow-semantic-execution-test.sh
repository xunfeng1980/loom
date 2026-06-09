#!/usr/bin/env bash
# native-arrow-semantic-execution-test.sh - Phase 35 native Arrow semantic gate.

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

info() { echo "${YLW}[native-arrow-semantic]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

check_marker() {
    local pattern="$1"
    local file="$2"
    local label="$3"
    rg -q --fixed-strings "${pattern}" "${file}" || fail "missing ${label}: ${pattern} in ${file}"
}

echo "=== Loom Phase 35 native Arrow semantic execution gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking native Arrow semantic implementation markers..."
check_marker "loom-native-arrow-semantic" crates/loom-core/src/native_arrow_semantic.rs "backend identity"
check_marker "verify_native_arrow_semantic_equivalence" crates/loom-core/src/native_arrow_semantic.rs "equivalence helper"
check_marker "native_arrow_semantic_runtime_cache_key" crates/loom-core/src/native_arrow_semantic.rs "runtime cache key helper"
check_marker "UnsupportedType" crates/loom-core/src/native_arrow_semantic.rs "unsupported type diagnostic"
ok "implementation markers are present"

info "Checking host-neutral native module vocabulary..."
if rg -i "duckdb|starrocks" crates/loom-core/src/native_arrow_semantic.rs crates/loom-core/tests/native_arrow_semantic.rs >/dev/null; then
    rg -n -i "duckdb|starrocks" crates/loom-core/src/native_arrow_semantic.rs crates/loom-core/tests/native_arrow_semantic.rs >&2
    fail "native Arrow semantic core evidence must remain host-neutral"
fi
ok "native Arrow semantic evidence is host-neutral"

info "Running native Arrow semantic execution tests..."
cargo test -p loom-core --test native_arrow_semantic
ok "native Arrow semantic execution tests"

info "Running runtime/cache identity regressions..."
cargo test -p loom-core --test runtime_execution_policy
cargo test -p loom-core --test runtime_cache_key
ok "runtime/cache regressions"

echo ""
echo "${GRN}=== Native Arrow semantic execution gate PASSED ===${RST}"
