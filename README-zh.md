[English](README.md) | **中文**

<p align="center">
  <img src="assets/loom-logo-minimal.svg" width="180" alt="Loom logo">
</p>

# Loom — 自解码数据集的 Decode IR

Loom 是 [AnyBlox](https://gienieczko.com/anyblox-paper) 论文（VLDB 2025 最佳论文）预言的 **Decode IR** 实现。它是一门故意受限、非图灵完备的总函数语言，唯一可能的输出是符合规范的 Apache Arrow。

Loom 实现了**自解码数据集**愿景：将经过验证的微型解码器与数据绑定，任何引擎无需学习源格式即可读取。Decode IR 足够小、可形式化验证，足够总函数以保证终止，输出 Arrow 形状使宿主引擎无需格式特定逻辑即可消费。

## 集成模型：Sidecar Overlay

Sidecar 是携带在宿主数据旁的可剥离元数据：

- **外部 sidecar**（生产）：`data.parquet.loomsidecar` — 不触碰原始文件
- **内嵌 sidecar**（开发）：Parquet 的 `loom.sidecar.v1` KeyValue 元数据
- **Iceberg/Puffin**（计划中）：指向外部 sidecar blob 的元数据引用

理解 Loom 的引擎走可验证的原生快速路径；不理解的引擎继续用自己的原生 reader 读取。

## 快速开始

### 1. 编译

```bash
cargo build --release -p loom-cli
```

### 2. 生成外部 sidecar（生产路径）

```bash
cargo run --release -p loom-cli -- sidecar embed-external data.parquet [program.l2ir]
```

生成 `data.parquet.loomsidecar` — 原始文件不变。

### 3. 验证 + 检查

```bash
# 验证 IR 并返回内容哈希标识
cargo run --release -p loom-cli -- verify-l2core program.l2ir
```

### 4. DuckDB 扩展

```bash
cd contrib/duckdb-ext && mkdir -p build && cd build
cmake .. && make -j$(sysctl -n hw.logicalcpu 2>/dev/null || nproc)
```

```sql
LOAD 'contrib/duckdb-ext/build/loom.duckdb_extension';
SELECT * FROM loom_scan('data.parquet');
```

### 5. 测试

```bash
cargo test --workspace
bash scripts/e2e-full.sh
```

## FFI 接口

七个 C ABI 入口点提供完整的 sidecar 生命周期：

```
loom_sidecar_extract       → 读取 sidecar（先外部文件，再内嵌元数据）
loom_sidecar_verify        → 语义验证 + BLAKE3 内容哈希
loom_sidecar_verify_json   → 结构化 facts/diagnostics JSON
loom_sidecar_route         → 4 关路由决策
loom_sidecar_decode         → 完整解码闭环（route → verify → decode）
loom_sidecar_free_cstr     → 释放返回的字符串
loom_sidecar_free_bytes    → 释放返回的字节缓冲
```

## 正确性模型

**三层差分验证**，离线/在线边界清晰。

```
                     ── 离线（构建/CI，固定语料） ──

 L2Core IR 程序
    │
    ├──→ kloom（K 形式化语义引擎）
    │      → krun → 解析 <events> → K trace（规范基准）
    │
    ├──→ Rust 解释器（K 规范的快速实现）
    │      → TracedBuilder → native trace
    │
    └──→ 对比（离线关）
           K trace == 解释器 trace          → 逐事件比对
           解释器输出 == K 输出              → RecordBatch 比对
           不一致 → 解释器 bug，CI 失败

 L2Core IR 程序 + 固定测试语料
    │
    ├──→ Rust 解释器 → 参考输出
    ├──→ JIT（melior/LLVM）→ 原生输出
    │
    └──→ 对比（离线关）
           JIT trace == 解释器 trace        → 逐事件比对
           JIT 输出 == 解释器输出            → RecordBatch 比对
           不一致 → JIT bug，CI 失败


                     ── 在线（生产查询） ──

 宿主数据 + sidecar
    │
    └──→ JIT（melior/LLVM）
           L2Core IR → MLIR → LLVM → 原生机器码
           → Arrow RecordBatch → DuckDB / Spark / Arrow 消费
```

- **kloom**（离线）— K 框架形式化语义，14 个规范测试（14/14 通过）
- **Rust 解释器**（离线验证，不在生产路径中）— K 规范的快速实现，CI 中经 kloom 验证
- **JIT**（离线验证，生产运行时）— melior/LLVM 原生代码生成，CI 中经解释器验证；生产独自运行
- **Lean**（离线，计划中）— IR 程序的形式化分类证明

## 4 关路由

每次 sidecar 读取经过四个 fail-closed 关：

1. 引擎已集成？→ 否 → 回退
2. sidecar 存在？→ 否 → 回退
3. 内容哈希匹配？→ 否 → 回退
4. 编码受支持？→ 否 → 回退
5. 全部通过 → Loom 原生解码

内容哈希使用 **BLAKE3-256**（`blake3:<hex>`），支持防篡改绑定。

## 端到端流程

```
 ┌──────────────────────────────────────────────────────────────────────┐
 │                        数据生产者                                     │
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
              分发 / Iceberg 快照
                          │
 ┌────────────────────────┼────────────────────────────────────────────┐
 │                        │         数据消费者                           │
 │                        ▼                                             │
 │  ┌─────────────────────────────────────────────────────────────┐    │
 │  │                    loom-ffi (C ABI)                          │    │
 │  │                                                              │    │
 │  │  extract ──→ verify ──→ 4-gate route ──→ decode             │    │
 │  │     │           │         ┌───┴───┐           │              │    │
 │  │     │    verify_l2core    │ Loom  │ Host     │              │    │
 │  │     │    + kloom diff     │ 原生  │ 原生     │              │    │
 │  │     │                     │ 解码  │ 回退     │              │    │
 │  │     │                     └───┬───┴─────┬────┘              │    │
 │  │     │                         │         │                   │    │
 │  │     │                    interp/JIT  host reader            │    │
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

Decode IR 是契约：写入时验证一次，读取时重放。sidecar 可剥离——不理解
它的引擎无缝回退到自己的原生 reader。

## 仓库结构

| 路径 | 用途 |
|------|------|
| `crates/loom-ir-core` | 零依赖解码 IR — L2Core 程序模型、sidecar overlay、BLAKE3 内容哈希、4 关路由、验证器 |
| `crates/loom-ffi` | 生产核心 + C ABI — `interp/` Rust 解码器（kloom 验证）、`jit/` melior/LLVM 加速、7 函数 FFI |
| `crates/loom-parquet-ingress` | Parquet 适配器 — sidecar 提取/嵌入、解码 IR 自动生成、chunk binding 计算 |
| `crates/loom-vortex-ingress` | Vortex 适配器 — 真实 Vortex 文件入口 |
| `crates/loom-lance-ingress` | Lance 适配器 — 入口边界（sidecar 暂缓） |
| `crates/loom-source-ingress` | 共享的源无关合约类型 |
| `crates/loom-cli` | 命令行 — `sidecar embed`、`sidecar embed-external`、`verify-l2core` |
| `contrib/duckdb-ext` | C++ DuckDB 扩展（链接 `libloom_ffi.a`） |
| `contrib/kloom` | K 框架 spec-oracle，用于差分验证 |

## 架构

```
Parquet / Lance / Vortex
        │
  .loomsidecar (外部)  或  内嵌元数据
        │
        ▼
  loom-ffi (C ABI)
  提取 → 验证 → 4 关路由 → 解码
        │
   ┌────┴────┐
   ▼         ▼
Loom 原生    宿主原生
   解码        回退
(JIT 默认 →      │
  LLVM 机器码)    │
    │             │
    └─────┬───────┘
          ▼
  DuckDB / Arrow 消费者
```

## 编码

Raw、bitpack、frame-of-reference、dictionary、RLE、FSST、dict-over-FSST、
ALP Float32/Float64。L2Core IR 支持 Boolean、Int32、Int64、Float32、
Float64、Utf8。

## JIT 后端

melior/LLVM JIT 已编译在二进制中，并在线上与解释器对比验证。生产级 JIT
代码生成测试对受支持的 shape 通过。当前 sidecar FFI surface 暴露
extract/verify/route/decode。JIT 在生产路由测试中充分覆盖
（`production_arrow_semantic_*`）。

## 为什么需要 Loom

数据引擎共享了查询计划和列式内存，但没有共享解码器。AnyBlox 预言 Decode IR
是桥梁。Loom 将它实现：一个足够小、可验证、总函数保证终止、Arrow 形状输出
的解码器，让任何宿主引擎无需学习每种源格式就能消费结果。
