#!/usr/bin/env bash
# production-native-codegen-stabilization-test.sh - Phase 43.2 focused gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

echo "=== Phase 43.2: production native codegen stabilization ==="
echo "Repository: ${REPO_ROOT}"
echo ""

if [ "${LOOM_ALLOW_NATIVE_TOOL_SKIP:-}" = "1" ]; then
    echo "[phase43.2] ERROR: positive stabilization evidence must not rely on LOOM_ALLOW_NATIVE_TOOL_SKIP=1" >&2
    exit 1
fi
unset LOOM_ALLOW_NATIVE_TOOL_SKIP

echo "[phase43.2] Checking production/stabilization sources do not use zero-buffer placeholders..."
if rg -n "reference_zeroed_value_bytes" \
    crates/loom-core/src/native_arrow_semantic.rs \
    crates/loom-native-melior/src/jit.rs \
    crates/loom-core/tests/native_arrow_semantic_codegen.rs \
    crates/loom-core/tests/native_arrow_semantic_codegen_stability.rs \
    crates/loom-core/tests/native_arrow_semantic_codegen_adversarial.rs \
    crates/loom-native-melior/tests/production_arrow_semantic_jit.rs \
    crates/loom-native-melior/tests/production_arrow_semantic_route.rs \
    crates/loom-native-melior/tests/production_arrow_semantic_soak.rs; then
    echo "[phase43.2] ERROR: production Arrow semantic codegen stabilization references zero-buffer placeholder" >&2
    exit 1
fi

echo "[phase43.2] Checking diagnostics, replay, cache, and resource fields remain versioned/tested..."
rg -q "NativeArrowSemanticCodegenReplayEvidence" crates/loom-core/src/native_arrow_semantic.rs
rg -q "validated_native_arrow_semantic_codegen_runtime_cache_key_with_shape" crates/loom-core/src/native_arrow_semantic.rs
rg -q "ArrowSemanticCodegenResourceEvidence" crates/loom-native-melior/src/jit.rs
rg -q "phase43.1-production-codegen" crates/loom-core/src/native_arrow_semantic.rs
rg -q "validation=native-model:phase40" crates/loom-core/src/native_arrow_semantic.rs
rg -q "raw_pointer_identity_used" crates/loom-native-melior/tests/production_arrow_semantic_soak.rs

echo "[phase43.2] Running Phase 43.1 production realization gate as prerequisite..."
bash scripts/production-native-codegen-realization-test.sh

echo "[phase43.2] Running deterministic replay/cache stability tests..."
cargo test -p loom-core --test native_arrow_semantic_codegen_stability

echo "[phase43.2] Running adversarial output validation tests..."
cargo test -p loom-core --test native_arrow_semantic_codegen_adversarial

echo "[phase43.2] Running production route tests..."
cargo test -p loom-native-melior --features melior --test production_arrow_semantic_route

echo "[phase43.2] Running soak/resource/cancellation tests..."
cargo test -p loom-native-melior --features melior --test production_arrow_semantic_soak

echo ""
echo "=== Phase 43.2 production native codegen stabilization PASSED ==="
