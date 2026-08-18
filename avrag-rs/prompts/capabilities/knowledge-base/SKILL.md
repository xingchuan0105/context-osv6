---
name: knowledge-base
description: >-
  Knowledge-base document retrieval via Python sandbox APIs
  (client.dense / lexical / grep / struct_* / doc_*). Use when the task needs facts,
  numbers, table rows, or citations from mounted knowledge-base documents.
  Not for pure chat without a knowledge base, and not for web-only questions
  (use web / search when only internet is mounted).
disclose_at: retrieve
atomic: true
applicable_modes: [rag]
version: "6.1"
---

## 证据（硬句）

知识库文档侧事实的权威来源 = 本轮及历史轮次 **宿主执行回传**（`<code_execution_result>` 等）中实际出现的内容。回传未覆盖的断言处于 **未知 / 未覆盖**（≠ 语料一定不存在）。覆盖以回传正文为准：`dense` / `lexical` 的 chunk 正文与 `grep` 命中行同权。题干并置「文档内概念」与「业界框架」时两侧各一主张槽。行级/计数结论需要行级回传（常见于 `grep` 的 `total_hits`）。大段 `print` 会占满回传窗口。

沙箱入口、首块执行、并行 `gather` 见本轮沙箱环境段（不在此复述）。本 skill 检索入口是 **`client.方法名(...)`**；同名点选式原生工具不在此沙箱契约内。

**docscope** 是 skill_request 注入的文档清单/画像（拿 `doc_id`），不是沙箱方法。

## 可用方法（签名卡）

| 作用 | 调用 | 适用 / 局限 |
|------|------|-------------|
| 语义 | `await client.dense(query)` | 概念/换说法；宿主以 query 为种子扩邻。多实体宜拆多次并行 dense，勿整句单一种子 |
| 词面 | `await client.lexical(query)` | 编号/日期/金额/表内字面；同义改写可能 0 命中 |
| 行级 | `await client.grep(pattern, doc_ids=None, regex=False, context=0, max_hits=50)` | 行定位/计数；pattern 字面匹配优先，字面 0 命中且含正则元字符（`|` `.*` 等）时宿主自动按正则重试一次，`matched_by` 标注实际语义；`total_hits`=命中**行**数，`chunks[]`=命中行所属 chunk 全文 |
| 表目录 | `await client.struct_catalog(doc_ids=None)` | 表名/列/行数/样例；`relations=[]`=无表存储，非失败 |
| 表查询 | `await client.struct_query(sql, doc_ids=None)` | 单条 SELECT；答案在 `rows`；`row_count` 是结果集行数 |
| 档案 | `await client.doc_summary(doc_ids=None)` | metadata+summary+sections；非逐字证据正文 |
| 跨块 | `await client.save(path, data)` / `await client.load(path)` | 仅相对路径 |

无 `top_k`；无独立 `client.graph`。图扩邻由 `dense` 触发，命中与 dense 同 alias 空间。entity-first / 双端种子见 **strategies**。

**返回形状：** `dense`/`lexical`/`doc_*` → list[dict]（`chunk_id`/`content`/`doc_id`/`score`/`alias`）；`grep` → `{total_hits, matched_by, hits[], chunks[], truncated}`；`struct_catalog` → `{relations[]}`（描述表，不含答案单元格）；`struct_query` → `{ok, columns, rows, row_count, …}`；`fts: true` 时可用 `match_bm25`（细节见 **api-detail**）。

空结果 / 截断 / 失败对照表、最小成功代码形态、回传块格式：见 **api-detail**（首轮或沙箱错误后披露；也可 `{"skill_request":["knowledge-base/api-detail"]}`）。

## KEEP（工作集）

多轮工作集：

```text
KEEP: #3, #7
```

（可选 `KEEP_DROP: #5`。）无 KEEP 时宿主 sticky 上一工作集。本通道不写用户终答。

## 表格 / 策略

管道表一行一条记录；读法见 **how-to-read-tables**；表路径 few-shot 见 **strategies-tables**。  
首轮默认薄层 **strategies**（覆盖清单、entity-first、spoke 目录）。长 spoke 按需：`{"skill_request":["knowledge-base/strategies-…"]}`。
