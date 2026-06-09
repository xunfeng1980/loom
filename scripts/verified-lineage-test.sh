#!/usr/bin/env bash
# verified-lineage-test.sh - Phase 41 MVP1.5 verified-lineage closeout gate.

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

info() { echo "${YLW}[verified-lineage]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

export PATH="${REPO_ROOT}/.tools/bin:${HOME}/.elan/bin:${PATH}"

LEAN_FILE="formal/lean/LoomCore.lean"
CONTRACT=".planning/phases/36-verified-lineage-contract-and-tcb-declaration/36-VERIFIED-LINEAGE-CONTRACT.md"
PHASE38_SUMMARY=".planning/phases/38-lean-stage-c-operational-semantics-and-soundness-theorem/38-02-SUMMARY.md"
PHASE39_SUMMARY=".planning/phases/39-model-rust-interpreter-consistency/39-02-SUMMARY.md"
PHASE40_SUMMARY=".planning/phases/40-native-model-validation/40-02-SUMMARY.md"

echo "=== Loom Phase 41 verified-lineage closeout gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

if ! command -v lean >/dev/null 2>&1; then
    fail "lean is required. Run: mise run formal-tools"
fi

info "Checking verified-lineage contract artifacts..."
for file in \
    "${LEAN_FILE}" \
    "${CONTRACT}" \
    "${PHASE38_SUMMARY}" \
    "${PHASE39_SUMMARY}" \
    "${PHASE40_SUMMARY}"; do
    [ -f "${file}" ] || fail "required lineage artifact missing: ${file}"
done
ok "required lineage artifacts exist"

info "Running Lean modeled executor and no-sorry check..."
lean "${LEAN_FILE}" >/dev/null
if rg -n '\bsorry\b' "${LEAN_FILE}"; then
    fail "Lean proof contains sorry"
fi
for marker in \
    "accepted_program_safe" \
    "ModeledExecutionSafe" \
    "(execProgram p).readSafety" \
    "inBounds := false" \
    "appendModeledReadOutOfBoundsFailed" \
    "modeled executor only"; do
    rg -q -F "${marker}" "${LEAN_FILE}" \
        || fail "Lean modeled soundness marker missing: ${marker}"
done
ok "Lean modeled executor evidence"

info "Running Lean/Rust verifier differential gate..."
bash scripts/lean-rust-correspondence-test.sh
ok "Lean/Rust verifier differential"

info "Running model/Rust interpreter trace consistency gate..."
bash scripts/model-rust-interpreter-consistency-test.sh
ok "model/Rust interpreter trace consistency"

info "Running native/model validation gate..."
bash scripts/native-model-validation-test.sh
ok "native/model validation"

info "Checking verified-lineage non-claim and TCB markers..."
for marker in \
    "Loom guarantees safety + well-formedness, never correctness." \
    "explicit TCB trust assumption" \
    "Rust compiler/std" \
    "LLVM + MLIR toolchain" \
    "Rust<->C ABI" \
    "DuckDB host process" \
    "Arrow C Data Interface" \
    "Any \"verified\" row without a backing layer or TCB assignment"; do
    rg -q -F "${marker}" "${CONTRACT}" \
        || fail "verified-lineage contract marker missing: ${marker}"
done
rg -q -F "not a proof of all-program Rust/model equivalence" "${PHASE39_SUMMARY}" \
    || fail "Phase 39 summary missing per-run validation non-claim"
rg -q -F "not verified compilation" "${PHASE40_SUMMARY}" \
    || fail "Phase 40 summary missing verified-compilation non-claim"
rg -q -F "per-run-validation;mlir-llvm-native-lowering-tcb" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "native cache identity missing permanent TCB toolchain marker"
rg -q -F "native-model-trace-mismatch" crates/loom-core/src/native_arrow_semantic.rs \
    || fail "native/model validation missing stable mismatch diagnostic"
ok "non-claim and TCB markers"

echo ""
echo "${GRN}=== Verified-lineage closeout gate PASSED ===${RST}"
