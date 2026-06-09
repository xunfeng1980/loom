#!/usr/bin/env bash
# production-native-codegen-realization-test.sh - Phase 43.1 focused gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

echo "=== Phase 43.1: production native codegen realization ==="
echo "Repository: ${REPO_ROOT}"
echo ""

unset LOOM_ALLOW_NATIVE_TOOL_SKIP

echo "[phase43.1] Checking production-codegen sources do not use zero-buffer placeholders..."
if rg -n "reference_zeroed_value_bytes" \
    crates/loom-core/src/native_arrow_semantic.rs \
    crates/loom-native-melior/src/jit.rs \
    crates/loom-native-melior/tests/production_arrow_semantic_jit.rs; then
    echo "[phase43.1] ERROR: production Arrow semantic codegen path references zero-buffer placeholder" >&2
    exit 1
fi

echo "[phase43.1] Running Loom-core codegen support/admission tests..."
cargo test -p loom-core --test native_arrow_semantic_codegen

echo "[phase43.1] Running existing native Arrow semantic validation tests..."
cargo test -p loom-core --test native_arrow_semantic

echo "[phase43.1] Running default non-feature JIT guard..."
cargo test -p loom-native-melior --test production_arrow_semantic_jit

echo "[phase43.1] Running real melior ExecutionEngine Arrow semantic JIT..."
cargo test -p loom-native-melior --features melior --test production_arrow_semantic_jit

echo ""
echo "=== Phase 43.1 production native codegen realization PASSED ==="

