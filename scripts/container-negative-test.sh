#!/usr/bin/env bash
# container-negative-test.sh - malformed LMC1 regression gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

cargo build -q -p loom-cli
cargo run -q -p loom-fixtures --bin emit_duckdb_payloads >/dev/null

python3 - "${tmpdir}" <<'PY'
import pathlib
import struct
import sys

out_dir = pathlib.Path(sys.argv[1])
source = pathlib.Path("target/loom-duckdb-fixtures/bitpack-i32.loom")
data = bytearray(source.read_bytes())

if data[:4] != b"LMC1":
    raise SystemExit("expected LMC1 fixture")


def sections(buf):
    section_count = struct.unpack_from("<I", buf, 24)[0]
    pos = 28
    entries = []
    for _ in range(section_count):
        kind, flags, offset, length, crc, reserved = struct.unpack_from("<HHQQII", buf, pos)
        entries.append(
            {
                "pos": pos,
                "kind": kind,
                "flags": flags,
                "offset": offset,
                "length": length,
                "crc": crc,
                "reserved": reserved,
            }
        )
        pos += 28
    return entries


def layout_entry(buf):
    for entry in sections(buf):
        if entry["kind"] == 2:
            return entry
    raise SystemExit("expected layout payload section")


def write_case(name, buf):
    (out_dir / f"{name}.loom").write_bytes(bytes(buf))


unknown_required = bytearray(data)
required = struct.unpack_from("<Q", unknown_required, 8)[0]
struct.pack_into("<Q", unknown_required, 8, required | (1 << 63))
write_case("unknown-required-feature", unknown_required)

unsupported_version = bytearray(data)
struct.pack_into("<H", unsupported_version, 4, 99)
write_case("unsupported-version", unsupported_version)

truncated_section = bytearray(data)
entry = layout_entry(truncated_section)
struct.pack_into("<Q", truncated_section, entry["pos"] + 12, len(truncated_section) + 1024)
write_case("truncated-section", truncated_section)

offset_overflow = bytearray(data)
entry = layout_entry(offset_overflow)
struct.pack_into("<Q", offset_overflow, entry["pos"] + 4, (1 << 64) - 1)
write_case("offset-overflow", offset_overflow)

duplicate_payload = bytearray()
old_header_len = struct.unpack_from("<H", data, 6)[0]
old_entries = sections(data)
new_header_len = old_header_len + 28
new_section_count = len(old_entries) + 1
duplicate_payload.extend(data[:28])
struct.pack_into("<H", duplicate_payload, 6, new_header_len)
struct.pack_into("<I", duplicate_payload, 24, new_section_count)

layout = None
payload_parts = []
cursor = new_header_len
for entry in old_entries:
    adjusted = dict(entry)
    section_bytes = data[entry["offset"]:entry["offset"] + entry["length"]]
    adjusted["offset"] = cursor
    cursor += adjusted["length"]
    payload_parts.append(section_bytes)
    duplicate_payload.extend(
        struct.pack(
            "<HHQQII",
            adjusted["kind"],
            adjusted["flags"],
            adjusted["offset"],
            adjusted["length"],
            adjusted["crc"],
            adjusted["reserved"],
        )
    )
    if adjusted["kind"] == 2:
        layout = adjusted
        layout_bytes = section_bytes

if layout is None:
    raise SystemExit("expected layout payload section")

layout["offset"] = cursor
payload_parts.append(layout_bytes)
duplicate_payload.extend(
    struct.pack(
        "<HHQQII",
        layout["kind"],
        layout["flags"],
        layout["offset"],
        layout["length"],
        layout["crc"],
        layout["reserved"],
    )
)
for part in payload_parts:
    duplicate_payload.extend(part)
duplicate_payload.extend(b"LMC1")
write_case("duplicate-payload-section", duplicate_payload)
PY

expect_failure() {
    local file="$1"
    local pattern="$2"
    local output

    set +e
    output="$(target/debug/loom inspect "${file}" 2>&1)"
    local rc=$?
    set -e

    if [ "${rc}" -eq 0 ]; then
        echo "[FAIL] expected container failure for ${file}" >&2
        echo "${output}" >&2
        exit 1
    fi
    if ! grep -q "${pattern}" <<<"${output}"; then
        echo "[FAIL] missing failure pattern ${pattern} for ${file}" >&2
        echo "${output}" >&2
        exit 1
    fi
    echo "[PASS] container failure: ${file}"
}

expect_failure "${tmpdir}/unknown-required-feature.loom" "unknown required feature"
expect_failure "${tmpdir}/unsupported-version.loom" "unsupported version"
expect_failure "${tmpdir}/duplicate-payload-section.loom" "exactly one payload section"
expect_failure "${tmpdir}/truncated-section.loom" "section outside container"
expect_failure "${tmpdir}/offset-overflow.loom" "section offset overflow"

echo "[PASS] malformed LMC1 containers fail closed"
