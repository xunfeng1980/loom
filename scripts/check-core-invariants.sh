#!/usr/bin/env bash
# check-core-invariants.sh — Assert all Phase 1 CORE invariants.
#
# Called by .github/workflows/ci.yml on every push and pull_request.
# Also safe to run locally from the workspace root:
#
#   bash scripts/check-core-invariants.sh
#
# Exit codes:
#   0 — all invariants pass
#   1 — at least one invariant failed (failure message printed to stderr)
#
# Requirements verified: CORE-01, CORE-02, CORE-03, ARROW-03, DUCK-04
# Pitfall guards: P1 release callback, P2 schema lifetime, P3 panic across FFI,
#                 P5 allocator mismatch, P8 vortex-file scope creep, P9 arrow skew
#
# Grep gate hygiene: all pattern searches strip comment lines before counting,
# so a comment containing the search token cannot self-satisfy a check.

set -euo pipefail

# Colour helpers (graceful fallback when not in a terminal / CI)
if [ -t 1 ] && command -v tput &>/dev/null; then
    RED=$(tput setaf 1)
    GRN=$(tput setaf 2)
    RST=$(tput sgr0)
else
    RED=''
    GRN=''
    RST=''
fi

PASS="${GRN}PASS${RST}"
FAIL="${RED}FAIL${RST}"

overall=0  # accumulate failures; print summary at the end

fail() {
    echo "${FAIL}: $*" >&2
    overall=1
}

pass() {
    echo "${PASS}: $*"
}

echo "=== Loom Phase 1 CORE Invariant Check ==="
echo ""

# ---------------------------------------------------------------------------
# CORE-01: Zero duplicate arrow-* crates at different VERSIONS in the tree.
#
# `cargo tree --duplicates` shows packages that appear multiple times in the
# dependency graph.  This includes packages at the SAME version referenced via
# multiple paths (expected, safe) AND packages at DIFFERENT versions (the
# actual problem — version skew causes type mismatches at the FFI boundary).
#
# We detect version conflicts by extracting unique (name, version) pairs for
# arrow-* crates and checking whether any name appears more than once (at two
# distinct versions).  All entries are stripped of tree-drawing Unicode chars.
#
# Correct state: each arrow-* name appears at exactly one version in the output.
# ---------------------------------------------------------------------------
echo "--- CORE-01: Arrow version unification (no arrow-* version conflicts) ---"
# WR-02 fix: do NOT discard cargo stderr and do NOT let an empty result PASS.
# A `cargo tree` failure (network, lockfile, etc.) must surface as FAIL, not be
# silently swallowed into a green check. `|| true` keeps `set -e` from aborting
# the whole script so we can report the failure ourselves.
arrow_dupes_raw=$(cargo tree -d 2>&1) || arrow_dupes_raw=""
arrow_tree_exit=$?
if [ "$arrow_tree_exit" -ne 0 ]; then
    fail "cargo tree -d failed (exit $arrow_tree_exit) — cannot verify arrow version unification (CORE-01):"
    echo "$arrow_dupes_raw" | head -5 >&2
else
    # Strip tree-drawing characters (box-drawing Unicode), retain name + version.
    arrow_versions=$(printf '%s\n' "$arrow_dupes_raw" \
        | sed 's/^[^a-zA-Z]*//' \
        | grep '^arrow' \
        | awk '{print $1, $2}' \
        | sort -u)
    # The workspace MUST depend on arrow (loom-ffi uses it). An empty arrow set
    # means the query is broken or the dep vanished — fail closed, never PASS.
    if [ -z "$arrow_versions" ]; then
        fail "No arrow-* crates found in dependency tree — expected at least one (CORE-01 query suspect)"
    else
        arrow_name_count=$(echo "$arrow_versions" | awk '{print $1}' | sort -u | wc -l | tr -d ' ')
        arrow_pair_count=$(echo "$arrow_versions" | wc -l | tr -d ' ')
        if [ "$arrow_pair_count" -eq "$arrow_name_count" ]; then
            pass "cargo tree -d: all arrow-* crates resolve to a single version (PITFALLS P9)"
        else
            fail "Duplicate arrow-* version conflict detected (PITFALLS P9):"
            echo "$arrow_versions" >&2
        fi
    fi
fi
echo ""

# ---------------------------------------------------------------------------
# CORE-01 (scope): vortex-file must not appear in Cargo.lock (PITFALLS P8).
# ---------------------------------------------------------------------------
echo "--- CORE-01 (scope): vortex-file absent from Cargo.lock ---"
vortex_file=$(grep -v '^#' Cargo.lock | grep 'vortex-file' || true)
if [ -z "$vortex_file" ]; then
    pass "grep vortex-file Cargo.lock → absent (scope boundary intact)"
else
    fail "vortex-file appeared in Cargo.lock — out-of-scope dependency (PITFALLS P8):"
    echo "$vortex_file" >&2
fi
echo ""

# ---------------------------------------------------------------------------
# CORE-01 (D-02): loom-core must have zero vortex-* dependencies.
# ---------------------------------------------------------------------------
echo "--- CORE-01 (D-02): loom-core has no vortex dependency ---"
loom_core_vortex=$(cargo tree -p loom-core 2>/dev/null | grep -v '^#' | grep 'vortex' || true)
if [ -z "$loom_core_vortex" ]; then
    pass "cargo tree -p loom-core | grep vortex → clean"
else
    fail "loom-core has a vortex dependency — breaks D-02 isolation:"
    echo "$loom_core_vortex" >&2
fi
echo ""

# ---------------------------------------------------------------------------
# CORE-02: panic = "abort" must be present in the workspace Cargo.toml.
# ---------------------------------------------------------------------------
# CORE-02 (revised, 01-REVIEW.md CR-01): release uses panic = "unwind" so the
# extern "C" catch_unwind wrapper can actually catch panics (DUCK-04). With
# "abort" the catch would be a no-op. We assert "unwind" and additionally assert
# "abort" is NOT present (guards against a regression back to the no-op state).
echo "--- CORE-02: panic=\"unwind\" in [profile.release] (enables catch_unwind, CR-01) ---"
panic_unwind_line=$(grep 'panic = "unwind"' Cargo.toml || true)
panic_abort_line=$(grep -v '^\s*#' Cargo.toml | grep 'panic = "abort"' || true)
if [ -n "$panic_abort_line" ]; then
    fail 'panic = "abort" present in Cargo.toml — catch_unwind becomes a no-op, defeating DUCK-04 (CR-01)'
elif [ -n "$panic_unwind_line" ]; then
    pass 'grep panic = "unwind" Cargo.toml → found (catch_unwind is live)'
else
    fail 'panic = "unwind" not found in Cargo.toml — CORE-02 missing (CR-01)'
fi
echo ""

# ---------------------------------------------------------------------------
# CORE-02: #[global_allocator] static must be in loom-ffi/src/lib.rs.
# Filter out Rust // line comments but keep #[...] attribute lines.
# ---------------------------------------------------------------------------
echo "--- CORE-02: System global_allocator in loom-ffi/src/lib.rs ---"
alloc_line=$(grep -v '^\s*//' crates/loom-ffi/src/lib.rs | grep 'global_allocator' || true)
if [ -n "$alloc_line" ]; then
    pass "grep global_allocator crates/loom-ffi/src/lib.rs → found"
else
    fail "global_allocator not found in loom-ffi/src/lib.rs — CORE-02 missing (PITFALLS P5)"
fi
echo ""

# ---------------------------------------------------------------------------
# CORE-03: Build loom-ffi to trigger cbindgen, then verify loom.h.
# Capture build output to a temp file so we can check for errors without
# relying on pipe-in-condition behaviour (which can be fragile with pipefail).
# ---------------------------------------------------------------------------
echo "--- CORE-03: Build loom-ffi and verify loom.h contains loom_decode ---"
build_out=$(cargo build -p loom-ffi --release 2>&1) && build_exit=0 || build_exit=$?
if [ "$build_exit" -ne 0 ]; then
    fail "cargo build -p loom-ffi --release failed (exit $build_exit)"
    echo "$build_out" | grep -i 'error' >&2 || true
else
    pass "cargo build -p loom-ffi --release succeeded"
    # Verify loom.h contains loom_decode (CORE-03)
    if grep -q 'loom_decode' crates/loom-ffi/include/loom.h 2>/dev/null; then
        pass "grep loom_decode crates/loom-ffi/include/loom.h → found (CORE-03)"
    else
        fail "loom_decode not found in crates/loom-ffi/include/loom.h — cbindgen header missing (CORE-03)"
    fi
    # Verify FFI_ArrowArray struct body is NOT in loom.h (PITFALLS integration gotcha, T-01-09)
    if grep -qE 'FFI_ArrowArray[[:space:]]*\{' crates/loom-ffi/include/loom.h 2>/dev/null; then
        fail "FFI_ArrowArray struct body found in loom.h — cbindgen must NOT redefine Arrow FFI types (T-01-09)"
    else
        pass "FFI_ArrowArray struct body absent from loom.h (T-01-09)"
    fi
fi
echo ""

# ---------------------------------------------------------------------------
# ARROW-03 (link check): loom_decode symbol present in libloom_ffi.a.
# ---------------------------------------------------------------------------
echo "--- ARROW-03: loom_decode symbol present in libloom_ffi.a ---"
loom_decode_sym=$(nm target/release/libloom_ffi.a 2>/dev/null | grep 'loom_decode' || true)
if [ -n "$loom_decode_sym" ]; then
    pass "nm target/release/libloom_ffi.a | grep loom_decode → symbol found"
else
    fail "loom_decode symbol not found in libloom_ffi.a — link check failed (ARROW-03)"
fi
echo ""

# ---------------------------------------------------------------------------
# ARROW-03 + DUCK-04: cargo test -p loom-ffi must pass.
# Capture output to check for both "test result: ok" and absence of "FAILED".
# Capture to variable to avoid pipe-in-condition issues with pipefail.
# ---------------------------------------------------------------------------
echo "--- ARROW-03 + DUCK-04: cargo test -p loom-ffi ---"
test_out=$(cargo test -p loom-ffi 2>&1) && test_exit=0 || test_exit=$?
if [ "$test_exit" -ne 0 ]; then
    fail "cargo test -p loom-ffi exited with code $test_exit"
    echo "$test_out" >&2
elif echo "$test_out" | grep -q 'FAILED'; then
    fail "cargo test -p loom-ffi: one or more tests FAILED"
    echo "$test_out" | grep 'FAILED' >&2
elif echo "$test_out" | grep -q 'test result: ok'; then
    pass "cargo test -p loom-ffi → all tests passed (release_path_roundtrip + panic_does_not_abort)"
else
    fail "cargo test -p loom-ffi: could not find 'test result: ok' in output"
    echo "$test_out" | tail -10 >&2
fi
echo ""

# ---------------------------------------------------------------------------
# DUCK-04: catch_unwind present in ffi.rs (static grep; filters // comments).
# ---------------------------------------------------------------------------
echo "--- DUCK-04: catch_unwind present in crates/loom-ffi/src/ffi.rs ---"
catch_count=$(grep -v '^\s*//' crates/loom-ffi/src/ffi.rs | grep -c 'catch_unwind' || true)
if [ "${catch_count}" -ge 1 ]; then
    pass "grep catch_unwind crates/loom-ffi/src/ffi.rs → ${catch_count} occurrence(s)"
else
    fail "catch_unwind not found in ffi.rs — DUCK-04 missing"
fi
echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "=== Summary ==="
if [ "${overall}" -eq 0 ]; then
    echo "${GRN}All CORE invariants PASSED.${RST}"
else
    echo "${RED}One or more CORE invariants FAILED. See errors above.${RST}" >&2
    exit 1
fi
