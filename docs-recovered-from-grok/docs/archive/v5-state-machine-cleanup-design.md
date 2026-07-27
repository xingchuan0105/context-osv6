# V5 状态机残留一次性清理设计方案

> ⚠️ **ARCHIVED 2026-06-13** — 本文档已实现并归档。
> 实际删除：`rig_adapter.rs`、`LoopBudget` 旧字段与方法、`AgentRunResult.state_history` 与 `StateRecord`、`AgentEvent::StateTransition` 与 `StateTransitionType`。
> 现行交付记录：`docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md`（§2.1）。
> 保留供历史追溯（删除清单 / 风险矩阵 / 数据库列误判说明）。

> Status: **Implemented** (2026-06-09, commit `1c89852`)
> Author: 架构审核讨论（候选 4：废弃状态机残留）  
> Date: 2026-06-09  
> Related: ADR-0006 (ReActLoop 迁移)、`evaluator.rs` 废弃、`router.rs` 废弃

---

## 1. 背景与动机

ADR-0006 完成了从 v5 策略状态机（`StrategyExecutor` + 状态图）到 `ReActLoop`（原生迭代循环）的迁移。但迁移后，大量 v5 遗留代码仍以 `#[deprecated]` 标记或孤儿事件的形式存在于编译单元中：

- `LoopBudget` 的废弃 search round 计数器
- `AgentRunResult` 的废弃 `state_history` 字段 + `StateRecord` 类型
- `AgentEvent::StateTransition` 孤儿事件 + `StateTransitionType` 枚举
- `rig_adapter.rs` 的未实现占位符模块

这些代码被编译但不被任何生产路径消费，属于**活着的 dead code**——给维护者带来持续的心智负担。

经全仓库 grep 确认：所有四项残留的 consumer 均为零（或为零 emit + 仅被动 match）。本方案一次性清理全部残留。

---

## 2. 清理范围（四项）

### 2.1 A 类：`react_loop.rs` — 废弃 search round 计数器

**删除内容**：

| 符号 | 位置 | 说明 |
|------|------|------|
| `LoopBudget.max_search_rounds` | line 84 | 字段 + `#[deprecated]` + `#[serde(default)]` |
| `LoopBudget.current_search_rounds` | line 87 | 字段 + `#[deprecated]` + `#[serde(default)]` |
| `default_max_search_rounds()` | line 90-92 | 仅被 `max_search_rounds` 的 `serde(default)` 使用 |
| `LoopBudget::tick_search_round()` | line 151-153 | 方法 + `#[deprecated]` |
| `LoopBudget::search_rounds_exhausted()` | line 159-161 | 方法 + `#[deprecated]` |

**修改内容**：

- `LoopBudget::new()`（line 103-110）：删除 `max_search_rounds: 2` 和 `current_search_rounds: 0` 的初始化
- `LoopBudget::rag()` / `search()` / `chat()`：不变（通过 `Self::new(...)` 间接调用）
- `#[derive(Serialize, Deserialize)]`：删除后 `LoopBudget` 的序列化格式不再包含 `max_search_rounds` / `current_search_rounds`。由于 `LoopBudget` 是**内存运行时对象，不持久化到磁盘/数据库**，序列化兼容性不是风险。

**测试影响**：
- `react_loop.rs` 内联测试（line 305-436）：确认无测试调用 `tick_search_round()` 或 `search_rounds_exhausted()`。`LoopBudget::new()` 的测试只需验证 `current` 和 `max_iterations`。

---

### 2.2 B 类：`runtime.rs` — 废弃 `state_history` + `StateRecord`

**删除内容**：

| 符号 | 位置 | 说明 |
|------|------|------|
| `AgentRunResult.state_history` | line 275-277 | 字段 + `#[deprecated]` + `#[serde(default)]` |
| `StateRecord` | line 305-313 | 结构体 + `#[deprecated]` |

**修改内容**：

- `AgentRunResult::default()`：字段已在 `Default` derive 中处理，删除后自动移除
- `loop/mod.rs:1254`：`state_history: None` → 删除该字段初始化
- `runtime.rs` 内联测试（line 883-884）：legacy JSON 反序列化测试包含 `"state_history": null`。删除字段后，旧 JSON 中多出未知字段 `"state_history"`，但 serde 的 `deny_unknown_fields` **未启用**，所以旧 JSON 仍可反序列化（多余字段被忽略）。

**⚠️ 注意**：`migrations/0008_api_keys_memory_notifications.up.sql:41` 中的 `state_history JSONB` 列属于 **`memory_states` 表**（记忆状态历史），是业务概念，与 `AgentRunResult.state_history`（v5 状态机执行历史）**完全无关**。数据库列不受影响。

---

### 2.3 C 类：`events.rs` — 孤儿 `StateTransition` 事件

**删除内容**：

| 符号 | 位置 | 说明 |
|------|------|------|
| `AgentEvent::StateTransition` | line 46-60 | 变体 + 所有字段 |
| `StateTransitionType` | line 132-138 | 枚举（仅被 `StateTransition` 使用） |

**修改内容**：

- `sse_sink.rs:202-223`：删除 `AgentEvent::StateTransition` 的 match arm。该 arm 将 `StateTransition` 映射到 `ChatEvent::Trace`，但**没有任何代码 emit `StateTransition` 事件**，所以删除后行为不变。
- `events.rs` 内联测试（line 315-321）：删除 `AgentEvent::StateTransition` 的 serde roundtrip 测试用例。

**验证**：`ingestion::AuditAction::StateTransition`（line 85）和 `InvalidStateTransition` 错误（`ingestion/src/error.rs`）是 **ingestion worker 的任务状态转换审计**，与 agent 事件无关。删除 `AgentEvent::StateTransition` 不影响它们。

---

### 2.4 D 类：`rig_adapter.rs` — 未实现占位符模块

**删除内容**：

| 符号 | 位置 | 说明 |
|------|------|------|
| `rig_adapter.rs` 整个文件 | ~348 行 | 完整模块 |
| `agents/mod.rs:55` 的 `pub mod rig_adapter;` | line 55 | 模块声明 |

**删除范围**：

- `RigModelConfig`
- `RigModelClient` trait
- `RigChatMessage`
- `RigCompletion`
- `FakeRigClient` + `FakeRigEvent`
- `RigCoreClient`（占位符实现）
- 全部内联测试

**理由**：
- 用户确认**永远不会接入 rig-core**；
- 零外部 consumer（除 `agents/mod.rs` 的模块声明外，没有任何 `use` 引用）；
- `RigCoreClient` 的 `complete()` / `complete_stream()` 永远返回空内容 + degrade trace（"rig_core_not_yet_wired"），没有生产价值；
- 未来若需 rig 集成，从头设计比维护这个占位符更好。

---

## 3. 文件级修改清单

### 3.1 删除的文件

```
avrag-rs/crates/app/src/agents/rig_adapter.rs
```

### 3.2 修改的文件

| 文件 | 修改内容 |
|------|----------|
| `crates/app/src/agents/react_loop.rs` | 删除 `max_search_rounds`、`current_search_rounds` 字段；删除 `default_max_search_rounds()`；删除 `tick_search_round()`、`search_rounds_exhausted()`；更新 `new()` 初始化 |
| `crates/app/src/agents/runtime.rs` | 删除 `state_history` 字段；删除 `StateRecord` 结构体 |
| `crates/app/src/agents/events.rs` | 删除 `StateTransition` 变体；删除 `StateTransitionType` 枚举；更新 serde roundtrip 测试 |
| `crates/app/src/agents/sse_sink.rs` | 删除 `StateTransition` 的 match arm |
| `crates/app/src/agents/mod.rs` | 删除 `pub mod rig_adapter;` |
| `crates/app/src/agents/loop/mod.rs` | 删除 `build_run_result` 中的 `state_history: None` |

---

## 4. 测试修复清单

| 测试文件 | 修复内容 |
|----------|----------|
| `react_loop.rs` tests | 确认无 `tick_search_round` / `search_rounds_exhausted` 调用；若有则删除对应断言 |
| `runtime.rs` tests | 更新 legacy JSON 反序列化测试：确认 `"state_history": null` 在字段删除后不会导致反序列化失败（serde 默认忽略未知字段） |
| `events.rs` tests | 删除 `StateTransition` 的 serde roundtrip 测试用例 |
| `sse_sink.rs` tests | 确认无 `StateTransition` → `ChatEvent::Trace` 的测试 |
| `rig_adapter.rs` tests | 随文件一并删除 |

---

## 5. 风险评估与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `LoopBudget` 序列化兼容性 | 极低 | 若 LoopBudget JSON 被持久化，反序列化会失败 | LoopBudget 是内存对象，不持久化。且 serde 默认行为会忽略未知字段。 |
| `AgentRunResult` 旧 JSON 反序列化 | 极低 | 旧 JSON 含 `"state_history": null`，删除字段后可能报错 | serde 未启用 `deny_unknown_fields`，未知字段被静默忽略。 |
| 前端依赖 `StateTransition` SSE 事件 | 极低 | 前端解析 SSE 时遇到不认识的 event type | 无任何代码 emit 该事件，前端永远不会收到它。 |
| 日志 pipeline 解析 `RoutingDecision` 变体 | 无 | — | 本次清理不涉及 `RoutingDecision`（已在 RouterPolicy 移除方案中处理）。 |
| 误删 `memory_states.state_history` 数据库列 | 无 | — | 明确区分：`memory_states.state_history` 是业务列，不在清理范围内。 |

---

## 6. 实施 Checklist

- [ ] 删除 `crates/app/src/agents/rig_adapter.rs`
- [ ] 删除 `crates/app/src/agents/mod.rs` 中的 `pub mod rig_adapter;`
- [ ] 修改 `react_loop.rs`：删除 search round 相关字段和方法
- [ ] 修改 `runtime.rs`：删除 `state_history` 和 `StateRecord`
- [ ] 修改 `events.rs`：删除 `StateTransition` 变体和 `StateTransitionType`
- [ ] 修改 `sse_sink.rs`：删除 `StateTransition` match arm
- [ ] 修改 `loop/mod.rs`：删除 `build_run_result` 中的 `state_history: None`
- [ ] 修复 `events.rs` 的 serde roundtrip 测试
- [ ] 运行 `cargo test` 全量通过
- [ ] 运行 `cargo clippy` 无新 warning
- [ ] 更新 `CONTEXT.md`：删除 v5 相关术语（如需要）

---

## 7. 术语表

| 术语 | 定义 |
|------|------|
| **StateRecord** | 废弃的 v5 结构体。记录 StrategyExecutor 中每个状态的进入/完成时间。被 `ReActIterationRecord` 替代。 |
| **StateTransition** | 废弃的 `AgentEvent` 变体。原用于 SSE 流中报告 StrategyExecutor 的状态转移。StrategyExecutor 已删除，该事件无人 emit。 |
| **search round** | 废弃的 `LoopBudget` 概念。原用于单独计数 search API 调用轮次（与 LLM 迭代轮次区分）。已被 YAML `budget.max_iterations` 统一替代。 |
| **RigCoreClient** | 废弃的 rig-core 适配器占位符。`complete()` 和 `complete_stream()` 永远返回空内容和 degrade trace。用户确认永远不会接入 rig-core。 |
