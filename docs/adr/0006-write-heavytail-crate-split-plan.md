# ADR 0006 follow-up: Write + heavytail crate 拆分计划

## Status

**Partial** — 2026-07-09（本地 `master` 继续）  
约束（ADR addendum #8）：**先契约/行为测试锁 Write 行为，再搬 crate**。禁止先搬文件后补测试。

**已落地：**

- `write-core`：`MaterialPack`、refine types、**pure refine helpers**、empty-topic / unified-billing / `write:*` 契约测试  
- `app-chat::writer`：`run_write_mode`、`SubagentInvoker`、`WriteRefineLoopRunner` + handlers（agent 胶水）；helpers 为 re-export  

**边界（刻意不整包硬搬）：** runner/invoker 依赖 `ChatContext` / `UnifiedAgentService` / `AgentEventSink`；迁入 `write-core` 会反转依赖。下一批再抽 port 或保持胶水在 app-chat。## 现状锚点（勿先搬）

| 区域 | 路径 | 已有测试 |
|------|------|----------|
| Write 编排 | `app-chat/src/writer/` | `writer/mod.rs` tests、`material_pack` tests、`refine_loop/tests.rs`（含 tokio） |
| Heavytail | `heavytail` crate（已独立 member） | crate 内单测 / experiment bin |
| 模式枚举 | `app-chat/src/agents/mod.rs` `AgentKind::Write` | parse/canonical 单测 |
| 产品 e2e | `app/tests/product_e2e/.../write_real.rs`（real LLM） | nightly |

## 拆分目标形状（建议）

```text
crates/write-core/     # 编排 + refine 契约（从 app-chat::writer 迁出）
crates/heavytail/      # 已存在；write-core 依赖它，禁止反向
app-chat               # 仅 pipeline 入口 + ChatContext 适配
```

接口保持小：`run_write_mode(ctx, request, session, stream) -> ChatExecution` 一级入口；计量仍走 `UsageObserver` + `write:*` feature 标签。

## 门禁顺序

1. **锁行为**：补齐/稳定 mock 路径 Write 契约测试（无 real LLM）— 断言 agent_type、session、usage 记账、高用量不改变账单拆行。  
2. **锁 refine**：现有 `refine_loop/tests` 全绿且列入 merge gate 相关 crate 测试。  
3. **搬 crate**：`git mv` + 改 workspace members；禁止同时改行为。  
4. **删 app-chat 内重复模块**。  

## 非目标

- 本计划不实现 HTTP 新端点。  
- 不在账单拆 Write 行（ADR §2）。  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 计划初稿；执行未开始 |
