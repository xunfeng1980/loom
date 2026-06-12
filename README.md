**English** | [Chinese](README-zh.md)

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

## Integration Model: Sidecar Overlay

The sidecar is strippable metadata carried alongside host data:

- **External sidecar** (production): `data.parquet.loomsidecar` — never touches the original file
- **Embedded sidecar** (dev): `loom.sidecar.v1` KeyValue metadata in Parquet
- **Iceberg/Puffin** (planned): metadata references to external sidecar blobs

Engines that understand Loom take the verifiable-native fast path; engines that
don't keep reading with their own host-native reader.

## Quickstart

All commands run from the repository root. Copy-paste ready.

### 1. Build

```bash
cargo build --release -p loom-cli
```

### 2. Inspect the example Decode IR

```bash
cat assets/quickstart.ron
```

This is a 5-row program: for each row, append `Int32(42)` to the output.
Edit `assets/quickstart.ron` or write your own.

### 3. Convert and verify

```bash
cargo run --release -p loom-cli -- convert assets/quickstart.ron assets/quickstart.l2ir
cargo run --release -p loom-cli -- verify-l2core assets/quickstart.l2ir
```

### 4. Embed as external sidecar

```bash
cargo run --release -p loom-cli -- sidecar embed-external assets/data.parquet assets/quickstart.l2ir
```

Writes `assets/data.parquet.loomsidecar`. The original `assets/data.parquet` is never touched.

### 5. Run the DuckDB extension

Build the extension:

```bash
cd contrib/duckdb-ext && mkdir -p build && cd build && cmake .. && make -j$(sysctl -n hw.logicalcpu) && cd ../../..
```

Launch DuckDB (vendored CLI, no external install needed):

```bash
./contrib/duckdb-ext/vendor/duckdb-cli/duckdb -unsigned \
  -c "LOAD '$(pwd)/contrib/duckdb-ext/build/loom.duckdb_extension'; SELECT * FROM loom_scan('$(pwd)/assets/data.parquet');"
```

The decoded data is real Arrow — any SQL works. Try complex queries:

```bash
./contrib/duckdb-ext/vendor/duckdb-cli/duckdb -unsigned -c "
LOAD '$(pwd)/contrib/duckdb-ext/build/loom.duckdb_extension';

SELECT out_name, COUNT(*), MIN(out_id), MAX(out_count), AVG(out_score)
FROM loom_scan('$(pwd)/assets/data.parquet')
GROUP BY out_name
ORDER BY out_name;
"
```

```bash
./contrib/duckdb-ext/vendor/duckdb-cli/duckdb -unsigned -c "
LOAD '$(pwd)/contrib/duckdb-ext/build/loom.duckdb_extension';

SELECT * FROM loom_scan('$(pwd)/assets/data.parquet')
WHERE out_ratio > 2.0 AND out_flag = true;
"
```

### 6. Run tests

```bash
cargo test --workspace
bash scripts/e2e-full.sh
```

## FFI Surface

Seven C ABI entry points provide the complete sidecar lifecycle:

```
loom_extract       → read sidecar (external file first, then embedded)
loom_verify        → semantic verification + BLAKE3 content hash
loom_verify_json   → structured facts/diagnostics JSON
loom_route         → 4-gate routing decision
loom_decode         → full decode loop (route → verify → decode)
loom_free_cstr     → free returned strings
loom_free_bytes    → free returned byte buffers
```

## Correctness Model

**Three-layer differential verification** with a clear offline/online boundary.

```
                     ── OFFLINE (build/CI, fixed corpus) ──

 L2Core IR program
    │
    ├──→ kloom (K formal semantics engine)
    │      → krun → parse <events> → K trace (spec baseline)
    │
    ├──→ Rust interp (fast implementation of K spec)
    │      → TracedBuilder → native trace
    │
    └──→ compare (offline gate)
           K trace == interp trace          → per-event diff
           interp output == K output        → final RecordBatch diff
           mismatch → interp bug, CI fails

 L2Core IR program + fixed test corpus
    │
    ├──→ Rust interp → reference output
    ├──→ JIT (melior/LLVM) → native output
    │
    └──→ compare (offline gate)
           JIT trace == interp trace        → per-event diff
           JIT output == interp output      → RecordBatch diff
           mismatch → JIT bug, CI fails


                     ── ONLINE (production query) ──

 Host data + sidecar
    │
    └──→ L2Core interpreter  (loom_decode)
           extract → verify → 4-gate route → interpret
           → Arrow RecordBatch (IPC + C Data Interface)
           → DuckDB / Spark / Arrow consumer
```

- **kloom** (offline) — K framework formal semantics, 14 spec tests (14/14 passing)
- **Rust interp** (production decoder) — the general L2Core interpreter
  (`interp/l2core_interp.rs`) wired into `loom_decode`; verified against
  kloom in CI. Decodes i32/i64/f32/f64/bool, nullable, and Utf8 columns from
  auto-generated IR and emits real Arrow (`StreamWriter` IPC +
  `arrow::ffi` C Data Interface), materialized into typed rows by the DuckDB
  `loom_scan` table function.
- **JIT** (offline-verified, not yet wired to the production FFI) — melior/LLVM
  native codegen, validated against the interpreter in CI. The decode-chain
  work made the interpreter the production runtime; routing the sidecar FFI to
  the JIT is future work.
- **Lean** (offline, planned) — formal classification of IR programs

## 4-Gate Routing

Every sidecar read passes through four fail-closed gates:

1. Engine integrated? → no → fallback
2. Sidecar present? → no → fallback
3. Content-hash match? → no → fallback
4. Encoding supported? → no → fallback
5. All pass → Loom-native decode

Content hashes use **BLAKE3-256** (`blake3:<hex>`) for tamper-resistant binding.

## End-to-End Flow

```
 ┌──────────────────────────────────────────────────────────────────────┐
 │                        DATA PRODUCER                                 │
 │                                                                      │
 │  Parquet / Lance / Vortex                        L2Core Decode IR    │
 │  ┌──────────────────┐                           ┌──────────────┐    │
 │  │  id  name  score │     loom sidecar          │ for i in 0..N │    │
 │  │   1  alice  0.5  │──── embed-external ──────→│   copy input  │    │
 │  │   2  bob    1.2  │                           │   → output    │    │
 │  └──────────────────┘                           └──────────────┘    │
 │         │                                              │             │
 │         │         data.parquet.loomsidecar             │             │
 │         └──────────────┬───────────────────────────────┘             │
 └────────────────────────┼────────────────────────────────────────────┘
                          │
               ship / distribute / Iceberg snapshot
                          │
 ┌────────────────────────┼────────────────────────────────────────────┐
 │                        │         DATA CONSUMER                       │
 │                        ▼                                             │
 │  ┌─────────────────────────────────────────────────────────────┐    │
 │  │                    loom-ffi (C ABI)                          │    │
 │  │                                                              │    │
 │  │  extract ──→ verify ──→ 4-gate route ──→ decode             │    │
 │  │     │           │         ┌───┴───┐           │              │    │
 │  │     │    verify_l2core    │ Loom  │ Host     │              │    │
 │  │     │    + kloom diff     │ native│ native   │              │    │
 │  │     │                     │ decode│ fallback │              │    │
 │  │     │                     └───┬───┴─────┬────┘              │    │
 │  │     │                         │         │                   │    │
 │  │     │                    interp/JIT   host reader           │    │
 │  └─────┼─────────────────────────┼─────────┼──────────────────┘    │
 │        │                         │         │                        │
 │        ▼                         ▼         ▼                        │
 │  ┌─────────────────────────────────────────────────────────────┐    │
 │  │              DuckDB / Spark / DataFusion / Arrow             │    │
 │  │                                                              │    │
 │  │  SELECT * FROM loom_scan('data.parquet')                     │    │
 │  │  ┌──────────────────────────────┐                            │    │
 │  │  │  id  name   score            │                            │    │
 │  │  │   1  alice   0.5             │                            │    │
 │  │  │   2  bob     1.2             │                            │    │
 │  │  └──────────────────────────────┘                            │    │
 │  └─────────────────────────────────────────────────────────────┘    │
 └──────────────────────────────────────────────────────────────────────┘
```

The Decode IR is the contract: verified once at write time, replayed at read time.
The sidecar is strippable — engines that don't understand it fall back to their
own native reader seamlessly.

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
(L2Core interp)  │
   │             │
   └─────┬───────┘
         ▼
  DuckDB / Arrow consumer
```

## Encodings

Raw, bitpack, frame-of-reference, dictionary, RLE, FSST, dict-over-FSST,
ALP Float32/Float64. L2Core IR supports Boolean, Int32, Int64, Float32,
Float64, Utf8.

## Decode runtime & JIT

The production decode runtime is the **L2Core interpreter**: `loom_decode`
extracts the sidecar, verifies the IR, evaluates the 4-gate route, and — on the
Loom-native path — interprets the program to a real Arrow `RecordBatch`, returned
as a bare IPC stream and via the Arrow C Data Interface. The DuckDB `loom_scan`
table function materializes those columns into typed SQL rows. Auto-generated IR
(`generate_decode_ir_from_parquet`) covers non-null i32/i64/f32/f64/bool, nullable
fixed-width, and non-null Utf8 columns.

The melior/LLVM **JIT** is compiled in and validated against the interpreter in
CI (offline differential, `production_arrow_semantic_*`), but is **not yet wired
to the production sidecar FFI** — routing decode through the JIT is future work.
The DuckDB extension links `libloom_ffi.a` built `--no-default-features`, so the
JIT/LLVM is excluded from the loadable extension.

## Why Loom

Data engines share query plans and columnar memory. They don't share the
decoder with the data. AnyBlox predicted Decode IR as the bridge. Loom builds
it: a decoder small enough to verify, total enough to terminate, and
Arrow-shaped enough that any host engine can consume the result without
learning every source format.
