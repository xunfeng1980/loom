# Phase 49 Summary: Independent L2Core Decode IR Codec and Content-Hash Identity

**Phase:** 49 — Repositioning (整理稿) 决定一: decode IR 与 container 分离
**Completed:** 2026-06-11
**Commit:** 933f4e1

## What Was Delivered

### Plan 49-01: Independent L2Core IR Codec

Created `l2core_codec.rs` — a standalone, deterministic binary wire format for `L2CoreProgram`:

- `L2IR` magic bytes header + `u16` version
- Little-endian fixed-width integers and floats
- Length-prefixed (UInt16LE) strings, vectors, and maps
- `u8` enum discriminants for all AST types: `Capability`, `ScalarValue`, `ScalarExpr`, `L2CoreStmt`
- Narrow `DataType` subset: Boolean, Int32, Int64, Float32, Float64, Utf8
- `ResourceBudget` and feature set encoding
- Round-trip stability: encode→decode→reencode produces byte-identical output
- Verified via source grep: zero import of `container_codec`, `table_codec`, `layout_codec`, `arrow_semantic_codec`

### Plan 49-02: Content-Hash Identity

Added `L2CoreProgram::content_hash()` that computes `l2ir:<hex>` identity:

- FNV-1a hashing over canonical codec bytes
- Deterministic: identical programs produce identical bytes → identical hash across processes/runs
- Collision-free: the diverse equivalence-class corpus (Phase 48) confirms differing programs produce differing hashes
- The IR now has a stable, packaging-independent identity — the foundation every later piece binds to

### Plan 49-03: Fail-Closed Parse-and-Verify

Added `verify_l2_core_bytes` in `full_verifier.rs`:

- Decodes from wire bytes, then verifies the AST
- Rejects bad magic (`ExplicitFailClosed`), unsupported version, truncated payload, bad discriminants
- Valid bytes yield identical `VerifiedArtifactFacts` to the in-memory AST path
- Acceptance/rejection parity: same result whether parsed from bytes or constructed in memory
- The verified object and distributed object are now byte-identical

## Test Coverage

- All 141 existing loom-core lib tests pass
- 15 new Phase 49 tests covering:
  - Roundtrip encode→decode (byte-identical)
  - Reencode stability (encode output from decoded input matches original)
  - Hash stability (same program → same hash)
  - Hash collision freedom (diverse programs → different hashes)
  - Bad magic rejection
  - Unsupported version rejection
  - Bad discriminant rejection
  - Truncated payload rejection
  - Verify-from-bytes acceptance/rejection parity with in-memory path

## Gate Evidence

- `bash scripts/full-verifier-test.sh` — passes
- `bash scripts/verified-lineage-test.sh` — passes
- `cargo test -p loom-core` — 141/141 pass

## Key Files Changed

| File | Change |
|------|--------|
| `crates/loom-core/src/l2core_codec.rs` | New — independent binary wire format |
| `crates/loom-core/src/l2_core.rs` | Added `content_hash()` method |
| `crates/loom-core/src/full_verifier.rs` | Added `verify_l2_core_bytes` entry point |
| `crates/loom-core/src/lib.rs` | Re-exported l2core_codec module |

## Non-Claims

- No sidecar overlay model (Phase 50)
- No artifact-level signing/attestation (deferred)
- No new L2Core constructs (existing 22-construct surface only)
- No container deletion (LMC1/LMC2/LMA1 demotion is Phase 50.1)
- No correctness claims — safety + well-formedness + stable identity only
