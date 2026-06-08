[English](README.md) | **中文**

# 分发式解码 IR 设计方案

**工作代号:Loom** ·（把字节“织”成 Arrow 列；名称暂定）

---

## 0. 一句话定位

Loom 是一种**随数据一起分发的解码器表示**:面向服务端数据引擎,可以低成本静态验证,可以编译为高性能原生代码,输出合法 Arrow,并且保持目标平台中立和长期版本稳定。

它**不是一个更小的 WebAssembly**,而是另一类东西:**一门非通用、非图灵完备的全函数(total function)领域语言,唯一输出是良构的 Arrow**。这些限制正是它的价值来源:Wasm / eBPF / LLVM-MLIR 因为要支持任意计算,无法同时获得这么小的语义边界、验证边界和长期稳定性。

---

## 当前 MVP1 实现与 MVP0/MVP1 范围

当前仓库已经越过 MVP0,现在处于 **MVP1 / v3 分发与验证阶段**。下文完整设计仍是长期目标;当前实现范围如下:

| 阶段 | 状态 | 已覆盖 | 边界 |
|---|---|---|---|
| MVP0 | 已完成 | `LMP1` 单列 layout payload、Rust interpreter、Arrow C Data Interface、DuckDB `loom_scan(...)` SQL 验收;覆盖 bitpack/FOR/dict/RLE/FSST/dict-over-FSST | 不包含分发容器、完整 verifier、真实 Vortex ingress、MLIR/native lowering 或原生速度声明 |
| MVP1 | 当前已推进到 Phase 22 | `LMT1` 多列、ALP Float32/Float64、失败即关闭的 verifier、`LMC1` container、Safety Proof MVP、Full Verifier foundation、textual MLIR spike、窄范围 native backend evidence、统一 artifact verifier、完整 Vortex reader facts/emission、Bitwuzla-backed SMT discharge evidence、第一版 verifier-gated production native-lowering surface seed、带明确 lowering disposition 的真实 Vortex expanded coverage matrix,以及 host-neutral runtime ABI/policy model | Phase 22 不是任意 Vortex encoding/layout 支持;任意 Vortex 语义兼容性已预留给 Phase 28。当前也不包含稳定外部 `L2Core` codec、host-engine native execution、checked proof objects、remote/object-store ingress、签名/attestation、完整语义正确性证明、冻结完整 MLIR dialect 或 production JIT execution |

验收边界:generated fixtures 的 DuckDB SQL 行结果和聚合结果必须与 oracle 一致;curated negative verifier/container/artifact-verifier/safety/full-verifier/native-lowering/ingress/backend case 必须在产生成功输出前 fail closed。

形式化工具边界:full-verifier gate 必须实际运行 Lean 和 TLC。使用
`mise install && mise run formal-tools`;缺失形式化工具视为失败,不能当作 skipped evidence。

外部 backend/solver 工具边界:LLVM/MLIR 22 和 Bitwuzla 通过
`mise run external-tools` 管理,macOS 下由 Homebrew 安装。release gate 默认要求这些工具存在;只有显式配置 `LOOM_ALLOW_NATIVE_TOOL_SKIP=1` 或 `LOOM_ALLOW_SOLVER_SKIP=1` 才允许跳过,不能因为工具缺失而自动 skip。

---

## 1. 目标与非目标

**目标**

- 把任意列式/半结构化格式(Vortex、ROOT、Parquet、自定义编码……)的解码逻辑,**安全地**(沙箱)、**可移植地**(一份表示可被多引擎消费)、**持久地**(数十年后仍可读取)交给数据引擎。
- 在不可信解码器 + 不可信数据的前提下,保证它**不能破坏宿主进程,也不能无限挂起查询**。
- 解码输出固定为 **Apache Arrow**,与宿主零拷贝交接。

**显式非目标(同等重要)**

- **不做通用计算**。写不出 web server、写不出查询引擎。这是力量来源,不是缺陷。
- **不面向浏览器 / 边缘 / IoT**。目标场景只有“服务端数据引擎”,因此可以假设 64 位、SIMD、mmap 和长驻宿主进程。
- **验证器不单独承诺语义正确性**。Loom 保证安全 + 良构;数据是否按格式语义被正确解码,需要 oracle/等价性测试、签名、校验等正交机制补足(见 [§7](#section-7))。
- **不负责调度并行**。解码核按单线程语义建模;并行切分、调度和线程所有权归宿主(见 [§5](#section-5))。
- **不规定执行后端**。分发规范只定义“随数据分发的那一层”;如何编译成原生码由各个引擎自行决定(见 [§8](#section-8))。

---

## 2. 在数据系统 IR 版图中的位置

数据系统里有三类彼此正交的 IR 职责:

| 轴 | 代表 | 职责 | 跨系统? |
|---|---|---|---|
| 查询计划分发 | Substrait | 描述关系计算,让前端和后端自由组合 | 是(交换) |
| **解码器分发** | **Loom** | 安全、可移植地随数据分发解码逻辑 | **是(随数据)** |
| 引擎内编译 | MLIR / LingoDB | 把查询或执行片段编译成原生码 | 否(进程内) |

Loom 与另两者**互补,不竞争**。它和执行后端的关系是接力:

```
[随数据分发] Loom 分发 IR ──验证──▶ MLIR `decode` dialect ──lower──▶ LLVM IR ──▶ 原生码
      ↑ 必须独立设计                 ↑ 信任边界之后才进入 MLIR
      稳定/中立/可验证/沙箱          进程内、可信、贴近硬件——这是 MLIR 的主场
```

**为什么分发层不能复用 MLIR/LLVM**:编译 IR 与分发 IR 的设计目标相反。编译 IR 需要强表达力、贴近硬件、跨版本可演化,并且通常默认信任输入;分发 IR 需要表达力受限、目标中立、跨版本长期稳定,并能验证不可信输入。PNaCl 曾经尝试把 LLVM bitcode 当作分发格式,其历史教训说明这条路风险很高。

---

## 3. 核心设计原则:能声明就别写代码

观察:一个真实解码器约 90% 是**结构性布局**(偏移、重复、RLE、字典),只有约 10% 是**真正的计算内核**(FSST 符号表、ALP 指数搜索、解压)。

因此 Loom 是**两层**:

- **L1 声明式布局层** —— 是**数据**,不是代码。验证成本极低(数据本身不会越界、也不会不终止),稳定性最高。引擎据此**自动生成**可向量化的解码读循环。先例包括 Kaitai Struct、DFDL(说明这种需求真实存在),但它们主要是声明式描述,并不是为高性能沙箱化分发而设计。
- **L2 全函数内核层** —— 只有声明式表达不了的计算逻辑才进入这一层,并在验证后 lower 成原生代码。

**原则:能声明的就不要写成代码;必须写成代码的,就让它是全函数的。** 这样可以把“需要验证的代码表面积”压到极小,同时显著降低验证负担、攻击面和需要长期冻结的语义规模。

---

## 4. L1:声明式布局层

L1 描述“数据如何物理组织”,是一棵带类型的物理布局树:

- **原始字段**:定宽整数/浮点、varint、定长/变长字节串,带字节序、对齐。
- **重复**:`count` 来自常量、来自另一字段的值(`length-prefixed`)、或来自外层 extent。
- **偏移驱动**:字段位置可由同一记录内其它已解析字段计算(`offset = f(other_fields)`)。
- **声明式编码**:RLE、bit-packing、FOR(frame-of-reference)、dictionary 作为**带参数的内置编码**直接声明(`bitpack(width=11)`、`dict(ref=...)`),无需写代码。
- **进入 L2**:当某段需要自定义 codec,声明一个对 L2 内核的引用(`codec = kernel#3`)。

L1 是纯数据,所以验证成本极低;它也比代码更不容易随时间漂移,因此是稳定性最高的一层。引擎拿到 L1 后,自行合成向量化的读循环。

---

<a id="section-5"></a>

## 5. L2:全函数内核层

只在 L1 表达不了时使用。这是一门**故意不通用**的语言。

**5.1 全函数,非图灵完备**

- 无任意递归,无 `while(true)`。
- 迭代只有两种合法形态:
  1. **计数有界**:对 `N` 个元素循环,`N` 是验证器可见的、来自输入/输出 extent 的计数。
  2. **数据单调**:每轮至少消耗 ≥1 字节有限输入,或朝一个已知有界的输出推进。
- 终止性由绑定在 **(剩余输入 ‖ 剩余输出)** 上的**递减度量**在**验证期**证明,不需要运行时 fuel 计数器。
- 对 schema 嵌套结构的递归是**结构有界**的(嵌套深度由 schema 静态决定),不破坏全函数性。

**5.2 数据并行表达成结构,而非具体 SIMD**

- IR 里**禁止出现任何具体向量指令或宽度**。操作描述为"在一个抽象 lane 结构上彼此独立地施加"。
- 物理向量宽度(128/256/512/SVE-可伸缩)的选择**全部下放给引擎的 MLIR 后端**。
- 借 FastLanes"统一虚拟 ISA + 强制自动向量化"的洞见:IR 目标无关(故稳、故可移植),但因并行是**显式结构**,后端不可能漏掉向量化(故快)。这一刀化解了"快 vs 稳"。

**5.3 内存模型**

- `input`:只读 mmap 视图(capability 句柄),整个编码文件,原生 64 位寻址——无 4 GB 上限、无 Memory64 检查税。
- `scratch`:验证器能算出上界的有界工作 arena。
- **无裸输出写**:输出只能经 [§6](#section-6) 的 builder 原语。

**5.4 host-call 表面积 = 整个信任接口**

解码器能调用的宿主能力只有两类:**读取输入区间**、**申请输出 buffer / 产出批次**。没有文件、没有网络、没有 syscall。这组极小的回调就是全部攻击面,小到可以逐行审计。

---

<a id="section-6"></a>

## 6. 输出契约:产出带类型的 Arrow 事件

L2 的输出原语**不是“写内存”**,而是一组**带类型的 builder 操作**:`append_value`、`append_null`、`begin_list`/`end_list`、`begin_struct`/`end_struct` 等。

结果是:**输出在构造时即保持合法**。offset、null bitmap 与长度的一致性,以及嵌套类型 child 数组的完整性,都由 builder 语义保证,验证器无需重新证明这些结构约束。后端再把这些 builder 原语融合、优化成向量化的直接写入。

> 分工仍然相同:**IR 层保证安全,原生层负责性能。**

输出最终物化为 Arrow C Data Interface 的 `ArrowArray` / `ArrowSchema`,与宿主零拷贝交接。

---

<a id="section-7"></a>

## 7. 安全边界:保证安全与良构,语义正确性走正交验证

**验证器证明的义务**

- 内存安全:所有访问落在声明区域内,无任意指针算术。
- capability-only:无 syscall、无环境权限。
- 全函数性:递减度量保证终止(编译期)。
- 输出良构:经 builder 构造 + schema 类型检查。

**验证器不单独证明的**:**语义正确性**。一个恶意或有 bug 的解码器完全可以安全地、良构地产出**内容错误**的 Arrow。这与今天的原生 reader 一样(原生 reader 也可能解错),因此不能只靠 Loom verifier 解决。但必须明确:

> 自解码让你免于“解码器是否会破坏进程”,但**不**让你免于“解码器作者是否可信”。

语义正确性需要通过正交机制补足,例如 oracle/等价性测试、签名、校验和、格式级证明义务或独立实现对照。很多真实场景没有独立的第二份解码结果可比对,所以这里不把语义正确性作为 Loom verifier 的单独承诺。

---

<a id="section-8"></a>

## 8. 执行:经 MLIR lower 到原生

分发形态(Loom)→ 引擎内一个 `decode` MLIR dialect:

- 把 L1 合成的读循环、以及 L2 的解码原语(bit-unpack、FOR、delta、dict、FSST、ALP……)表达成 MLIR op。
- lower 到 LLVM IR → 原生代码,**这一步才选择物理 SIMD 宽度**,并复用 MLIR 现成的 CSE、常量折叠、自动向量化 pass(LingoDB 已证明这条路径可行)。
- [§6](#section-6) 的 builder 事件在此被融合成向量化直接写入。

分发形态的任何细节都**不**绑定到目标机器;目标相关性只存在于 lower 之后。**信任边界 = Loom 与 MLIR 的边界**:边界之前不能使用 MLIR 作为不可信分发格式,边界及之后才进入 MLIR。

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

## 10. 分发、信任与快速路径

**分发产物** = 一个版本化容器:`{ schema, L1 布局描述, L2 内核模块, feature flags, (可选)多档内核 }`。

- **随数据走**(自解码),或由**内容哈希 URI** 引用。
- **混合快速路径**:哈希命中宿主已审计的知名格式 → 直接使用宿主**原生实现**,跳过验证/JIT(沿用 AnyBlox 的 decoder-URI + 校验和机制)。
- 验证器是安全边界;签名/远程证明可选,不是边界本身。

---

## 11. 版本演化与持久性

- **冻结一个极小、永不改变的核心** + header 里的 **feature flags** 声明本解码器用了哪些特性。
- 老引擎遇到不认识的特性 → **干净地拒绝**,绝不乱执行;新引擎对老数据**永远向后兼容**。
- **分发式解码器的特殊能力**:同一解码器可以把**多档实现**(baseline + 激进优化版)一起打包进容器,引擎选择它能理解的最高档执行。系统内置格式通常做不到这一点。

---

## 12. 与既有方案的对位

详细对比文档：[.planning/research/POSITIONING.md](.planning/research/POSITIONING.md)。

| | 分发可移植 | 不可信沙箱 | 全函数(可证终止) | 原生性能 | 目标中立/版本稳定 | 强制 Arrow 输出 |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| Wasm / AnyBlox | ✓ | ✓ | ✗(图灵完备,靠 fuel) | △(~1.5x 沙箱税) | △ | ✗ |
| eBPF / uBPF | △ | ✓ | ✓(但限制过死) | ✓ | ✗ | ✗ |
| LLVM IR / 裸原生 | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ |
| MLIR / LingoDB | ✗(编译内部) | ✗ | ✗ | ✓ | ✗ | △ |
| Substrait | ✓(查询计划分发) | n/a | n/a | n/a | ✓ | n/a |
| **Loom** | **✓** | **✓** | **✓** | **✓(验证期付税)** | **✓** | **✓** |

Loom 能同时满足这些约束,根本原因只有一个:**它刻意不做通用计算**。其余方案都为“能运行任意计算”付出了代价:验证困难、语义过大,或不可移植。

---

## 13. 仍需正视的困难

1. **验证器与 JIT 自身进入 TCB**。IR 小且结构化,使“形式化验证过的验证器”成为可能(eBPF verifier 出过 CVE 是反例提醒),但这需要严肃工程投入。
2. **语义正确性不由验证器单独承诺**([§7](#section-7)),必须通过正交机制处理。
3. **谁来冻结 v1,且冻结得足够正确?** 一个声称“长期兼容”的格式,第一版就必须非常谨慎。这是最困难的工程要求之一,也是最可能失败的地方。
4. **采纳困境**。LingoDB 证明雄心勃勃的、基于 IR 的数据基础设施**可以被构建并扩展**,但它是**引擎内部**系统(只需说服自己);Loom 是**跨系统交换**系统(需要说服多个引擎采纳共享格式并接受不可信输入威胁模型),难度更高。

> 结论与历史规律一致:Loom 不会仅仅因为“设计正确”而自然出现。更现实的路径是:某个自带宿主的 MPP 引擎被不可信数据和格式爆炸逼到必须解决这个问题,先把它造出来,随后其他系统复用这条路径。WebAssembly 从 PNaCl 的经验中演化出来,就是类似的例子。

---

## 14. 一段话总纲

Loom 把自身限定为“**消费有限字节、产出良构 Arrow 的全函数语言**”:能声明的走 L1(数据,低验证成本),必须计算的走 L2(全函数,验证期证明终止),并行性表达成抽象 lane 结构而非具体 SIMD,输出通过 typed builder 构造从而天然保持合法。这样它获得了**小语义、稳定性、低成本验证和长期可移植性**;而性能则交给引擎内的 MLIR `decode` dialect 和后续原生编译来获得。信任边界清晰落在 Loom 与 MLIR 之间:**Loom 负责安全、可移植、持久地分发解码逻辑,MLIR 负责在可信引擎内部将其编译成原生代码。**
