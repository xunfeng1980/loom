# Phase 49 Plan

**Phase:** 49 - Independent L2Core Decode IR Codec and Content-Hash Identity
**Status:** Complete (backfilled)
**Plans:** 3 plans across 2 waves

## Wave 1: Independent IR Codec + Content-Hash Identity (Plans 49-01, 49-02)

### Plan 49-01: Independent L2Core IR Codec

**Goal:** Create `l2core_codec.rs` — a standalone, deterministic binary wire format for
`L2CoreProgram` with zero dependency on any container codec.

**Deliverables:**
- [x] `l2core_codec.rs` with `L2IR` magic bytes + `u16` version header
- [x] Little-endian fixed-width primitives (integers, floats)
- [x] Length-prefixed strings and vectors (UInt16LE + payload)
- [x] `u8` enum discriminants for `Capability`, `ScalarValue`, `ScalarExpr`, `L2CoreStmt`
- [x] Narrow `DataType` subset encoding: Boolean, Int32, Int64, Float32, Float64, Utf8
- [x] `ResourceBudget` encoding
- [x] Feature set encoding
- [x] Round-trip encode→decode→reencode produces byte-identical output (deterministic stability)
- [x] Verified zero import of `container_codec`, `table_codec`, `layout_codec`, `arrow_semantic_codec`

**Key file:** `crates/loom-core/src/l2core_codec.rs`

### Plan 49-02: Content-Hash Identity

**Goal:** Compute a stable, collision-free content-hash over the canonical codec bytes so the
L2Core IR has an independent identity decoupled from any container.

**Deliverables:**
- [x] `L2CoreProgram::content_hash()` → `l2ir:<hex>` via FNV-1a over canonical codec bytes
- [x] Deterministic encode→decode→reencode proven byte-identical (enables stable hash)
- [x] Division of the diverse-program equivalence-class corpus across runs proves different
      programs produce different hashes (no collisions)
- [x] Same program → same bytes → same hash across processes/runs

**Key files:** `crates/loom-core/src/l2_core.rs`, `crates/loom-core/src/l2core_codec.rs`

## Wave 2: Fail-Closed Verification from Wire Bytes (Plan 49-03)

### Plan 49-03: Fail-Closed Parse-and-Verify

**Goal:** The verifier consumes IR parsed from its own codec bytes and rejects malformed/garbled/
truncated input before any facts are produced, so the verified object and the distributed object
are byte-identical.

**Deliverables:**
- [x] `verify_l2_core_bytes` entry point in `full_verifier.rs`
- [x] Rejects bad magic bytes with `ExplicitFailClosed` diagnostic
- [x] Rejects unsupported version with typed error
- [x] Rejects truncated/payload-torun off input (buffer underrun detection)
- [x] Rejects bad enum discriminants with typed error
- [x] Valid bytes from codec yield identical `VerifiedArtifactFacts` to in-memory AST path
- [x] Acceptance/rejection parity: same program → same accept/reject whether parsed from bytes
      or constructed in memory
- [x] 15 new tests: roundtrip, reencode stability, hash stability, hash collision freedom,
      bad magic/version/discriminant/truncated rejection, verify-from-bytes parity

**Key files:** `crates/loom-core/src/full_verifier.rs`, `crates/loom-core/src/l2core_codec.rs`

## Release Gate

- [x] All 141 loom-core lib tests pass (including 15 new Phase 49 tests)
- [x] `scripts/full-verifier-test.sh` passes
- [x] `scripts/verified-lineage-test.sh` passes
