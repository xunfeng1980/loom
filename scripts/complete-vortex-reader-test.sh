#!/usr/bin/env bash
# complete-vortex-reader-test.sh - Phase 18 complete Vortex reader boundary gate.

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

info() { echo "${YLW}[complete-vortex-reader]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

PHASE_DIR=".planning/phases/18-complete-vortex-reader"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "=== Loom Phase 18 complete Vortex reader gate ==="
echo "Repository: ${REPO_ROOT}"
echo ""

info "Checking Phase 18 planning and closeout artifacts..."
for file in \
    "${PHASE_DIR}/18-RESEARCH.md" \
    "${PHASE_DIR}/18-CONTEXT.md" \
    "${PHASE_DIR}/18-READER-CONTRACT.md" \
    "${PHASE_DIR}/18-READER-REPORT.md" \
    "${PHASE_DIR}/18-SUMMARY.md"; do
    if [ ! -f "${file}" ]; then
        fail "required Phase 18 artifact missing: ${file}"
    fi
done
rg -q "Complete Reader Boundary" "${PHASE_DIR}/18-READER-REPORT.md" \
    || fail "reader report missing Complete Reader Boundary"
rg -q "Supported Emission Matrix" "${PHASE_DIR}/18-READER-REPORT.md" \
    || fail "reader report missing Supported Emission Matrix"
rg -q "Phase 19 Handoff" "${PHASE_DIR}/18-READER-REPORT.md" \
    || fail "reader report missing Phase 19 Handoff"
rg -q "Self-Check" "${PHASE_DIR}/18-SUMMARY.md" \
    || fail "summary missing self-check"
ok "Phase 18 docs are present"

info "Checking complete-reader implementation markers..."
rg -q "pub struct VortexReaderFacts" ingress/loom-vortex-ingress/src/lib.rs \
    || fail "missing VortexReaderFacts"
rg -q "pub enum VortexReaderEmissionKind" ingress/loom-vortex-ingress/src/lib.rs \
    || fail "missing VortexReaderEmissionKind"
rg -q "fn scan_supported_table" ingress/loom-vortex-ingress/src/lib.rs \
    || fail "missing supported table scan"
rg -q "scan_i64_values_from_vortex_buffer" ingress/loom-vortex-ingress/src/lib.rs \
    || fail "missing i64 scan oracle helper"
rg -q "scan_f32_values_from_vortex_buffer" ingress/loom-vortex-ingress/src/lib.rs \
    || fail "missing f32 scan oracle helper"
rg -q "scan_f64_values_from_vortex_buffer" ingress/loom-vortex-ingress/src/lib.rs \
    || fail "missing f64 scan oracle helper"
rg -q "extract_sidecar_bytes_from_vortex_buffer" ingress/loom-vortex-ingress/src/source_contract.rs \
    || fail "missing thin adapter Vortex sidecar extract"
ok "implementation markers are present"

info "Running full ingress crate tests..."
cargo test -p loom-vortex-ingress
ok "cargo test -p loom-vortex-ingress"

info "Running focused complete-reader accepted tests..."
cargo test -p loom-vortex-ingress reader_facts_contract
cargo test -p loom-vortex-ingress reader_recursive_facts
cargo test -p loom-vortex-ingress single_column_to_loom
cargo test -p loom-vortex-ingress table_to_loom
ok "focused complete-reader tests"

info "Running explicit malformed and unsupported negative tests..."
cargo test -p loom-vortex-ingress reader_facts_contract_malformed_buffer_fails_closed
cargo test -p loom-vortex-ingress single_column_to_loom_unsupported_utf8_emits_no_bytes
cargo test -p loom-vortex-ingress table_to_loom_unsupported_field_fails_closed
ok "negative fail-closed tests"

info "Running artifact verifier handoff tests..."
cargo test -p loom-core --test artifact_verifier
ok "cargo test -p loom-core --test artifact_verifier"

info "Generating deterministic Vortex fixtures..."
cargo run -q -p loom-vortex-ingress --bin emit_vortex_ingress_fixtures
test -f fixtures/vortex/int32-flat.vortex || fail "missing int32 Vortex fixture"
test -f fixtures/loom/int32-flat.loom || fail "missing int32 Loom fixture"
ok "deterministic fixtures generated"

info "Checking CLI inspect complete-reader report..."
inspect_output="$(cargo run -q --bin loom -- ingest-vortex --inspect fixtures/vortex/int32-flat.vortex)"
grep -q "status: accepted" <<<"${inspect_output}" \
    || fail "CLI inspect missing accepted status"
grep -q "row_count: 4" <<<"${inspect_output}" \
    || fail "CLI inspect missing row count"
grep -q "reader_support: accepted" <<<"${inspect_output}" \
    || fail "CLI inspect missing reader support"
grep -q "emission_kind: LMP1" <<<"${inspect_output}" \
    || fail "CLI inspect missing emission kind"
grep -q "reader_layout_facts:" <<<"${inspect_output}" \
    || fail "CLI inspect missing layout facts"
grep -q "reader_segment_facts:" <<<"${inspect_output}" \
    || fail "CLI inspect missing segment facts"
grep -q "reader_artifact_verification: pass" <<<"${inspect_output}" \
    || fail "CLI inspect missing artifact verifier status"
ok "CLI inspect report"

info "Checking CLI emit and artifact verifier handoff..."
emitted="${TMP_DIR}/int32-flat.loom"
cargo run -q --bin loom -- ingest-vortex --emit-loom fixtures/vortex/int32-flat.vortex "${emitted}" >/dev/null
test -s "${emitted}" || fail "CLI emit produced no artifact"
verify_output="$(cargo run -q --bin loom -- verify-artifact "${emitted}")"
grep -q "artifact_verification: pass" <<<"${verify_output}" \
    || fail "emitted artifact did not verify"
grep -q "artifact: LMC2" <<<"${verify_output}" \
    || fail "emitted artifact not identified as LMC2"
grep -q "payload: Arrow semantic payload" <<<"${verify_output}" \
    || fail "emitted artifact missing Arrow semantic payload"
ok "CLI emit verifies through artifact verifier"

info "Running dependency-boundary guards..."
bash scripts/check-core-invariants.sh
ok "scripts/check-core-invariants.sh"

echo ""
echo "${GRN}=== Phase 18 complete Vortex reader gate PASSED ===${RST}"
