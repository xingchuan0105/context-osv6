---
name: codegen
description: "RAG retrieval SDK — minimal v0"
disclose_at: retrieve
atomic: true
applicable_modes: [rag]
---

## 执行模型

每轮输出 **一个** `<code language="python">` 代码块（不要输出多个代码块——沙箱只执行第一个）。沙箱执行后，返回值以 `<code_execution_result>...</code_execution_result>` 出现在下一轮 observation。

- 只有 `await client.*(...)` 的返回值会回到你这里；`print` 不会。
- **同一块内**可写多条 `await client.*(...)`（例如语义 + 关键词各查一次），一次执行、observation 合并各次结果——**推荐**在子查询彼此独立时同块并行，节省 iteration budget。
- 若上一轮 observation 才能决定下一轮 query（串行精化），再拆到下一检索轮。

### 禁止 import 的模块

```
os, subprocess, socket, sys, ctypes, shutil, posix, fcntl, pty,
pwd, grp, resource, signal, multiprocessing, threading
```

其余 Python 标准库可用。不能联网、不能读写本地文件、不能起子进程。

## client 方法

| 方法 | Use when |
|------|----------|
| `dense_search` | 概念、定义、观点、语义相近表述；不确定精确关键词时 |
| `lexical_search` | 精确术语、编号/代码、年份/日期、金额、地名、表格单元格里的字面值 |
| `graph_search` | 两实体/概念的关系、影响链、关联分析（A 与 B 什么关系） |
| `chunk_fetch` | 已有 `chunk_id`，需要该 chunk 完整正文 |
| `doc_profile` | 需要文档 **metadata**（作者/语言/体裁等）或 **sections**（章节标题→`chunk_id` 映射）；全量载入 doc_scope，无需事先知道 doc_id |
| `doc_summary` | 需要整篇 **纯摘要**（结构化压缩正文，无 metadata、无章节目录）；全量载入 doc_scope |

```python
# 语义检索 — Use when: 概念/定义/观点/语义相似
chunks = await client.dense_search(query="…", top_k=10, method="auto")

# 关键词检索 — Use when: 精确术语、编号、年份/日期、金额、地名
chunks = await client.lexical_search(query="…", top_k=10)

# 图/关系检索 — Use when: 实体关系、影响链、A 与 B 的关联
chunks = await client.graph_search(query="…", depth=2)

# 按 chunk_id 取完整正文
chunk = await client.chunk_fetch(chunk_id="…")

# metadata + sections（章节→chunk_id）
profile = await client.doc_profile()

# 整篇纯摘要（不传 doc_ids → doc_scope 全量）
summary = await client.doc_summary(level="doc")
```

**同块多路检索示例**（语义 + 关键词，一次执行）：

```python
semantic = await client.dense_search(query="Y冷冻设备 大连 建厂", top_k=10)
literal = await client.lexical_search(query="2019 大连", top_k=10)
```

### 不存在的方法

- 无 `client.rerank`（dense 管道内服务端自动 rerank）
- 无 `client.hybrid_search` → 用 `dense_search(..., method="auto")`
- 无 `dense_retrieval` / `lexical_retrieval` / `graph_retrieval` → 用上面对应的 `client.*_search`
- 无 `doc_summary(level="section")` → 章节目录用 `doc_profile()`

## 返回值

所有方法返回 **list**（已是 chunks 数组，无需再解包）。

每个 chunk 常见字段：

| 字段 | 说明 |
|------|------|
| `chunk_id` | UUID，用于 `[[cite:]]` 和 `chunk_fetch` |
| `content` | 正文（字段名是 `content`，不是 `text`） |
| `doc_id` | 所属文档 |
| `score` | 相关性（检索类方法） |
| `page` | 页码（可选） |

`doc_profile` 返回对象含 `sections` 数组，每项有 `title`、`heading_level`、`chunk_id` 等。

`doc_summary` 返回对象含 `summary` 字段（纯摘要正文）。

## doc_scope 行为

| 方法 | doc_ids |
|------|---------|
| `dense_search` / `lexical_search` / `graph_search` | 不需要传；服务端按工作区 doc_scope 限定 |
| `doc_profile` / `doc_summary` | **不需要传**；省略时服务端用 doc_scope **全量**载入各文档 |
| `chunk_fetch` | 不传 doc_ids；多文档 scope 时内部可能只用 first doc |

## 沙箱报错

读 `<code_execution_result>` 里的 stderr：

- `AttributeError: ... has no attribute 'X'` → 对照上文换合法方法名
- `ImportError` → 去掉被禁 import，只用 `client`

下一轮只输出 **一个** 修正后的 `<code>` 块。
