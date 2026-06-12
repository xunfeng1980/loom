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

所有命令在项目根目录执行，可直接复制粘贴。

### 1. 编译

```bash
cargo build --release -p loom-cli
```

### 2. 查看示例 Decode IR

```bash
cat assets/quickstart.ron
```

这是一个 5 行的程序：每行向输出追加 `Int32(42)`。
可编辑 `assets/quickstart.ron` 或自行编写。

### 3. 转换并验证

```bash
cargo run --release -p loom-cli -- convert assets/quickstart.ron assets/quickstart.l2ir
cargo run --release -p loom-cli -- verify-l2core assets/quickstart.l2ir
```

### 4. 嵌入为外部 sidecar

```bash
cargo run --release -p loom-cli -- sidecar embed-external assets/data.parquet assets/quickstart.l2ir
```

生成 `assets/data.parquet.loomsidecar`。原始 `assets/data.parquet` 不变。

### 5. 运行 DuckDB 扩展

编译扩展：

```bash
cd contrib/duckdb-ext && mkdir -p build && cd build && cmake .. && make -j$(sysctl -n hw.logicalcpu) && cd ../../..
```

启动 DuckDB（内置 CLI，无需额外安装）：

```bash
./contrib/duckdb-ext/vendor/duckdb-cli/duckdb -unsigned \
  -c "LOAD '$(pwd)/contrib/duckdb-ext/build/loom.duckdb_extension'; SELECT * FROM loom_scan('$(pwd)/assets/data.parquet');"
```

> 解码后的数据是真实 Arrow 列——DuckDB 可对其执行任意 SQL，包括聚合、过滤、连接。
> Sidecar 对查询引擎完全透明。

尝试复杂查询：

```sql
SELECT out_name, COUNT(*), MIN(out_id), MAX(out_count), AVG(out_score)
FROM loom_scan('$(pwd)/assets/data.parquet')
GROUP BY out_name
ORDER BY out_name;
```

```sql
SELECT * FROM loom_scan('$(pwd)/assets/data.parquet')
WHERE out_ratio > 2.0 AND out_flag = true;
```

### 6. 运行测试

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
    └──→ L2Core 解释器（loom_sidecar_decode）
           extract → verify → 4 门路由 → 解释执行
           → Arrow RecordBatch（IPC + C Data Interface）
           → DuckDB / Spark / Arrow 消费
```

- **kloom**（离线）— K 框架形式化语义，14 个规范测试（14/14 通过）
- **Rust 解释器**（生产解码器）— 通用 L2Core 解释器（`interp/l2core_interp.rs`）
  接进 `loom_sidecar_decode`，CI 中经 kloom 验证。从自动生成的 IR 解码
  i32/i64/f32/f64/bool、可空、Utf8 列，产出真实 Arrow（`StreamWriter` IPC +
  `arrow::ffi` C Data Interface），由 DuckDB `loom_scan` 表函数物化为 typed 行。
- **JIT**（离线验证，尚未接入生产 FFI）— melior/LLVM 原生代码生成，CI 中经解释器
  对比验证。decode-chain 工作让解释器成为生产运行时；把 sidecar FFI 路由到 JIT 是后续工作。
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
(L2Core 解释器)  │
    │             │
    │             │
    └─────┬───────┘
          ▼
  DuckDB / Arrow 消费者
```

## 编码

Raw、bitpack、frame-of-reference、dictionary、RLE、FSST、dict-over-FSST、
ALP Float32/Float64。L2Core IR 支持 Boolean、Int32、Int64、Float32、
Float64、Utf8。

## 解码运行时与 JIT

生产解码运行时是 **L2Core 解释器**：`loom_sidecar_decode` 提取 sidecar、验证 IR、
评估 4 门路由，并在 Loom 原生路径上解释执行程序得到真实 Arrow `RecordBatch`，
以裸 IPC 流和 Arrow C Data Interface 返回；DuckDB `loom_scan` 表函数把这些列物化
为 typed SQL 行。自动生成的 IR（`generate_decode_ir_from_parquet`）覆盖非空
i32/i64/f32/f64/bool、可空定宽、以及非空 Utf8 列。

melior/LLVM **JIT** 已编译在二进制中，并在 CI 中与解释器离线对比验证
（`production_arrow_semantic_*`），但**尚未接入生产 sidecar FFI**——把解码路由到
JIT 是后续工作。DuckDB 扩展链接的 `libloom_ffi.a` 以 `--no-default-features` 构建，
因此 JIT/LLVM 不进入可加载扩展。

## 为什么需要 Loom

数据引擎共享了查询计划和列式内存，但没有共享解码器。AnyBlox 预言 Decode IR
是桥梁。Loom 将它实现：一个足够小、可验证、总函数保证终止、Arrow 形状输出
的解码器，让任何宿主引擎无需学习每种源格式就能消费结果。
