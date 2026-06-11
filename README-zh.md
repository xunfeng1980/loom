[English](README.md) | **中文**

<p align="center">
  <img src="assets/loom-logo-minimal.svg" width="180" alt="Loom logo">
</p>

# Loom

Loom 是一个**面向分发的解码器 IR**：一门故意受限、非图灵完备的总函数语言，其唯一可能的输出是符合规范的 Apache Arrow。

集成模式是 **sidecar overlay**：将 Loom IR 程序作为可剥离的元数据嵌入到已有的 Parquet、Lance 或 Vortex 文件中。理解 Loom 的引擎走可验证的原生快速路径；不理解的引擎继续用自己的原生 reader 读取。

## 快速开始

### 1. 编译 CLI

```bash
cargo build -p loom-cli --release
```

### 2. 将 sidecar 嵌入 Parquet 文件

```bash
cargo run -p loom-cli -- sidecar embed data.parquet [program.l2ir]
```

这会添加 `loom.sidecar.v1` KeyValue 元数据，原始数据不变。

### 3. 编译 DuckDB 扩展

```bash
cd contrib/duckdb-ext && mkdir -p build && cd build
cmake .. && make -j$(sysctl -n hw.logicalcpu 2>/dev/null || nproc)
```

### 4. 查询

```sql
LOAD 'contrib/duckdb-ext/build/loom.duckdb_extension';
SELECT * FROM loom_scan('data.parquet');
```

### 5. 运行测试

```bash
cargo test --workspace
bash scripts/sidecar-overlay-test.sh
```

## 正确性模型

```
kloom (K 语义 + krun) ──离线差分──→ Rust 解码器 (ground truth)
                                          │
                                    JIT (melior/LLVM) ──在线对比──→ 解码器输出
                                          │
                                    一致？→ JIT 结果
                                    不一致？→ 丢弃，回退宿主 native reader
```

- **kloom** — 离线，K 形式化语义 + krun 执行，trace 逐条对齐，证明解码器实现正确
- **解码器** — 在线，纯 Rust L1/L2 解码器，被 kloom 验证过的 ground truth
- **JIT** — 在线，每次执行后和解码器逐行对比，不一致则丢弃

## 仓库结构

| 路径 | 用途 |
|------|------|
| `crates/loom-ir-core` | 零依赖解码 IR — L2Core 程序模型、sidecar overlay、内容哈希标识、4 关路由、验证器 |
| `crates/loom-ffi` | 生产核心 + C ABI — `interp/` Rust 解码器（kloom 验证）、`jit/` melior/LLVM 加速、sidecar C ABI |
| `crates/loom-parquet-ingress` | Parquet 入口适配器 — 通过 KeyValue 元数据实现 sidecar 提取/嵌入 |
| `crates/loom-vortex-ingress` | Vortex 入口适配器 + oracle 测试夹具 |
| `crates/loom-lance-ingress` | Lance 入口适配器（sidecar 暂缓 — 7.0.0 manifest 缺乏可写元数据） |
| `crates/loom-source-ingress` | 共享的源无关合约类型 |
| `crates/loom-cli` | 命令行工具 — `sidecar embed`、`verify-l2core` |
| `contrib/duckdb-ext` | C++ DuckDB 扩展（链接 `libloom_ffi.a`） |
| `contrib/kloom` | K 框架 spec-oracle，用于差分验证 |

## 架构

```
Parquet / Lance / Vortex
        │
        ▼
  loom-ffi (C ABI)
  提取 → 验证 → 4 关路由
        │
   ┌────┴────┐
   ▼         ▼
Loom 原生    宿主原生
  解码        回退
(JIT 加速       │
 --features     │
  melior)       │
   │           │
   └─────┬─────┘
         ▼
  DuckDB / Arrow 消费者
```

**4 关路由：**
1. 引擎已集成？→ 否 → 回退
2. sidecar 存在？→ 否 → 回退
3. 内容哈希匹配？→ 否 → 回退
4. 编码受支持？→ 否 → 回退
5. 全部通过 → Loom 原生解码

安全模型：每关都是 fail-closed。sidecar 可剥离 — 未知的 `loom.*` 键会被标准 reader 静默忽略。如果 Loom 失败，文件仍然是有效的 Parquet/Lance/Vortex。

**JIT 加速：** Loom 原生解码将 L2Core IR → MLIR → LLVM → 本地机器码，通过 melior 编译执行以达到最高速度。JIT 编译失败或输出校验不匹配时回退到纯 Rust 解释器。

## 编码

Raw、bitpack、frame-of-reference、dictionary、RLE、FSST、dict-over-FSST、
ALP Float32/Float64。

## 为什么需要 Loom

数据引擎已经共享了查询计划和列式内存，但还没有共享解码器。Loom 把解码器做得够小、够可验证、够 Arrow 化，让宿主引擎不需要学习每种源格式就能消费结果。
