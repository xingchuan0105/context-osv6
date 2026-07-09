# ADR 0006 follow-up: Execute-plan 运行时删除清单

## Status

**Done — physical DTO delete** — 2026-07-09  

产品路由、App 入口、内部 harness、**contracts DTO** 均已移除。  
产品路径：**AgentLoop + ToolCall** only。

**执行波次**：见 [`docs/engineering/TN_CODE_QUALITY_REMEDIATION_2026-07-09.md`](../engineering/TN_CODE_QUALITY_REMEDIATION_2026-07-09.md) **Wave 2**。

## Decision (from ADR 0006 §5)

- 运行时 **只认 AgentLoop + ToolCall**。  
- Execute-plan **不得**再作为产品主路径（已删除）。

## Inventory（终态）

| 层级 | 状态 |
|------|------|
| HTTP `/rag/execute-plan` | 物理删除（404） |
| App `execute_rag_execute_plan` | 删除；仅 `execute_runtime_tools`（ToolCall） |
| `RagRuntime::execute_plan` multi-channel harness | **删除** |
| `execute_plan_policy` | **删除** |
| Prompt legacy parse / convert helpers | **删除** |
| `contracts::ExecutePlanRequest` / `Response` / Item / Budget / SummaryMode / Trace / ValidationError | **删除** |
| `ExecutePlanRequest::from_tool_calls` / `ToolCallAdapterError` | **删除** |
| `ChannelBudget`（仅 plan 用） | **删除** |
| 保留 | `RetrievalBundle`、`RetrievedChunk`、`BackendTrace`、`GraphHint`、`PlaceholderTriplet` 等结果 DTO |

## 验收

- [x] 生产入口无 execute-plan  
- [x] App / runtime 无 `execute_plan` 产品 API  
- [x] contracts **无** `ExecutePlanRequest` 类型定义  
- [x] `rg ExecutePlanRequest` 仅剩注释/文档说明  
- [x] RAG 主路径 chat SSE + ToolCall  
- [x] `cargo test -p contracts --all-targets` / `avrag-rag-core --lib` / `app-chat --lib` 绿  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 初版清单与目标日 2026-09-30 |
| 2026-07-09 | HTTP + App 产品路径删除 |
| 2026-07-09 | Wave 2.2 prompt 拒收；runtime/policy 密封 + deprecated DTO |
| 2026-07-09 | **物理删除** ExecutePlan DTO、adapter、policy、multi-channel harness |
