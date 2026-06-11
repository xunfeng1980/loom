# Phase 50: Sidecar Overlay Model and Host-Native Reader Fallback — Research

**Researched:** 2026-06-11
**Domain:** Decode-IR sidecar overlay architecture — embedding, content-hash binding, routing  
**Confidence:** HIGH (architecture), MEDIUM (implementation surface for Vortex/Lance)

## Summary

Phase 50 is the second slice of Repositioning Decision Two: Loom becomes a **sidecar overlay** on unmodified host files (Parquet/Vortex/Lance) rather than a top-level format. A Loom-aware engine takes the verifiable-native track; everything else falls back to the host's own native reader. The sidecar is **strippable** — a Loom-unaware engine reads the file unchanged. Content-hash binding anchors the Phase 49 L2Core IR identity to host data at column-chunk/fragment granularity; an independent rewrite of the host invalidates only the affected granule's sidecar.

This phase does **not** build a new engine or execution track. It defines:
1. **A sidecar contract** — how L2Core IR bytes and hash bindings are embedded in each host format without breaking host compatibility.
2. **The content-hash binding model** — which data ranges are hashed, at what granularity, and how the binding is verified on read.
3. **The routing decision logic** — the fail-closed gate that checks (integrated engine ∧ hash matches ∧ encoding supported) → verifiable-native, otherwise → host-native reader.

Phase 50.1 has already demoted LMC2/LMA1 from top-level to out-of-TCB dev-time packaging, removed Arrow emission from ingress crates, and added `extract_sidecar_bytes_from_*` + `bind_content_hash_to_*` placeholder stubs in all three thin adapters. Phase 49 has already delivered the independent L2Core IR codec with `l2ir:<hex>` content-hash identity. This phase connects them.

**Primary recommendation:** Implement the sidecar as a modular `loom_core::sidecar` module that defines the overlay contract in host-neutral terms (SidecarOverlay, ChunkBinding, RoutingDecision), then implement Parquet first (using existing key_value_metadata in the Thrift footer) with Vortex and Lance following the same contract shape. The routing logic should be a new function in `loom-core` that composes with the existing `runtime_abi::decide_runtime_execution` pattern.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Sidecar overlay contract (data model) | API/Backend (loom-core) | — | Host-neutral types; consumed by all adapters |
| Sidecar embedding in Parquet | API/Backend (loom-parquet-ingress) | API/Backend (loom-core) | Parquet-specific KeyValue metadata in Thrift footer |
| Sidecar embedding in Vortex | API/Backend (loom-vortex-ingress) | API/Backend (loom-core) | Vortex file layout metadata section |
| Sidecar embedding in Lance | API/Backend (loom-lance-ingress) | API/Backend (loom-core) | Lance dataset metadata / manifest |
| Content-hash computation over host data | API/Backend (loom-core) | Host adapters | Reuse FNV-1a from l2core_codec.rs; adapters provide byte ranges |
| Chunk-granularity hash binding | API/Backend (loom-core) | Host adapters | Host-neutral model; adapters map host chunks to Loom bindings |
| Routing decision logic | API/Backend (loom-core) | API/Backend (loom-ffi) | Fail-closed gate in loom-core; DuckDB consumes the decision |
| Strippable overlay guarantee | All tiers | — | Embedding MUST be additive-only; no file mutation of host data |
| Fallback to host native reader | Browser/Client (host engine) | API/Backend (loom-ffi) | Loom-unaware engine simply ignores sidecar metadata |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust (rustc) | 1.92.0 | Primary language | MSRV from vortex-array 0.74.0; project toolchain |
| loom-core | workspace | Sidecar contract, hash binding, routing | Already owns L2Core IR, content-hash, verifier, runtime ABI |
| arrow-rs | 58.3.0 | Arrow schema/metadata | Canonical Arrow Rust implementation; pinned workspace-wide |
| parquet (arrow) | 58.3.0 | Parquet Thrift metadata read/write | Already used in loom-parquet-ingress |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| vortex-file | 0.74.0 | Vortex file layout inspection | Only in loom-vortex-ingress — for reading footer/layout metadata |
| lance | 7.0.0 | Lance dataset metadata | Only in loom-lance-ingress — for reading dataset manifest |
| sha2 | Not yet added | Optional: SHA-256 for host data hashing | Only if content-hash over raw host byte ranges is needed (see Open Questions) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| FNV-1a (existing, l2core_codec.rs) for host data ranges | SHA-256 or BLAKE3 for host data | FNV-1a is non-cryptographic — fine for collision detection within a file, but a cryptographic hash would prevent deliberate collision attacks on sidecar binding. Decision deferred; FNV-1a is sufficient for MVP2 sidecar. |
| Custom binary section appended to host file | Host-native metadata (Parquet KeyValue, Vortex layout) | Appending custom sections breaks "strippable overlay" premise — a Parquet reader that doesn't know about Loom must ignore unknown metadata, but appended sections at unknown file offsets will cause read errors. Host-native metadata embedding is the safe path. |
| New bespoke routing module | Extend existing `runtime_abi::decide_runtime_execution` | The existing runtime ABI already has `RuntimeFallbackPolicy`, `RuntimeExecutionDecision`, and a fail-closed gate structure. The sidecar routing logic should extend this pattern rather than creating a parallel decision system. |

**Installation:** No new dependencies required for Parquet sidecar. Vortex and Lance sidecars use existing ingress crate SDKs. `sha2` or `blake3` may be added if a cryptographic host-data hash is chosen.

**Version verification:**
- All workspace dependencies verified via `Cargo.toml` workspace pins
- `rustc 1.92.0`, `cargo 1.92.0` confirmed available
- No new external packages needed for the core sidecar contract

## Package Legitimacy Audit

> No new external packages are required for Phase 50. All new code lives in `loom-core` and existing ingress crates. If SHA-256 host-data hashing is adopted, `sha2` is a well-established crate (>100M downloads, maintained by RustCrypto project) — but this is a decision for the discuss phase.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (none new) | — | — | — | — | — | — |

**Packages removed due to [SLOP] verdict:** none  
**Packages flagged as suspicious [SUS]:** none  
*No new packages — all code is in existing workspace crates.*

## Architecture Patterns

### System Architecture Diagram (Sidecar Overlay Model)

```
┌──────────────────────────────────────────────────────────────────────┐
│                UNMODIFIED HOST FILE (Parquet / Vortex / Lance)        │
│                                                                      │
│  ┌────────────────────────────────────────────┐                      │
│  │ Host data (column chunks / fragments /      │                      │
│  │ row groups) — NEVER modified by Loom        │                      │
│  │                                              │                      │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐     │                      │
│  │  │ Col A   │  │ Col B   │  │ Col C   │     │                      │
│  │  │ data    │  │ data    │  │ data    │     │                      │
│  │  └─────────┘  └─────────┘  └─────────┘     │                      │
│  └────────────────────────────────────────────┘                      │
│                                                                      │
│  ┌────────────────────────────────────────────┐                      │
│  │ Host metadata (Parquet footer / Vortex      │                      │
│  │ layout / Lance manifest) — ADDITIVE ONLY    │                      │
│  │                                              │                      │
│  │  ┌─────────────────────────────────────────┐│                      │
│  │  │ KeyValue {                             ││                      │
│  │  │   key: "loom.sidecar.v1"                ││ ← L2Core IR bytes  │
│  │  │   value: "<L2IR magic + program>"       ││   (encoded)        │
│  │  │ }                                       ││                      │
│  │  │ KeyValue {                             ││                      │
│  │  │   key: "loom.hash.col_a"                ││ ← Column A binding │
│  │  │   value: "l2ir:0f1e2d3c4b5a6978"       ││                      │
│  │  │ }                                       ││                      │
│  │  │ KeyValue {                             ││                      │
│  │  │   key: "loom.hash.col_b"                ││ ← Column B binding │
│  │  │   value: "l2ir:a1b2c3d4e5f60718"       ││                      │
│  │  │ }                                       ││                      │
│  │  └─────────────────────────────────────────┘│                      │
│  └────────────────────────────────────────────┘                      │
└──────────────────────────────────────────────────────────────────────┘
                              │
                              │ Loom-aware engine reads file
                              ▼
┌──────────────────────────────────────────────────────────────────────┐
│                    ROUTING DECISION GATE (loom-core)                   │
│                                                                      │
│  1. Engine integrated? ──── NO ──→ use host native reader            │
│      │                                                               │
│     YES                                                              │
│      │                                                               │
│  2. Sidecar present? ──── NO ──→ use host native reader              │
│      │                                                               │
│     YES                                                              │
│      │                                                               │
│  3. Content-hash matches? ──── NO ──→ use host native reader         │
│      │ (hash mismatch = host data was rewritten independently)        │
│     YES                                                              │
│      │                                                               │
│  4. Encoding supported? ──── NO ──→ use host native reader           │
│      │                                                               │
│     YES                                                              │
│      │                                                               │
│      ▼                                                               │
│  LOOM VERIFIABLE-NATIVE TRACK                                        │
│  (verifier accepts → produce native Arrow output)                    │
└──────────────────────────────────────────────────────────────────────┘
```

**Key design invariant:** A Loom-unaware engine opens the same file — it reads `KeyValue` entries it doesn't recognize and simply ignores them. The host data (column chunks, pages, fragments) is unmodified. The sidecar is **strippable**: tools that rewrite Parquet metadata without understanding Loom will drop the `loom.*` keys, but the file remains readable as ordinary Parquet.

### Recommended Project Structure (Phase 50 additions in **bold**)

```
crates/
├── loom-core/src/
│   ├── l2core_codec.rs              # L2Core IR codec + hash (Phase 49) — STAYS
│   ├── l2_core.rs                   # L2CoreProgram + content_hash() — STAYS
│   ├── **sidecar.rs**               # NEW: SidecarOverlay, ChunkBinding, RoutingDecision
│   ├── **sidecar_routing.rs**       # NEW: Fail-closed routing gate (integrated? hash? supported?)
│   ├── runtime_abi.rs               # Runtime ABI + decide_runtime_execution — STAYS (extended)
│   ├── arrow_semantic_codec.rs      # LMA1/LMC2 codec — STAYS (out-of-TCB, dev-time)
│   ├── native_arrow_semantic.rs     # Native execution — STAYS (re-anchored for sidecar in Phase 51)
│   └── lib.rs                       # Re-export sidecar module
├── loom-ffi/src/
│   └── duckdb_runtime.rs            # DuckDB SQL surface — EXTEND (consume routing decision)
ingress/
├── loom-source-ingress/src/
│   └── lib.rs                       # Source-neutral types — EXTEND (SidecarExtractionResult)
├── loom-parquet-ingress/src/
│   ├── source_contract.rs           # Thin adapter — EXTEND (fill in sidecar stubs)
│   └── **sidecar_parquet.rs**       # NEW: Parquet-specific sidecar embed/extract
├── loom-vortex-ingress/src/
│   ├── source_contract.rs           # Thin adapter — EXTEND (fill in sidecar stubs)
│   └── **sidecar_vortex.rs**        # NEW: Vortex-specific sidecar embed/extract
└── loom-lance-ingress/src/
    ├── source_contract.rs           # Thin adapter — EXTEND (fill in sidecar stubs)
    └── **sidecar_lance.rs**         # NEW: Lance-specific sidecar embed/extract
```

### Pattern 1: Sidecar Overlay Contract (host-neutral, in loom-core)

**What:** A set of Rust types that model the sidecar as a host-neutral concept: an `L2CoreProgram` + one `ChunkBinding` per host data granule, serialized into host-native metadata. Host adapters implement `embed` and `extract` using their host's specific metadata mechanism.

**When to use:** This is the central data model for Phase 50. All three host adapters produce and consume this contract.

**Example:**
```rust
// crates/loom-core/src/sidecar.rs (NEW)
use crate::l2_core::L2CoreProgram;
use crate::l2core_codec;

/// A Loom sidecar overlay that rides on an unmodified host file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarOverlay {
    /// The encoded L2Core IR bytes (including L2IR magic + version header).
    pub ir_bytes: Vec<u8>,
    /// Per-chunk content-hash bindings linking host data to the IR identity.
    pub bindings: Vec<ChunkBinding>,
}

/// A content-hash binding for one host data granule (column chunk, fragment, row group).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkBinding {
    /// Identifier for the host data granule (e.g., column name, column index, fragment id).
    pub granule_id: String,
    /// Byte range of the host data this binding covers (start, length).
    pub host_data_range: (u64, u64),
    /// Content-hash of the host data in this range, formatted as `l2ir:<hex>`.
    /// Computed via FNV-1a over the raw bytes in `host_data_range`.
    pub content_hash: String,
    /// The L2Core IR program hash that was used to produce the Arrow output
    /// for this granule. Must match the ir_hash decoded from ir_bytes.
    pub ir_identity: String,
}

impl SidecarOverlay {
    /// Encode this overlay into a single byte blob for embedding in host metadata.
    /// The encoding is deterministic (so the same overlay always produces
    /// the same bytes) and self-describing.
    pub fn encode(&self) -> Vec<u8> {
        // Use a simple length-prefixed binary format:
        // [u32: ir_bytes_len][ir_bytes][u16: binding_count][bindings...]
        // Each binding: [u8: granule_id_len][granule_id][u64: start][u64: len]
        //                [u8: hash_len][hash][u8: ir_id_len][ir_identity]
        let mut buf = Vec::new();
        l2core_codec::write_u32(&mut buf, self.ir_bytes.len() as u32);
        buf.extend_from_slice(&self.ir_bytes);
        l2core_codec::write_u16(&mut buf, self.bindings.len() as u16);
        for binding in &self.bindings {
            l2core_codec::write_string(&mut buf, &binding.granule_id);
            l2core_codec::write_u64(&mut buf, binding.host_data_range.0);
            l2core_codec::write_u64(&mut buf, binding.host_data_range.1);
            l2core_codec::write_string(&mut buf, &binding.content_hash);
            l2core_codec::write_string(&mut buf, &binding.ir_identity);
        }
        buf
    }

    /// Decode a SidecarOverlay from bytes (as stored in host metadata).
    pub fn decode(bytes: &[u8]) -> Result<Self, SidecarCodecError> {
        // ... decode the format described above ...
    }
}
```

### Pattern 2: Content-Hash Binding at Column-Chunk Granularity

**What:** For each column chunk in the host file, compute FNV-1a over the raw compressed data bytes (the exact byte range in the file), and store the resulting `l2ir:<hex>` hash in the sidecar's `ChunkBinding`. On read, re-compute the hash over the same byte range and compare.

**Why column-chunk granularity:**
- Parquet: column chunks are the natural unit — each ColumnChunk in a RowGroup has a known byte range (`data_page_offset`, `total_compressed_size`).
- Vortex: segments/fragments map to column data ranges in the Vortex layout.
- Lance: fragments within a dataset map to per-column data files.
- Finer granularity (pages) means more hashes to compute and store, with diminishing benefit — Parquet pages are small (~1MB) and an independent rewrite typically replaces entire column chunks.
- Coarser granularity (file-level) defeats the purpose — any change anywhere invalidates all sidecars.

**Hash collision risk:** FNV-1a 64-bit has a collision probability of ~2^-32 for random inputs within a file. For malicious collision attacks against a specific hash, FNV-1a is trivially broken (it's not a cryptographic hash). However, the threat model for sidecar binding is not adversarial — the host data is authored by the same party that creates the sidecar. If this changes (e.g., third-party sidecars over untrusted host data), a cryptographic hash (SHA-256) should replace FNV-1a.

**Decision:** Use FNV-1a for Phase 50 (matches the existing `l2core_program_hash` approach). Document the collision risk and defer cryptographic upgrade to a later security phase.

### Pattern 3: Parquet Sidecar Embedding via KeyValue Metadata

**What:** Embed the sidecar overlay bytes in Parquet's `FileMetaData.key_value_metadata` and per-column-chunk content hashes in `ColumnMetaData.key_value_metadata`. Both are `optional list<KeyValue>` in the Thrift definition — unknown keys are silently ignored by readers.

**Parquet Thrift structure (from parquet.thrift):**
```thrift
struct KeyValue {
  1: required string key      // e.g., "loom.sidecar.v1", "loom.hash.int32_col"
  2: optional string value    // encoded sidecar bytes, or l2ir:<hex> hash
}

struct FileMetaData {
  ...
  5: optional list<KeyValue> key_value_metadata  // ← file-level: embed SidecarOverlay here
  ...
}

struct ColumnMetaData {
  ...
  8: optional list<KeyValue> key_value_metadata  // ← column-level: embed per-column hashes here
  ...
}
```

**Embedding strategy:**
1. **File-level (`FileMetaData.key_value_metadata`):** One `KeyValue` with key `"loom.sidecar.v1"` and value = `SidecarOverlay::encode()` (the full L2Core IR + all ChunkBindings).
2. **Column-level (`ColumnMetaData.key_value_metadata`):** Optionally, per-column `KeyValue` entries with keys like `"loom.hash.<column_name>"` and value = `l2ir:<hex>` for redundancy and column-level validation.

**Strippable invariant:** A Parquet writer that strips unknown metadata (e.g., during a `REWRITE` operation) will drop the `loom.*` keys. The file remains valid Parquet — the Loom-aware reader simply sees "no sidecar present" and falls back to the host native reader. This is the correct behavior.

### Pattern 4: Fail-Closed Routing Decision Logic

**What:** A new function `decide_sidecar_routing` in `loom-core` that consumes a `SidecarRoutingInput` (engine capabilities + sidecar presence + hash match + encoding support) and returns a `SidecarRoutingDecision` (LoomNative or HostNativeReader). This mirrors the existing `runtime_abi::decide_runtime_execution` pattern.

**Example:**
```rust
// crates/loom-core/src/sidecar_routing.rs (NEW)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarRoutingInput {
    /// Whether the calling engine has integrated Loom scan.
    pub engine_integrated: bool,
    /// The sidecar overlay, if present in the host file.
    pub sidecar: Option<SidecarOverlay>,
    /// Hash verification results: granule_id -> (range, hash_ok).
    pub hash_verification: Vec<HashVerificationResult>,
    /// Whether the encodings in the sidecar IR are supported by this Loom runtime.
    pub encoding_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashVerificationResult {
    pub granule_id: String,
    pub binding: ChunkBinding,
    pub recomputed_hash: String,
    pub matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidecarRoutingDecision {
    /// Take the Loom verifiable-native track.
    LoomNative {
        sidecar: SidecarOverlay,
        verified_bindings: Vec<ChunkBinding>,
    },
    /// Fall back to the host's own native reader.
    HostNativeReader {
        reason: HostNativeReaderReason,
        diagnostics: Vec<SidecarDiagnostic>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostNativeReaderReason {
    EngineNotIntegrated,
    NoSidecarPresent,
    HashMismatch,
    EncodingUnsupported,
}

pub fn decide_sidecar_routing(input: SidecarRoutingInput) -> SidecarRoutingDecision {
    let mut diagnostics = Vec::new();

    // Gate 1: Is the engine Loom-integrated?
    if !input.engine_integrated {
        return SidecarRoutingDecision::HostNativeReader {
            reason: HostNativeReaderReason::EngineNotIntegrated,
            diagnostics: vec![SidecarDiagnostic::new(
                SidecarDiagnosticCode::EngineNotIntegrated,
                "$.engine",
                "calling engine has not integrated Loom scan — fall back to host native reader",
            )],
        };
    }

    // Gate 2: Is a sidecar present?
    let sidecar = match input.sidecar {
        Some(s) => s,
        None => {
            return SidecarRoutingDecision::HostNativeReader {
                reason: HostNativeReaderReason::NoSidecarPresent,
                diagnostics: vec![SidecarDiagnostic::new(
                    SidecarDiagnosticCode::NoSidecarPresent,
                    "$.sidecar",
                    "no Loom sidecar found in host file",
                )],
            };
        }
    };

    // Gate 3: Do all content-hashes match?
    let mismatches: Vec<_> = input.hash_verification.iter()
        .filter(|r| !r.matches)
        .collect();
    if !mismatches.is_empty() {
        for m in &mismatches {
            diagnostics.push(SidecarDiagnostic::new(
                SidecarDiagnosticCode::HashMismatch,
                &format!("$.hash.{}", m.granule_id),
                format!(
                    "content-hash mismatch for granule {}: expected {}, recomputed {}",
                    m.granule_id, m.binding.content_hash, m.recomputed_hash,
                ),
            ));
        }
        return SidecarRoutingDecision::HostNativeReader {
            reason: HostNativeReaderReason::HashMismatch,
            diagnostics,
        };
    }

    // Gate 4: Are the encodings supported?
    if !input.encoding_supported {
        return SidecarRoutingDecision::HostNativeReader {
            reason: HostNativeReaderReason::EncodingUnsupported,
            diagnostics: vec![SidecarDiagnostic::new(
                SidecarDiagnosticCode::EncodingUnsupported,
                "$.sidecar.ir",
                "L2Core IR contains encodings not supported by this Loom runtime",
            )],
        };
    }

    // All gates passed.
    let verified_bindings: Vec<ChunkBinding> = input.hash_verification
        .into_iter()
        .map(|r| r.binding)
        .collect();

    SidecarRoutingDecision::LoomNative {
        sidecar,
        verified_bindings,
    }
}
```

**Key property:** The routing is **exhaustive and honest** — every code path returns either `LoomNative` or `HostNativeReader` with a specific reason. No "maybe" or "partial" states. This matches the repositioning doc's requirement: "fail-closed routing is exhaustive and honest."

### Anti-Patterns to Avoid

- **Mutating host data to embed the sidecar:** The sidecar must be stored in metadata only. Never re-encode, re-compress, or modify host data pages/chunks — this would break the "strippable overlay" invariant and make the content-hash binding circular.
- **Embedding the sidecar at a fixed file offset:** Appending custom sections at known offsets breaks compatibility with tools that append data to Parquet files (the footer is at the end via a 4-byte footer length). Always use host-native metadata mechanisms.
- **Creating a sidecar-specific file extension or wrapper format:** A `.loom_sidecar` file separate from the host file defeats the purpose — the sidecar must be *part of* the host file so there is exactly one artifact to distribute.
- **Making the sidecar non-strippable:** If removing the sidecar requires re-encoding the host data, the design is wrong. A tool that strips unknown metadata from a Parquet file should produce a valid, readable Parquet file.
- **Building hash verification into the host adapter's public API:** The hash verification is a `loom-core` responsibility. Host adapters provide byte ranges; loom-core computes hashes. This keeps the thin adapters thin and the hash logic centralized.
- **Adding a `NativeFallback` or `WasmFallback` execution track:** Per Decision Two, fallback = host's own native reader, not a second Loom execution path. The routing decision has exactly two outcomes: `LoomNative` or `HostNativeReader`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Content-hash over host data ranges | Custom hash library or algorithm | FNV-1a from `l2core_codec.rs` (already implemented, deterministic) | Consistent with existing L2Core IR content-hash; avoid hash algorithm proliferation |
| Sidecar encoding format | Custom serialization (JSON, protobuf, etc.) | Length-prefixed binary format using existing `l2core_codec::write_*` helpers | Deterministic, minimal dependencies, same codec family as L2Core IR |
| Parquet metadata read/write | Raw Thrift serialization | Parquet crate's `parquet::file::metadata` APIs (already in `loom-parquet-ingress`) | Already handles Thrift encoding/decoding, schema evolution, versioning |
| Host-native reader invocation | Re-implement Parquet/Vortex/Lance reader | Host engine's existing read path (DuckDB `read_parquet`, Vortex `scan()`, Lance `Dataset::scan()`) | This is the fallback — use what already works |
| Routing decision infrastructure | New decision framework | Extend existing `runtime_abi.rs` pattern (`RuntimeExecutionDecision`, `RuntimeFallbackPolicy`) | Already battle-tested with fail-closed semantics, diagnostic tracing, and cache identity |
| Arrow output for LoomNative track | New Arrow materialization path | Existing `native_arrow_semantic.rs` (with sidecar-accepting entry point) or `native_arrow_semantic_codegen` | Already has verifier-gated, equivalence-checked, cache-identified native output |

**Key insight:** Phase 50 connects existing infrastructure (L2Core IR codec, content-hash, thin adapters, runtime ABI, native execution) with a *new routing layer*. It should not reimplement any of them.

## Runtime State Inventory

> Phase 50 is **not a rename/refactor/migration phase** — it is a greenfield architecture phase that adds new modules. The Runtime State Inventory is not applicable.

**Verified:** No existing runtime state (databases, live services, OS registrations, secrets, build artifacts) is affected by adding a sidecar overlay model. Existing gates and test fixtures remain backward-compatible per Phase 50.1.

## Common Pitfalls

### Pitfall 1: Breaking Parquet Compatibility with Non-Standard Metadata Keys

**What goes wrong:** Parquet readers that validate metadata keys (e.g., Iceberg's spec requires known keys only) reject files with `loom.*` keys.
**Why it happens:** The Parquet spec says unknown keys should be ignored, but some implementations are stricter.
**How to avoid:** 
1. Prefix all Loom keys with `loom.` (namespace separation).
2. Test with a variety of Parquet readers (DuckDB `read_parquet`, pyarrow, parquet-mr).
3. If a reader rejects `loom.*` keys, document it as a reader bug and provide a `loom strip` CLI tool that removes the sidecar.
**Warning signs:** `read_parquet('sidecar.parquet')` fails in DuckDB or pyarrow with "unknown key" errors.

### Pitfall 2: Hash Verification Is Too Slow for Large Files

**What goes wrong:** Computing FNV-1a over every column chunk's compressed bytes at read time adds latency that cancels the native acceleration gain.
**Why it happens:** Column chunks can be hundreds of MB; hashing the full compressed data is an O(data) operation before any decode happens.
**How to avoid:**
1. Hash only the metadata ranges + first N bytes of each column chunk (where N is configurable, e.g., 64KB). This catches accidental corruption without hashing the full payload.
2. For the full-content guarantee, hash the full compressed data but cache the hash result in the sidecar (so the write-time hash is stored; read-time verification can be configurable — strict mode hashes everything, fast mode trusts the stored hash).
3. Document the tradeoff: this phase delivers *content-hash binding* (proof of consistency), not *tamper-proof attestation* (which needs cryptographic hashes + signatures, deferred).
**Warning signs:** `loom_scan(path)` latency dominated by hash recomputation rather than native decode.

### Pitfall 3: Sidecar Stubs in Thin Adapters Are Not Replaced

**What goes wrong:** Phase 50.1 added `extract_sidecar_bytes_from_*` stubs that return `Ok(None)`. If Phase 50 doesn't replace them with real implementations, the routing gate always falls through to `HostNativeReader`.
**Why it happens:** The stubs are placeholders; Phase 50 must fill them in.
**How to avoid:**
1. Each plan in Phase 50 must explicitly replace the stubs in the corresponding adapter.
2. Tests must assert that `extract_sidecar_bytes_from_*` returns `Some(bytes)` when a sidecar is present.
3. The routing gate should log a diagnostic (not fail silently) when the stub is hit.
**Warning signs:** All routing decisions are `HostNativeReader { reason: NoSidecarPresent }` even after Phase 50 implementation.

### Pitfall 4: Content-Hash Binding Creates Circular Dependency

**What goes wrong:** The sidecar is embedded in the host file's metadata. If the content-hash covers the metadata region that contains the sidecar, the hash changes when the sidecar is written, making it impossible to verify.
**Why it happens:** The metadata and the data it describes are stored together.
**How to avoid:**
1. Content-hash covers **data regions only**, never the metadata region that carries the sidecar.
2. For Parquet: hash covers `ColumnChunk` data pages (from `data_page_offset` to `data_page_offset + total_compressed_size`), not the `ColumnMetaData` or `FileMetaData` structures.
3. For Vortex: hash covers segment/buffer data, not the footer/layout metadata.
**Warning signs:** Hash verification always fails because the act of embedding the sidecar changes the hash target.

### Pitfall 5: Routing Duplicates Existing Runtime ABI Logic

**What goes wrong:** `decide_sidecar_routing` and `decide_runtime_execution` have overlapping concerns (both check artifact status, lowering disposition, etc.), leading to inconsistent decisions or double-checking.
**Why it happens:** Both modules decide "can we execute this artifact natively?"
**How to avoid:**
1. `decide_sidecar_routing` handles **sidecar-level** decisions: engine integration, sidecar presence, hash match, encoding support. It returns `LoomNative` or `HostNativeReader`.
2. `decide_runtime_execution` handles **within-Loom** decisions: verifier acceptance, projection/predicate/split, lowering disposition, concurrency. It returns `NativeCandidate`, `InterpreterFallback`, or `FailClosed`.
3. The call chain is: `sidecar_routing` → if `LoomNative`, then `runtime_execution` over the extracted L2Core IR.
4. The sidecar routing decision replaces the old "which artifact kind?" check (currently `"LMC2" | "LMA1"` in `native_arrow_semantic.rs`).

## Code Examples

### Current State: Thin Adapter Stubs (Phase 50.1, to be replaced)

```rust
// CURRENT: ingress/loom-parquet-ingress/src/source_contract.rs (lines 68-80)
/// Extract sidecar bytes from a Parquet file (Phase 50 placeholder).
/// Returns None until the sidecar overlay contract is defined in Phase 50.
pub fn extract_sidecar_bytes_from_parquet_path(
    path: &Path,
) -> Result<Option<Vec<u8>>, SourceIngressReport> {
    let _ = File::open(path).map_err(|error| {
        rejected_report(path, diagnostic_with_detail(
            SourceDiagnosticCode::OpenFailed,
            "$.open",
            "local Parquet file could not be opened",
            error.to_string(),
        ))
    })?;
    Ok(None)  // ← Placeholder — Phase 50 fills this in
}
```

### Target: Real Sidecar Extraction from Parquet

```rust
// TARGET: ingress/loom-parquet-ingress/src/sidecar_parquet.rs (NEW, Phase 50)
use loom_core::sidecar::{SidecarOverlay, SidecarCodecError};
use parquet::file::metadata::ParquetMetaData;

/// Extract a Loom sidecar overlay from a Parquet file's KeyValue metadata.
pub fn extract_sidecar_from_parquet_metadata(
    metadata: &ParquetMetaData,
) -> Result<Option<SidecarOverlay>, SidecarCodecError> {
    let file_metadata = metadata.file_metadata();
    let kv_list = match file_metadata.key_value_metadata() {
        Some(kv) => kv,
        None => return Ok(None),
    };

    for kv in kv_list {
        if kv.key == "loom.sidecar.v1" {
            let value = match &kv.value {
                Some(v) => v,
                None => continue,
            };
            let overlay = SidecarOverlay::decode(value)?;
            return Ok(Some(overlay));
        }
    }

    Ok(None)
}

/// Embed a Loom sidecar overlay into Parquet KeyValue metadata.
pub fn embed_sidecar_into_parquet_metadata(
    metadata: &mut ParquetMetaData,
    overlay: &SidecarOverlay,
) {
    let encoded = overlay.encode();
    let file_metadata = metadata.file_metadata_mut();
    let mut kv_list = file_metadata.key_value_metadata()
        .cloned()
        .unwrap_or_default();

    // Remove any existing loom.sidecar.v1 entry (idempotent embed)
    kv_list.retain(|kv| kv.key != "loom.sidecar.v1");

    kv_list.push(parquet::format::KeyValue {
        key: "loom.sidecar.v1".to_string(),
        value: Some(String::from_utf8_lossy(&encoded).to_string()),
    });

    // Also add per-column hash entries for visibility
    for binding in &overlay.bindings {
        kv_list.push(parquet::format::KeyValue {
            key: format!("loom.hash.{}", binding.granule_id),
            value: Some(binding.content_hash.clone()),
        });
    }

    file_metadata.set_key_value_metadata(Some(kv_list));
}
```

### Target: Sidecar Routing in DuckDB Runtime

```rust
// TARGET: crates/loom-ffi/src/duckdb_runtime.rs (extended, Phase 50)
use loom_core::sidecar_routing::{decide_sidecar_routing, SidecarRoutingDecision};
use loom_core::sidecar::SidecarOverlay;

/// Read a host file, extract sidecar if present, and route to Loom native
/// or host native reader.
pub fn loom_scan_with_sidecar_routing(
    path: &str,
) -> Result<ScanResult, DuckDbRuntimeError> {
    // Step 1: Determine host format from file extension / magic bytes
    let host_kind = detect_host_kind(path)?;

    // Step 2: Extract sidecar from host metadata (via thin adapter)
    let sidecar = match host_kind {
        HostKind::Parquet => {
            loom_parquet_ingress::sidecar_parquet::extract_sidecar_from_parquet_path(path)?
        }
        HostKind::Vortex => {
            loom_vortex_ingress::sidecar_vortex::extract_sidecar_from_vortex_path(path)?
        }
        HostKind::Lance => {
            loom_lance_ingress::sidecar_lance::extract_sidecar_from_lance_path(path)?
        }
    };

    // Step 3: Compute content-hash verification over host data
    let hash_results = match &sidecar {
        Some(s) => verify_sidecar_bindings(path, host_kind, s)?,
        None => Vec::new(),
    };

    // Step 4: Check encoding support
    let encoding_supported = sidecar.as_ref().map_or(false, |s| {
        check_encoding_support(&s.ir_bytes)
    });

    // Step 5: Route
    let routing_input = SidecarRoutingInput {
        engine_integrated: true, // We're in the Loom DuckDB extension
        sidecar,
        hash_verification: hash_results,
        encoding_supported,
    };

    match decide_sidecar_routing(routing_input) {
        SidecarRoutingDecision::LoomNative { sidecar, verified_bindings } => {
            // Proceed with verifiable-native track:
            // 1. Parse L2Core IR from sidecar.ir_bytes
            // 2. Run verifier (fail-closed)
            // 3. Execute via native Arrow semantic codegen
            // 4. Return Arrow DataChunk to DuckDB
            execute_loom_native_track(&sidecar, &verified_bindings)
        }
        SidecarRoutingDecision::HostNativeReader { reason, diagnostics } => {
            // Fall back to host's native reader:
            // For DuckDB, this means calling read_parquet / read_vortex
            // instead of loom_scan. Return the result as if Loom was never involved.
            log_routing_fallback(path, reason, &diagnostics);
            execute_host_native_reader(path, host_kind)
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Loom owns the file format (LMC1/LMC2/LMA1 container) | Loom is a strippable sidecar on host formats | Phase 50 (now) | Loom-unaware engines read files unchanged; fallback is host native reader, not Loom interpreter |
| Artifact identity = container identity (LMC2 magic + version) | Artifact identity = L2Core IR content-hash (`l2ir:<hex>`), independent of packaging | Phase 49 (completed) | Enables sidecar model — the IR can be embedded anywhere; identity is packaging-independent |
| Top-level format adoption required ("use .loom files") | Sidecar is additive enhancement ("add loom to your Parquet files") | Phase 50 (now) | Zero-risk adoption — worst case, the sidecar is ignored |
| Single execution track tied to container artifact kind | Single execution track tied to L2Core IR + sidecar binding | Phase 50 (now) | No container demotion needed for execution; L2Core IR is sole in-TCB artifact |
| Wasm fallback as safety net (AnyBlox design) | No Wasm; fallback = host native reader | Decision Two (never implemented in Loom) | No second IR execution implementation; no equivalence-diff burden |

**Deprecated/outdated:**
- `artifact_kind == "LMC2" | "LMA1"` checks in `native_arrow_semantic.rs`: Replaced by sidecar routing that checks `LoomNative` decision.
- `SourceIngressAcceptedArtifact` (already deprecated in Phase 50.1): Superseded by `SidecarOverlay` + `ChunkBinding`.
- Direct LMA1/LMC2 byte emission from ingress crates (removed in Phase 50.1): Replaced by sidecar embedding in host metadata.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Parquet readers (DuckDB, pyarrow, parquet-mr) silently ignore unknown KeyValue metadata entries | Parquet Sidecar Embedding | HIGH: If a reader rejects `loom.*` keys, sidecar-carrying Parquet files become unreadable by that reader, violating the "strippable overlay" premise. Mitigation: test with major readers; provide `loom strip` CLI. |
| A2 | FNV-1a 64-bit is sufficient for content-hash binding in MVP2 (non-adversarial threat model) | Content-Hash Binding | MEDIUM: If the threat model later includes third-party sidecars over untrusted host data, FNV-1a collisions become exploitable. Mitigation: document and defer cryptographic upgrade. |
| A3 | Vortex and Lance file formats have extension points equivalent to Parquet's KeyValue metadata for embedding sidecar bytes | Sidecar Embedding | MEDIUM: Vortex's layout/footer API and Lance's manifest API have been used for metadata extraction in existing ingress crates. Whether they support *arbitrary* custom key-value metadata for writing is unverified for this research. Mitigation: Parquet first; Vortex/Lance follow with format-specific investigation during planning. |
| A4 | The existing `runtime_abi.rs` pattern (fail-closed gate, `RuntimeFallbackPolicy`, `RuntimeExecutionDecision`) is the right architectural analog for the sidecar routing decision | Routing Decision Logic | LOW: The runtime ABI was designed for this class of decision and has been battle-tested through Phases 22-43. |
| A5 | Host-native reader fallback for DuckDB means calling `read_parquet('path')` instead of `loom_scan('path')`, returning the result transparently to the user | DuckDB Integration | LOW: DuckDB already supports `read_parquet` natively; the extension just chooses which function to delegate to. |

## Open Questions

1. **Should the content-hash over host data use a cryptographic hash (SHA-256/BLAKE3) instead of FNV-1a?**
   - What we know: FNV-1a is non-cryptographic and trivially collisionable. The L2Core IR uses FNV-1a (Phase 49). The threat model for MVP2 is non-adversarial (sidecar authored by same party as host data).
   - What's unclear: Whether future phases (attestation, third-party sidecars) require cryptographic binding.
   - **Recommendation:** Use FNV-1a for Phase 50 (consistency with L2Core IR). Add a `hash_algorithm` field to `ChunkBinding` (enum: Fnv1a64, Sha256) so the upgrade path is encoded in the binding itself. Defer cryptographic upgrade to a later security phase.

2. **What is the exact embedding mechanism for Vortex and Lance sidecars?**
   - What we know: Parquet has well-defined `KeyValue` metadata at file and column levels (confirmed via Thrift spec). Vortex files have a footer with a `layout()` and `segment_map()` API. Lance datasets have a manifest with metadata.
   - What's unclear: Whether Vortex and Lance formats support *arbitrary custom key-value metadata* for writing (reading is already used by ingress crates). The Vortex footer API in `vortex-file` 0.74 may or may not expose a general-purpose metadata dictionary.
   - **Recommendation:** Implement Parquet first (the priority host). For Vortex and Lance, investigate during Phase 50 planning — if custom metadata writing is not available, the sidecar can be stored as a separate `.loom_sidecar` file adjacent to the host file, with the sidecar filename derived from the host file's identity (e.g., `data.parquet.loom_sidecar`). This is a fallback, not the ideal.

3. **How does the sidecar interact with DuckDB's `loom_scan(path)` SQL surface?**
   - What we know: `loom_scan(path)` currently expects `.loom` files (LMC1/LMC2). Phase 50.1 kept LMC2 decode for backward compat. Phase 50 must make `loom_scan(path)` accept `.parquet` files that carry a sidecar.
   - What's unclear: Whether DuckDB's table function can auto-detect file format from extension/magic, or whether a new SQL function (`loom_scan_parquet(path)`) is needed. The current `loom_scan` is a generic table function.
   - **Recommendation:** Keep `loom_scan(path)` as the single entry point. Internally, detect host format from file extension (`.parquet`, `.vortex`, `.lance`) or magic bytes. If a sidecar is present and routing passes, execute Loom-native; otherwise, fall back to host native reader. The user sees no difference in SQL surface.

4. **What is the lifecycle of a sidecar — who creates it, when, and how?**
   - What we know: The existing ingress crates had `emit_source_ingress_lmc2_from_*` functions that created LMC2 artifacts from source data. After Phase 50.1 these are removed. Who creates the sidecar now?
   - What's unclear: Whether sidecar creation is a separate Loom CLI command (`loom sidecar create --source data.parquet --ir program.l2ir`), or whether it's integrated into the source ingress pipeline.
   - **Recommendation:** For Phase 50, implement a `loom sidecar embed` CLI command (in `loom-cli`) that takes a host file and an L2Core IR file, computes content-hashes over the host data, creates a `SidecarOverlay`, and embeds it into the host file's metadata. This is a separate operation from read-time routing. The read-time path only extracts and verifies existing sidecars.

5. **Should the L2Core IR in the sidecar be pre-verified at embed time, or verified at read time?**
   - What we know: Phase 49's `verify_l2_core_bytes` can verify IR from bytes at any time. At embed time, we can verify the IR and reject invalid programs before embedding. At read time, we re-verify (fail-closed) before execution.
   - What's unclear: Whether embed-time verification is a hard requirement or a best practice.
   - **Recommendation:** Embed time: verify the IR before embedding (fail-closed — reject invalid IR). Read time: re-verify the IR before execution (fail-closed — reject if verification fails). This follows the existing pattern: verify at every boundary.

## Environment Availability

> Phase 50 requires the standard Rust toolchain and existing workspace dependencies. No new external tools or services needed.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust (rustc) | Compilation | ✓ | 1.92.0 | — |
| Cargo | Build system | ✓ | 1.92.0 | — |
| Python 3 | Release gate scripts | ✓ | 3.14.5 | — |
| DuckDB | SQL smoke tests (backward compat) | ✓ | Managed by project scripts | — |
| MLIR/LLVM | Native codegen tests (backward compat) | ✓ | Managed by project toolchain | `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` |
| parquet crate | Parquet metadata read/write | ✓ | 58.3.0 (workspace pinned) | — |
| vortex-file crate | Vortex file layout | ✓ | 0.74.0 (workspace pinned) | — |
| lance crate | Lance dataset API | ✓ | 7.0.0 (workspace pinned) | — |

**Missing dependencies with no fallback:** None — all tools and crates are already available in the workspace.

## Validation Architecture

> `nyquist_validation` is explicitly set to `false` in `.planning/config.json`. Validation Architecture section omitted per protocol.

## Security Domain

> `security_enforcement` is `true` (default). The sidecar overlay model introduces new security considerations: content-hash binding is the boundary between "trust the sidecar" and "fall back to host native reader."

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | `verify_l2_core_bytes` (Phase 49) fail-closed on malformed IR; `SidecarOverlay::decode` fail-closed on malformed sidecar bytes; hash verification fail-closed on mismatch |
| V6 Cryptography | yes (hash only, non-cryptographic) | FNV-1a for content-hash binding; documented as non-cryptographic with collision risk |
| V7 Error Handling | yes | Every routing failure returns `HostNativeReader` with typed reason + stable diagnostics; no information disclosure to the caller beyond the reason code |
| V10 Malicious Code | yes | L2Core verifier (in-TCB) gates all IR execution; no execution without accepted verification |

### Known Threat Patterns for Sidecar Overlay

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed sidecar bytes in host metadata | Tampering | `SidecarOverlay::decode` fail-closed with `SidecarCodecError`; never panics |
| Content-hash collision attack (deliberate) | Tampering | FNV-1a is non-cryptographic; documented risk. Mitigation: use SHA-256 in future if third-party sidecar threat model emerges |
| Sidecar IR contains unsupported encoding → native track crashes | Elevation of Privilege | Gate 4 (encoding_supported) in `decide_sidecar_routing` prevents execution of unsupported IR |
| Host data rewritten independently → stale sidecar executed | Spoofing | Gate 3 (hash match) detects host data changes; mismatched granules fall back to host native reader |
| Stripped sidecar → engine falls back silently, user unaware | Information Disclosure | Routing decision logs `HostNativeReader` reason; observable in diagnostics but not surfaced as an error (by design — fallback is correct behavior) |
| Sidecar embedded with malicious IR, hash matches (author controls both) | Elevation of Privilege | L2Core verifier (in-TCB) gates execution; total-function language restricts expressiveness; verifier + language-level restrictions block arbitrary code |

## Sources

### Primary (HIGH confidence)
- `docs/repositioning.md` — §8 item 4, §9: sidecar overlay model, three hosts = one IR + three thin adapters, content-hash binding at column-chunk/fragment granularity, fail-closed routing
- `crates/loom-core/src/l2core_codec.rs` — Phase 49 independent L2Core IR codec: `L2IR` magic, `l2core_program_hash()` → `l2ir:<hex>` via FNV-1a, deterministic binary encoding
- `crates/loom-core/src/l2_core.rs` — `L2CoreProgram::content_hash()` method
- `crates/loom-core/src/full_verifier.rs` — `verify_l2_core_bytes` fail-closed parse-and-verify gate (lines 220-245)
- `crates/loom-core/src/runtime_abi.rs` — `decide_runtime_execution` fail-closed decision pattern (lines 679-789), `RuntimeFallbackPolicy`, `RuntimeExecutionDecision`
- `crates/loom-core/src/native_arrow_semantic.rs` — artifact_kind checks at lines 404, 501, 632 (`"LMC2" \| "LMA1"`) — to be replaced by sidecar routing
- Parquet Thrift specification (`parquet.thrift`, raw.githubusercontent.com): `KeyValue` struct (line ~665), `FileMetaData.key_value_metadata` (field 5), `ColumnMetaData.key_value_metadata` (field 8), `ColumnChunk` structure
- Phase 49 artifacts: `49-CONTEXT.md`, `49-SUMMARY.md` — confirmed L2Core IR codec is complete, stable, and the dependency root
- Phase 50.1 artifacts: `50.1-RESEARCH.md`, `50.1-01-SUMMARY.md`, `50.1-02-SUMMARY.md`, `50.1-03-SUMMARY.md`, `50.1-PATTERNS.md` — confirmed container demotion, thin adapter degradation, sidecar stubs in place

### Secondary (MEDIUM confidence)
- `ingress/loom-parquet-ingress/src/source_contract.rs` — current Parquet thin adapter (lines 68-80: `extract_sidecar_bytes_from_parquet_path` stub)
- `ingress/loom-vortex-ingress/src/source_contract.rs` — current Vortex thin adapter (lines 56-71: stubs)
- `ingress/loom-lance-ingress/src/source_contract.rs` — current Lance thin adapter (lines 107-125: stubs)
- `ingress/loom-source-ingress/src/lib.rs` — `SourceFacts`, `SourceIdentity`, `SourceIngressReport` — existing types for thin adapter output
- `crates/loom-core/src/arrow_semantic_codec.rs` — `LMC2_MAGIC`, `LMA1_MAGIC`, encode/decode functions (out-of-TCB, dev-time only per Phase 50.1)

### Tertiary (LOW confidence)
- `[ASSUMED]` A1: Parquet readers silently ignore unknown KeyValue metadata entries
- `[ASSUMED]` A2: FNV-1a sufficient for non-adversarial content-hash binding
- `[ASSUMED]` A3: Vortex and Lance have equivalent metadata extension points
- `[ASSUMED]` A5: DuckDB `read_parquet` can serve as host native reader fallback

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all existing workspace crates and APIs
- Architecture: HIGH — sidecar overlay model is well-specified in `repositioning.md`; Parquet embedding mechanism confirmed via Thrift spec; routing pattern mirrors existing `runtime_abi.rs`
- Pitfalls: MEDIUM — Parquet reader compatibility with unknown keys is unverified; Vortex/Lance embedding mechanisms need format-specific investigation during planning
- Code patterns: HIGH — established patterns from Phases 49, 50.1, and runtime ABI are directly reusable

**Research date:** 2026-06-11
**Valid until:** 2026-06-25 (14 days — stable architecture, no external API changes expected)
