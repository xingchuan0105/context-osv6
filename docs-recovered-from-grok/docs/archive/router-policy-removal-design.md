# RouterPolicy 移除设计方案

> ⚠️ **ARCHIVED 2026-06-13** — 本文档已实现并归档。
> 实际删除：`capability/router.rs`（~660 行）。`unified::mod` 直接基于 `request.kind` 生成数据；`runtime.routing_decision` 简化为 `Option<String>`；SSE `RoutingDecision` telemetry 仍保留。
> 现行交付记录：`docs/agents/ARCHITECTURE-REVIEW-2026-06-09-SUMMARY.md`（§2.2）。

> Status: **Implemented** (2026-06-09, commit `1c89852`)
> Author: 架构审核讨论（候选 2：RouterPolicy 浅层透传）  
> Date: 2026-06-09  
> Related: `capability/router.rs` 废弃、ADR-0006 (ReActLoop 迁移)

---

## 1. 背景与动机

### 1.1 问题陈述

`RouterPolicy`（`capability/router.rs`）是一个**接口几乎与实现一样复杂的浅层模块**。它实现了完整的规则引擎（条件匹配、优先级排序、风险等级 tie-breaking、意图分类），但 `UnifiedAgent::run()` 的实际执行路径完全不依赖它的决策结果——始终直接按 `request.kind` 硬编码匹配。

当前 `RouterPolicy` 的唯一产出是：
- `AgentEvent::RoutingDecision`（SSE 流事件）
- `audit::routing_decision_record`（审计日志记录）
- `AgentRunResult.routing_decision`（返回字段）

这些产出都是 `request.kind` 的**镜像数据**（`strategy_id` = `"chat"` / `"rag"` / `"search"`），不产生任何增量信息。

### 1.2 产品决策

经讨论确认：
- **`RoutingDecision` telemetry 无产品价值**——dashboard 和日志可直接从 `request.kind` 获取相同信息；
- **Auto mode 有产品计划，但不会用 RouterPolicy 实现**——将以 **subagents 方式**实现（多个 specialist agent 协作，而非单一路由器规则引擎）；
- **`doc_scope` 空检查保留在 `UnifiedAgent`**——这是执行边界验证，不属于路由策略。

### 1.3 核心洞察

> RouterPolicy 是为一个"假设的 future work"（auto-routing）支付的架构债务，而这个 future work 已被产品决策否决（subagents 方案替代）。

保留它只会给每个维护者增加认知负担："这个看起来很复杂的规则引擎，为什么主循环不用它？"

---

## 2. 设计原则

| 原则 | 说明 |
|------|------|
| **Deletion test 通过** | 删除 RouterPolicy 不会将复杂性推回调用方——因为路由行为本就实际存在于 `UnifiedAgent` 的 `match request.kind` 中。 |
| **Observability 不降级** | 若现有 consumer 依赖 `routing_decision` 字段，保留字段但直接由 `UnifiedAgent` 赋值；若不依赖，彻底删除。 |
| **Auto mode 不阻塞** | 移除 RouterPolicy 不会阻塞未来的 subagents-based auto mode——两者架构方向完全不同。 |

---

## 3. 废弃范围

### 3.1 删除项

| 文件/符号 | 说明 |
|-----------|------|
| `crates/app/src/agents/capability/router.rs` | 完整模块（~480 行代码 + ~180 行测试） |
| `capability/mod.rs` 中的 `pub mod router` | 模块引用 |
| `capability/mod.rs` 中的 `pub use router::{standard_policy, ...}` | 公共 re-export |
| `AgentEvent::RoutingDecision` | SSE 事件变体（若确认无 consumer） |
| `audit::routing_decision_record` | 审计记录生成函数（若确认无 consumer） |
| `AgentRunResult.routing_decision` | 返回字段（若确认无 consumer） |

### 3.2 保留项

| 文件/符号 | 说明 |
|-----------|------|
| `UnifiedAgent::run()` 中的 `match request.kind` | 这是真正的路由逻辑，保留 |
| `UnifiedAgent::run()` 中的 `doc_scope.is_empty()` 检查 | RAG 执行前置验证，保留 |
| `Intent` 枚举（若其他模块使用） | 需 grep 确认；若仅 router.rs 使用，一并删除 |

---

## 4. 对现有代码的修改

### 4.1 `unified/mod.rs`

**当前代码**（约 line 79-114）：

```rust
// v5: RouterPolicy produces an observable routing decision.
let router_policy = crate::agents::capability::standard_policy();
let routing_decision = router_policy.resolve(&request);
let _ = sink
    .emit(AgentEvent::RoutingDecision {
        strategy_id: routing_decision.strategy_id.clone(),
        matched_rule: routing_decision.matched_rule.clone(),
        confidence: routing_decision.confidence,
        explanation: routing_decision.explanation.clone(),
    })
    .await;

// Emit audit record for routing decision.
let org_id = ...;
let actor_id = ...;
let audit_record = audit::routing_decision_record(...);
let _ = sink.emit(AgentEvent::Audit { record: audit_record }).await;
```

**修改后**（删除 RouterPolicy 调用，直接基于 `request.kind` 生成必要数据）：

```rust
// 若 telemetry/audit 仍需保留，直接内联：
let strategy_id = request.kind.as_canonical_str().to_string();
let _ = sink
    .emit(AgentEvent::RoutingDecision {
        strategy_id: strategy_id.clone(),
        matched_rule: format!("user-{}", strategy_id),
        confidence: 1.0,
        explanation: format!("user explicitly selected {:?} mode", request.kind),
    })
    .await;

// 若 RoutingDecision / audit 确认无 consumer，则整段删除。
```

**注意**：`result.routing_decision = Some(routing_decision.clone());`（line 153, 212, 258）同步修改。

### 4.2 `events.rs`

**若 `AgentEvent::RoutingDecision` 确认无 consumer**：
- 删除 `RoutingDecision` 变体
- 删除相关序列化/反序列化测试

**若有 consumer**（如前端依赖此事件显示当前模式）：
- 保留变体，但简化字段（`matched_rule` / `confidence` / `explanation` 可去重）
- 由 `UnifiedAgent` 直接构造，不再经过 RouterPolicy

### 4.3 `runtime.rs`

**若 `AgentRunResult.routing_decision` 确认无 consumer**：
- 删除该 `Option<RoutingDecision>` 字段
- 更新 `Default` 实现

**若有 consumer**（如后端日志分析依赖）：
- 保留字段，但将类型改为 `Option<String>`（只保留 `strategy_id`）
- 或直接保留 `RoutingDecision` 结构体（从 router.rs 迁移到 runtime.rs）

### 4.4 `audit.rs`

**若 `audit::routing_decision_record` 确认无 consumer**：
- 删除该函数

**若有 consumer**：
- 简化函数签名，直接接收 `strategy_id: &str` 而非 `RoutingDecision`

---

## 5. 与未来 Auto Mode 的关系

### 5.1 产品方向：Subagents

未来的 auto mode 将以 **subagents** 架构实现：

```
User Query
    │
    ▼
[Orchestrator Agent] ──► 分析意图、上下文、可用资源
    │
    ├──► [Chat Subagent]     ──► ReActLoop + chat.yaml
    ├──► [RAG Subagent]      ──► ReActLoop + rag.yaml + doc_scope
    ├──► [Search Subagent]   ──► ReActLoop + search.yaml + web_search
    └──► [Specialist Agent N] ──► 未来扩展（如 code_interpreter、data_analyst）
```

与 RouterPolicy 的关键差异：

| | RouterPolicy（废弃） | Subagents（未来） |
|---|---|---|
| **决策方式** | 规则引擎（关键字匹配、优先级排序） | LLM-based 意图理解 + 上下文感知 |
| **执行模型** | 单 agent，路由后进入不同模式配置 | 多 agent 协作，各自持有独立状态 |
| **扩展性** | 新增规则需修改代码 + 部署 | 新增 specialist 是添加新 agent 模块 |
| **可解释性** | 规则名 + priority 数字 | agent reasoning chain |

### 5.2 移除 RouterPolicy 不阻塞 Subagents

两者在代码层面无耦合：
- RouterPolicy 位于 `capability/router.rs`，依赖 `AgentRequest` 和 `AgentKind`；
- Subagents 将位于新的 `agents/subagents/` 目录（或类似位置），依赖 `ReActLoop` 和 `Agent` trait。

移除 RouterPolicy 反而**清理了命名空间**——`strategy_id`、`RoutingDecision`、`RouterRule` 等术语不再与过时的"单 agent 路由"概念绑定，未来 subagents 可以用自己的术语体系（`orchestrator`、`specialist`、`delegation` 等）。

---

## 6. 测试影响

### 6.1 需删除的测试

| 测试文件 | 测试内容 | 说明 |
|----------|----------|------|
| `router.rs` 内联 tests | ~180 行 router 规则引擎单元测试 | 随模块一并删除 |

### 6.2 需更新的测试

| 测试文件 | 更新内容 |
|----------|----------|
| `unified/mod.rs` tests | 移除对 `routing_decision` 字段的断言（若字段删除） |
| `events.rs` tests | 移除 `RoutingDecision` 的 serde roundtrip 测试（若变体删除） |
| 集成测试 | 若测试断言 SSE 流中包含 `RoutingDecision` 事件，更新为 `Activity` 事件或直接移除断言 |

---

## 7. 实施 Checklist

### Phase 1：确认 Consumer（关键阻塞项）

- [ ] Grep 全仓库确认 `RoutingDecision` 的 consumer（前端、日志 pipeline、测试）
- [ ] Grep 全仓库确认 `routing_decision` 字段的 consumer
- [ ] Grep 全仓库确认 `audit::routing_decision_record` 的 consumer
- [ ] Grep 全仓库确认 `Intent` / `infer_intent` / `RouterPolicy` / `RouterRule` / `RouterCondition` 的外部使用
- [ ] 基于 consumer 确认结果，决定是"彻底删除"还是"简化内联"

### Phase 2：代码清理

- [ ] 删除 `capability/router.rs` 文件
- [ ] 清理 `capability/mod.rs` 中的引用和 re-export
- [ ] 修改 `unified/mod.rs`：删除 RouterPolicy 调用，直接基于 `request.kind` 处理（或整段删除 telemetry/audit）
- [ ] 修改 `unified/mod.rs`：更新 `result.routing_decision` 赋值（或直接删除字段）
- [ ] 修改 `events.rs`：删除或简化 `RoutingDecision` 变体
- [ ] 修改 `runtime.rs`：删除或简化 `routing_decision` 字段
- [ ] 修改 `audit.rs`：删除或简化 `routing_decision_record` 函数

### Phase 3：测试修复

- [ ] 删除 `router.rs` 的测试文件（若测试在独立文件中）
- [ ] 更新 `events.rs` 的 serde roundtrip 测试
- [ ] 更新集成测试中涉及 `RoutingDecision` 事件的断言
- [ ] 运行 `cargo test` 全量通过

### Phase 4：文档更新

- [ ] 更新 `CONTEXT.md`：删除或更新 `RouterPolicy` 相关术语
- [ ] 更新 `docs/agents/` 中的相关文档
- [ ] 在 `loop-optimizer-design.md` 中交叉引用本方案（两者同属"废弃指挥官模型残留"的清理工作）

---

## 8. 术语表

| 术语 | 定义 |
|------|------|
| **RouterPolicy** | 废弃的 v5 路由策略模块。基于规则引擎的条件匹配 + 优先级排序，决定 agent 执行模式。因实际执行路径不依赖其输出，且 auto mode 将以 subagents 实现，故移除。 |
| **RoutingDecision** | 废弃的（或简化的）SSE 事件/返回字段。原用于记录 RouterPolicy 的决策结果，现为 `request.kind` 的镜像数据，无增量价值。 |
| **Subagents** | 未来的 auto mode 架构方向。由 Orchestrator Agent 根据意图分析将任务委派给多个 Specialist Subagent（Chat/RAG/Search/Code 等），各自独立执行。与 RouterPolicy 的规则引擎有本质区别。 |
| **Deletion test** | 架构审核原则：想象删除一个模块。若复杂度消失 → 它是透传层；若复杂度被推回 N 个调用方 → 它在创造价值。RouterPolicy 通过 deletion test（删除后无复杂度增加）。 |

---

## 9. 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 遗漏 consumer | 中 | 编译/运行时错误 | Phase 1 必须完成全仓库 grep，确认所有 consumer |
| 前端依赖 `RoutingDecision` 事件 | 低 | SSE 解析失败 | 若存在，保留事件变体但简化生成逻辑，不移除 |
| 日志 pipeline 依赖 `routing_decision` 字段 | 低 | 日志缺失 | 若存在，保留字段但直接赋值 `request.kind` 字符串 |
| Auto mode 被误伤 | 低 | 未来功能受阻 | 已在 §5 明确说明 subagents 与 RouterPolicy 无耦合 |
