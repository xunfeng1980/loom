# Phase 48 Formal Assurance Architecture

**Date:** 2026-06-10  
**Scope:** Post-48 narrowed Plan-A — no Rust interpreter leg; all deferred items P1–P5 closed.

---

## 1. Phase 48 完成度

| 交付目标 | 状态 |
|---------|------|
| `KOracleOutcome` 三态枚举（ProducedTrace / SkippedRefereeAbsent / UnsupportedProgram） | ✅ |
| krun-absent skip 语义（ENOENT / 超时 / 定义目录缺失） | ✅ |
| 乱码输出硬失败（`<events>` 缺失检测） | ✅ |
| Min / Max / Bytes 不支持构造守卫 | ✅ |
| per-shape native-route 禁用表（预检查 + 后验证钩子） | ✅ |
| strict skip 接线（kloom-diff.sh / CI 无 skip env var） | ✅ |
| LLVM backend 可行性脚本 + findings doc | ✅ |
| contrib/kloom 文档同步（v4 覆盖表 + 四态分类法） | ✅ |
| **P1** Real Min/Max K semantic rules in kloom.k | ✅ |
| **P3** Persistent cross-process disable store（JSON + 原子写入 + env 覆盖） | ✅ |
| **P4** Equivalence-class corpus generator（loom-fixtures::corpus） | ✅ |
| **P5** L2Core three-place sync checklist（kloom.k / Rust / Lean） | ✅ |
| Rust reference interpreter leg（三路对账） | ❌ deferred indefinitely |
| Extract LLVM backend interpreter into production | ❌ deferred indefinitely |

**结论：** Phase 48 在 narrowed scope 下**完全实现**，包括 base 交付物（48-01/02/03）和所有 deferred items P1/P3/P4/P5。P2（LLVM interpreter in production）明确保留为 deferred indefinitely。

---

## 2. 三层保障架构

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 3 — Formal (Offline, Human-Auditable)                             │
│                                                                         │
│   Lean LoomCore.lean                                                    │
│   ├─ Static checker: builder_events_typed, no_ambient_authority,        │
│   │                  finite_bounds                                      │
│   ├─ Modeled executor: execProgram (proof-friendly, NOT byte-accurate) │
│   ├─ Theorems: structural_safe_projection, classified_stmt_exec_progress│
│   └─ Role: "Why do we believe the static verifier is sound?"            │
│                                                                         │
│   ↔ Rust correspondence (Phase 37): AST 同构 + RejectCode 镜像         │
│   ⚠️  Lean 不生成 trace，不参与 JIT 对账，不在 production path          │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ (人工维护语义同步，无自动化证明)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 2 — Spec-Oracle (Runtime Differential Validation)                 │
│                                                                         │
│   K Framework (kloom.k + krun)                                          │
│   ├─ Operational semantics for L2Core                                   │
│   ├─ krun 解释执行 → builder-event trace (append-value / append-null)  │
│   ├─ Role: "What should the native output have done?"                   │
│   └─ Conditional: 不支持 Min/Max/Bytes; krun 缺失时 skip; 30s 超时      │
│                                                                         │
│   ↔ Native differential gate (Phase 40/48): trace-by-trace 比较        │
│   ⚠️  Authoritative but SLOW (进程调用 + 解释执行 + 文本解析)            │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ (reference_trace == native_trace ?)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Layer 1 — Production (Fail-Closed Default Execution)                    │
│                                                                         │
│   Rust Native Codegen (loom-native-melior)                              │
│   ├─ MLIR/LLVM JIT 执行                                                 │
│   ├─ 生成 Arrow RecordBatch                                             │
│   ├─ Role: "What actually happened?"                                    │
│   ├─ Fail-closed on divergence from K oracle                            │
│   └─ Per-shape disable registry (process lifetime)                      │
│                                                                         │
│   ↔ DuckDB / Host Adapter: value/validity buffer handoff               │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. K 现在可以生成权威但 Slow 的 Oracle 了吗？

**答案是：有条件地可以（Bounded Yes）。**

### 权威（Authoritative）

- kloom.k 定义了 L2Core 的操作语义（configuration: `<k>`, `<events>`, `<builders>`, `<inputs>`）。
- krun 是 K Framework 的**官方解释器**，执行 kloom.k 中定义的 rewrite rules。
- 生成的 builder-event trace（`append-value:col0_int32`, `append-null:col1_float64`, `terminal:finished`）是形式化语义的实际执行结果。
- 相对于 Rust native 实现，这个 trace 是**spec-baseline**，具有权威性。

### Slow

- 每次验证都需要：序列化 → 写临时文件 → `Command::new("krun")` → 解释执行 → 解析 pretty-printed XML-like 输出。
- 30 秒超时保护（`KRUN_TIMEOUT_SECS = 30`）。
- 比 Rust native codegen 慢 **3~5 个数量级**（进程启动 + 解释器开销）。

### Bounded（有条件）

| 条件 | 处理 |
|------|------|
| 程序包含 `Min` / `Max` / `Bytes` | `UnsupportedProgram` — harness 拒绝序列化，K 不执行 |
| krun 不在 PATH | `SkippedRefereeAbsent` —  referee 缺失，不阻塞 production |
| kompile 定义目录缺失 | `SkippedRefereeAbsent` — 同上 |
| krun 执行超过 30s | `SkippedRefereeAbsent` — 超时 kill |
| krun 非零退出 | **Hard Error** — referee 存在但拒绝，必须调查 |
| 输出无 `<events>` 标签 | **Hard Error** — 乱码输出，必须调查 |

### 为什么不 fail-close Skip？

`Skipped` 和 `Unsupported` 设置 `model_trace_matches: true`，`oracle_skip_reason: Some(...)`。这确保了：

1. K oracle 的缺失**不会阻塞** production native path。
2. 开发者/CI 可以选择 `LOOM_ALLOW_K_ORACLE_SKIP=1` 继续工作。
3. `kloom-diff.sh` 和 CI **不设置**这个变量，保持严格。

---

## 4. Native 与 K 的对账流程（Differential Validation）

### 4.1 数据流

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Differential Validation Flow                         │
└─────────────────────────────────────────────────────────────────────────┘

Artifact Bytes (LMC2 / LMA1)
        │
        ▼
┌─────────────────┐     ┌─────────────────────────────────────┐
│ Reference Batch │────►│ reference_model_trace_for_batch()   │
│ (decode from    │     │   1. record_batch_from_codegen()   │
│  artifact)      │     │   2. reference_program_for_batch() │
└─────────────────┘     │      → L2CoreProgram               │
                        │   3. kloom_trace_for_program()     │
                        │      → serialize to kloom text     │
                        │      → krun --output pretty        │
                        │      → parse_trace()               │
                        └─────────────────────────────────────┘
                                │
                    ┌───────────┼───────────┐
                    ▼           ▼           ▼
              ProducedTrace  Skipped    Unsupported
                    │         (timeout)    (Min/Max/Bytes)
                    │           │            │
                    ▼           ▼            ▼
            reference_trace  oracle_skip_reason  oracle_skip_reason
                    │           │                │
                    │           └───────┬────────┘
                    │                   │
                    ▼                   ▼
            Native JIT Output    (skip — no K trace,
            (Arrow RecordBatch)   but value_equivalence
                    │             still checked)
                    ▼
            native_model_trace_for_batch()
                    │
                    ▼
            compare: reference_trace == native_trace ?
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
    Compared                 Diverged
        │                       │
        ▼                       ▼
    cacheable = true      disable_shape(schema_fingerprint)
    replay_evidence       cacheable = false
        │                 replay_evidence = None
        ▼                       │
    Runtime Cache               ▼
                        fallback_or_fail_closed(policy)
```

### 4.2 四态分类法（Four-State Taxonomy）

`verify_native_arrow_semantic_model_for_output()` 将结果归类为四态之一：

| 状态 | K oracle | Native trace | `model_trace_matches` | `oracle_skip_reason` | 动作 |
|------|----------|-------------|----------------------|---------------------|------|
| **Compared** | ✅ ProducedTrace | ✅ 匹配 | `true` | `None` | cacheable ✅ |
| **Diverged** | ✅ ProducedTrace | ❌ 不匹配 | `false` | `None` | disable shape ❌ |
| **Skipped** | ⚠️ 缺失/超时 | ✅ 存在 | `true` | `Some(reason)` | skip, no disable ⚠️ |
| **Unsupported** | ⚠️ Min/Max/Bytes | ✅ 存在 | `true` | `Some(reason)` | skip, no disable ⚠️ |

**关键设计**：`Skipped` 和 `Unsupported` 的 `model_trace_matches` 设为 `true`，因为这不是 native 的错，是 K oracle 无法提供参考。此时仍然检查 `value_equivalent`（`output == &reference`，即 RecordBatch 级相等）。

### 4.3 per-shape 禁用机制

```rust
// jit.rs 后验证钩子（Phase 48-02）
let has_divergence = execution.validation().is_some_and(|v| {
    v.oracle_skip_reason.is_none()                      // 不是 skip
        && v.diagnostics().iter().any(|d| {
            d.code == NativeModelTraceMismatch          // 真实分歧
        })
});

if has_divergence {
    disable_shape(&execution.schema_fingerprint);        // 禁用此形状
    // force cacheable=false, replay_evidence=None, status=fallback
}
```

**预检查**（fast-fallback）：在 JIT 执行之前，如果 `is_shape_disabled(schema_fingerprint)`，直接返回 fallback，不运行 JIT，不调用 krun。

**持久化**（Phase 48 P3）：禁用状态通过 `DisableStore` JSON 持久化到磁盘（默认 `$XDG_CACHE_HOME/loom/disabled-shapes.json`，可通过 `LOOM_DISABLE_STORE_PATH` 覆盖），支持原子写入（temp → rename）和进程重启后自动加载。`disabled_shapes_registry()` 在 `OnceLock` 初始化时从磁盘读取已有记录。

---

## 5. Lean ↔ K ↔ Native 的分工

| 职责 | Lean | K (kloom.k) | Native (Rust JIT) |
|------|------|------------|-------------------|
| **执行** | 不执行 | 解释执行（slow） | JIT / AOT 编译（fast） |
| **输出** | `ModeledState`（抽象状态） | builder-event trace | Arrow RecordBatch |
| **验证时机** | 离线 / CI | 运行时（Phase 40/48） | 运行时（production） |
| **验证对象** | 静态结构 | builder-event 序列 | Arrow buffer 内容 |
| ** fail-close** | N/A（离线） | 分歧 → disable shape | 分歧 → fallback |
| **当前状态** | `StaticVerified` / `Safe` 定理 | kloom v4，含 Min/Max EvalConst + TypeOf 规则 | 生产默认路径 |
| **与 Rust 的连接** | Phase 37 AST 对应 | Phase 40/48 trace 比较 | 同一代码库 |

### 为什么 Lean 不参与运行时对账？

1. **Lean 是 proof assistant，不是 interpreter**。`execProgram` 是 proof-friendly modeled executor，不是 byte-accurate Rust 解释器。
2. **Lean 不生成 builder-event trace**。它的输出是 `ModeledState`（抽象状态和定理），不是可比较的 trace 列表。
3. **Lean 的 role 是"为什么我们相信静态检查器是 sound 的"**，而不是"native 输出是否正确"。
4. **K 是唯一的运行时 spec-oracle**（在 narrowed Plan-A 下）。

### 理想的未来连接（deferred）

```
Lean modeled executor trace
        │
        │ (未来：提取 Lean execProgram 的 event 列表)
        ▼
   compare with K trace
        │
        │ (未来：证明 K semantics ≡ Lean operational semantics)
        ▼
   compare with Native trace
        │
        ▼
   Three-way reconciliation
```

当前状态：Lean 和 K 之间**没有自动化等价证明**，只有人工维护的 AST 同构。

---

## 6. 关键文件映射

| 职责 | 文件 |
|------|------|
| K oracle harness | `crates/loom-core/src/kloom_harness.rs` |
| Differential validation | `crates/loom-core/src/native_arrow_semantic.rs` (lines 1230-1393) |
| Native↔K trace comparison | `crates/loom-core/src/native_arrow_semantic.rs` (lines 2054-2073) |
| per-shape disable registry | `crates/loom-native-melior/src/jit.rs` (lines 467-500) |
| Lean static checker | `formal/lean/LoomCore.lean` |
| Lean modeled executor | `formal/lean/LoomCore.lean` (lines 711-864) |
| K spec definition | `contrib/kloom/kloom.k` |
| K diff script | `contrib/kloom/scripts/kloom-diff.sh` |
| Equivalence-class corpus generator | `crates/loom-fixtures/src/corpus.rs` |
| AST sync checklist (kloom.k / Rust / Lean) | `scripts/l2core-sync-checklist.py` |
| Feasibility findings | `contrib/kloom/docs/LLVM-BACKEND-FEASIBILITY.md` |
