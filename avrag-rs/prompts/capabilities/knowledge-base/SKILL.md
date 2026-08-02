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
version: "4.2"
---

## 证据与权威源

**知识库**（knowledge base）中的文档是本任务里**文档侧事实的权威来源**。可引用证据 = 本轮及历史轮次 **代码执行回传**（`<code_execution_result>`）中实际出现的内容。

- 回传未覆盖的断言处于 **未知** 状态：可写「当前回传未覆盖」，不等于「语料中一定不存在」。
- 高 `score` 的片段仍可能只覆盖概念、不覆盖数字/表行；行级结论需要行级回传（常见于 `grep` 的 hits / `total_hits`）。
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

同块并行多种方法时，各方法的空/非空彼此独立。

## 表格（管道行）

知识库文档中的表格转文本后多为 `| 列1 | 列2 | … |`：

- **一行 = 一条记录**；表头给列命名；完整事实是「表头/邻列含义 + 该格」。
- **`total_hits` = 命中行数**；某一列值重复不改变行数含义。
- 表内「第一个」= **`row_ord`（表出现序）升序第一行**，或显式序号列，不是编码标签的字典序。
- 单元格两侧常有 `|`；`grep(..., regex=True, context=…)` 的 before/after 提供邻行邻列。

完整 ontology、**误读对照**与虚构示例见 **how-to-read-tables**（默认随本说明披露）。

## 多主张覆盖（轻量清单）

用户问题含 **多个可独立核验的主张**（多个数字、多个阶段、两篇文档对照、知识库+联网各一侧等）时，常见覆盖形态：

```
Claim checklist (copy and tick against returns):
- [ ] claim A — 回传中出现支撑字段/数字/表行
- [ ] claim B — 同上
- [ ] …（按问题拆）
- [ ] 联合结论 — 仅在 A/B… 均有回传支撑时写出；缺侧标「当前回传未覆盖」
```

- 只覆盖部分主张时，未覆盖侧保持 **未知**，不拿已覆盖侧的叙述填补。
- 双源（知识库 + 联网）时，两侧证据分源引用；一侧未取回传则该侧未知。
- 最终答复前，清单上仍为未勾的项对应「回传未覆盖」，而非「语料一定没有」。
- 连续轮次回传的新 alias 为 0 = 当前检索面已饱和：同义换形重扫返回的是同一批命中，覆盖状态不再改变；此时继续加宽查询词不再产生新证据，应转入收窄（doc_ids / 结构下钻）或开始定稿。

## Known gotchas

轨迹中反复出现的读法陷阱（均可用虚构表自检；细节见 how-to-read-tables）：

| 现象 | 回传实际含义 | 常见误读 |
|------|--------------|----------|
| 标签 `ROW-04` 数值小于 `ROW-03` | 标签是名字，不是排序键 | 把「第一个」读成编号最小/字典序最先 |
| 表中先出现 `STEP-03` 再出现 `STEP-04` | 「第一个」= 出现顺序在前的那一行 | 按步骤号重排后再取 min |
| `total_hits=12` 且品名列有重复 | 12 = **命中行数** | 按品名去重后改成更小的数 |
| `dense` 高分片段只有概念叙述 | 主题相关；目标数字/表行可能仍未知 | 用叙述段「推」出未出现的数字 |
| `struct_catalog` 中 `confidence=low` | 灌入监督未全部通过；该表数字处于低置信状态 | 与 high 置信表同等引用 |
| `struct_query` 的 `row_count` | SQL **结果集**的行数；COUNT 的答案在 `rows` 单元格内 | 把结果行数当成 COUNT 值 |
| `truncated=true` 或 hits 长度 < total_hits | hits 是样本；计数以 `total_hits` 为准 | 用 `len(hits)` 当全库计数 |
| 问题字面与某段很像 | 相似 ≠ 主张已覆盖 | 跳过 lexical/grep 精确核对 |
| 多数字题只见一个数 | 其余主张仍未知 | 只答一半即结束 |
| 知识库与联网同时挂载，问题含「文章称/文中提到/报告称」 | 该前提通常指向**文档库**，可用 `dense`/`grep` 直接核实原文 | 当成外部事实只走 `client.web`，文档侧前提未覆盖 |

**默认低自由度路径（易碎结论）：**

表类问题（表内计数 / 过滤 / 表序 / 排序 / 聚合）的默认工作流是**「摸范围 → 收窄 → 下钻」一条链**：

1. **摸范围**：并行扇出 `dense` / `lexical` / `grep` 或 `struct_catalog`，确认问题落在哪个 doc、哪张表。
2. **收窄**：取到 doc_id 或表名后，后续调用带 `doc_ids=[...]`（多 doc 同名表时防止静默归属首个 doc）。
3. **下钻**：`struct_catalog` 给出可见表名与列名后，**继续**用 `struct_query` 发单条 SELECT 取答案——catalog 只描述表，答案只在 `rows` 里。

分流规则：**`grep` 数的是文本行，`struct_query` 的 COUNT/SUM 数的是记录**。表内计数、过滤、排序、聚合场景下，grep 是近似路径（按文本行/子串，可能与表结构错位），`struct_query` 是确定路径（按列与谓词）——两类场景一律先走 struct 两段式，`grep` 降为无表格存储（`relations=[]`）或纯子串/邻域场景的退路。

- **表内计数 / 过滤 / 表序 / 聚合（表类问题首选）** → **struct 两段式**：`struct_catalog`（看可见表名与列名）→ `struct_query`（COUNT/WHERE/ORDER BY/GROUP BY，单条 SELECT）；「第一个」= `row_ord` 升序第一行（表出现序），非编号字典序；`struct_query` 的 `row_count` = 结果集行数，COUNT 数值在 `rows` 单元格。
- **行计数 / 纯文本行** → `grep` + 采用 `total_hits`（不要肉眼数 hits、不要按列去重）；`struct_catalog` 返回 `relations=[]`（该 doc 无表格存储）时 grep 是可用退路。
- **表内总数（如某类对象的总数）** → `struct_query` 聚合（COUNT/SUM/GROUP BY）是确定路径；看到部分分域计数而未见总数时，总数仍处于未覆盖状态，聚合查询可闭合它。表级证据未水合（回传无 alias 编号）时，以 `evidence`/`rows` 文本核对，勿虚构编号。
- **表内「第一个 / 先后」** → 按 **回传中该过滤条件下的出现顺序**（或显式序号列）；编码字符串不做排序键。
- **金额 / 活动号 / 表内字面** → 优先 `lexical` 或 `grep`；`dense` 仅作定位线索。
- **元数据字段（日期/状态/作者/阶段数）** → 语料字段常为**英文**（如 `Date`、`Status`、`Phase`），中文语料正文用中文——检索词**中英双词并行**（`grep "Date"` 与 `grep "日期"` 都试；`Phase` 与 `阶段` 都试）；英文 0 命中不代表中文侧也无，反之亦然。

## 采用了哪些命中

命中常带编号字段 **`alias`**（如 `#1`）。最终答复若采用其中几条，**末行**写：

`SELECTED: #1, #3`

编号来自回传中的 alias。历史轮次回传里已出现的 alias 仍有效。

终答采用的**每个主张**都应能指向回传中的 alias：只圈部分命中时，未圈的主张在证据面仍处于无引用状态（judge 按引用圈定的命中核对支撑）。
