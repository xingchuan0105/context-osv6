---
name: search
description: "Web search — fan-out queries and fetch pages in a Python sandbox"
disclose_at: retrieve
atomic: true
applicable_modes: [search]
version: "3.0"
---

## 环境

在 **Python 沙箱**调网页检索（与工作区检索同一套多轮：写代码 → 看回传 → 再写）。每轮只执行**第一个** `<code language="python">` 块；块内可并行多条 `await`。

可引用网页事实 = 回传中实际出现的搜索摘要或 `fetch` 正文。URL 与序号以回传为准。

## 可用方法（本能力开通）

| 作用 | 调用 | 说明 | 局限 |
|------|------|------|------|
| 网页搜索 | `await client.web(query)` | 事实/新闻；可并行多条 query | 单语 query 易漏另一语种索引；摘要可能过短 |
| 打开页面 | `await client.fetch(url)` | 摘要不够时拉全文 | 未 fetch 的页面全文未知；print 宜截取要点 |
| 工作区（若开通） | `await client.dense(query)` 等 | 对照本地文档；完整方法见 **codegen** | 与网页证据分源引用 |
| 跨块存储 | `await client.save` / `load` | 相对路径 | 新进程不保留变量 |

未列入上表的方法（如 `grep`）在本能力单独开通时不可用。

```python
import asyncio
zh, en = await asyncio.gather(
    client.web("立项报告 IT 转型"),
    client.web("project initiation report IT transformation"),
)
print(zh)
print(en)
page = await client.fetch(zh["results"][0]["url"])  # 字段以回传为准
```

## 查询与覆盖（环境事实）

| 因素 | 对回传的影响 |
|------|----------------|
| 中 / 英 query | 两侧索引不同；双语并行通常扩大覆盖 |
| 多实体 / 对比两侧 | 各侧独立 query 时，覆盖状态按侧分别判断 |
| 时效词（年份、latest） | 影响新闻/行情类命中是否贴近年份 |
| 来源类型 | 官方/标准可信度较高；媒体/维基居中；论坛/营销较低，冲突时并陈 |
| 摘要 vs `fetch` | 摘要够支撑主张则不必 fetch；需要原文句子或数字时 fetch 后以正文为准 |
| 空结果 | 该 query 未命中；更换表述或语言后状态可改变 |

## 引用

最终答复中网页序号写作 `[[web:n]]`，与回传结果序号一致。若同时有工作区命中，文档侧用末行 `SELECTED: #n`，与 `[[web:n]]` 分源，不混挂。
