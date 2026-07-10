# ADR 0007: Product Apps + Composition Root

## Status

**Accepted** — 2026-07-10  

**Implementation status** — **Phase A + Phase B shipped**（R0–R5）。

## Context

TN 拆分后产品入口仍集中在 `AppState` Bound faces。功能上线加速时，门面债会指数堆积。需要在不大爆炸重写的前提下迁到最佳实践形态。

## Decision

1. **Composition root:** `AppState` 只装配产品 App 与运行时依赖，**禁止**新增业务方法。
2. **Product Apps:** 用例入口按产品面：`WorkspaceApp`、`AgentApp`/`ConversationApp`、`WriteApp`、`ShareApp`、`BillingApp`、`AdminOpsApp`（含 admin API keys）、`PrefsApp`。
3. **Strangler:** 按面迁移 Bound 方法体进对应 App；切片内删生产调用 Bound；日常 L1。
4. **Write 永久独立:** `write_refine_*` 与写作控制环 **永不** 注册进 ReAct `ToolCatalog`；执行不经 Chat/RAG/Search 的 `execute_chat` write arm；Chat/RAG/Search 工具执行只走 `dispatch_tool`。
5. **C4 不做:** Capability / Skill / Tool 三层保留（ADR-0006 §5a / #13）。
6. **目录（阶段 A）:** `app-bootstrap/src/product_apps/`；后期可再拆 crate。
7. **会话执行单入口（Phase B）：** Transport 不写 mode if；`ConversationApp::execute[_stream]` 内部分发；Write 直达 writer 公开 API。

## Implementation status

| Phase | 内容 | 状态 |
|-------|------|------|
| **A** | Bound → `product_apps` 命名；部分 transport 改 `agent`/`write_app`；catalog skip write_refine | **Shipped** |
| **B** | Conversation 单入口；Write 出 pipeline；工具单一真相；API 收口 | **Done** — [`PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md`](../engineering/PRODUCT_APP_TN_REMEDIATION_PLAN_2026-07-10.md) |

## Consequences

- 新功能只进目标 Product App / Conversation。
- Transport/MCP 薄接线；回归按 App 面。
- Phase A  alone **不等于** 架构债清零；以 Phase B 关闭 TN FAIL 项。

## Non-goals

- Write 并入 ReAct ToolCatalog
- 大爆炸删光 AppState
- 强制 PR 真 LLM / 全 Playwright
