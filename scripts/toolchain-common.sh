#!/usr/bin/env bash
# Shared external toolchain helpers for release gates.

toolchain_find_tool() {
    local name="$1"
    if command -v "${name}" >/dev/null 2>&1; then
        command -v "${name}"
        return 0
    fi
    for candidate in \
        "/opt/homebrew/opt/llvm/bin/${name}" \
        "/usr/local/opt/llvm/bin/${name}"; do
        if [ -x "${candidate}" ]; then
            echo "${candidate}"
            return 0
        fi
    done
    return 1
}

toolchain_major_version() {
    sed -E 's/[^0-9]*([0-9]+).*/\1/'
}

toolchain_llvm_bin_dir() {
    local expected_major="${LOOM_EXPECTED_MLIR_MAJOR:-22}"
    local allow_skip="${LOOM_ALLOW_NATIVE_TOOL_SKIP:-0}"
    local llvm_config mlir_opt mlir_translate lli llvm_version llvm_major

    llvm_config="$(toolchain_find_tool llvm-config || true)"
    mlir_opt="$(toolchain_find_tool mlir-opt || true)"
    mlir_translate="$(toolchain_find_tool mlir-translate || true)"
    lli="$(toolchain_find_tool lli || true)"

    if [ -z "${llvm_config}" ] || [ -z "${mlir_opt}" ] || [ -z "${mlir_translate}" ] || [ -z "${lli}" ]; then
        if [ "${allow_skip}" = "1" ]; then
            return 2
        fi
        echo "compatible LLVM/MLIR ${expected_major} tools are required. Run: mise run external-tools" >&2
        return 1
    fi

    llvm_version="$("${llvm_config}" --version)"
    llvm_major="$(printf '%s\n' "${llvm_version}" | toolchain_major_version)"
    if [ "${llvm_major}" != "${expected_major}" ]; then
        if [ "${allow_skip}" = "1" ]; then
            return 2
        fi
        echo "detected LLVM/MLIR major ${llvm_major}, expected ${expected_major}. Run: mise run external-tools" >&2
        return 1
    fi

    dirname "${llvm_config}"
}
