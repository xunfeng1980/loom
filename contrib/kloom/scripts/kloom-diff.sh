#!/usr/bin/env bash
# kloom-diff.sh — Differential validation gate: K spec-oracle vs native.
#
# This script is the Phase 40+ differential gate. It:
#   1. kompiles kloom.k (Haskell backend) if needed
#   2. Runs krun on the semantics test corpus, extracting K trace
#   3. Runs loom-core tests (which exercise K harness vs native trace)
#   4. Any divergence → fail-closed (exit non-zero)
#
# Environment:
#   KLOOM_DEF: path to kompiled kloom definition (default: contrib/kloom/.build)
#   KLOOM_SRC: path to kloom.k (default: contrib/kloom/src/kloom.k)
#
# Trust model: K is the specification baseline. Native execution is
# validated against K inside cargo test. This script is part of CI,
# not production.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

# Ensure K tools are on PATH (nix profile or system).
if [ -f /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]; then
    . /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
fi

KLOOM_DIR="${REPO_ROOT}/contrib/kloom"
KLOOM_SRC="${KLOOM_DIR}/src/kloom.k"
KLOOM_DEF="${KLOOM_DIR}/.build"
TEST_DIR="${KLOOM_DIR}/tests/semantics"
TMPDIR="${KLOOM_DIR}/.tmp"

if [ -t 1 ] && command -v tput >/dev/null 2>&1; then
    GRN="$(tput setaf 2)"
    RED="$(tput setaf 1)"
    RST="$(tput sgr0)"
else
    GRN=""
    RED=""
    RST=""
fi

ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }
info() { echo "[INFO] $*"; }

mkdir -p "${TMPDIR}"

# ---------------------------------------------------------------------------
# Step 0: Verify K tools are available
# ---------------------------------------------------------------------------
if ! command -v kompile >/dev/null 2>&1; then
    fail "kompile not found. Install K Framework (e.g. nix profile install nixpkgs#kframework)"
fi
if ! command -v krun >/dev/null 2>&1; then
    fail "krun not found. Install K Framework."
fi

# ---------------------------------------------------------------------------
# Step 1: kompile kloom.k if needed
# ---------------------------------------------------------------------------
if [ ! -d "${KLOOM_DEF}" ] || [ "${KLOOM_SRC}" -nt "${KLOOM_DEF}/timestamp" ]; then
    info "kompile ${KLOOM_SRC} (Haskell backend) ..."
    kompile "${KLOOM_SRC}" --backend haskell -o "${KLOOM_DEF}"
    touch "${KLOOM_DEF}/timestamp"
    ok "kompile"
else
    ok "kloom definition up to date"
fi

# ---------------------------------------------------------------------------
# Step 2: Run krun on each semantics test, extract K trace
# ---------------------------------------------------------------------------
info "Running kloom semantics tests ..."
K_OK=0
K_FAIL=0
for testfile in "${TEST_DIR}"/*.kloom; do
    [ -f "$testfile" ] || continue
    basename="$(basename "$testfile" .kloom)"
    ktrace="${TMPDIR}/${basename}.k.trace"

    if krun "$testfile" --definition "${KLOOM_DEF}" \
        --output pretty \
        > "${TMPDIR}/${basename}.krun.out" 2>/dev/null; then
        # Extract trace from <events> cell in pretty output.
        awk '
            /<events>/{in_events=1; next}
            /<\/events>/{in_events=0; next}
            in_events {
                gsub(/^ *ListItem \( /, "");
                gsub(/ \)$/, "");
                gsub(/ : /, ":");
                print;
            }
        ' "${TMPDIR}/${basename}.krun.out" > "$ktrace"

        # Verify we got at least one event or an explicit empty trace.
        if [ -s "$ktrace" ] || grep -q '<events>' "${TMPDIR}/${basename}.krun.out"; then
            ok "${basename} — krun (${K_OK} ok so far)"
            K_OK=$((K_OK + 1))
        else
            fail "${basename} — krun produced no events"
            K_FAIL=$((K_FAIL + 1))
        fi
    else
        fail "${basename} — krun failed"
        K_FAIL=$((K_FAIL + 1))
    fi
done

info "krun results: ${K_OK} passed, ${K_FAIL} failed"
[ "$K_FAIL" -eq 0 ] || fail "kloom semantics tests failed"

# ---------------------------------------------------------------------------
# Step 3: Run loom-core tests (K harness vs native trace)
# ---------------------------------------------------------------------------
info "Running loom-core differential tests (K harness vs native) ..."

# Ensure krun is on PATH for the test subprocesses.
if ! cargo test -p loom-core --test native_arrow_semantic 2>&1; then
    fail "loom-core differential tests failed"
fi
ok "loom-core differential tests passed"

# ---------------------------------------------------------------------------
# Step 4: Full loom-core test suite
# ---------------------------------------------------------------------------
info "Running full loom-core test suite ..."
if ! cargo test -p loom-core 2>&1; then
    fail "loom-core full test suite failed"
fi
ok "loom-core full test suite passed"

echo ""
echo "${GRN}=== kloom differential gate completed ===${RST}"
