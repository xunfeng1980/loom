[English](README.md) | **中文**

# 分发型解码 IR 设计方案

**工作代号:Loom** ·（把字节"织"成 Arrow 列；名字是占位符）

---

## 0. 一句话定位

Loom 是一种**随数据分发的解码器表示**:面向服务端数据引擎,能被廉价静态验证、编译成满速原生码、输出合法 Arrow,并且目标中立、版本稳定到可保存数十年。

它**不是一个更小的 WebAssembly**,而是另一个物种:**一门不通用、非图灵完备的全函数(total function)领域语言,其唯一可能的输出是良构的 Arrow**。它的每一条约束,都是 Wasm / eBPF / LLVM-MLIR 因为选择"能跑任意计算"而付不起的那份自由。

---

## 当前 MVP0 实现

当前仓库实现的是基于解释器的 MVP0,不是下文描述的完整分发型 IR。当前跑通的链路是:

```
in-memory Vortex fixtures -> Loom layout payload -> loom-core interpreter
  -> Arrow C Data Interface -> DuckDB loom_scan(...) -> SQL checks
```

MVP0 支持 bitpack、frame-of-reference、dictionary、RLE、FSST 字符串、dictionary-over-FSST 字符串。当前表路径用 `LMT1` table payload 包装多个单列 layout payload,保留 `LMP1` 单列兼容性,同时让 CLI 和 DuckDB 能扫描具名多列。验收标准是生成 fixture 后,通过 DuckDB SQL 查询得到的行结果和聚合结果都与 Vortex 自身 decoder/oracle 一致。

运行完整 Phase 6 release gate:

```bash
bash scripts/mvp0-verify.sh
```

该 gate 覆盖的底层检查也可以手动运行:

```bash
cargo test --workspace
cargo tree -p loom-core | awk '/vortex|fastlanes/{c++} END{print c+0}'
rg -n 'vortex_file|vortex-file|\.vortex|VortexFile|from_path|read_file' crates/loom-fixtures
bash scripts/duckdb-smoke-test.sh
```

当前 `.loom` payload 格式是 MVP0 内部 fixture 格式。verifier、MLIR/native lowering、Arrow stream ABI、完整 `.vortex` 文件支持都属于后续 milestone。

Phase 7 增加面向 reviewer 的 descriptor 和 CLI 工具:

```bash
cargo run -p loom-fixtures --bin emit_duckdb_payloads
cargo run --bin loom -- inspect target/loom-duckdb-fixtures/bitpack-i32.loom
cargo run --bin loom -- decode target/loom-duckdb-fixtures/fsst-utf8.loom
cargo run -p loom-fixtures --bin loom_fixture_timing
```

timing 命令只输出 Loom interpreter decode 与 Vortex oracle decode 的示意性 wall-clock 数字。它不是 benchmark,也没有速度阈值。

Phase 8 增加一个小型多列表 fixture 和 DuckDB SQL 验收路径:

```bash
cargo run -p loom-fixtures --bin emit_duckdb_payloads
cargo run --bin loom -- inspect target/loom-duckdb-fixtures/mixed-table.loom
cargo run --bin loom -- decode target/loom-duckdb-fixtures/mixed-table.loom
bash scripts/duckdb-smoke-test.sh
```

`mixed-table.loom` 通过 `loom_scan(...)` 暴露 `id INT32`、`flag BOOLEAN`、`label VARCHAR`。扩展当前仍采用直接填充 DataChunk 的路径;ArrowArrayStream 仍是后续 ABI 决策,不是 Phase 8 的实现路径。

---

## 1. 目标与非目标

**目标**

- 把任意列式/半结构化格式(Vortex、ROOT、Parquet、自定义编码……)的解码逻辑,**安全地**(沙箱)、**可移植地**(一份字节码多引擎)、**持久地**(数十年后可读)送到数据引擎门口。
- 在不可信解码器 + 不可信数据的前提下,保证它**炸不穿宿主、挂不死查询**。
- 解码输出固定为 **Apache Arrow**,与宿主零拷贝交接。

**显式非目标(同等重要)**

- **不做通用计算**。写不出 web server、写不出查询引擎。这是力量来源,不是缺陷。
- **不伺候浏览器 / 边缘 / IoT**。目标集只有"服务端数据引擎",于是可以假设 64 位、SIMD、mmap、长驻宿主。
- **不保证正确性**。只保证安全 + 良构(见 §7)。
- **不负责并行**。解码核单线程;并行归宿主(见 §5)。
- **不规定执行后端**。分发规范只定义"travels 的那层";如何编成原生码是每个引擎自己的事(见 §8)。

---

## 2. 在数据系统 IR 版图中的位置

数据系统里有三种彼此正交的"IR 活计":

| 轴 | 代表 | 干什么 | 跨系统? |
|---|---|---|---|
| 运计划 | Substrait | 描述关系计算,让前端/后端自由组合 | 是(交换) |
| **运解码器** | **Loom** | 安全可移植地随数据分发解码逻辑 | **是(随数据)** |
| 引擎内编译 | MLIR / LingoDB | 把查询编译成原生码 | 否(进程内) |

Loom 与另两者**互补,不竞争**。它和执行后端的关系是接力:

```
[随数据走] Loom 分发 IR ──验证──▶ MLIR `decode` dialect ──lower──▶ LLVM IR ──▶ 原生码
   ↑ 必须全新                       ↑ 信任边界之后才轮到 MLIR
   稳定/中立/可验证/沙箱            进程内、可信、贴机器——正是 MLIR 的主场
```

**为什么分发层不能复用 MLIR/LLVM**:编译 IR 与分发 IR 是设计目标相反的两个物种——编译 IR 要表达力无限、向下贴机器、跨版本可变、默认信任输入;分发 IR 要表达力受限、目标中立、跨版本永稳、验证不可信代码。PNaCl 已用 LLVM bitcode 把"编译 IR 当分发格式"这条路血写过一遍。

---

## 3. 核心设计原则:能声明就别写代码

观察:一个真实解码器约 90% 是**结构性布局**(偏移、重复、RLE、字典),只有约 10% 是**真正的计算内核**(FSST 符号表、ALP 指数搜索、解压)。

因此 Loom 是**两层**:

- **L1 声明式布局层** —— 是**数据**,不是代码。零验证(数据无法越界、无法不终止),稳定性最高。引擎据此**自动生成**被向量化的解码读循环。先例:Kaitai Struct、DFDL(证明这种胃口存在),但它们纯声明、不为速度/沙箱分发而设计。
- **L2 全函数内核层** —— 只有声明式表达不了的计算重活才掉进来,被验证、被 lower 成原生。

**原则:能声明的就别写成代码;必须写成代码的,就让它是全函数的。** 这把"需要验证的代码表面积"压到极小——验证负担、攻击面、要永久冻结的语义随之塌缩。

---

## 4. L1:声明式布局层

L1 描述"数据长什么样",是一棵带类型的物理布局树:

- **原始字段**:定宽整数/浮点、varint、定长/变长字节串,带字节序、对齐。
- **重复**:`count` 来自常量、来自另一字段的值(`length-prefixed`)、或来自外层 extent。
- **偏移驱动**:字段位置可由同一记录内其它已解析字段计算(`offset = f(other_fields)`)。
- **声明式编码**:RLE、bit-packing、FOR(frame-of-reference)、dictionary 作为**带参数的内置编码**直接声明(`bitpack(width=11)`、`dict(ref=...)`),无需写代码。
- **逃逸到 L2**:当某段需要自定义 codec,声明一个对 L2 内核的引用(`codec = kernel#3`)。

L1 是纯数据,所以它对验证器是免费的;它也比代码更不易随时间漂移,因而是稳定性最高的一层。引擎拿到 L1,自己合成那个向量化的读循环。

---

## 5. L2:全函数内核层

只在 L1 表达不了时使用。这是一门**故意不通用**的语言。

**5.1 全函数,非图灵完备**

- 无任意递归,无 `while(true)`。
- 迭代只有两种合法形态:
  1. **计数有界**:对 `N` 个元素循环,`N` 是验证器可见的、来自输入/输出 extent 的计数。
  2. **数据单调**:每轮至少消耗 ≥1 字节有限输入,或朝一个已知有界的输出推进。
- 终止性由绑在 **(剩余输入 ‖ 剩余输出)** 上的**递减度量**在**验证期**证明——免费,严格优于运行时 fuel 计数器。
- 对 schema 嵌套结构的递归是**结构有界**的(嵌套深度由 schema 静态决定),不破坏全函数性。

**5.2 数据并行表达成结构,而非具体 SIMD**

- IR 里**禁止出现任何具体向量指令或宽度**。操作描述为"在一个抽象 lane 结构上彼此独立地施加"。
- 物理向量宽度(128/256/512/SVE-可伸缩)的选择**全部下放给引擎的 MLIR 后端**。
- 借 FastLanes"统一虚拟 ISA + 强制自动向量化"的洞见:IR 目标无关(故稳、故可移植),但因并行是**显式结构**,后端不可能漏掉向量化(故快)。这一刀化解了"快 vs 稳"。

**5.3 内存模型**

- `input`:只读 mmap 视图(capability 句柄),整个编码文件,原生 64 位寻址——无 4 GB 上限、无 Memory64 检查税。
- `scratch`:验证器能算出上界的有界工作 arena。
- **无裸输出写**:输出只能经 §6 的 builder 原语。

**5.4 host-call 表面积 = 整个信任接口**

解码器能调用的宿主能力只有两类:**取输入区间**、**申请输出 buffer / 发射批次**。没有文件、没有网络、没有 syscall。这组极小的回调就是全部攻击面,小到可逐行审计。

---

## 6. 输出契约:发射带类型的 Arrow 事件

L2 的输出原语**不是"写内存"**,而是一组**带类型的 builder 操作**:`append_value`、`append_null`、`begin_list`/`end_list`、`begin_struct`/`end_struct` 等。

后果:**输出构造即合法**——offset、null bitmap 与长度的一致性、嵌套类型的 child 数组完整性,全部由 builder 语义保证,验证器对这些**一个字都不用检**。而这堆 builder 原语在 MLIR 后端被**融合、优化成向量化的裸写**。

> 又是同一条分工:**IR 层保证安全,原生层拿回速度。**

输出最终物化为 Arrow C Data Interface 的 `ArrowArray` / `ArrowSchema`,与宿主零拷贝交接。

---

## 7. 安全边界:保证安全与良构,**不**保证正确

**验证器证明的义务**

- 内存安全:所有访问落在声明区域内,无任意指针算术。
- capability-only:无 syscall、无环境权限。
- 全函数性:递减度量保证终止(编译期)。
- 输出良构:经 builder 构造 + schema 类型检查。

**验证器不证明的**:**正确性**。一个恶意/有 bug 的解码器完全可以安全地、良构地产出**数据全错**的 Arrow。这与今日原生 reader 同等(原生 reader 也会解错),故划到范围外。但要清醒:

> 自解码让你免于"解码器会不会炸穿进程",**不**让你免于"解码器作者是否靠谱"。

补正确性只能靠正交手段(对输出算校验等),而你通常没有独立第二份解码可对照——这条基本无解,只能接受。

---

## 8. 执行:经 MLIR lower 到原生

分发形态(Loom)→ 引擎内一个 `decode` MLIR dialect:

- 把 L1 合成的读循环、以及 L2 的解码原语(bit-unpack、FOR、delta、dict、FSST、ALP……)表达成 MLIR op。
- lower 到 LLVM IR → 原生码,**这一步才挑物理 SIMD 宽度**,并复用 MLIR 现成的 CSE、常量折叠、自动向量化 pass(LingoDB 已证明这条可行)。
- §6 的 builder 事件在此被融合成向量化裸写。

分发形态的任何细节都**不**泄漏到目标机器;目标相关性只存在于 lower 之后。**信任边界 = Loom 与 MLIR 的接缝**:边界之前一切不能是 MLIR,边界及之后才轮到 MLIR。

---

## 9. ABI:解码器入口

```
schema()                                              -> ArrowSchema
decode_batch(input, range, projection_mask, state)    -> ArrowArray
statistics(input, range)                              -> ColumnStats   // 可选
```

- `range`:行区间,给随机访问。
- `projection_mask`:列裁剪,只解需要的列。
- `state`:显式、自有的状态,给有跨记录依赖的格式(如 ROOT 的帧间依赖)。
- `statistics`:返回每列 min/max/null-count,供引擎跳过整段——谓词的可移植表达接 **Substrait**。

---

## 10. 分发、信任与快路

**分发产物** = 一个版本化容器:`{ schema, L1 布局描述, L2 内核模块, feature flags, (可选)多档内核 }`。

- **随数据走**(自解码),或由**内容哈希 URI** 引用。
- **混合快路**:哈希命中宿主已审计的知名格式 → 直接用宿主**原生实现**,跳过验证/JIT(沿用 AnyBlox 的 decoder-URI + 校验和机制)。
- 验证器是安全边界;签名/远程证明可选,不是边界本身。

---

## 11. 版本演化与持久性

- **冻结一个极小、永不改变的核心** + header 里的 **feature flags** 声明本解码器用了哪些特性。
- 老引擎遇到不认识的特性 → **干净地拒绝**,绝不乱执行;新引擎对老数据**永远向后兼容**。
- **独门武器(代码随数据走反送的礼物)**:同一解码器可把**多档实现**(baseline + 激进优化版)一起捆进容器,引擎挑它能看懂的最高档跑。系统内嵌的格式做不到这件事。

---

## 12. 与既有方案的对位

详细对比文档：[.planning/research/POSITIONING.md](.planning/research/POSITIONING.md)。

| | 分发可移植 | 不可信沙箱 | 全函数(可证终止) | 原生满速 | 目标中立/版本稳定 | 强制 Arrow 输出 |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| Wasm / AnyBlox | ✓ | ✓ | ✗(图灵完备,靠 fuel) | △(~1.5x 沙箱税) | △ | ✗ |
| eBPF / uBPF | △ | ✓ | ✓(但限制过死) | ✓ | ✗ | ✗ |
| LLVM IR / 裸原生 | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ |
| MLIR / LingoDB | ✗(编译内部) | ✗ | ✗ | ✓ | ✗ | △ |
| Substrait | ✓(运计划) | n/a | n/a | n/a | ✓ | n/a |
| **Loom** | **✓** | **✓** | **✓** | **✓(验证期付税)** | **✓** | **✓** |

Loom 能同时拿满,根本原因只有一个:**它敢于不通用**。其余方案都为"能跑任意计算"付了代价——验证难、语义大、或不可移植。

---

## 13. 诚实留在桌上的硬骨头

1. **验证器与 JIT 自身进了 TCB**。IR 小且结构化,使"形式化验证过的验证器"可做(eBPF 验证器出过 CVE 是反例),但这是真功夫,不免费。
2. **正确性那道缝补不上**(§7),只能接受。
3. **谁来冻结 v1、且冻得足够对?** 一个号称"永不破坏"的格式,第一版就得几乎不留遗憾——最反人性的工程要求,也是最可能死在起跑线的地方。
4. **采纳困境**。LingoDB 证明雄心勃勃的、基于 IR 的数据基础设施**能被造出来并扩展**,但它是**引擎内部**的游戏(只需说服自己);Loom 是**跨系统交换**的游戏(要说服每个引擎采纳共享格式 + 接受不可信威胁模型),难度高一个量级。

> 结论与历史规律一致:Loom 不会因为"它正确"而被造出来,只会被某个**自带宿主的 MPP 引擎**——被不可信数据/格式爆炸逼到非造不可时——造出来,然后其余人搭便车;就像 WebAssembly 当年从 PNaCl 的坟头上站起来那样。在那之前,它会一直显得"养不活"。

---

## 14. 一段话总纲

Loom 把自己锁死成"**吃有限字节、发射良构 Arrow 的全函数语言**":能声明的走 L1(数据,零验证),必须算的走 L2(全函数,验证期证终止),并行表达成抽象 lane 结构而非具体 SIMD,输出经 typed builder 故构造即合法。于是它同时拿到**小、稳、可廉价验证、永远可移植**;而把"满速"通过下沉到引擎内的 MLIR `decode` dialect 拿回来。信任边界恰好落在 Loom 与 MLIR 的接缝上:**Loom 负责安全可移植持久地把解码逻辑送到门口,MLIR 负责进门后编成原生码。**
