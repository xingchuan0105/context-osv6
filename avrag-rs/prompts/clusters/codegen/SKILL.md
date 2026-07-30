---
name: codegen
description: "Workspace retrieval — write Python in a sandbox to search documents"
disclose_at: retrieve
atomic: true
applicable_modes: [rag]
version: "3.0"
---

## 证据与权威源

工作区文档是本任务中**文档侧事实的权威来源**。可引用证据 = 本轮及历史轮次里 **代码执行回传**（`<code_execution_result>`）中实际出现的内容。

- 回传未覆盖的断言处于 **未知** 状态：可写「当前回传未覆盖」，不等于「语料中一定不存在」。
- 高 `score` 的片段仍可能只覆盖概念、不覆盖数字/表行；行级结论需要行级回传（常见于 `grep` 的 hits / `total_hits`）。
- 大段原文 `print` 会占满回传窗口；短列表与关键字段更利于后续轮次使用。

## 沙箱

检索在 **Python 沙箱**完成。每轮回复若有多个 `<code language="python">` 块，**只执行第一个**；同一块内可多条 `await` 并行。

- 结果以 `<code_execution_result>` 回传。每个代码块是**新进程**：变量不跨块；跨块用 `save` / `load`（相对路径）。
- 不允许 `import os, subprocess, socket, sys, ctypes, shutil` 等。
- 本能力下的检索入口是 **`client.方法名(...)`**。同名点选式原生工具不在此沙箱契约内。

## 可用方法

| 作用 | 调用 | 适用 | 局限 / 风险 |
|------|------|------|-------------|
| 语义相近 | `await client.dense(query)` | 概念、定义、换说法 | 易偏主题叙述；金额/编号/表内字面可能漏 |
| 关键词 | `await client.lexical(query)` | 编号、日期、金额、表内字面；结果可能附带关系上下文 | 同义改写、简称可能 0 命中 |
| 按行查找 | `await client.grep(pattern, doc_ids=None, regex=False, context=0, max_hits=50)` | 行计数、表记录、精确字面、顺序邻域 | pattern 与库内空白/管道格式不对齐时假 0；`total_hits` 是**命中行数** |
| 文档结构 | `await client.doc_profile(doc_ids=None)` | 章节地图 | 不是证据正文 |
| 摘要 | `await client.doc_summary(level="doc", doc_ids=None)` | 整篇/章节概览 | 摘要不是逐字证据 |
| 跨块存储 | `await client.save(path, data)` / `await client.load(path)` | 中间结果 | 仅相对路径 |

没有 `top_k`；没有 `graph_search`、`read_lines`。

**返回形状**

- `dense` / `lexical` / `doc_*` → **list[dict]**（常见字段 `chunk_id`、`content`、`doc_id`、`score`、`alias`）。
- `grep` → **dict**：`total_hits`（命中行数）、`hits[]`、`truncated`（是否因上限截断）。

```python
chunks = await client.dense("概念定义")
hits = await client.lexical("保修年限")
g = await client.grep(r"\|\s*概念阶段\s*\|", regex=True, context=2)
print("total_hits=", g["total_hits"], "truncated=", g.get("truncated"))
for h in g["hits"][:5]:
    print(h["line"], h["text"][:100], h.get("before"), h.get("after"))
await client.save("cands.json", chunks)
```

## 空结果、截断与失败

| 观察 | 含义 |
|------|------|
| `dense`/`lexical` 返回 `[]` | 该 query 下无片段入选；换说法/换方法后可能仍有结果 |
| `grep`：`total_hits=0` | 该 pattern 无行命中；pattern 形态（如 `\| 值 \|`）常影响结果 |
| `truncated=true` 或 hits 达 `max_hits` | 回传是样本，不是全库枚举；计数结论以 `total_hits` 为准，正文以已见 hits 为准 |
| list 非空但无目标字段 | 主题相关 ≠ 主张已覆盖 |
| `stderr` 非空 | 执行失败；下一轮可给修正后的同一形式代码块 |
| 未调用某方法 | 该方法下的证据状态仍为未知，不是 0 命中 |

同块并行多种方法时，各方法的空/非空彼此独立。

## 表格（管道行）

工作区表格转文本后多为 `| 列1 | 列2 | … |`：

- **一行 = 一条记录**；表头给列命名；完整事实是「表头/邻列含义 + 该格」。
- **`total_hits` = 命中行数**；某一列值重复不改变行数含义。
- 表内「第一个」常指 **表中出现顺序** 或显式序号列，不是编码标签的字典序。
- 单元格两侧常有 `|`；`grep(..., regex=True, context=…)` 的 before/after 提供邻行邻列。

完整 ontology 与虚构示例见 **how-to-read-tables**（默认随本说明披露）。

## 采用了哪些命中

命中常带编号字段 **`alias`**（如 `#1`）。最终答复若采用其中几条，**末行**写：

`SELECTED: #1, #3`

编号来自回传中的 alias。历史轮次回传里已出现的 alias 仍有效。
