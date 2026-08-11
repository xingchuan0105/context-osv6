---
name: search
description: "Web search — Lead+Workers host leaf (all modes with web); EvidencePack then synthesis"
disclose_at: retrieve
atomic: true
applicable_modes: [search]
version: "3.3"
---

## 环境

### 联网已挂载（search-only 与 dual）

网页检索由 **Web Worker / 宿主检索叶子**完成（可多 query 并行；常带 auto-scrape/CRW 厚 snippet）。证据以宿主注入的 **`[evidence_pack]`**（及 tool 回传）为准，**不是**把上游搜索引擎全文当用户气泡。

用户可见终答由 **Lead 合成**写成自然语言；网页引用 `[[web:n]]`。

本路径**不**依赖沙箱多轮 `client.web` fan-out 作为常态。若运行时仍暴露 `client.web` / `fetch`（历史 dual SaC 面），仅作补充；主证据仍以 pack 为准。

**宿主 auto-scrape（环境事实）**：检索返回后，宿主可对排序靠前、snippet 过短的若干 URL **自动**拉页（CRW），正文写入对应 `results[].snippet`。

## 可用方法（沙箱仍挂载时）

| 作用 | 调用 | 说明 | 局限 |
|------|------|------|------|
| 网页搜索 | `await client.web(query)` | 事实/新闻；可并行多条 query | 单语 query 易漏另一语种 |
| 打开页面 | `await client.fetch(url)` | 自动读页未覆盖时 | 未 fetch 且 snippet 空则全文未知 |
| 知识库（若开通） | `await client.dense(query)` 等 | 对照本地文档 | 与网页证据分源引用 |
| 跨块存储 | `await client.save` / `load` | 相对路径 | 新进程不保留变量 |

```python
import asyncio
zh, en = await asyncio.gather(
    client.web("立项报告 IT 转型"),
    client.web("project initiation report IT transformation"),
)
print(zh)
print(en)
```

## 查询与覆盖（环境事实）

| 因素 | 对回传的影响 |
|------|----------------|
| 中 / 英 query | 两侧索引不同；dual 双语并行通常扩大覆盖 |
| 时效词（年份、latest） | 影响新闻/行情类命中是否贴近年份 |
| 来源类型 | 官方/标准可信度较高；媒体/维基居中；论坛/营销较低 |
| snippet 已含正文 | 宿主 auto-scrape 后常见 |
| 空结果 | 该 query 未命中 |

## 引用

最终答复中网页序号写作 `[[web:n]]`，与 results 顺序一致。若同时有知识库命中，文档侧用末行 `SELECTED: #n`，与 `[[web:n]]` 分源，不混挂。
