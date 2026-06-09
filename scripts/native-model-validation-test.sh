#!/usr/bin/env bash
# native-model-validation-test.sh - Phase 40 native/model validation gate.

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

info() { echo "${YLW}[native-model-validation]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

echo "=== Loom Phase 40 native/model validation gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Running Lean modeled executor and correspondence checks..."
lean formal/lean/LoomCore.lean >/dev/null
bash scripts/lean-rust-correspondence-test.sh
ok "Lean model and Lean/Rust correspondence"

info "Running model/Rust reference executor checks..."
bash scripts/model-rust-interpreter-consistency-test.sh
ok "model/Rust consistency"

info "Running native/model validation tests..."
cargo test -p loom-core --test native_arrow_semantic native_model
ok "native/model validation tests"

info "Checking native/model validation markers..."
rg -q "verify_native_arrow_semantic_model" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing native/model validation API"
rg -q "reference_trace" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing reference trace exposure"
rg -q "native_trace" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing native trace exposure"
rg -q "native-model-trace-mismatch" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing native/model trace mismatch diagnostic"
rg -q "validated_native_arrow_semantic_runtime_cache_key" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing validation-aware runtime cache key"
rg -q "per-run-validation;mlir-llvm-native-lowering-tcb" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "missing permanent TCB toolchain marker"
rg -q "verified compilation" .planning/phases/40-native-model-validation/40-02-SUMMARY.md \
    || fail "missing verified-compilation non-claim in Phase 40 summary"
ok "native/model validation markers"

echo ""
echo "${GRN}=== Native/model validation gate PASSED ===${RST}"
