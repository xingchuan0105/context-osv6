# ADR 0003: RouterPolicy 移除与 Auto Mode 架构方向

## Status

Decided

## Context

`RouterPolicy`（`capability/router.rs`）是一个实现了完整规则引擎的路由策略模块：

- 条件匹配：`Kind`、`HasDocScope`、`QueryContains`、`IntentClassified`、`ContextLength`；
- 优先级排序 + 风险等级 tie-breaking + 字典序 tie-breaking；
- 6 条规则（`user-chat`、`user-rag`、`user-search`、`auto-rag-factual`、`auto-search-external`、`default-chat`）。

但 `UnifiedAgent::run()` 的实际执行路径完全绕过 `RouterPolicy`——它直接 `match request.kind { Chat => ..., Rag => ..., Search => ... }` 硬编码匹配。`RouterPolicy` 的 `resolve()` 结果只用于 telemetry（`AgentEvent::RoutingDecision`）和 audit，从不影响执行。

**根本原因**：`request.kind` 永远是显式的（Chat/Rag/Search），`user_overridable` 规则总是优先命中。`auto-rag-factual` 和 `auto-search-external` 规则虽然有定义，但永远不会被触发。

这意味着 `RouterPolicy` 是一个**接口几乎与实现一样复杂的浅层模块**——删除它不会将复杂性推回调用方，因为路由行为已经实际存在于 `UnifiedAgent::run()` 中。

### Auto Mode 的产品需求

产品层面有"auto mode"的计划（用户不选 Chat/RAG/Search，系统自动判断最佳模式），但团队明确否决了 `RouterPolicy` 的规则引擎方案：

- 关键字启发式（`infer_intent`）过于粗糙，无法处理复杂查询；
- 规则引擎的扩展性受限于代码部署，新增规则需要修改代码 + 发布；
- 规则的可解释性差（priority 数字 + risk level 对用户不透明）。

## Decision

**删除 `RouterPolicy`，未来 auto mode 以 subagents 架构实现。**

具体措施：

1. 删除 `capability/router.rs` 完整模块（~480 行代码 + ~180 行测试）；
2. 删除 `AgentEvent::RoutingDecision`（确认无 consumer 后）或简化为直接由 `UnifiedAgent` 基于 `request.kind` 构造；
3. 删除 `AgentRunResult.routing_decision` 字段（确认无 consumer 后）或简化为 `Option<String>`；
4. 删除 `audit::routing_decision_record`（确认无 consumer 后）；
5. `UnifiedAgent::run()` 中的 `match request.kind` 硬编码匹配保留——这是真正的路由逻辑；
6. `doc_scope` 空检查保留在 `UnifiedAgent` 中（RAG 执行前置验证）。

### Subagents 架构方向

未来的 auto mode 将采用以下架构：

```
User Query
    │
    ▼
[Orchestrator Agent] ──► LLM-based 意图分析 + 上下文感知
    │
    ├──► [Chat Subagent]     ──► ReActLoop + chat.yaml
    ├──► [RAG Subagent]      ──► ReActLoop + rag.yaml + doc_scope
    ├──► [Search Subagent]   ──► ReActLoop + search.yaml + web_search
    └──► [Specialist Agent N] ──► 未来扩展
```

与 `RouterPolicy` 的关键差异：

| | RouterPolicy（废弃） | Subagents（未来） |
|---|---|---|
| 决策方式 | 规则引擎（关键字匹配、优先级排序） | LLM-based 意图理解 + 上下文感知 |
| 执行模型 | 单 agent，路由后进入不同模式配置 | 多 agent 协作，各自持有独立状态 |
| 扩展性 | 新增规则需修改代码 + 部署 | 新增 specialist 是添加新 agent 模块 |
| 可解释性 | 规则名 + priority 数字 | agent reasoning chain |

## Consequences

- **Locality 提升**：路由逻辑不再分散在"看起来智能的规则引擎"和"实际上硬编码的分支"两处；
- **删除测试通过**：删除 `RouterPolicy` 不会增加任何调用方的实际工作量；
- **API 清理**：`/agent/capabilities` 响应中不再包含无意义的 `RoutingDecision` 元数据；
- **命名空间清理**：`strategy_id`、`RoutingDecision`、`RouterRule` 等过时术语被移除，为未来 subagents 的术语体系（`orchestrator`、`specialist`、`delegation`）让出空间；
- **不阻塞未来**：subagents 与 `RouterPolicy` 在代码层面无耦合，移除后者不影响前者。

## Related

- `docs/agents/router-policy-removal-design.md`
- `docs/agents/schema-terminology-alignment-design.md`
