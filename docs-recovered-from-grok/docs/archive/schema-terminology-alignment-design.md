# Strategy Schema 语义漂移 + 术语漂移合并治理方案

> ⚠️ **ARCHIVED 2026-06-13** — 本文档已实现并归档。
> 实际改名：`StrategySchema → ModeSchema`、`strategy_id → mode_id`、`api_version "v5" → "v6"`；删除 `StrategySchema.states/transitions/max_budget` 与 `TransitionSchema`。
> 现行交付记录：`docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md`（§2.4）。

> Status: **Implemented** (2026-06-09, commit `1c89852`)
> Author: 架构审核讨论（候选 3 + 候选 5）  
> Date: 2026-06-09  
> Related: ADR-0006 (ReActLoop 迁移)、`router-policy-removal-design.md`

---

## 1. 背景与动机

### 1.1 候选 3：Strategy Schemas 语义漂移

`capability/schemas.rs` 定义了三个 `StrategySchema`，每个都包含 `states` 和 `transitions`：

| Schema | States | Transitions |
|--------|--------|-------------|
| chat | Plan → ExecuteAtomic → Answer | 3 条 |
| rag | Plan → ExecuteRetrieve → Answer（含 replan 循环） | 4 条 |
| search | Decompose → ParallelSearch → Aggregate → Answer | 3 条 |

但 `ReActLoop`（`loop/mod.rs`）的实际实现是**无状态的 native `loop {}`**——LLM 输出 tool call / code block / content，循环继续或退出。不存在 `Plan`、`ExecuteRetrieve`、`Answer` 等状态，也没有状态之间的转移。

这意味着 `/agent/capabilities` API 向消费者描述了一个**不存在的执行模型**。前端（或任何 API 消费者）看到的 schema 暗示"后端运行一个状态机"，但实际上后端运行的是一个迭代循环。

此外，`StrategySchema.max_budget` 是**静态硬编码值**（chat=1, rag=4, search=3），与实际 `LoopBudget` 的 tier-dependent 动态值不符（chat 最小为 2）。这个字段在撒谎。

### 1.2 候选 5：术语漂移

同一概念（chat / rag / search 三种执行模式）在代码库中使用多个不同名字：

| 层级 | 名字 | 位置 |
|------|------|------|
| Enum | `AgentKind` | `agents/mod.rs` |
| Config ID | `ModeConfig.id` | `loop/config.rs` |
| Schema ID | `StrategySchema.id` | `capability/schemas.rs` |
| Registry 参数 | `strategy: &str` | `capability/registry.rs` |
| API 版本 | `api_version: "v5"` | `capability/api.rs` |

这些名字描述的是**同一个东西**——三种 YAML 配置驱动的 ReAct loop 变体。命名分裂造成认知负担，开发者需要在脑中维护映射表。

### 1.3 核心洞察

> 两个候选共享同一个根因：v5 状态机的概念（strategy、state、transition）仍然污染着 v6 ReActLoop 的代码和 API。

治理策略：**删除状态机语义，统一术语为 `Mode`**。

---

## 2. 设计原则

| 原则 | 说明 |
|------|------|
| **删除虚假语义** | `states`/`transitions` 描述的是不存在的状态机，必须删除。 |
| **统一术语为 `Mode`** | `StrategySchema` → `ModeSchema`，`strategy` → `mode`，消除 strategy/state machine 概念残留。 |
| **不扩散重命名** | `AgentKind` 在 19 个文件中被引用，大规模重命名收益/成本比不高，保留但添加注释。 |
| **API 兼容性可控** | `/agent/capabilities` 前端零 consumer，修改 API 响应格式风险极低。 |

---

## 3. 具体改动

### 3.1 `capability/schemas.rs` — 删除状态机语义 + 重命名

**删除内容**：

| 符号 | 说明 |
|------|------|
| `StrategySchema.states` | 字段：`Vec<String>` |
| `StrategySchema.transitions` | 字段：`Vec<TransitionSchema>` |
| `StrategySchema.max_budget` | 字段：`u8`（静态值与实际 tier-dependent budget 不符） |
| `TransitionSchema` | 结构体（仅被 `StrategySchema.transitions` 使用） |

**重命名内容**：

| 旧名 | 新名 |
|------|------|
| `StrategySchema` | `ModeSchema` |
| `chat_schema()` | `chat_mode_schema()` |
| `rag_schema()` | `rag_mode_schema()` |
| `search_schema()` | `search_mode_schema()` |
| `standard_strategy_schemas()` | `standard_mode_schemas()` |

**保留内容**：

| 字段 | 理由 |
|------|------|
| `id` | 模式标识 |
| `requires_internet` | 有意义的产品元数据（search 需要联网） |
| `external_tools_used` | 有意义的产品元数据（search 使用 web_search） |

**修改后的 `ModeSchema` 形状**：

```rust
pub struct ModeSchema {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_tools_used: Vec<String>,
    #[serde(default)]
    pub requires_internet: bool,
}
```

### 3.2 `capability/api.rs` — 更新 API 响应

**修改内容**：

| 旧名/值 | 新名/值 |
|---------|---------|
| `CapabilitiesResponse.strategies` | `modes`（字段名 + 类型 `BTreeMap<String, ModeSchema>`） |
| `CapabilitiesResponse.api_version: "v5"` | `"v6"` |
| `build_capabilities_response()` 中的 `strategies` 构建逻辑 | 改为 `modes` |

**API 响应变化示例**：

```json
// Before
{
  "api_version": "v5",
  "strategies": {
    "chat": {
      "id": "chat",
      "states": ["Plan", "ExecuteAtomic", "Answer"],
      "transitions": [...],
      "max_budget": 1,
      "requires_internet": false
    }
  }
}

// After
{
  "api_version": "v6",
  "modes": {
    "chat": {
      "id": "chat",
      "requires_internet": false
    }
  }
}
```

### 3.3 `capability/registry.rs` — 统一内部术语

**重命名内容**：

| 旧名 | 新名 |
|------|------|
| `CapabilityRegistry.strategies` | `modes` |
| `CapabilityRegistry.strategy()` | `mode()` |
| `CapabilityRegistry.list_strategies()` | `list_modes()` |
| `CapabilityRegistry.strategy_count()` | `mode_count()` |
| `plan_tools(strategy: &str)` | `plan_tools(mode_id: &str)` |
| `answer_format_skills(strategy: &str)` | `answer_format_skills(mode_id: &str)` |
| `answer_writing_styles(strategy: &str)` | `answer_writing_styles(mode_id: &str)` |
| `answer_behavior_modes(strategy: &str)` | `answer_behavior_modes(mode_id: &str)` |

**注意**：`plan_tools()` 等方法内部从 `self.tools` 过滤，而 `self.tools` 永远是空的（ADR-0007）。这些方法已经死了，但本次改动**只重命名参数**，不删除方法（方法删除属于单独的 dead code 清理）。

### 3.4 `capability/mod.rs` — 更新 re-export

```rust
// Before
pub use api::{CapabilitiesResponse, SkillCapability, StrategySchema, ToolCapability, TransitionSchema, build_capabilities_response};
pub use schemas::{chat_schema, rag_schema, search_schema, standard_strategy_schemas};

// After
pub use api::{CapabilitiesResponse, ModeSchema, SkillCapability, ToolCapability, build_capabilities_response};
pub use schemas::{chat_mode_schema, rag_mode_schema, search_mode_schema, standard_mode_schemas};
```

### 3.5 `agents/mod.rs` — `AgentKind` 保留，清理注释

**不改动**：`AgentKind` enum 本身（19 文件引用，大规模重命名收益/成本比不高）。

**修改**：在 `parse("general")` 处添加明确注释：

```rust
/// Parse agent type string into canonical AgentKind.
/// `general` is accepted as a **legacy compatibility alias** for `Chat`
/// (retained because E2E tests and historical API clients still use it).
pub fn parse(agent_type: &str) -> Option<Self> {
    match agent_type.to_ascii_lowercase().as_str() {
        "chat" | "general" => Some(AgentKind::Chat),
        // ...
    }
}
```

### 3.6 其他文件的引用更新

以下文件中有 `StrategySchema` / `TransitionSchema` / `strategy_id` / `standard_strategy_schemas` 的引用，需同步更新：

| 文件 | 更新内容 |
|------|----------|
| `agents/sse_sink.rs` | 若 `RoutingDecision` 事件保留（简化版），其 `strategy_id` 字段已在 RouterPolicy 移除方案中处理 |
| `agents/runtime.rs` | `routing_decision` 字段（若保留）中的 `strategy_id` → `mode_id` |
| `agents/unified/mod.rs` | 若直接构造简化版 `RoutingDecision`，使用 `mode_id` |

---

## 4. `AgentKind` 为什么不重命名？

| 因素 | 分析 |
|------|------|
| **引用范围** | 19 个文件中有 `AgentKind` 引用，涉及 enum 定义、`.parse()` 调用、`match` 分支、`Display` 实现、serde 序列化、大量测试 |
| **收益** | `AgentMode` 与 `ModeConfig`/`ModeSchema` 语义对齐，提升可读性 |
| **成本** | 重命名 19 个文件中的所有引用，需要大量机械改动 + 全量测试验证 |
| **收益/成本比** | 低。`AgentKind` 本身是一个清晰的命名（kind = 类型/种类），不会造成显著困惑 |
| **最混乱的术语** | 实际上是 `strategy`（StrategySchema、strategy_id），而非 `kind`。`strategy` 已在本次方案中全面替换为 `mode` |

结论：**保留 `AgentKind`，通过文档统一术语定义**。

---

## 5. 测试修复清单

| 测试文件 | 修复内容 |
|----------|----------|
| `schemas.rs` tests | 删除 `states`/`transitions`/`max_budget` 断言；更新结构体名 `StrategySchema` → `ModeSchema` |
| `api.rs` tests | 更新 `strategies` → `modes`；更新 `api_version` 断言 `"v5"` → `"v6"`；删除 `max_budget` 断言 |
| `registry.rs` tests | 更新 `list_strategies()` → `list_modes()`，`strategy_count()` → `mode_count()` 等 |
| `events.rs` tests | 若 `RoutingDecision` 保留简化版，更新其字段名（已在 RouterPolicy 移除方案中处理） |

---

## 6. 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 外部 API 消费者依赖 `/agent/capabilities` 的 `strategies` 字段 | 极低 | API break | 前端零 consumer；API version 从 v5 改为 v6，语义变化是预期的 |
| 外部 API 消费者依赖 `states`/`transitions`/`max_budget` | 极低 | API break | 这些字段描述的是不存在的状态机，消费者即使依赖也是在依赖虚假信息 |
| `AgentKind::parse("general")` 删除导致 E2E 失败 | 无 | — | 本次方案**保留** `general` 别名，仅添加注释 |
| 重命名遗漏 | 低 | 编译错误 | `cargo build` 会捕获所有遗漏的引用 |

---

## 7. 实施 Checklist

- [ ] 修改 `capability/schemas.rs`：
  - [ ] `StrategySchema` → `ModeSchema`
  - [ ] 删除 `states`、`transitions`、`max_budget` 字段
  - [ ] 删除 `TransitionSchema`
  - [ ] 函数重命名：`chat_schema` → `chat_mode_schema` 等
- [ ] 修改 `capability/api.rs`：
  - [ ] `strategies` → `modes`
  - [ ] `api_version: "v5"` → `"v6"`
  - [ ] 更新 `build_capabilities_response()`
- [ ] 修改 `capability/registry.rs`：
  - [ ] `strategies` → `modes`
  - [ ] `strategy()` → `mode()`
  - [ ] `list_strategies()` → `list_modes()`
  - [ ] `strategy_count()` → `mode_count()`
  - [ ] 方法参数名 `strategy` → `mode_id`
- [ ] 修改 `capability/mod.rs`：更新 re-export
- [ ] 修改 `agents/mod.rs`：在 `parse("general")` 处添加 legacy 注释
- [ ] 修改 `agents/runtime.rs`（若 `routing_decision` 保留）：`strategy_id` → `mode_id`
- [ ] 修改 `agents/unified/mod.rs`（若构造简化版 `RoutingDecision`）：`strategy_id` → `mode_id`
- [ ] 更新所有测试（schemas/api/registry）
- [ ] 运行 `cargo test` 全量通过
- [ ] 运行 `cargo clippy` 无新 warning
- [ ] 更新 `CONTEXT.md`：统一术语定义（AgentKind / ModeConfig.id / ModeSchema.id）

---

## 8. 术语表

| 术语 | 定义 |
|------|------|
| **ModeSchema** | 重命名后的 `StrategySchema`。描述 ReAct loop 执行模式的**静态元数据**（id、requires_internet、external_tools_used），不再包含虚假的状态转移描述。 |
| **StrategySchema** | 废弃的 v5 概念。原用于描述状态机的 states 和 transitions。已删除。 |
| **TransitionSchema** | 废弃的 v5 概念。描述状态机中两个状态之间的允许转移。已删除。 |
| **AgentKind** | 保留的 enum 名（Chat/Rag/Search）。虽然从严格术语统一角度应改名为 `AgentMode`，但引用范围过广（19 文件），收益/成本比不高。在 `CONTEXT.md` 中与 `ModeConfig.id`、`ModeSchema.id` 定义为同义词。 |
| **general** | `AgentKind::parse()` 的遗留兼容别名，映射到 `Chat`。E2E 测试和历史 API 客户端仍在使用，保留但标记为 legacy。 |
| **api_version** | `/agent/capabilities` API 的版本标识。从 `"v5"` 更新为 `"v6"`，反映从状态机架构到 ReActLoop 架构的迁移完成。 |
