**English** | [中文](README-zh.md)

<p align="center">
  <img src="assets/loom-logo-minimal.svg" width="180" alt="Loom logo">
</p>

# Loom

Loom is a **distribution-oriented decoder IR**: a deliberately non-Turing-complete,
total-function language whose only possible output is well-formed Apache Arrow.

The integration model is the **sidecar overlay**: embed a Loom IR program as
strippable metadata in an existing Parquet, Lance, or Vortex file. Engines that
understand Loom take the verifiable-native fast path; engines that don't keep
reading with their own host-native reader.

## Quickstart

### 1. Build the CLI

```bash
cargo build -p loom-cli --release
```

### 2. Embed a sidecar into a Parquet file

```bash
cargo run -p loom-cli -- sidecar embed data.parquet [program.l2ir]
```

Adds `loom.sidecar.v1` KeyValue metadata. The original data is untouched.

### 3. Build the DuckDB extension

```bash
cd contrib/duckdb-ext && mkdir -p build && cd build
cmake .. && make -j$(sysctl -n hw.logicalcpu 2>/dev/null || nproc)
```

### 4. Query

```sql
LOAD 'contrib/duckdb-ext/build/loom.duckdb_extension';
SELECT * FROM loom_scan('data.parquet');
```

### 5. Run tests

```bash
cargo test --workspace
bash scripts/sidecar-overlay-test.sh
```

## Correctness Model

```
kloom (K spec + krun) ──离线差分──→ Rust interp (ground truth)
                                          │
                                    JIT (melior/LLVM) ──在线对比──→ interp 输出
                                          │
                                    一致？→ JIT 结果
                                    不一致？→ 丢弃，回退宿主 native reader
```

- **kloom** — 离线，K 形式化语义 + krun 执行，trace 逐条对齐，证明 interp 实现正确
- **interp** — 在线，纯 Rust L1/L2 解码器，被 kloom 验证过的 ground truth
- **JIT** — 在线，每次执行后和 interp 逐行对比，不一致则丢弃

## Repository Map

| Path | Purpose |
|------|---------|
| `crates/loom-ir-core` | Zero-dependency decode IR — L2Core program model, sidecar overlay, content-hash identity, 4-gate routing, verifier |
| `crates/loom-ffi` | Production core + C ABI — `interp/` Rust decoder (verified by kloom), `jit/` melior/LLVM acceleration, sidecar C ABI |
| `crates/loom-parquet-ingress` | Parquet ingress adapter — sidecar extract/embed via KeyValue metadata |
| `crates/loom-vortex-ingress` | Vortex ingress adapter + oracle fixtures |
| `crates/loom-lance-ingress` | Lance ingress adapter (sidecar deferred — 7.0.0 manifest lacks writable metadata) |
| `crates/loom-source-ingress` | Shared source-neutral contract types |
| `crates/loom-cli` | CLI — `sidecar embed`, `verify-l2core` |
| `contrib/duckdb-ext` | C++ DuckDB extension (links `libloom_ffi.a`) |
| `contrib/kloom` | K framework spec-oracle for differential verification |

## Architecture

```
Parquet / Lance / Vortex
        │
        ▼
  loom-ffi (C ABI)
  extract → verify → 4-gate route
        │
   ┌────┴────┐
   ▼         ▼
Loom-native  Host-native
  decode      fallback
(JIT → LLVM)   │
   │           │
   └─────┬─────┘
         ▼
  DuckDB / Arrow consumer
```

**4-gate routing:**
1. Engine integrated? → no → fallback
2. Sidecar present? → no → fallback
3. Content-hash match? → no → fallback
4. Encoding supported? → no → fallback
5. All pass → Loom-native decode

Safety model: fail-closed at every gate. The sidecar is strippable — unknown
`loom.*` keys are ignored by standard readers. If Loom fails, the file is still
valid Parquet/Lance/Vortex.

**JIT acceleration (planned):** JIT backend (melior/LLVM) is compiled in and validates
against the interpreter online. The current sidecar ABI exposes extract/verify/route gate.
Full sidecar decode integration (Loom-native decode → JIT → Arrow → DuckDB) is the next
integration milestone. JIT output is always validated against the interpreter — mismatches
are discarded and fall back to the host-native reader.

## Encodings

Raw, bitpack, frame-of-reference, dictionary, RLE, FSST, dict-over-FSST,
ALP Float32/Float64.

## Why Loom

Data engines share query plans and columnar memory. They don't share the decoder
with the data. Loom makes the decoder small enough to verify, total enough to
terminate, and Arrow-shaped enough that a host engine can consume the result
without learning every source format.
