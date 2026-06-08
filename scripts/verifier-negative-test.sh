#!/usr/bin/env bash
# verifier-negative-test.sh - malformed descriptor regression gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

cargo build -q -p loom-cli
cargo run -q -p loom-fixtures --bin emit_duckdb_payloads >/dev/null

write_invalid_raw_bytes() {
    cat >"${tmpdir}/invalid-raw-bytes.ron" <<'RON'
(
    version: 1,
    data_type: Int32,
    row_count: 1,
    root: Raw(
        elem_size: 4,
        count: 1,
        data: [1, 0],
    ),
)
RON
}

write_invalid_bit_width() {
    cat >"${tmpdir}/invalid-bit-width.ron" <<'RON'
(
    version: 1,
    data_type: Int64,
    row_count: 1,
    root: BitPack(
        bit_width: 65,
        offset: 0,
        count: 1,
        values_buf: [],
        validity: None,
        all_null: false,
    ),
)
RON
}

write_invalid_validity_len() {
    cat >"${tmpdir}/invalid-validity-len.ron" <<'RON'
(
    version: 1,
    data_type: Int32,
    row_count: 2,
    root: BitPack(
        bit_width: 1,
        offset: 0,
        count: 2,
        values_buf: [],
        validity: Some([true]),
        all_null: true,
    ),
)
RON
}

write_non_monotonic_run_end() {
    cat >"${tmpdir}/non-monotonic-run-end.ron" <<'RON'
(
    version: 1,
    data_type: Boolean,
    row_count: 2,
    root: RunEnd(
        count: 2,
        run_ends: Raw(
            elem_size: 4,
            count: 2,
            data: [2, 0, 0, 0, 1, 0, 0, 0],
        ),
        values: Raw(
            elem_size: 1,
            count: 2,
            data: [1, 0],
        ),
    ),
)
RON
}

write_unknown_kernel() {
    cat >"${tmpdir}/unknown-kernel.ron" <<'RON'
(
    version: 1,
    data_type: Utf8,
    row_count: 0,
    root: KernelEscape(
        kernel_id: 42,
        count: 0,
        params: [],
    ),
)
RON
}

write_malformed_table_row_count() {
    cp target/loom-duckdb-fixtures/mixed-table.loom "${tmpdir}/table-row-mismatch.loom"
    python3 - "${tmpdir}/table-row-mismatch.loom" <<'PY'
import struct
import sys

path = sys.argv[1]
with open(path, "r+b") as f:
    data = bytearray(f.read())
    if data[:4] != b"LMT1":
        raise SystemExit("expected LMT1 table payload")
    data[6:14] = struct.pack("<Q", 999)
    f.seek(0)
    f.write(data)
    f.truncate()
PY
}

write_truncated_binary() {
    printf 'LMP1' >"${tmpdir}/truncated.loom"
}

expect_verifier_failure() {
    local file="$1"
    local code="$2"
    local output

    set +e
    output="$(target/debug/loom inspect "${file}" 2>&1)"
    local rc=$?
    set -e

    if [ "${rc}" -eq 0 ]; then
        echo "[FAIL] expected verifier failure for ${file}" >&2
        echo "${output}" >&2
        exit 1
    fi
    if ! grep -q 'verification: fail' <<<"${output}"; then
        echo "[FAIL] missing verifier failure status for ${file}" >&2
        echo "${output}" >&2
        exit 1
    fi
    if ! grep -q "code=${code}" <<<"${output}"; then
        echo "[FAIL] missing verifier code ${code} for ${file}" >&2
        echo "${output}" >&2
        exit 1
    fi
    echo "[PASS] ${code}: ${file}"
}

expect_parse_failure() {
    local file="$1"
    local pattern="$2"
    local output

    set +e
    output="$(target/debug/loom inspect "${file}" 2>&1)"
    local rc=$?
    set -e

    if [ "${rc}" -eq 0 ]; then
        echo "[FAIL] expected parse failure for ${file}" >&2
        echo "${output}" >&2
        exit 1
    fi
    if ! grep -q "${pattern}" <<<"${output}"; then
        echo "[FAIL] missing parse failure pattern ${pattern} for ${file}" >&2
        echo "${output}" >&2
        exit 1
    fi
    echo "[PASS] parse failure: ${file}"
}

write_invalid_raw_bytes
write_invalid_bit_width
write_invalid_validity_len
write_non_monotonic_run_end
write_unknown_kernel
write_malformed_table_row_count
write_truncated_binary

expect_verifier_failure "${tmpdir}/invalid-raw-bytes.ron" "buffer-too-short"
expect_verifier_failure "${tmpdir}/invalid-bit-width.ron" "invalid-bit-width"
expect_verifier_failure "${tmpdir}/invalid-validity-len.ron" "validity-mismatch"
expect_verifier_failure "${tmpdir}/non-monotonic-run-end.ron" "invalid-run-end"
expect_verifier_failure "${tmpdir}/unknown-kernel.ron" "unknown-kernel"
expect_verifier_failure "${tmpdir}/table-row-mismatch.loom" "count-mismatch"
expect_parse_failure "${tmpdir}/truncated.loom" "malformed layout payload"

echo "[PASS] verifier negative descriptors fail closed"
