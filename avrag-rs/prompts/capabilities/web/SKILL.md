---
name: search
description: "Web search — Lead+Workers host leaf (all modes with web); EvidencePack then synthesis"
disclose_at: retrieve
atomic: true
applicable_modes: [search]
version: "3.4"
---

## 环境

### 联网已挂载（search-only 与 dual）

网页检索由 **Web Worker / 宿主检索叶子**完成（可多 query 并行；常带 auto-scrape/CRW 厚 snippet）。证据以宿主注入的 **`[evidence_pack]`**（及 tool 回传）为准，**不是**把上游搜索引擎全文当用户气泡。

用户可见终答由 **Lead 合成**写成自然语言；网页引用 `[[web:n]]`。

本路径**不**依赖沙箱多轮 `client.web` fan-out 作为常态。若运行时仍暴露 `client.web` / `fetch`（历史 dual SaC 面），仅作补充；主证据仍以 pack 为准。

**宿主 auto-scrape（环境事实）**：检索返回后，宿主可对排序靠前、snippet 过短的若干 URL **自动**拉页（CRW），正文写入对应 `results[].snippet`。

## 中英双语检索（环境事实）

| 事实 | 含义 |
|------|------|
| `queries[]` 逐条搜索 | 宿主对每条 query 单独调用搜索（可 DeepSeek Responses `web_search`）后按 URL 合并 |
| 中文 vs 英文 | 两侧索引与站点集合不同；**同一意图各 ≥1 条中文 + 英文** 时覆盖通常更大 |
| 质量线索词 | 中文：官方 / 标准 / 规范 / 最佳实践；英文：`official` / `standard` / `best practice` / 机构或标准名 — 提高高质量源出现概率（非硬保证） |
| 条数 | 建议 2–5；≤5；空数组时宿主回退 `original_query`（易单语偏窄） |

```python
import asyncio
# 沙箱仍挂载 client.web 时：双语并行扩大覆盖（与 host queries[] 同理）
zh, en = await asyncio.gather(
    client.web("立项报告 数字化转型 SMART 目标 最佳实践 官方"),
    client.web("project initiation report digital transformation SMART goals best practices official"),
)
print(zh)
print(en)
```

## 可用方法（沙箱仍挂载时）

| 作用 | 调用 | 说明 | 局限 |
|------|------|------|------|
| 网页搜索 | `await client.web(query)` | 事实/新闻；可并行多条 query | 单语 query 易漏另一语种 |
| 打开页面 | `await client.fetch(url)` | 自动读页未覆盖时 | 未 fetch 且 snippet 空则全文未知 |
| 知识库（若开通） | `await client.dense(query)` 等 | 对照本地文档 | 与网页证据分源引用 |
| 跨块存储 | `await client.save` / `load` | 相对路径 | 新进程不保留变量 |

## 来源质量与覆盖

| 因素 | 对回传的影响 |
|------|----------------|
| 中 / 英 query | 两侧索引不同；双语并行通常扩大覆盖 |
| 质量词 / 机构名 | 提高官方、标准、权威媒体进入 top 结果的概率 |
| 时效词（年份、latest） | 影响新闻/行情类命中是否贴近年份 |
| 来源类型 | 官方/标准较高；媒体/维基居中；论坛/营销较低 |
| snippet 已含正文 | 宿主 auto-scrape 后常见 |
| 空结果 | 该 query 未命中 |

合成侧对冲突事实：并陈并标来源层级；未见 observation 的正文视为未知。

## 引用

最终答复中网页序号写作 `[[web:n]]`，与 results 顺序一致。若同时有知识库命中，文档侧用末行 `SELECTED: #n`，与 `[[web:n]]` 分源，不混挂。
