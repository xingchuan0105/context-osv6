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
version: "4.3"
---

## 证据与权威源

**知识库**（knowledge base）中的文档是本任务里**文档侧事实的权威来源**。可引用证据 = 本轮及历史轮次 **代码执行回传**（`<code_execution_result>`）中实际出现的内容。

- 回传未覆盖的断言处于 **未知** 状态：可写「当前回传未覆盖」，不等于「语料中一定不存在」。
- 覆盖判定以**回传正文**为准：`dense` / `lexical` 取回的 chunk 正文里实际出现的表述与数字，证据地位与 `grep` 命中行相同；`grep` 同词 0 命中只说明该 pattern 未命中行（词面差异、跨文档噪声是已知局限），不否决已在回传中的 chunk 内容。
- 高 `score` 的片段仍可能只覆盖概念、不覆盖数字/表行；**行级/计数类结论**（共几行、第几行）需要行级回传（常见于 `grep` 的 hits / `total_hits`）。
- 大段原文 `print` 会占满回传窗口；短列表与关键字段更利于后续轮次使用。

## 沙箱

检索在 **Python 沙箱**完成；沙箱基座（唯一执行入口 `<code language="python">`、每轮仅首块执行、每块新进程、`save` / `load` 跨块）见 agent-base「沙箱基座」。结果以 `<code_execution_result>` 回传。

- 本 skill 下的检索入口是 **`client.方法名(...)`**。同名点选式原生工具不在此沙箱契约内。
- 不允许 `import os, subprocess, socket, sys, ctypes, shutil` 等。

### 并行扇出

相互独立的检索调用在**同一个块**内并行发出是默认工作方式：一轮一块把多次独立调用一次回传，比一轮一个调用节省整轮 LLM 往返。同块内各方法的空/非空彼此独立；存在依赖的调用（后一调用的参数来自前一调用的返回值，如 doc_profile 的 doc_id 来自 docscope 清单）按顺序 await。

## 可用方法

| 作用 | 调用 | 适用 | 局限 / 风险 |
|------|------|------|-------------|
| 语义相近 | `await client.dense(query)` | 概念、定义、换说法 | 易偏主题叙述；金额/编号/表内字面可能漏 |
| 关键词 | `await client.lexical(query)` | 编号、日期、金额、表内字面；结果可能附带关系上下文 | 同义改写、简称可能 0 命中 |
| 按行查找 | `await client.grep(pattern, doc_ids=None, regex=False, context=0, max_hits=50)` | 行计数、表记录、精确字面、顺序邻域 | pattern 与库内空白/管道格式不对齐时假 0；`total_hits` 是**命中行数** |
| 表结构目录 | `await client.struct_catalog(doc_ids=None)` | 查看表格存储里的表：表名、列名、行数、样例行、置信度 | `relations` 为空 = 当前 scope 无表格存储，不是检索失败；多 doc 同名表时响应含 `ambiguous_relations` 表名列表，同名表查询静默归属首个出现的 doc |
| 表格查询 | `await client.struct_query(sql, doc_ids=None)` | 表内 COUNT / 过滤 / 排序 / 分组（单条 SELECT） | 仅单条 SELECT；禁 DDL/DML/文件函数；表名与列名以 catalog 为准；多 doc 同名表查询静默归属首个 doc——用 `doc_ids` 收窄范围后再查 |
| 文档画像与章节 | `await client.doc_profile(doc_ids=None)` | 单篇画像（标题/作者/文体/年代/语言）+ 章节结构；fields 为空时全量返回 | 画像与章节不是证据正文 |
| 摘要 | `await client.doc_summary(level="doc", doc_ids=None)` | 整篇/章节概览 | 摘要不是逐字证据 |
| 跨块存储 | `await client.save(path, data)` / `await client.load(path)` | 中间结果 | 仅相对路径 |

没有 `top_k`；没有 `graph_search`、`read_lines`。

**返回形状**

- `dense` / `lexical` / `doc_*` → **list[dict]**（常见字段 `chunk_id`、`content`、`doc_id`、`score`、`alias`）。
- `grep` → **dict**：`total_hits`（命中行数）、`hits[]`、`truncated`（是否因上限截断）。
- `struct_catalog` → **dict**：`relations[]`（`name`、`headers`、`n_rows`、`sample_rows`、`caption`、`unit`、`confidence`、`fts`）。**catalog 只描述表**（表名/列名/行数/样例行/置信度）；表内数值与答案只出现在 `struct_query` 的 `rows` 单元格里，catalog 本身不返回答案。
- `struct_query` → **dict**：`ok`、`columns`、`rows`、`row_count`、`truncated`、`evidence`；`ok=false` 时含 `error.code`（`forbidden` / `unknown_relation` / `no_relations` 等）。`row_count` 是 SQL **结果集**的行数，不是表总行数；COUNT/SUM 的数值在 `rows` 单元格内。
- `fts: true` = 该表建有全文索引，`WHERE fts_main_<表名>.match_bm25(row_ord, '关键词') IS NOT NULL` 是表内值检索谓词（空格分隔 token 有效；整串中文是单 token，子串发现归 grep）。`fts: false` = 无索引，此情形 match_bm25 会报 schema 不存在。

```python
import asyncio

chunks, hits, g = await asyncio.gather(
    client.dense("概念定义"),
    client.lexical("保修年限"),
    client.grep(r"\|\s*概念阶段\s*\|", regex=True, context=2),
)
print("dense n=", len(chunks), "| lexical n=", len(hits),
      "| grep total_hits=", g["total_hits"], "| truncated=", g.get("truncated"))
for h in g["hits"][:5]:
    print(h["line"], h["text"][:100], h.get("before"), h.get("after"))
await client.save("cands.json", chunks)
```

## 空结果、截断与失败

| 观察 | 含义 |
|------|------|
| `dense`/`lexical` 返回 `[]` | 该 query 下无片段入选；换说法/换方法后可能仍有结果 |
| `grep`：`total_hits=0` | 该 pattern 无行命中；pattern 形态（如 `\| 值 \|`）常影响结果 |
| `struct_catalog`：`relations=[]` | 当前 scope 无表格存储；grep/dense 仍可用，非回归 |
| `struct_query`：`ok=false, code=unknown_relation` | 所查表名不在 catalog；catalog 中有当前可见表名列表 |
| `truncated=true` 或 hits 达 `max_hits` | 回传是样本，不是全库枚举；计数结论以 `total_hits` 为准，正文以已见 hits 为准 |
| list 非空但无目标字段 | 主题相关 ≠ 主张已覆盖 |
| `stderr` 非空 | 执行失败；下一轮可给修正后的同一形式代码块 |
| 未调用某方法 | 该方法下的证据状态仍为未知，不是 0 命中 |
| 连续轮次新 alias = 0 | 同一查询形态下检索面已饱和：同义重扫返回同一批命中，覆盖状态不再改变；收窄（doc_ids / 结构下钻）或定稿是此时的典型下一步。饱和判断仅限已试过的查询形态——实体名、英文面、结构面等未试角度不受此推断 |

同块并行多种方法时，各方法的空/非空彼此独立。

## 表格（管道行）

知识库文档中的表格转文本后多为 `| 列1 | 列2 | … |`：

- **一行 = 一条记录**；表头给列命名；完整事实是「表头/邻列含义 + 该格」。
- **`total_hits` = 命中行数**；某一列值重复不改变行数含义。
- 表内「第一个」= **`row_ord`（表出现序）升序第一行**，或显式序号列，不是编码标签的字典序。
- 单元格两侧常有 `|`；`grep(..., regex=True, context=…)` 的 before/after 提供邻行邻列。

完整 ontology、**误读对照**与虚构示例见 **how-to-read-tables**（默认随本说明披露）。

## 策略层（strategies spoke）

多主张覆盖清单、读法误读对照（gotchas）、表类「摸范围 → 收窄 → 下钻」工作流与各场景默认路径，见 **knowledge-base/strategies**——首轮已随本说明披露；后续轮次若不在上下文，可用 `{"skill_request": ["knowledge-base/strategies"]}` 重新加载。

## 采用了哪些命中

命中常带编号字段 **`alias`**（如 `#1`）。最终答复若采用其中几条，**末行**写：

`SELECTED: #1, #3`

编号来自回传中的 alias。历史轮次回传里已出现的 alias 仍有效。

终答采用的**每个主张**都应能指向回传中的 alias：只圈部分命中时，未圈的主张在证据面仍处于无引用状态（judge 按引用圈定的命中核对支撑）。

与联网同时挂载时，doc 侧结论同样以 `SELECTED: #n` 末行圈定（联网侧 `[[web:n]]`）；doc 侧只陈述不圈 alias，其主张即处于无引用状态。

## 引用标记与代码执行回传格式

下列线格式由单一 grammar（`rag-core/runtime/markers`）统一解析/产出，各处含义一致：

- `[[cite:<chunk_id>]]` —— 终答文本中的**文档块引用**标记；渲染时替换为 citation 编号。只圈 `[[cite:]]` 不圈 alias 的块，其引用状态与 SELECTED 无关。
- `[[image:<chunk_id>]]` —— **内联图片块引用**标记，与 `[[cite:]]` 同为块引用（不因是图片而被丢弃）；渲染时替换为 `[[image:<编号>]]`。
- `[[web:n]]` —— 联网引用索引（挂载联网侧时使用；旧式裸 `[[n]]` 按 web 索引兼容）。

代码执行回传 `<code_execution_result>` 内部为**逐块观察行**，每块一行：

`[block N] stdout: <stdout 内容>\nstderr: <stderr 内容>`

- 成功块格式固定为 `stdout:` / `stderr:` 两个字段；失败块为 `[block N] Execution failed: <错误>`。
- 多个块按 `[block 0]`、`[block 1]`… 递增编号；回传证据判定按块内 stdout 是否携带 chunk 载体（uuid 形 id）识别，占位性输出不算证据。
