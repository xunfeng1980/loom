**English** | [中文](README-zh.md)

<p align="center">
  <img src="assets/loom-logo-minimal.svg" width="180" alt="Loom logo">
</p>

# Loom — Decode IR for Self-Decoding Datasets

Loom is the **Decode IR** implementation predicted by the [AnyBlox](https://gienieczko.com/anyblox-paper) paper (VLDB 2025 Best Paper). It is a deliberately
non-Turing-complete, total-function language whose only possible output is
well-formed Apache Arrow.

Loom realizes the **self-decoding dataset** vision: bundle a verified, tiny
decoder with the data so any engine can read it without learning every source
format. The Decode IR is small enough to formally verify, total enough to
guarantee termination, and Arrow-shaped so host engines consume the result
without format-specific logic.

Also referenced in [F3](https://dl.acm.org/doi/pdf/10.1145/3749163) — Open-Source
Data File Format for the Future (ACM SIGMOD).

## Integration Model: Sidecar Overlay

The sidecar is strippable metadata carried alongside host data:

- **External sidecar** (production): `data.parquet.loomsidecar` — never touches the original file
- **Embedded sidecar** (dev): `loom.sidecar.v1` KeyValue metadata in Parquet
- **Iceberg/Puffin** (planned): metadata references to external sidecar blobs

Engines that understand Loom take the verifiable-native fast path; engines that
don't keep reading with their own host-native reader.

## Quickstart

### 1. Build

```bash
cargo build --release -p loom-cli
```

### 2. Generate external sidecar (production)

```bash
cargo run --release -p loom-cli -- sidecar embed-external data.parquet [program.l2ir]
```

Writes `data.parquet.loomsidecar` — original file unchanged.

### 3. Verify + inspect

```bash
# Verify IR and return content-hash identity
cargo run --release -p loom-cli -- verify-l2core program.l2ir
```

### 4. DuckDB extension

```bash
cd contrib/duckdb-ext && mkdir -p build && cd build
cmake .. && make -j$(sysctl -n hw.logicalcpu 2>/dev/null || nproc)
```

```sql
LOAD 'contrib/duckdb-ext/build/loom.duckdb_extension';
SELECT * FROM loom_scan('data.parquet');
```

### 5. Tests

```bash
cargo test --workspace
bash scripts/e2e-full.sh
```

## FFI Surface

Seven C ABI entry points provide the complete sidecar lifecycle:

```
loom_sidecar_extract       → read sidecar (external file first, then embedded)
loom_sidecar_verify        → semantic verification + BLAKE3 content hash
loom_sidecar_verify_json   → structured facts/diagnostics JSON
loom_sidecar_route         → 4-gate routing decision
loom_sidecar_decode         → full decode loop (route → verify → decode)
loom_sidecar_free_cstr     → free returned strings
loom_sidecar_free_bytes    → free returned byte buffers
```

## Correctness Model

```
      kloom (K trace) ──── offline diff ─────┐
      Lean (proof) ────── classification ────┤
                                              ├──→ Rust interp (ground truth)
                                              │         │
                                      JIT (melior/LLVM) ── online compare ──→ interp output
                                              │
                                      match? → JIT result
                                      diverge? → discard, fallback host-native
```

- **Rust interp** — pure-Rust L1/L2 decoder, differentially verified against kloom offline
- **JIT** — melior/LLVM compiles L2Core IR → native code, validated against interp online
- **kloom** — K framework spec-oracle for offline differential verification
- **Lean** — formal classification of IR programs (planned)

## 4-Gate Routing

Every sidecar read passes through four fail-closed gates:

1. Engine integrated? → no → fallback
2. Sidecar present? → no → fallback
3. Content-hash match? → no → fallback
4. Encoding supported? → no → fallback
5. All pass → Loom-native decode

Content hashes use **BLAKE3-256** (`blake3:<hex>`) for tamper-resistant binding.

## Repository Map

| Path | Purpose |
|------|---------|
| `crates/loom-ir-core` | Zero-dependency decode IR — L2Core program model, sidecar overlay, BLAKE3 content-hash identity, 4-gate routing, verifier |
| `crates/loom-ffi` | Production core + C ABI — `interp/` Rust decoder (kloom-verified), `jit/` melior/LLVM acceleration, 7-function FFI surface |
| `crates/loom-parquet-ingress` | Parquet adapter — sidecar extract/embed, decode IR auto-generation, chunk binding computation |
| `crates/loom-vortex-ingress` | Vortex adapter — ingress boundary for real Vortex files |
| `crates/loom-lance-ingress` | Lance adapter — ingress boundary (sidecar deferred) |
| `crates/loom-source-ingress` | Shared source-neutral contract types |
| `crates/loom-cli` | CLI — `sidecar embed`, `sidecar embed-external`, `verify-l2core` |
| `contrib/duckdb-ext` | C++ DuckDB extension (links `libloom_ffi.a`) |
| `contrib/kloom` | K framework spec-oracle for differential verification |

## Architecture

```
Parquet / Lance / Vortex
        │
  .loomsidecar (external)  or  embedded metadata
        │
        ▼
  loom-ffi (C ABI)
  extract → verify → 4-gate route → decode
        │
   ┌────┴────┐
   ▼         ▼
Loom-native  Host-native
   decode      fallback
(JIT → LLVM)     │
   │             │
   └─────┬───────┘
         ▼
  DuckDB / Arrow consumer
```

## Encodings

Raw, bitpack, frame-of-reference, dictionary, RLE, FSST, dict-over-FSST,
ALP Float32/Float64. L2Core IR supports Boolean, Int32, Int64, Float32,
Float64, Utf8.

## JIT Backend

melior/LLVM JIT is compiled in and validates against the interpreter online.
Production-level JIT codegen tests pass for supported shapes. The current
sidecar FFI surface exposes extract/verify/route/decode. JIT is exercised
in the production route tests (`production_arrow_semantic_*`).

## Why Loom

Data engines share query plans and columnar memory. They don't share the
decoder with the data. AnyBlox predicted Decode IR as the bridge. Loom builds
it: a decoder small enough to verify, total enough to terminate, and
Arrow-shaped enough that any host engine can consume the result without
learning every source format.
