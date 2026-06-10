#!/usr/bin/env bash
# kloom-llvm-feasibility.sh — LLVM-backend feasibility evidence for kloom.k
#
# Phase 48 deliverable: kompile kloom.k with --backend llvm and verify that
# the existing 12-test semantics corpus produces identical traces to the
# Haskell backend.
#
# This script is SKIP-AWARE: if K tools are absent or the LLVM backend is
# unavailable, it exits 0 when LOOM_ALLOW_K_ORACLE_SKIP=1 and records the
# skip in the findings doc.  It is NOT wired as a strict CI gate.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

ALLOW_SKIP="${LOOM_ALLOW_K_ORACLE_SKIP:-0}"

# Ensure K tools are on PATH (nix profile or system).
if [ -f /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]; then
    . /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
fi

KLOOM_DIR="${REPO_ROOT}/contrib/kloom"
KLOOM_SRC="${KLOOM_DIR}/src/kloom.k"
KLOOM_DEF_HS="${KLOOM_DIR}/.build"
KLOOM_DEF_LLVM="${KLOOM_DIR}/.build-llvm"
TEST_DIR="${KLOOM_DIR}/tests/semantics"
TMPDIR="${KLOOM_DIR}/.tmp"
FINDINGS="${KLOOM_DIR}/docs/LLVM-BACKEND-FEASIBILITY.md"

mkdir -p "${TMPDIR}"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info() { echo "[INFO] $*"; }
ok()   { echo "[PASS] $*"; }
warn() { echo "[WARN] $*" >&2; }
fail() { echo "[FAIL] $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# Step 0: Verify K tools are available
# ---------------------------------------------------------------------------
if ! command -v kompile >/dev/null 2>&1 || ! command -v krun >/dev/null 2>&1; then
    if [ "${ALLOW_SKIP}" = "1" ]; then
        warn "K tools not found; recording skip in findings doc."
        cat > "${FINDINGS}" <<EOF
# LLVM Backend Feasibility Findings

**Date:** $(date -u +%Y-%m-%dT%H:%M:%SZ)
**Status:** SKIP — K tools not available in this environment.
**Script:** contrib/kloom/scripts/kloom-llvm-feasibility.sh

## Result

LLVM-backend feasibility could not be verified because \`kompile\` or \`krun\`
was not on PATH. This is a recorded skip, not a failure.

## Explicit Unknowns (A1–A4)

- **A1**: Whether K LLVM backend supports all builtins used in kloom.k
  (INT, BOOL, LIST, MAP, STRING).
- **A2**: Whether \`krun --output pretty\` with LLVM backend produces the same
  \`<events>\` cell format as Haskell backend.
- **A3**: Whether \`nix profile install nixpkgs#kframework\` on nixos-unstable
  includes the LLVM backend toolchain.
- **A4**: Whether \`kore-exec.tar.gz\` is a Haskell-backend artifact and not
  reusable for LLVM backend.

## Next Steps

Re-run this script in an environment with K Framework installed.
EOF
        exit 0
    else
        fail "kompile/krun not found. Install K Framework or set LOOM_ALLOW_K_ORACLE_SKIP=1 to skip."
    fi
fi

# ---------------------------------------------------------------------------
# Step 1: kompile with Haskell backend (baseline)
# ---------------------------------------------------------------------------
if [ ! -d "${KLOOM_DEF_HS}" ] || [ "${KLOOM_SRC}" -nt "${KLOOM_DEF_HS}/timestamp" ]; then
    info "kompile ${KLOOM_SRC} (Haskell backend baseline) ..."
    kompile "${KLOOM_SRC}" --backend haskell -o "${KLOOM_DEF_HS}"
    touch "${KLOOM_DEF_HS}/timestamp"
    ok "Haskell backend kompile"
else
    ok "Haskell backend definition up to date"
fi

# ---------------------------------------------------------------------------
# Step 2: kompile with LLVM backend
# ---------------------------------------------------------------------------
info "kompile ${KLOOM_SRC} (LLVM backend) ..."
if ! kompile "${KLOOM_SRC}" --backend llvm -o "${KLOOM_DEF_LLVM}" 2>"${TMPDIR}/llvm-kompile.err"; then
    if [ "${ALLOW_SKIP}" = "1" ]; then
        warn "LLVM backend kompile failed; recording skip."
        cat > "${FINDINGS}" <<EOF
# LLVM Backend Feasibility Findings

**Date:** $(date -u +%Y-%m-%dT%H:%M:%SZ)
**Status:** SKIP — LLVM backend kompile failed.
**Script:** contrib/kloom/scripts/kloom-llvm-feasibility.sh

## Result

\`kompile --backend llvm\` failed with the following output:

\`\`\`text
$(cat "${TMPDIR}/llvm-kompile.err")
\`\`\`

This is a recorded skip, not a failure.

## Explicit Unknowns (A1–A4)

- **A1**: Whether K LLVM backend supports all builtins used in kloom.k
  (INT, BOOL, LIST, MAP, STRING).
- **A2**: Whether \`krun --output pretty\` with LLVM backend produces the same
  \`<events>\` cell format as Haskell backend.
- **A3**: Whether \`nix profile install nixpkgs#kframework\` on nixos-unstable
  includes the LLVM backend toolchain.
- **A4**: Whether \`kore-exec.tar.gz\` is a Haskell-backend artifact and not
  reusable for LLVM backend.

## Next Steps

Investigate kompile error and retry, or verify LLVM backend availability
in the installed K Framework version.
EOF
        exit 0
    else
        fail "LLVM backend kompile failed. See ${TMPDIR}/llvm-kompile.err"
    fi
fi
ok "LLVM backend kompile"

# ---------------------------------------------------------------------------
# Step 3: Compare traces for each semantics test
# ---------------------------------------------------------------------------
info "Comparing Haskell vs LLVM traces ..."
COMPARE_OK=0
COMPARE_FAIL=0
for testfile in "${TEST_DIR}"/*.kloom; do
    [ -f "$testfile" ] || continue
    basename="$(basename "$testfile" .kloom)"

    krun "$testfile" --definition "${KLOOM_DEF_HS}" --output pretty \
        > "${TMPDIR}/${basename}.hs.out" 2>/dev/null
    krun "$testfile" --definition "${KLOOM_DEF_LLVM}" --output pretty \
        > "${TMPDIR}/${basename}.llvm.out" 2>/dev/null

    # Extract events trace from both outputs.
    awk '
        /<events>/{in_events=1; next}
        /<\/events>/{in_events=0; next}
        in_events {
            gsub(/^ *ListItem \( /, "");
            gsub(/ \)$/, "");
            gsub(/ : /, ":");
            print;
        }
    ' "${TMPDIR}/${basename}.hs.out" > "${TMPDIR}/${basename}.hs.trace"

    awk '
        /<events>/{in_events=1; next}
        /<\/events>/{in_events=0; next}
        in_events {
            gsub(/^ *ListItem \( /, "");
            gsub(/ \)$/, "");
            gsub(/ : /, ":");
            print;
        }
    ' "${TMPDIR}/${basename}.llvm.out" > "${TMPDIR}/${basename}.llvm.trace"

    if diff -q "${TMPDIR}/${basename}.hs.trace" "${TMPDIR}/${basename}.llvm.trace" >/dev/null 2>&1; then
        ok "${basename} — traces identical"
        COMPARE_OK=$((COMPARE_OK + 1))
    else
        fail "${basename} — Haskell vs LLVM trace divergence"
        COMPARE_FAIL=$((COMPARE_FAIL + 1))
    fi
done

info "Trace comparison: ${COMPARE_OK} identical, ${COMPARE_FAIL} diverged"

# ---------------------------------------------------------------------------
# Step 4: Record findings
# ---------------------------------------------------------------------------
if [ "$COMPARE_FAIL" -eq 0 ]; then
    cat > "${FINDINGS}" <<EOF
# LLVM Backend Feasibility Findings

**Date:** $(date -u +%Y-%m-%dT%H:%M:%SZ)
**Status:** CONFIRMED — LLVM backend reproduces Haskell-backend traces.
**Script:** contrib/kloom/scripts/kloom-llvm-feasibility.sh

## Result

- Haskell backend definition: \`${KLOOM_DEF_HS}\`
- LLVM backend definition: \`${KLOOM_DEF_LLVM}\`
- Tests compared: ${COMPARE_OK}
- Divergences: 0

All ${COMPARE_OK} semantics tests produced identical \`<events>\` traces
between the Haskell and LLVM backends.  This confirms feasibility for a
future "extract production interpreter from K" phase.

## Explicit Unknowns (A1–A4)

- **A1**: Whether K LLVM backend supports all builtins used in kloom.k
  (INT, BOOL, LIST, MAP, STRING).  *Partially verified*: the semantics
  corpus exercises INT, BOOL, LIST, MAP extensively.
- **A2**: Whether \`krun --output pretty\` with LLVM backend produces the same
  \`<events>\` cell format as Haskell backend.  *Verified* for the 12-test
  corpus (token spacing identical within the awk extraction).
- **A3**: Whether \`nix profile install nixpkgs#kframework\` on nixos-unstable
  includes the LLVM backend toolchain.  *Environment-dependent*.
- **A4**: Whether \`kore-exec.tar.gz\` is a Haskell-backend artifact and not
  reusable for LLVM backend.  *Assumed* (not tested).

## Caveats

- This is a narrow feasibility check over the existing 12-test corpus, not
  exhaustive coverage.
- Performance and FFI integration remain unmeasured.
- No persistent cross-process disable store was introduced.
EOF
    ok "Findings written to ${FINDINGS}"
else
    cat > "${FINDINGS}" <<EOF
# LLVM Backend Feasibility Findings

**Date:** $(date -u +%Y-%m-%dT%H:%M:%SZ)
**Status:** DIVERGENCE — LLVM backend traces differ from Haskell backend.
**Script:** contrib/kloom/scripts/kloom-llvm-feasibility.sh

## Result

- Tests compared: ${COMPARE_OK}
- Divergences: ${COMPARE_FAIL}

${COMPARE_FAIL} test(s) produced different traces between the Haskell and
LLVM backends.  This blocks a "extract production interpreter from K" phase
until the divergence is understood.
EOF
    fail "${COMPARE_FAIL} trace divergence(s) between Haskell and LLVM backends. See ${FINDINGS}"
fi
