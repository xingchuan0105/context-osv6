# ADR 0007: Product Apps + Composition Root

## Status

**Accepted** — 2026-07-10

## Context

TN 拆分后产品入口仍集中在 `AppState` Bound faces。功能上线加速时，门面债会指数堆积。需要在不大爆炸重写的前提下迁到最佳实践形态。

## Decision

1. **Composition root:** `AppState` 只装配产品 App 与运行时依赖，**禁止**新增业务方法。
2. **Product Apps:** 用例入口按产品面：`WorkspaceApp`、`AgentApp`、`WriteApp`、`ShareApp`、`BillingApp`、`AdminOpsApp`（含 admin API keys）、`PrefsApp`。
3. **Strangler:** 按面迁移 Bound 方法体进对应 App；切片内删生产调用 Bound；日常 L1。
4. **Write 永久独立:** `write_refine_*` 与写作控制环 **永不** 注册进 `ToolCatalog` / mode `tool_pool` / Capabilities 全表；Chat/RAG/Search 工具执行只走 `dispatch_tool`。
5. **C4 不做:** Capability / Skill / Tool 三层保留（ADR-0006 §5a / #13）。
6. **目录（阶段 A）:** `app-bootstrap/src/product_apps/`；后期可再拆 crate。

## Consequences

- 新功能只进目标 Product App。
- Transport/MCP 薄接线；回归按 App 面。
- 迁移计划：`docs/engineering/PRODUCT_APP_ARCHITECTURE_MIGRATION_PLAN_2026-07-10.md`。

## Non-goals

- Write 并入 ReAct ToolCatalog
- 大爆炸删光 AppState
- 强制 PR 真 LLM / 全 Playwright
