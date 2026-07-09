# ADR 0006 follow-up: Write + heavytail crate 拆分计划

## Status

**Partial** — 2026-07-09（本地 `master`）  
约束（ADR addendum #8）：**先契约/行为测试锁 Write 行为，再搬 crate**。

**已落地：**

- `write-core`：`MaterialPack`、refine types、helpers、**`WriteRefineLoopRunner` + handlers**  
- Ports：`WriteResearchPort` / `WriteActivitySink` / `WriteRefineModeHost` / `WriteParentMeta`  
- `app-chat`：adapters + `run_write_refine` + `run_write_mode` / initial research fan-out  

**仍在 app-chat：** 初始 research fan-out、card 抽取、ModeConfig 加载、ChatContext 入口。

## 目标形状

```text
write-core     → contracts + MaterialPack + WriteRefineLoopRunner (ports)
app-chat       → adapters + run_write_mode + SubagentInvoker + cards
heavytail      → draft/skeleton/metrics (write-core depends; no reverse)
```

## 门禁顺序

1. ~~锁行为~~ ✅ write-core contract tests  
2. ~~锁 refine~~ ✅ write-core helpers + app-chat handler tests  
3. ~~抽 port + 迁 runner~~ ✅  
4. 可选：初始 research 再 port 化；删 app-chat 重复薄封装  

## 非目标

- 不实现 HTTP 新端点。  
- 不在账单拆 Write 行（ADR §2）。  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 计划初稿 |
| 2026-07-09 | helpers → write-core；runner → write-core via ports |
