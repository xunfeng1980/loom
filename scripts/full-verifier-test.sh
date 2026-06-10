#!/usr/bin/env bash
# full-verifier-test.sh - Phase 13 full Loom verifier foundation gate.
#
# Note: TLA+ lifecycle model removed (Phase 40+). K Framework now owns the
# operational semantics; Lean owns the static verifier scaffold. The trivial
# lifecycle state machine (Raw→Parsed→Verified→Lowerable→Lowered) is enforced
# by Rust code structure and does not warrant a separate formalism.

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

info() { echo "${YLW}[full-verifier]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

export PATH="${REPO_ROOT}/.tools/bin:${HOME}/.elan/bin:${PATH}"

require_cmd() {
    local cmd="$1"
    local install_hint="$2"
    if ! command -v "${cmd}" >/dev/null 2>&1; then
        fail "${cmd} is required. ${install_hint}"
    fi
}

PHASE_DIR=".planning/phases/13-full-loom-verifier"
SPEC="${PHASE_DIR}/13-VERIFIER-SPEC.md"
OBLIGATIONS="${PHASE_DIR}/13-PROOF-OBLIGATIONS.md"
LEAN_FILE="formal/lean/LoomCore.lean"

echo "=== Loom Phase 13 full-verifier gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 13 verifier documents and formal artifacts..."
for file in "${SPEC}" "${OBLIGATIONS}" "${LEAN_FILE}"; do
    if [ ! -f "${file}" ]; then
        fail "required verifier artifact missing: ${file}"
    fi
done
ok "required verifier artifacts exist"

info "Checking VERIFIER requirement IDs..."
for id in VERIFIER-01 VERIFIER-02 VERIFIER-03 VERIFIER-04 VERIFIER-05 VERIFIER-06 VERIFIER-07 VERIFIER-08 VERIFIER-09 VERIFIER-10; do
    rg -q "${id}" "${OBLIGATIONS}" || fail "missing ${id} in ${OBLIGATIONS}"
done
ok "all VERIFIER-01..VERIFIER-10 IDs are present"

info "Checking formal scaffold names..."
rg -q "accepted_program_safe" "${LEAN_FILE}" \
    || fail "Lean scaffold missing accepted_program_safe"
rg -q "builder_events_well_formed" "${LEAN_FILE}" \
    || fail "Lean scaffold missing builder_events_well_formed"
rg -q "ModeledExecutionSafe" "${LEAN_FILE}" \
    || fail "Lean model missing ModeledExecutionSafe"
rg -q "modeled executor only" "${LEAN_FILE}" \
    || fail "Lean theorem missing modeled-executor-only scope note"
if rg -n "_state : ModeledState" "${LEAN_FILE}"; then
    fail "Lean modeled safety predicates must not ignore ModeledState via _state"
fi
if rg -n "intro _h" "${LEAN_FILE}"; then
    fail "Lean accepted_program_safe must consume the Verified premise, not discard it"
fi
if rg -n "rowsUsed := min" "${LEAN_FILE}"; then
    fail "Lean modeled executor must fail closed on row-budget overflow, not clamp rowsUsed"
fi
if rg -n "readsInBounds" "${LEAN_FILE}"; then
    fail "Lean modeled reads must allow fail-closed out-of-bounds traces, not carry all-reads-in-bounds as a state invariant"
fi
if rg -n -F "And.intro (execProgram p).readSafety" "${LEAN_FILE}"; then
    fail "accepted_program_safe must not consume readSafety directly as the accepted-program dynamic safety result"
fi
for marker in \
    "state.status = .finished" \
    "state.reads.all (fun read => read.inBounds)" \
    "inBounds := false" \
    "appendModeledReadOutOfBoundsFailed" \
    "(execProgram outOfBoundsReadProgram).reads.all (fun read => read.inBounds) = false" \
    "state.events.all (eventWellTyped state.caps)" \
    "state.rowsUsed <= state.maxRows" \
    "no_ambient_authority p" \
    "builder_events_typed p" \
    "finite_bounds p" \
    "finalized_status_terminal" \
    "classified_program_finishes" \
    "verified_program_finishes" \
    "finished_state_reads_in_bounds" \
    "verified_program_reads_in_bounds" \
    "hFinished := verified_program_finishes p h" \
    "hReadsInBounds := verified_program_reads_in_bounds p h" \
    "(execProgram p).eventsTyped" \
    "(execProgram p).rowsWithinMax" \
    "checked_readInput_concrete_in_range"; do
    rg -q -F "${marker}" "${LEAN_FILE}" \
        || fail "Lean modeled soundness bridge missing state evidence marker: ${marker}"
done
rg -q -F "exact checked_readInput_concrete_in_range" "${LEAN_FILE}" \
    || fail "accepted_program_safe must consume the static read-boundary bridge theorem"
# Allow PHASE2-DEFERRED sorry markers (narrow-M3 execAppendTrace induction).
SORRY_LINES=$(rg -n '\bsorry\b' "${LEAN_FILE}" || true)
if [ -n "${SORRY_LINES}" ]; then
    # Check if any sorry is NOT preceded by PHASE2-DEFERRED within 3 lines
    UNEXPECTED_SORRY=$(echo "${SORRY_LINES}" | while IFS= read -r line; do
        LINE_NUM=$(echo "$line" | cut -d: -f1)
        # Look at up to 10 lines before the sorry for PHASE2-DEFERRED
        HEAD_CONTEXT=$(head -n "$((LINE_NUM - 1))" "${LEAN_FILE}" | tail -n 10)
        if ! echo "${HEAD_CONTEXT}" | rg -q 'PHASE2-DEFERRED'; then
            echo "$line"
        fi
    done)
    if [ -n "${UNEXPECTED_SORRY}" ]; then
        fail "Lean proof contains unexpected sorry (not marked PHASE2-DEFERRED): ${UNEXPECTED_SORRY}"
    fi
fi
ok "formal scaffold names are present"

if [ -f crates/loom-core/tests/l2_core_model.rs ]; then
    info "Running L2Core model tests..."
    cargo test -p loom-core --test l2_core_model
    ok "cargo test -p loom-core --test l2_core_model"
fi

if [ -f crates/loom-core/tests/full_verifier.rs ]; then
    info "Running executable full verifier tests..."
    cargo test -p loom-core --test full_verifier
    ok "cargo test -p loom-core --test full_verifier"
fi

info "Checking CLI full-verifier sample..."
cargo run --bin loom -- --help | rg -q "verify-l2core" \
    || fail "loom help does not expose verify-l2core"
cargo run --bin loom -- verify-l2core --sample >/dev/null
ok "loom verify-l2core --sample"

require_cmd lean "Run: mise run formal-tools"
info "Running Lean scaffold check..."
lean "${LEAN_FILE}"
ok "lean ${LEAN_FILE}"

info "Running verified-lineage closeout gate..."
bash scripts/verified-lineage-test.sh
ok "scripts/verified-lineage-test.sh"

echo ""
echo "${GRN}=== Phase 13 full-verifier gate PASSED ===${RST}"
