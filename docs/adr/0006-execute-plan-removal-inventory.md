# ADR 0006 follow-up: Execute-plan 运行时删除清单

## Status

**Mostly done** — product route **physically removed** (2026-07-09)  
目标：**2026-09-30** 前清掉 DTO/测试夹具残余；内部 `RagRuntime::execute_plan` 仍作单测检索 harness。

## Decision (from ADR 0006 §5)

- 运行时 **只认 AgentLoop + ToolCall**。  
- **Execute-plan 运行时弃用**；入口 / 前端 / 文档设删除期限。  
- DTO 可过渡保留以便外部/脚本兼容，但 **不得** 再作为产品主路径。

## Inventory（当前代码面）

| 层级 | 路径 / 符号 | 处置 |
|------|-------------|------|
| HTTP | `POST /rag/execute-plan` — `transport-http` `routes/rag.rs` + `rag_execute_plan_handler` | **删除或 410 Gone**（窗口内） |
| App | `ChatContext::execute_rag_execute_plan` — `app-chat/src/rag_execute.rs` | 删除 |
| Runtime | `RagRuntime::execute_plan` — `rag-core/src/runtime/execute.rs` | 删除或内联到仅测试夹具 |
| Policy | `rag-core/src/execute_plan_policy.rs`（validate / convert） | 删除；检索计划仅 ToolCall 路径 |
| Prompts | `app-chat/src/prompts/plan.rs` 中 `ExecutePlanRequest` 归一化/fallback | 收敛为内部 ToolCall 构造或删除 |
| Contracts | `contracts::ExecutePlanRequest` / `ExecutePlanResponse` | 标记 deprecated → 下下个 minor 删除 |
| Tests | `transport-http/tests/rag_execute_plan_contract.rs`、`delegate_contract`、`rag-core` execute_plan 单测 | 改为 ToolCall 检索契约或删除 |
| Scripts | `.hermes/scripts/*execute_plan*` | 改 chat/agent 路径或归档 |
| Docs | 各 plan 中「深模块 execute-plan」描述 | 更新为 AgentLoop + tools |

## 非目标

- Agent loop 内部的「plan 步骤」语义（规划工具、RAG tool）**不是**本清单对象。  
- 历史 analytics 中 `execute_plan` 字段可保留只读。

## 删除步骤（建议顺序）

1. **文档/产品**：API 文档与前端确认无 execute-plan 调用（当前 frontend 不直调）。  
2. **观测**：对 `/rag/execute-plan` 打 deprecation 日志 + metric 一周，确认近零流量。  
3. **HTTP 410**：handler 固定返回 Gone + 迁移提示（保留 1 个 patch 周期）。  
4. **删运行时**：去掉 `execute_rag_execute_plan` 与 `RagRuntime::execute_plan` 产品路径。  
5. **删契约与测试**：contracts + 专属 contract 测试。  
6. **脚本/文档清扫**。

## 验收

- [x] 生产入口 **物理删除** `/rag/execute-plan`（404）  
- [x] App 层 `execute_rag_execute_plan` 已删除  
- [x] Contract 测试断言路由不存在  
- [ ] `semble search "execute_plan"` 仅剩注释/归档/changelog/legacy DTO  
- [x] RAG 产品主路径 chat SSE + ToolCall  
- [x] Contracts DTO 文档标注 deprecated  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 初版清单与目标日 2026-09-30 |
| 2026-07-09 | HTTP + App 产品路径 410；contract 测试更新 |
| 2026-07-09 | 路由物理删除；App 方法删除；删号级联含 usage export |
