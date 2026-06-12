# Phase: 生产解码链打通（sidecar decode → 真实 Arrow IPC，完全类型覆盖）

> 状态：执行中（绕过 GSD 直接产出，非 .planning/PLAN.md 格式）
> 产出日期：2026-06-12（修订：Plan 1 升级为通用解释器，Plan 3 改 tier 阶梯，目标=完全覆盖）
>
> **实施进度（2026-06-12）：**
> - **Plan 1 ✅ 完成**：通用 L2Core 解释器 [`l2core_interp.rs`](../crates/loom-ffi/src/interp/l2core_interp.rs) 落地并接进 `loom_sidecar_decode` 的 LoomNative 分支；吞掉 i32 硬路径（等价回归测试通过）；LMA1 路径标注为离线 oracle + interp-vs-LMA1 差分测试。109 lib 测试 + 集成全绿。
> - **Plan 2 ✅ 完整完成（含 typed-row 物化，已实跑端到端 SQL）**：
>   - `loom_sidecar_decode` 产出**真实 bare Arrow IPC**（`StreamWriter`）；新增 `loom_sidecar_decode_carray` 经 **Arrow C Data Interface**（`arrow::ffi::to_ffi` 导出 struct 数组）零拷贝交给宿主。E2E FFI 测试 [`sidecar_decode_ffi.rs`](../crates/loom-ffi/tests/sidecar_decode_ffi.rs)：IPC 经 `StreamReader` 读回正确 + carray 经 `from_ffi` 往返正确 + `free_bytes` 释放。loom.h 契约更新（裸 IPC）。
>   - **DuckDB 扩展端到端跑通**：JIT 经 cargo feature 门控（`--no-default-features`），扩展无 LLVM 符号、可被仓库自带 `vendor/duckdb-cli/duckdb`（v1.5.3）`LOAD`。`loom_scan` 现把解码列**物化为 DuckDB typed 行**（i32/i64/f32/f64/bool/utf8 泛型 `FillVector`，未知类型 fail-soft 回退诊断列）。实测：`SELECT * FROM loom_scan('<fixture>')` 返回 10 行 int32=42；`SELECT COUNT/SUM/MIN` → 10/420/42。fixture 生成器 [`examples/make_fixture.rs`](../crates/loom-ffi/examples/make_fixture.rs)。
>   - **DoD#2 达成**：SQL 查出真实解码值，端到端 DuckDB SQL → 解释器 → Arrow C 接口 → typed 结果行。
> - **Plan 3 ⏳ Tier 1a 完成 / Tier 1b 待 IR 扩展**：
>   - **Tier 1a ✅（非空 i32/i64）**：`generate_decode_ir_from_parquet` 产真实可执行 `body`（ForRange+ReadInput+AppendValue），`parquet_to_raw_host` 按列主序 LE 打包对应 host 缓冲。E2E 测试 [`decode_ir_gen_tier1.rs`](../crates/loom-ffi/tests/decode_ir_gen_tier1.rs)：parquet→自动 IR（过 full verifier）→解释器复现源 i32/i64 值；f32 列正确跳过；nullable+Utf8 仅发非空整型列。
>   - **关键发现 → Tier 1b（f32/f64/bool）需先扩展 IR**：full verifier 把 `ReadInput` 值类型**仅按字节宽推断**（4→Int32, 8→Int64）且要求 `AppendValue` 与 builder 类型精确匹配，无 bitcast/typed-read。浮点/布尔因此无法在当前 IR 表达——需一次 loom-ir-core 扩展（`ReadInput` 带类型或新增 bitcast ScalarExpr，连带 codec + full_verifier + kloom + 解释器），即 Tier 1b 工作量。
>   - 注：DuckDB 扩展物化层已支持全部 6 种 primitive + utf8，Tier 1b/2/3 一旦 IR 产出真实 body 即可直接消费。
> - **Plan 4/5**：未开始。
> 范围：把前次分析定位的五项未完成项收敛为一个 phase、拆成 5 个有依赖序的 plan。
> **终态目标：全类型覆盖**——i32/i64/f32/f64/bool + nullable + Utf8 + 字典，端到端经 sidecar 路径解码出真实 Arrow。i32 只是第一个打通用纵切片，不是终点。

---

## 0. 根因、合并方向与单一杠杆点

五项不是五个孤立缺口，而是同一条断链的切面：

> **生产 FFI 入口 [`loom_sidecar_decode`](../crates/loom-ffi/src/ffi.rs#L503) 的 `LoomNative` 分支不执行、不输出**——
> 它解码 IR、校验哈希、跑完 4 门路由后，到该解码那一步直接 [`let ipc_output: Vec<u8> = Vec::new();`](../crates/loom-ffi/src/ffi.rs#L580)，且该函数**当前无任何 caller**（仅 [loom.h:156](../crates/loom-ffi/include/loom.h#L156) 声明）。

打通这一点，①②④自动获得实质支撑；③⑤是其上的纵深与对齐工作。

### 关键架构真相（必须先认清，否则会"假合并"）

库里有**两套并行 decode 机器**，且代码已写明它们的命运：

| 层 | 文件 | 已支持 | 性质 |
|---|---|---|---|
| 底层 builder | [arrow_builder_output.rs:78-83](../crates/loom-ffi/src/interp/arrow_builder_output.rs#L78) | Bool/i32/i64/f32/f64/**Utf8** + **AppendNull** | 原语，6 类型 + null 已齐 |
| L2 kernel | [l2_kernel_registry.rs](../crates/loom-ffi/src/interp/l2_kernel_registry.rs) | **FSST**（字符串）、**ALP**（浮点） | 编码专用解码器，已存在 |
| L1 model | [l1_model.rs](../crates/loom-ffi/src/interp/l1_model.rs) | **bitpack**（[l1_model/bitpack.rs](../crates/loom-ffi/src/interp/l1_model/bitpack.rs)） | 物理 L1 解码原语 |
| **LMA1「Arrow 语义」机** | [native_arrow_semantic.rs:368](../crates/loom-ffi/src/interp/native_arrow_semantic.rs#L368) | Bool/i32/i64/f32/f64 | **不是解码器**——见下 |
| **L2Core IR 机** | [native_lowering.rs:168](../crates/loom-ffi/src/interp/native_lowering.rs#L168) `execute_supported_copy_i32` | **仅 i32 非空** | 注释明说"intentionally **not** a general interpreter" |

**两个不能搞错的事实：**

1. **`execute_native_arrow_semantic` 不是解码器，是「重放校验机」。** 它收的 LMA1 **内部已经嵌了答案（Arrow IPC）**——`decode_reference_batch` 解出参考 batch，再逐列 `copy_supported_column` 重物化。它证明"native 模型能否复现已嵌入的 Arrow"，不读物理字节。让 sidecar 去调它＝**假合并**（只因 LMA1 里埋了答案才"работает"）。

2. **合并方向代码已写明。** [native_arrow_semantic.rs:401-403](../crates/loom-ffi/src/interp/native_arrow_semantic.rs#L401)：
   > Phase 50 will **re-anchor native execution to sidecar overlay**. LMC2/LMA1 kept for backward compat with **test fixtures**. DO NOT remove until sidecar-native track is production-ready.

### 合并决策（本 phase 据此执行）

- **写一个通用 L2Core body 解释器**，作为**唯一生产解码器**接进 sidecar FFI。它**吞掉** `execute_supported_copy_i32`，走 `ForRange/ReadInput/AppendValue/AppendNull`，append 派发给 `arrow_builder_output`（6 类型 + null 现成），编码 op 派发给 FSST/ALP/bitpack 等 L1/L2 原语。
- **LMA1 路径降级为离线差分 oracle**（测试问："IR 解释器能否复现参考 LMA1？"），不接生产 FFI。这与 README"interpreter 离线 / 生产单跑"叙述自洽。
- **类型覆盖不是"加 match 分支"，而是先有通用解释器骨架**（现 i32 是硬编码捷径，刻意没做通用循环）；骨架就位后底层积木大多现成，类型推进＝分层接线。

---

## 1. 需求映射

| ID | 未完成项 | 承载 Plan |
|---|---|---|
| R1 | `loom_sidecar_decode` 真实 Arrow IPC 输出 | Plan 2（依赖 Plan 1） |
| R2 | L2Core interpreter/JIT 与 sidecar FFI 打通 | Plan 1 |
| R3 | Parquet raw physical byte binding | Plan 4 |
| R4 | README production JIT/online decode 叙述与代码对齐 | Plan 5 |
| R5 | auto IR gen 产真实 decode program（**全类型覆盖**） | Plan 3 |

## 2. 依赖序（执行顺序）

```
Plan 1 (R2: 通用 L2Core 解释器 + LMA1 降 oracle，Tier 1 引擎绿)
   └─> Plan 2 (R1: 回填真实 Arrow IPC + 打通 caller)
          ├─> Plan 3 (R5: tier 阶梯——全类型覆盖，每 tier 端到端)
          │       Tier 1 → Tier 2 → Tier 3 → Tier 4
          └─> Plan 5 (R4: 据实修正 README/correctness model)
Plan 4 (R3: Parquet 物理字节直读，含变长/字典 chunk) —— 与 Plan 3 平行
```

Plan 5 必须在 Plan 1/2 落地**之后**写。Plan 3 是本 phase 的主体工作量（爬完 tier 阶梯＝完全覆盖）。

---

## Plan 1 — 通用 L2Core 解释器接入 `LoomNative` 分支（R2）

**目标**：写一个**通用** `L2CoreProgram` body 解释器作为唯一生产解码器，接进 `loom_sidecar_decode`，吞掉 i32 硬路径；把 LMA1 路径降为离线 oracle。本 plan 交付**引擎骨架 + Tier 1（定宽原语非空）跑绿**，骨架从一开始就为后续 tier 预留派发点。

**依赖**：无。

**涉及文件**
- 新增：`crates/loom-ffi/src/interp/l2core_interp.rs`（通用解释器；`interpret_l2core(program, inputs) -> Result<Vec<ArrayData>>`）
- 改：[crates/loom-ffi/src/ffi.rs:566-600](../crates/loom-ffi/src/ffi.rs#L566)（`LoomNative` 分支调通用解释器）
- 复用：[arrow_builder_output.rs](../crates/loom-ffi/src/interp/arrow_builder_output.rs)（append 派发，6 类型 + null 现成）
- 吞掉/复用：[native_lowering.rs:168](../crates/loom-ffi/src/interp/native_lowering.rs#L168) `execute_supported_copy_i32`（其 i32 语义并入通用解释器；保留为 thin wrapper 或迁测试）
- 降级标注：[native_arrow_semantic.rs:401](../crates/loom-ffi/src/interp/native_arrow_semantic.rs#L401)（注释更新：LMA1＝offline oracle）

**任务**
1. **A（引擎骨架）**：实现 `interpret_l2core`，按顺序执行 `body`：`ForRange`/`CursorLoop` 驱动行游标，`ReadInput` 按 `InputSlice.offset/length` 从 host 字节切片读，`LetScalar` 绑定，`AppendValue`/`AppendNull` 派发给 `OutputBuilder`，`FailClosed` 立即降级。**派发用 `match` 覆盖所有 `L2DataType`，未实现的 arm 显式返回类型化 `Unsupported` 错误（为 Tier 2-4 预留）。**
   **AC**：解释器是通用的——加新类型＝填派发 arm，不改控制流；`execute_supported_copy_i32` 的 i32 用例由 `interpret_l2core` 复现，旧测试全过。
2. **B（FFI 接入）**：`LoomNative` 分支取 `program` + `verified_bindings` 对应 host 切片，调 `interpret_l2core` 得 `Vec<ArrayData>`，构 `ArrowSemanticPayload`（[arrow_semantic.rs:36](../crates/loom-ffi/src/interp/arrow_semantic.rs#L36) `try_new`）暂存供 Plan 2 序列化。
   **AC**：i32 非空程序 → 正确 `Int32Array`；越界/不支持 → fail-closed 转 `host-native`，无 panic。
3. **C（LMA1 降级）**：把 `execute_native_arrow_semantic` 标注/迁移为**离线差分 oracle**（仅测试调用），新增"interp vs LMA1 参考"差分测试骨架。
   **AC**：生产 FFI 路径不再引用 LMA1 执行；LMA1 仅出现在 `#[cfg(test)]`/oracle 模块。

**验证**
- 单测：手写 i32 非空 `L2CoreProgram` + host 字节 → 解释器出值正确。
- 回归：原 `execute_supported_copy_i32` 全部用例经 `interpret_l2core` 通过。
- 负路径：不支持 feature/类型/越界 → fail-closed 无 panic。
- 差分：interp 输出 == LMA1 oracle 参考（i32 fixture）。

**must-have**：解释器**必须是通用骨架**，不得再写第二个 i32 专用捷径。fail-closed 硬约束（CLAUDE.md）。

**风险/暗礁**
- 别让 sidecar 调 `execute_native_arrow_semantic`（假合并）。
- `verified_bindings` 与 program capability 的列对应（granule_id ↔ capability id）需显式核对，否则列错位。

---

## Plan 2 — 回填真实 Arrow IPC 输出 + 打通 caller（R1）

**目标**：把 Plan 1 的 payload 序列化为**真实 Arrow IPC 字节**回填 `out_ipc_bytes`，并让 DuckDB 扩展真正消费它。

**依赖**：Plan 1。

**涉及文件**
- 改：[crates/loom-ffi/src/ffi.rs:579-598](../crates/loom-ffi/src/ffi.rs#L579)（删 `Vec::new()` 空缓冲）
- 复用：[arrow_semantic_codec.rs:52](../crates/loom-ffi/src/interp/arrow_semantic_codec.rs#L52) `encode_arrow_semantic_payload`（`StreamWriter`）
- 改：[contrib/duckdb-ext/loom_extension.cpp:75-119](../contrib/duckdb-ext/loom_extension.cpp#L75)（新增 decode 调用 + IPC 摄取）
- 改：[crates/loom-ffi/include/loom.h:156](../crates/loom-ffi/include/loom.h#L156)（契约对齐）

**任务**
1. **A**：成功路径对 payload 取 IPC 段（或裸 `StreamWriter`）回填 `out_ipc_bytes/out_ipc_len`，写真实 `row_count/column_count`。
   **AC**：`ipc_len > 0`；arrow-rs `StreamReader` 能读回，行数与 program bound 一致。
2. **B**：明确**裸 IPC vs LMA1 容器**对外契约写进 `loom.h`。
   **AC**：头文件注释与返回字节格式逐字一致。
3. **C**：DuckDB 扩展在 `route=="loom-native"` 调 `loom_sidecar_decode`，IPC 喂 `arrow_scan`/nanoarrow；非 loom-native 保持回退。
   **AC**：一条 SQL 经 sidecar 路径查出 i32 列真实值（端到端）。
4. **D**：`loom_sidecar_free_bytes` 正确释放非空缓冲。
   **AC**：无泄漏/双 free（`Box::from_raw` 配对）。

**验证**：decode→`StreamReader` 往返单测；C++/SQL 冒烟；free 路径测试。

**must-have**：non-loom-native 路径**字节级不变**，不得回归。

**风险/暗礁**：`arrow_semantic_codec` 产 `LMA1`（带 magic+len 头），不是裸 IPC。直接喂 arrow_scan 会失败——必须剥头或改裸 `StreamWriter`。最易翻车点。

---

## Plan 3 — `decode_ir_gen` 全类型覆盖（tier 阶梯）（R5）

**目标**：让 [`generate_decode_ir_from_parquet`](../crates/loom-parquet-ingress/src/decode_ir_gen.rs#L37) 产真实可执行 `body`，并**爬完 tier 阶梯实现完全类型覆盖**。每个 tier 是一个垂直纵切片：(a) 解释器派发 arm（Plan 1 骨架的填空）+ (b) decode_ir_gen 产对应 body + (c) parquet→sidecar→IPC→值正确的端到端测试。**终态：四个 tier 全绿。**

**依赖**：Plan 1（引擎骨架）、Plan 2（可序列化+可验证下游）。Tier 间严格顺序：1→2→3→4。

**涉及文件**
- 改：[crates/loom-parquet-ingress/src/decode_ir_gen.rs:55](../crates/loom-parquet-ingress/src/decode_ir_gen.rs#L55)（`body = Vec::new()` → 逐 tier 产指令）
- 改：`crates/loom-ffi/src/interp/l2core_interp.rs`（逐 tier 填派发 arm）
- 读：[l2_core.rs:155](../crates/loom-ir-core/src/l2_core.rs#L155)（`L2CoreStmt`）、[l2_kernel_registry.rs](../crates/loom-ffi/src/interp/l2_kernel_registry.rs)（FSST/ALP）

### Tier 1 — 定宽原语非空（i32/i64/f32/f64/bool）
- **代价**：低（builder 全有，解释器骨架已能跑 i32）。
- **任务**：decode_ir_gen 对每个定宽非空列产 `ForRange + ReadInput(width) + AppendValue`；解释器补齐 i64/f32/f64/bool 派发 arm（bool 注意位宽）。
- **AC**：含混合定宽列的 parquet → 各列值正确；端到端测试覆盖 5 种类型。

### Tier 2 — nullable
- **代价**：中（IR 要携带 validity，解释器接 `AppendNull`，`ReadInput` 读 null bitmap）。
- **任务**：`L2CoreProgram` 表达 validity（每行先判 null 再 AppendValue/AppendNull）；decode_ir_gen 从 parquet definition levels 推 validity；解释器按 validity 派发。
- **AC**：含 null 的定宽列 → null 位置与值都正确（与 Arrow 参考逐行比对）。
- **暗礁**：Parquet 用 definition levels 表达 null，不是 bitmap——IR/解释器要约定一种 validity 表示，转换在 ingress 侧。

### Tier 3 — Utf8（变长，经 FSST）
- **代价**：中高（变长 InputSlice 语义 + L2 kernel 派发 + offset buffer）。
- **任务**：解释器支持变长读（offsets + data 两段）并派发 FSST kernel（[l2_kernel_registry.rs](../crates/loom-ffi/src/interp/l2_kernel_registry.rs) id 0）；decode_ir_gen 对 Utf8/LargeUtf8 列产变长 body（替换当前 256 宽度估算）；`StringBuilder` 输出（builder 现成）。
- **AC**：Utf8 列（含 FSST 压缩 fixture）→ 字符串值正确；空串/多字节 UTF-8 边界用例通过。
- **暗礁**：当前 [decode_ir_gen.rs:69](../crates/loom-parquet-ingress/src/decode_ir_gen.rs#L69) Utf8=256 是假估算，必须删除换成真实 offset 语义。

### Tier 4 — 字典（dictionary）【本 phase 新增、净新增工作】
- **代价**：高（**无现成 kernel、无 capability**，全新增）。
- **任务**：
  1. IR 侧：新增字典表达——dictionary 值表（InputSlice）+ indices（InputSlice）+ 输出 `DictionaryArray` 的 capability/语义。
  2. 解释器：新增字典派发 arm（读 indices → 查值表 → 构 `DictionaryArray`，或物化为普通数组二选一，需定语义）。
  3. decode_ir_gen：对 parquet dict-encoded 列产字典 body。
  4. builder：`arrow_builder_output` 当前无 Dictionary 变体——需新增。
- **AC**：parquet 字典编码列 → 正确 `DictionaryArray`（或约定的物化结果）；与 Arrow 参考一致。
- **暗礁**：字典是编码也是类型，需先定"输出 DictionaryArray vs 物化展开"的契约（影响下游 DuckDB 摄取）。这是本 phase 最重的一块。

**验证**：每 tier 一个端到端测试（真实 parquet → 自动生成 IR → 解释器 → IPC → 值正确）+ 与 LMA1/Arrow 参考差分。**四 tier 全绿＝R5 完成。**

**must-have**：`decode_ir_gen` 当前**无人调用、无测试**——必须新增调用点与逐 tier 端到端测试。任一 tier 未覆盖的类型/编码必须 fail-closed，不产占位。

---

## Plan 4 — Parquet raw physical byte binding（R3）

**目标**：绕过 Arrow 物化，按 footer 元数据 `File::seek` 直读 column chunk 原始字节，给出精确物理偏移，落实内容哈希绑定。**需覆盖 Tier 1-4 各类编码的 chunk（定宽/变长/字典）字节范围**，否则 Plan 3 的精确 offset 无来源。

**依赖**：无（与 Plan 3 平行；Plan 3 Tier 各步消费本 plan 的精确偏移）。

**涉及文件**
- 改：[sidecar_parquet.rs:233-286](../crates/loom-parquet-ingress/src/sidecar_parquet.rs#L233)（`chunk_bindings_from_parquet` 直读而非 Arrow buffer）
- 改：[source_contract.rs:110-116](../crates/loom-parquet-ingress/src/source_contract.rs#L110)（`bind_content_hash_to_parquet_data` 空操作 → 真实实现）
- 复用：[source_contract.rs:251-262](../crates/loom-parquet-ingress/src/source_contract.rs#L251)（已能拿 `column.byte_range()`）

**任务**
1. **A**：用 `column.byte_range() -> (start, length)`，`File::seek(start)` 读 `length` 字节得 column chunk 原始字节。
   **AC**：读出长度＝`byte_range` 长度；`host_data_range` 反映**真实文件偏移**。
2. **B**：实现 `bind_content_hash_to_parquet_data`：对直读字节算 BLAKE3 与 binding 声明哈希比对。
   **AC**：篡改字节 → 哈希不匹配 → Gate 3 失败 → `host-native`。
3. **C**：为 Tier 3/4 提供变长 data/offset chunk 与字典 chunk 的字节范围切分。
   **AC**：变长列、字典列各有一个 chunk 切分单测。

**验证**：真实 parquet 直读 chunk 长度/哈希正确；篡改 → fail-closed；变长/字典 chunk 切分测试。

**must-have**：page header 解析与 page 级解压**显式不在本 plan**——用注释/`log` 登记缺口，避免"全物理层已覆盖"的假象。

**风险/暗礁**：`byte_range` 含 page header + 压缩字节；直读得压缩字节，与解码后逻辑字节不同。哈希契约要明确"对哪一层字节"，否则 Plan 3 offset 对不上。

---

## Plan 5 — README / correctness model 与代码对齐（R4）

**目标**：把 README/README-zh 超前于代码的强叙述据实修正——**在 Plan 1/2 落地后**，按"代码真实做到什么"写。

**依赖**：Plan 1、Plan 2。

**涉及文件**：[README.md](../README.md)、[README-zh.md](../README-zh.md)

**任务**
1. **A**：把"JIT 是唯一生产运行时 / online decode → Arrow RecordBatch"改为与代码一致：生产路径经 `loom_sidecar_decode` 用**通用 L2Core 解释器**产 Arrow IPC（覆盖到哪个 tier 就写到哪）；JIT 仍为**离线差分验证**，未接生产 FFI（如本 phase 未接 JIT 就如实写"未接"）；LMA1 为离线 oracle。
   **AC**：每条强 claim 指到 `file:line` 证据，无悬空承诺。
2. **B**：校准"三层差分验证"叙述（kloom + interp + JIT 测试 + 新增 interp-vs-LMA1 oracle），确认未被改坏。
   **AC**：与 [contrib/kloom](../contrib/kloom)、interp oracle 现状一致。

**验证**：逐条 claim → 代码证据复核（沿用前次对齐表格式）。

**must-have**：不得再写入"计划中但代码未做"的能力为既成事实；aspirational 显式标注 roadmap。

---

## 3. 完成定义（Definition of Done）——完全覆盖

本 phase 视为完成，当且仅当：

1. **R2**：通用 L2Core 解释器为唯一生产解码器接进 sidecar FFI，吞掉 i32 硬路径；LMA1 降为离线 oracle 且有 interp-vs-LMA1 差分测试。
2. **R1**：DuckDB 扩展经 sidecar `loom-native` 路径消费非空真实 Arrow IPC。
3. **R5（完全类型覆盖）**：`generate_decode_ir_from_parquet` 产真实可执行 body，**Tier 1-4 全绿**——i32/i64/f32/f64/bool + nullable + Utf8 + 字典，每类型有 parquet→sidecar→IPC→值正确的端到端测试。
4. **R3**：parquet column chunk（含定宽/变长/字典）物理字节可直读，哈希绑定 fail-closed 生效。
5. **R4**：README 强叙述逐条有 `file:line` 支撑，无超前承诺。
6. **全程 fail-closed**：未覆盖的类型/编码/越界/哈希不匹配一律降级 `host-native`，绝不产半成品 IPC。
7. core/FFI 仍 **Vortex-free**；parquet 直读只在 `loom-parquet-ingress`。

## 4. 明确不在本 phase 范围

- **JIT 接入生产 FFI**（本 phase JIT 保持离线验证；接入另起 phase）。
- Parquet **page header 解析与 page 级解压**（Plan 4 登记缺口）。
- 嵌套/复合类型（Struct/List/Map）、Decimal、时间戳等 Tier 1-4 之外的类型。
- 字典之外的高级编码（RLE/FOR 直接执行——除非 Tier 推进顺带覆盖；bitpack 已有 L1 原语，按需接入）。
- Lance / Vortex ingress 的对等打通。
