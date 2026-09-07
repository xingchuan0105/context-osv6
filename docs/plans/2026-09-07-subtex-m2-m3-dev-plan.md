# Subtex M2 / M3 —— 收件箱确认制 + credits 耗材账本

**日期：** 2026-09-07 · **上游：** PRD F6 / F7 · **原则：** 最小端到端切片，确认后才移动；credits 只对照真实 API 耗材，不进 Agent 会话。

## 终态（本切片）

**M2：** 用户指定投放点（应用数据目录 `inbox.json`，唯一配置）。投放点出现新文件 → 规则命中则高置信建议、文件名对上唯一已接入根则中置信、否则低置信静默留队。默认不移动。`subtex.inbox` 看队列；`subtex.inbox_decide` 接受/拒绝/忽略/撤销。全自动仅 `autofill=true` 且高置信且目标已接入。移动可撤销。同模式接受 3 次后返回固化提议（接 F5 `correction_draft`）。无 LLM 分类器。

**M3：** 每笔 embedding / transcription usage 在全局仓写一条 millicredit 对照行。Agent 工具返回不含余额/单价/货币。不接 B 线 PG 钱包（T7）；云端代付账户体系后置。

## 不做

MCP elicitation 协议；OS 通知；自有复核 UI；AI 正文分类；新建目录；Agent 可见费用元素；app-billing 接线。

## 布局

```
~/.local/share/subtex/
  inbox.json     # drop_points / autofill / rules
  global.db      # suggestions / moves / accept_stats / credits
  roots/<hash>/  # 既有索引仓
```
