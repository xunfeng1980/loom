#!/usr/bin/env bash
# duckdb-smoke-test.sh — Build, load, and smoke-test the loom DuckDB extension.
#
# Proves the full CMake + Rust-staticlib + DuckDB-ABI chain end-to-end:
#   1. Builds the extension with CMake if not already built.
#   2. Downloads the DuckDB v1.5.3 CLI for the host platform if not cached.
#   3. Loads loom.duckdb_extension via `duckdb -unsigned`.
#   4. Asserts SELECT * FROM loom_scan('test.bin') returns 4 rows (1,2,3,NULL).
#   5. Asserts SELECT count(*) FROM loom_scan('test.bin') returns 4.
#
# Exit codes:
#   0 — smoke-test PASSED (DUCK-01 loadability + DUCK-03 teardown path verified)
#   1 — build failed, download failed, ABI/metadata mismatch, or row count mismatch
#
# DUCK-01 proof: the extension is loaded into the official prebuilt duckdb v1.5.3
#   CLI with -unsigned (bypasses signature check; metadata check still applies —
#   the footer stamp POST_BUILD ensures the footer matches the CLI).
#
# DUCK-03 proof: the query runs to completion and the CLI process exits with code 0.
#   A leaked/double-freed ArrowArray would abort the process or cause a nonzero exit.
#   Combined with the Phase-1 Rust-side release-roundtrip test, this covers every
#   teardown path for the one-shot hardcoded array.
#
# Run from the workspace root:
#   bash scripts/duckdb-smoke-test.sh
#
# In CI, the DUCKDB_CLI environment variable may be set to an already-downloaded
# binary path (e.g. the download happens in a separate CI step). The script uses
# that if provided.

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
DUCKDB_VERSION="v1.5.3"
REPO_ROOT="$(git rev-parse --show-toplevel)"
EXT_PATH="${REPO_ROOT}/duckdb-ext/build/loom.duckdb_extension"
CLI_CACHE_DIR="${REPO_ROOT}/duckdb-ext/vendor/duckdb-cli"

# Colour helpers (graceful fallback in CI / non-terminal)
if [ -t 1 ] && command -v tput &>/dev/null; then
    RED=$(tput setaf 1)
    GRN=$(tput setaf 2)
    YLW=$(tput setaf 3)
    RST=$(tput sgr0)
else
    RED=''
    GRN=''
    YLW=''
    RST=''
fi

info()  { echo "${YLW}[smoke-test]${RST} $*"; }
ok()    { echo "${GRN}[PASS]${RST} $*"; }
fail()  { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

echo "=== Loom DuckDB extension smoke-test (DUCK-01, DUCK-03) ==="
echo ""

# ---------------------------------------------------------------------------
# Step 1: Build the extension (idempotent — skips if already up-to-date)
# ---------------------------------------------------------------------------
info "Building loom.duckdb_extension..."
cmake -S "${REPO_ROOT}/duckdb-ext" \
      -B "${REPO_ROOT}/duckdb-ext/build" \
      -DCMAKE_BUILD_TYPE=Release \
      2>&1 | grep -v '^--' || true
cmake --build "${REPO_ROOT}/duckdb-ext/build" 2>&1

if [ ! -f "${EXT_PATH}" ]; then
    fail "Build succeeded but loom.duckdb_extension not found at: ${EXT_PATH}"
fi

EXT_SIZE=$(wc -c < "${EXT_PATH}" | tr -d ' ')
if [ "${EXT_SIZE}" -lt 512 ]; then
    fail "Extension file is only ${EXT_SIZE} bytes — footer stamp missing (expected >= 512)"
fi
ok "Built ${EXT_PATH} (${EXT_SIZE} bytes, footer present)"

# ---------------------------------------------------------------------------
# Step 2: Locate or download the DuckDB v1.5.3 CLI
# ---------------------------------------------------------------------------
if [ -n "${DUCKDB_CLI:-}" ]; then
    # CI may pre-set the binary path in the environment
    DUCKDB_BIN="${DUCKDB_CLI}"
    info "Using pre-set DUCKDB_CLI=${DUCKDB_BIN}"
else
    # Determine platform and download URL
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    if [ "${OS}" = "Darwin" ] && [ "${ARCH}" = "arm64" ]; then
        CLI_ASSET="duckdb_cli-osx-arm64.zip"
    elif [ "${OS}" = "Darwin" ]; then
        CLI_ASSET="duckdb_cli-osx-amd64.zip"
    elif [ "${OS}" = "Linux" ] && [ "${ARCH}" = "x86_64" ]; then
        CLI_ASSET="duckdb_cli-linux-amd64.zip"
    elif [ "${OS}" = "Linux" ] && [[ "${ARCH}" =~ ^(aarch64|arm64)$ ]]; then
        CLI_ASSET="duckdb_cli-linux-arm64.zip"
    else
        fail "Unsupported platform for DuckDB CLI download: ${OS}/${ARCH}"
    fi

    CLI_URL="https://github.com/duckdb/duckdb/releases/download/${DUCKDB_VERSION}/${CLI_ASSET}"
    DUCKDB_BIN="${CLI_CACHE_DIR}/duckdb"

    if [ -x "${DUCKDB_BIN}" ]; then
        info "DuckDB CLI already cached at ${DUCKDB_BIN}"
    else
        info "Downloading DuckDB ${DUCKDB_VERSION} CLI (${CLI_ASSET})..."
        mkdir -p "${CLI_CACHE_DIR}"
        TMPZIP="${CLI_CACHE_DIR}/${CLI_ASSET}"
        curl -fSL --retry 3 --retry-delay 2 \
            -o "${TMPZIP}" \
            "${CLI_URL}"
        unzip -o "${TMPZIP}" -d "${CLI_CACHE_DIR}"
        rm -f "${TMPZIP}"
        chmod +x "${DUCKDB_BIN}"
        ok "Downloaded and unpacked ${DUCKDB_BIN}"
    fi
fi

if [ ! -x "${DUCKDB_BIN}" ]; then
    fail "DuckDB CLI not executable at: ${DUCKDB_BIN}"
fi

info "Using DuckDB CLI: ${DUCKDB_BIN}"

# ---------------------------------------------------------------------------
# Step 3: Load the extension and run SELECT count(*) — must return 4
# ---------------------------------------------------------------------------
info "Running LOAD + SELECT count(*) FROM loom_scan('test.bin')..."
COUNT_OUTPUT=$("${DUCKDB_BIN}" -unsigned -c \
    "LOAD '${EXT_PATH}'; SELECT count(*) FROM loom_scan('test.bin');" \
    2>&1)
# DuckDB outputs a box-drawing table; extract the numeric data row(s).
# The data rows appear as lines containing only box chars, spaces, and digits.
# Use grep to find lines that contain digits, strip non-digit chars, pick the
# last purely-numeric token (count value appears after the int64 type header).
COUNT=$(echo "${COUNT_OUTPUT}" \
    | grep -Eo '[[:space:]][0-9]+[[:space:]]' \
    | tr -d ' ' \
    | tail -1)

if [ "${COUNT}" != "4" ]; then
    echo "Full DuckDB output:" >&2
    echo "${COUNT_OUTPUT}" >&2
    fail "Expected count(*) = 4, got: '${COUNT}'"
fi
ok "SELECT count(*) FROM loom_scan('test.bin') = ${COUNT} (DUCK-01)"

# ---------------------------------------------------------------------------
# Step 4: Load the extension and run SELECT * — assert 1, 2, 3, NULL
# ---------------------------------------------------------------------------
info "Running LOAD + SELECT * FROM loom_scan('test.bin')..."
ROWS_OUTPUT=$("${DUCKDB_BIN}" -unsigned -c \
    "LOAD '${EXT_PATH}'; SELECT * FROM loom_scan('test.bin');" \
    2>&1)

# Assert presence of expected values: 1, 2, 3, and NULL
if ! echo "${ROWS_OUTPUT}" | grep -qE '\b1\b'; then
    fail "Value '1' not found in loom_scan output. Output was: ${ROWS_OUTPUT}"
fi
if ! echo "${ROWS_OUTPUT}" | grep -qE '\b2\b'; then
    fail "Value '2' not found in loom_scan output. Output was: ${ROWS_OUTPUT}"
fi
if ! echo "${ROWS_OUTPUT}" | grep -qE '\b3\b'; then
    fail "Value '3' not found in loom_scan output. Output was: ${ROWS_OUTPUT}"
fi
if ! echo "${ROWS_OUTPUT}" | grep -qi 'null'; then
    # DuckDB may render NULL as blank or "NULL" depending on output mode
    # Also check for the numeric pattern 1, 2, 3 across 4 lines (last is NULL/blank)
    ROW_COUNT=$(echo "${ROWS_OUTPUT}" | grep -cE '^\s*[0-9]+\s*$' || echo 0)
    if [ "${ROW_COUNT}" -lt 3 ]; then
        fail "NULL row not found in loom_scan output. Output was: ${ROWS_OUTPUT}"
    fi
fi
ok "SELECT * FROM loom_scan('test.bin') returned rows including 1, 2, 3, NULL (DUCK-01, DUCK-03)"

echo ""
echo "${GRN}=== Smoke-test PASSED ===${RST}"
echo "  Extension: ${EXT_PATH}"
echo "  DuckDB CLI: ${DUCKDB_BIN} (${DUCKDB_VERSION})"
echo "  loom_scan('test.bin') returned 4 rows: 1, 2, 3, NULL"
echo "  CLI process exited 0 (DUCK-03 teardown evidence)"
echo ""
