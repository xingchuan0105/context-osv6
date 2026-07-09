# ADR 0006 follow-up: Write + heavytail crate 拆分计划

## Status

**Accepted / Done** — 2026-07-09（本地 `master`）

约束（ADR addendum #8）：先契约/行为测试，再搬 crate — **已满足**。

本拆分的目标是 **domain 与 agent 胶水分离**，不是把所有 chat 管道搬进 `write-core`。  
以下边界为**有意终态**，不是半截债：

| 层 | 归属 | 内容 |
|----|------|------|
| Domain | `write-core` | 契约、`MaterialPack`、refine types/helpers、**WriteRefineLoopRunner + handlers**、ports |
| Ports | `write-core` | `WriteResearchPort` / `WriteActivitySink` / `WriteRefineModeHost` / `WriteParentMeta` |
| Adapters + 入口 | `app-chat` | `adapters`、`run_write_refine`、`run_write_mode`、初始 research fan-out、card 抽取、ModeConfig |
| 写作内核 | `heavytail` | draft / skeleton / metrics；write-core → heavytail，禁止反向 |

**不再做（本计划范围外）：**

- 把 `run_write_mode` / `ChatContext` 整包迁入 write-core（会反转依赖）  
- 初始 research fan-out 再抽一层 port（收益低；refine 内 on-demand research 已走 `WriteResearchPort`）  
- 产品 e2e / real LLM write 套件（波次末 E2E，见 SOLO_DISCIPLINE）  

## 形状

```text
write-core     → contracts + MaterialPack + WriteRefineLoopRunner (ports)
app-chat       → adapters + run_write_mode + SubagentInvoker + cards
heavytail      → draft/skeleton/metrics
```

一级产品入口仍在 app-chat：`run_write_mode` → ChatExecution；计量 `write:*` feature + 统一 rolling 账单。

## 门禁（完成核对）

1. ~~锁行为~~ ✅ `write-core` contract tests（agent_type、empty topic、unified billing、`write:*` 标签）  
2. ~~锁 refine~~ ✅ helpers + context + handler 测（write-core 19 / app-chat writer 8）  
3. ~~抽 port + 迁 runner~~ ✅  
4. ~~边界文档化~~ ✅ 本页 Status = Accepted  

本地验证（commit stage）：

```bash
cd avrag-rs
cargo test -p write-core --lib
cargo test -p app-chat --lib writer::
```

## 非目标（不变）

- 不实现 HTTP 新端点  
- 不在账单拆 Write 行（ADR §2）  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 计划初稿 |
| 2026-07-09 | helpers → write-core；runner → write-core via ports |
| 2026-07-09 | **Accepted**：边界定为终态；ADR backlog #6 关闭 |
