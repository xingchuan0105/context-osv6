# 规格草案：用量导出与 1 年保留

> 来源：ADR 0006 §10 与 Accepted addendum #6。  
> 状态：**Draft** — 不阻塞软限/worker billable 实现；存储与法务另项。

## 目标

1. 用量分析数据保留 **1 年**（以 `llm_usage_events.created_at` 为主时钟）。  
2. 客户可 **导出** 其账号（user 主体）的用量明细。  
3. 明确 **账号删除** 与用量行的级联策略。  

## 数据源

| 表 / 流 | 角色 |
|---------|------|
| `llm_usage_events` | Rolling 真相；含 `billable`（客户配额只看 `true`） |
| 派生 analytics（若有） | 必须可追溯到上表实量，禁止第二套对账数字 |

**导出默认范围**：`billable = true` 的客户可见行。  
**内部/worker**（`billable = false`）：不进客户导出；可进内部成本看板。

## 保留

| 策略 | 值 |
|------|-----|
| 默认保留 | **365 天** |
| 清理 | 定时 job：删除/归档 `created_at < now() - 365 days` |
| 归档介质 | 可选对象存储冷归档（规格 v2） |

## 导出 API（拟）

```http
POST /api/v1/usage/export
Authorization: Bearer …
Content-Type: application/json

{
  "from": "2026-01-01T00:00:00Z",
  "to": "2026-07-01T00:00:00Z",
  "format": "csv"   // csv | jsonl
}
```

**响应（异步优先）**

```json
{
  "export_id": "…",
  "status": "pending"
}
```

- 大窗口走 **异步任务** + 下载 URL（预签名，短 TTL）。  
- 同步仅允许小窗口（例如 ≤ 7 天）——实现时再定阈值。  
- **审计**：谁在何时导出、行数、时间窗写入 audit log。  

### 导出字段（最小集）

| 字段 | 说明 |
|------|------|
| created_at | 事件时间 |
| feature / stage | 内部标签；**不**拆 Write 账单行，但可保留 feature 供客户自分析 |
| provider / model | 若对客户可见策略允许 |
| prompt_tokens / completion_tokens / total_tokens | 实量 |
| usage_units | 配额单位 |
| usage_source | actual / estimated |
| session_id / request_id | 可选关联 |

**默认不含** prompt/completion 原文。若未来含片段，须单独同意与脱敏规格。

## 账号删除

| 选项 | 含义 | 建议默认 |
|------|------|----------|
| A. 硬删用量 | 删号即删 `llm_usage_events`（user_id） | 隐私友好 |
| B. 匿名化 | 保留聚合、去掉 user_id | 合规分析 |
| C. 法务冻结 | 争议期保留 | 仅工单触发 |

**建议默认：A（硬删）**，企业合同可选 B/C。实现前需法务确认。

## 非目标（本草案）

- 账单 PDF / 税务发票  
- 跨 org 管理员导出他人明细（admin 另见 ADR §8；仍须租户隔离）  
- Desktop 本地用量（不上报，无云导出）  

## 实现 backlog（派生）

1. 导出任务表 + worker  
2. CSV/JSONL 生成与对象存储  
3. 365 日清理 job  
4. 删号钩子  
5. Admin/审计事件  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 初稿（ADR 0006 backlog #8） |
