# 文档关系面：表格存储 + DuckDB 查询（struct_query）落地计划

| 项目 | 内容 |
|------|------|
| 日期 | 2026-07-31 |
| 状态 | **已拍板，开工中（P0 全绿）** |
| 架构版本 | v3：统一 markitdown → CommonMark 提取 → 确定性校验 → **supervision agent loop** → per-doc DuckDB → host 只读查询 |
| 历史版本 | v1：runtime 解析 + TableIr 依赖（否）；v2：runtime 格式无关管道表解析（否，分片/检测风险移入灌入侧更稳） |
| 业务痛点 | xlsx/csv/pdf 都可能含大表，表文混排（财报）；按格式分派处理 → 效果与结构不一致 |
| 前置（硬依赖） | 生产 parser 统一走 **markitdown**（另案推进）；**灌原件**（xlsx/pdf），不灌导出 txt（markitdown 对 txt 纯透传，无结构可出，7-29 实证） |
| 非目标 | 真连租户 PG；LLM 逐行转录；按文件格式分派解析；TableIr 用于本功能；无表 doc 的运行时兜底猜表 |
| P0 证据 | `scripts/struct_query_poc/extract_tables.py` **4/4 PASS**（370 / 验证59·发布30 / LPDT-03 / 序号自校验） |
| P1a 证据 | `scripts/struct_query_poc/check_pipeline.py` **11/11 PASS**（pipeline 全链：IPD 370 high / 白药 9 布局网格全待诊断·638 banner 捕获 / txt 0 表） |

---

## 0. 一句话

所有格式经 **markitdown** 统一为 markdown；灌入 pipeline 用 **CommonMark parser（markdown-it-py）** 确定性提取表格为 `{headers, rows}`，**校验 SQL** 产出健康报告，**supervision agent loop** 做语义标注与修复指令（指令永远不含单元格的值），产出 **per-doc DuckDB 表格存储**；agent 查询时经 SDK 写受限 SQL，host 以**加固配置只读**打开对应 DuckDB 执行——**无表格存储返回「无表格」**，有则返回查询结果（查到/查不到）；**grep 配合**定位字段的准确写法。

---

## 1. 决策与已否方案

### 1.1 架构决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 格式统一 | 全部格式 → markitdown → markdown | 分格式处理效果/结构不一致（用户业务痛点）；markitdown 对 xlsx/pdf/docx 均产管道表（7-29 实证） |
| 表格物化时机 | **灌入时（切块前）** | 运行时方案的死结：通用切块把大表拦腰切断且块间无顺序（7-28 实测 309 接缝）；切块前整表抽走，分片/猜序问题不存在 |
| 存储 | **per-doc DuckDB 单文件** | 随对象存储生命周期（删 doc 删文件）；scope 隔离天然；灌入侧聚合不可知——只存原始行，聚合留给查询 SQL |
| 查询引擎 | **DuckDB 核心**（host 端 duckdb-rs core） | 通用查询功能的本职；COUNT/GROUP BY/NULL/类型语义全对，不写 300 行执行器 |
| 表格提取 | **markdown-it-py（gfm-like）** | ~~duckdb_markdown 扩展~~ **P0 实测丢 CJK 单元格，不可用**（见 §9）；markdown-it-py 是 Python 标准 CommonMark parser，CJK 无恙 |
| LLM 角色 | **识别 + 语义标注 + 诊断 + 修复指令**，组为 **agent loop** | 逐行转录精度与上下文窗口双重不可行；LLM 提议、确定性代码应用、SQL 复验裁决 |
| 无表 doc | 返回「无表格」，走 grep/dense | 不做运行时兜底猜表；覆盖率与重灌绑定（markitdown 进生产本来要重灌，同车） |

### 1.2 已否方案（归档，不再讨论）

| 方案 | 否决理由 |
|------|----------|
| runtime 解析 chunk 内管道表（v1/v2） | 通用切块切腰大表、块间 UUID 无顺序、运行时检测假阳；风险在灌入侧消解 |
| TableIr（`ir.rs:358`，已实现） | 按格式分派 + 灌库期结构，违反格式无关原则；本功能不使用（`text_table` 的管道表解析思路仅作参考） |
| LLM 逐行转录 | 370 行掉行/并格/幻觉必然且不可检测；大文件下窗口装不下 |
| DuckDB `pdf`/`excel` 扩展直读 | 又回到按格式分派，效果结构不一致 |
| TableRAG / RAGFlow / DocETL / flock 等 | 见 §9 |

---

## 2. 总体架构

```text
【灌入 pipeline —— 离线，Python】
任意格式原件 (xlsx/csv/pdf/docx, 表文混排)
  → markitdown → doc.md (统一 markdown, 管道表)
  → markdown-it-py 提取 → grids {headers, rows, 文档序}        ← 确定性
  → 建 per-doc DuckDB: 表(row_ord) + _meta                    ← 确定性
  → 校验 SQL 套件 → 健康报告(每表: 行数/列数/合计/序号/空列)    ← 确定性
  → supervision agent loop (LLM):
      语义标注(caption/单位/列义/表型)
      诊断失败校验 → apply_directive → 代码重跑 → SQL 复验
      终态: high / low / quarantine
  → 产出: <doc_id>.duckdb + _meta(confidence, checks, notes)
  → 全文照常切块进 rag_text_chunks(表文本留 chunk, 供 grep/dense 发现)
  → 证据映射: 表行区间 ↔ chunk_id (灌入时算好, 存 __chunk_id)

【查询 —— host, Rust, duckdb-rs core only】
client.struct_catalog(doc_ids?)
  → 无 .duckdb / 无表 → {"relations": []} (ok, 即「无表格」)
  → 有 → relations[](name/caption/headers/n_rows/sample_rows/unit/confidence/notes)
        (catalog 源自 information_schema + _meta; SUMMARIZE 可选增强)
client.struct_query(sql)
  → 校验(单语句 / 表名 ∈ catalog / 无禁函数)
  → 加固 config 只读执行 → rows + evidence(__chunk_id 回填)
  → 聚合查询附 scanned chunk 清单(归因)
grep → 字段准确写法发现 / 无表 doc 的退路
```

---

## 3. 灌入 pipeline（P0 已实证核心段）

### 3.1 步骤

| 步 | 内容 | 确定性/LLM |
|----|------|-----------|
| ① | markitdown 原件 → doc.md | 确定性 |
| ② | markdown-it-py 提取表 grids（含文档序、行序） | 确定性 |
| ③ | 跨页/跨块续表合并：同表头签名的相邻 grid 先由代码聚类，LLM 裁决候选对 | 确定性 + LLM |
| ④ | 建表：业务列 VARCHAR（原值）+ `row_ord`(0-based 行序) | 确定性 |
| ⑤ | 校验 SQL 套件 → 健康报告 | 确定性（SQL） |
| ⑥ | supervision loop：语义标注 + 诊断 + 指令修复（§4） | LLM + 确定性复验 |
| ⑦ | `_meta` 落库（confidence/checks/notes）；`__chunk_id` 证据映射 | 确定性 |

### 3.2 校验套件（健康报告的骨架）

| 校验 | 信号 | 说明 |
|------|------|------|
| 行数 vs 序号列 | `max(序号) == count == 序号连续` | IPD 实证：1..370 全连续（A3 PASS） |
| 合计对账 | `sum(明细金额列) == 合计行值` | 会计报表天赐自检；LLM 负责指认合计行/金额列 |
| 列数一致 | 每行单元格数 == 表头列数 | 行列错位立即现形 |
| 空列/空行 | 不该空的列全空、全空数据行 | xlsx 方言信号（见 3.3） |
| 表头可疑 | 列名匹配 `^Unnamed`、列名像数据值 | 假表头信号 |

### 3.3 P0 实证方言（markitdown xlsx）

- sheet 标题行被 pandas 吃成**假表头**（`华为IPD流程各阶段活动` + `Unnamed: 1..5`），真表头（编号/阶段/活动/活动号/活动描述/角色）降为数据第 1 行 → `rotate_header(header_row=1)` 适用。
- **`Unnamed: N` ≠ 空列**：pandas 仅因假表头行缺值而命名，列内全是数据。`drop_columns_matching` 必须带「该列在全部数据行为空」的**确定性守卫**，守卫不过则拒丢（P0 实测：不带守卫会把 阶段/活动/活动号 全列误删）。
- 单元格内换行为字面 `\n`，不影响提取。
- 提取产出：`1 表 × 372 行` = 假表头 + 真表头 + 370 数据行；阶段分布 概念81/计划86/开发92/验证59/发布30/生命22 = 370，与 7-28/7-29 真值全部一致。

### 3.4 P1a 实测：白药 PDF（slide deck，593 行）方言记录

| 发现 | 处置 |
|------|------|
| 9 张表全是**布局网格**（slide 版面重建），非记录表 | 全部 `needs_diagnosis`，待 supervision 的 table_kind 裁决（detail/layout/exclude） |
| **分隔行残迹**：PDF 重建在表中部重复 `\| --- \|` 行（8 表各 4–9 行） | 确定性剔除（`dropped_delimiter_artifact`），行数净化（如 64→56） |
| **banner 数字进表头**：「638」成列名、「个业务对象（L3）」在首行数据；子计数「(共6个)」散布数据行 | `header_numeric_banner` 校验捕获——64/638 混淆的结构根源在此；语义归标注，不靠 COUNT |
| 无 markdown 标题 | caption 只能取邻近散文行（PDF 无 heading 结构） |
| 跨页续表合并未触发 | 9 表表头签名互不相同；合并规则正确未动（IPD/财报样本再验） |
| 合计对账未触发 | 布局网格无干净合计行；待真实会计报表样本验证 |
| txt 透传文件 | 0 表——「无表格」路径成立 |

---

## 4. supervision agent loop

### 4.1 结构

```text
host → 简报(observation): doc 概况 + N 张表健康报告
┌─ loop ─────────────────────────────────────────────┐
│ agent → 工具调用(可并行)                            │
│ host  → 执行 + 回传 observation(第三人称, 有界)      │
└─────────────────────────────────────────────────────┘
结束: agent done() 或预算耗尽
```

### 4.2 工具面（6 个，全部有界）

| 工具 | 作用 | 返回 |
|------|------|------|
| `annotate(tables[])` | 批量语义标注（caption/单位/列义/表型） | 确认 |
| `fetch_slice(table_id, row_range | source_lines)` | 原文/表切片（行数封顶） | 切片 |
| `run_check(sql)` | ingest DuckDB 上跑只读校验 SQL | 结果（行数封顶） |
| `apply_directive(table_id, directive)` | 代码应用 → 重跑 → SQL 复验 | **新健康报告** |
| `quarantine(table_id, reason)` | 隔离 | 确认 |
| `done(summary)` | 结束 | — |

### 4.3 指令契约（安全命门：指令永远不含单元格的值）

```json
{"action": "rotate_header", "header_row": 1, "drop_columns_matching": "^Unnamed"}
{"action": "merge_tables", "table_ids": ["t3", "t4"]}
{"action": "insert_delimiter", "after_source_line": 51}
{"action": "set_header", "headers": ["..."], "evidence_source_line": 12}
{"action": "reparse_region", "start_line": 88, "end_line": 140}
{"action": "exclude", "reason": "kv_layout"}
```

- 每条指令经 **schema 校验 + 确定性守卫**（如 drop 列须数据区全空，`set_header` 文字须出现在 `evidence_source_line` 所引行），守卫不过则拒。
- 应用后 **SQL 复验**，结果以新健康报告回传；仍失败 → `low`（连失败说明入库）或 `quarantine`。
- **禁区**：任何需要 LLM 直接给出单元格值的修复一律不准——只能 low/quarantine，不用嘴宣布真理。

### 4.4 终态与降级

- 每表终态：`high` / `low` / `quarantine`，语义标注齐备；`low` 的 notes 与失败校验一并入 `_meta`，查询侧 catalog 如实透出。
- 预算耗尽：未处理表保持确定性初态 + notes「监督预算耗尽」；**pipeline 永不被 LLM 卡死**。
- 模型：中档即可——LLM 不稳定被「schema 校验 + 重跑复验」夹住，错指令不会变成错数据。

### 4.5 prompts 落点（repo law：不进代码）

| 文件 | 内容 |
|------|------|
| `prompts/pipeline/table-supervision/supervision.system.v1.md` | 监督 worker 契约（工作单元、健康报告语义、指令目录、禁区、终态、诊断信号表；虚构示例，无 golden 实体名） |
| `prompts/pipeline/table-supervision/obs-health-report.md` | 简报模板 |
| `prompts/pipeline/table-supervision/obs-slice.md` | fetch_slice 回传包装 |
| `prompts/pipeline/table-supervision/obs-check-result.md` | run_check 回传 |
| `prompts/pipeline/table-supervision/obs-directive-applied.md` | 指令应用+复验回传 |
| `prompts/pipeline/table-supervision/obs-directive-rejected.md` | 指令被拒回传 |

observation 一律第三人称（「指令 rotate_header 已应用；复验结果：…」），不写命令式。host 不做语义门禁，结束由 agent `done()` + 预算决定。

### 4.6 实现形态

v1 薄自研 loop（PoC `supervise.py` ~350 行）：固定 6 工具、离线、无流式、stdlib urllib 零新依赖；不套产品 agent-loop crate（那是用户会话形态）。LLM 配置复用 `INGESTION_LLM_*`。

### 4.7 P1b 实证记录（白药 9 烂表，真 LLM = DeepSeek v4 flash）

| 观察 | 结论 |
|------|------|
| 6 轮内 done，预算未耗尽 | 成本收敛符合预期 |
| agent 行为：先并行 fetch_slice 9 表 → 诊断 → exclude/quarantine/annotate | 工具面够用；并行调用自然发生 |
| agent 尝试 `rotate_header(header_row=0)` 删空列 | **守卫拒绝**（真表头在 row 0 不可旋转）——错指令不变错数据，实证 |
| agent 对 t2(638 表)给 quarantine，理由含「重建需改写单元格值属禁区」 | 禁区条款被理解并遵守 |
| **发现的 bug**：annotate 覆盖 quarantine/exclude 终态 → 已修（终态 sticky）+ 回归 | 验证驱动修复 |
| 两次 live 运行终态不同（7 low+2 排除 vs 9 全排除） | LLM 方差真实存在，但被结构不变量夹住；slide deck 全排除→「无表格」→grep 路径是正确产品行为 |

结构不变量（live 断言）：预算内 done、每表有终态、excluded 表不入库、无「校验失败却 high」。

---

## 5. 大文件策略（超 LLM 窗口）

原则：**监督的工作单元是「表」和「失败证据」，不是「文档」**；全量覆盖由确定性层承担，LLM 只吃小切片。

| 任务 | 输入 | 为什么不需要全文 |
|------|------|------------------|
| 语义标注 | 表头 + 头/中/尾采样行 + 邻近 ±20 行（`read_markdown_sections` 类 breadcrumb 可选） | 列义整列一致；单位/口径在表邻近行 |
| 校验 | 无 LLM 输入 | 纯 SQL，10 万行无感 |
| 诊断 | 失败信号 + **行区间原文切片**（健康报告自带定位） | 一次看十几行 |
| 续表裁决 | 候选对表头 + 边界切片 | 代码先按表头签名聚类 |
| **漏表发现** | 启发式扫描器（连续管道行/对齐列）→ 候选区域 | **funnel**：扫描器召回，LLM 只裁决候选 |

**漏网边界（诚实声明）**：表中部单值错误若不被算术锚点（合计/序号/列数）触及、采样亦未抽中 → 漏网。这是物理现实（窗口装不下逐行复核），残余风险走 confidence + notes「未经逐行复核」的诚实通道，不假装全覆盖。

---

## 6. DuckDB 复用清单与安全加固

### 6.1 直接复用（不造轮子）

| 需求 | 复用 |
|------|------|
| catalog 列统计 | `information_schema` + `DESCRIBE` + **`SUMMARIZE`**（min/max/approx_distinct/null% 白拿） |
| SQL 校验 | `parser_tools` 社区扩展（`num_statements`/`parse_table_names`/`parse_functions`/`is_parsable`）；兜底 = prepare API 拒多语句 + 关键词黑名单 |
| 字段值发现（可选） | `fts` 扩展 `match_bm25`（表内值检索，配合 chunk grep） |
| 数值列规整（v1.1，可选） | `finetype` 扩展（货币/日期等语义类型）/ TRY_CAST |
| 灌入提取 | ~~duckdb_markdown~~ → **markdown-it-py**（§9 实证） |

### 6.2 查询侧加固（模型写 SQL = untrusted SQL，配方来自 Simon Willison duckdb-security 研究）

```
access_mode = READ_ONLY            # 只读打开 per-doc 文件
enable_external_access = false     # read_csv/COPY/ATTACH 文件系统层拦截(PR #14568)
lock_configuration = true          # 防 SET 撤销上述锁定
allow_community_extensions / autoinstall / autoload = false
+ 查询超时 + 结果行数硬顶 + 标识符 ∈ catalog 校验
```

---

## 7. SDK 契约

### 7.1 `struct_catalog`

```python
cat = await client.struct_catalog(doc_ids=None)
# → {"relations": [
#      {"name": "ipd_activities", "doc_id": "...", "caption": "...",
#       "headers": [...], "n_rows": 370, "sample_rows": [...],   # ≤3
#       "unit": None, "confidence": "high|low",
#       "notes": ["markitdown_xlsx", "rotate_header_applied"]}
#    ]}
# 无表格存储 / 无表 → {"relations": []}  (ok, 不是 error —— 即「无表格」)
```

### 7.2 `struct_query`

```python
r = await client.struct_query(sql)
# → {"ok": true, "columns": [...], "rows": [...], "row_count": n,
#    "truncated": false,
#    "evidence": [{"row_ord": 0, "chunk_id": "...", "doc_id": "..."}],   # 明细查询
#    "scanned_chunks": ["..."]}                                          # 聚合查询归因
# → {"ok": false, "error": {"code": "parse|forbidden|unknown_relation|unknown_column|limit", ...}}
```

- 空结果：`ok=true, rows=[]`（「查不到」≠ error，与 grep `total_hits=0` 一致）。
- `COUNT(DISTINCT)` v1 默认关（子类陷阱，q106-白药 64/638 的教训）；skill 观察句说明。
- 明细行 `__chunk_id` 直接回填证据；聚合附 `scanned_chunks`（答案归因军规适配）。

### 7.3 与 grep 的配合（skill 增补，第三人称）

- struct_catalog 为空 → 当前 scope 无表格存储，grep/dense 可用（非回归）。
- 字段准确写法（是「验证阶段」还是「验证」）→ 先 grep 定位字面，再写 WHERE。
- `total_hits` 是 pattern 命中行数；`struct_query` 的 COUNT 是表格存储上的引擎结果。

### 7.4 host 落点

`rag-core/src/runtime/tools/struct_query.rs`（+ catalog），经 `runtime::tools::dispatch` 注册（同 `doc_grep`，T3）；bridge shim 增 `struct_catalog`/`struct_query` 两方法（ADR-0009 同构）。

---

## 8. 存储与证据

| 项 | 设计 |
|----|------|
| 文件 | `<doc_id>.duckdb` 随对象存储；doc_version 变更 → 重建；删 doc → 删文件 |
| 表名 | `t{index}_{slug(caption)}`；catalog 暴露稳定 name |
| 业务列 | VARCHAR 原值（LLM/parser 均不改单元格） |
| 系统列 | `row_ord` INTEGER（0-based）；`__chunk_id` VARCHAR（证据） |
| `_meta` | table_name, caption, unit, n_rows, confidence, checks JSON, notes JSON |
| 数值规整 | v1 不做；v1.1 另存规整列（去逗号/统一单位），原值列不动 |

---

## 9. 不造轮子调研结论（2026-07-31）

### 9.1 采用

| 轮子 | 用途 | 备注 |
|------|------|------|
| **markdown-it-py**（gfm-like + linkify-it-py） | 灌入表格提取 | P0 实证：IPD 370 行、CJK 完整、真值全对 |
| DuckDB core（json / SUMMARIZE / information_schema / fts） | 存储、查询、catalog | host 端仅需 duckdb-rs core |
| `parser_tools` 社区扩展 | SQL 白名单校验 | 27★ 小项目，仅便利层；核心安全不依赖它 |
| Simon Willison duckdb-security 配方 | 查询侧加固 | 场景一致（untrusted SQL） |

### 9.2 实测否决

| 轮子 | 否决原因（实证） |
|------|------------------|
| **duckdb_markdown 社区扩展** | **丢 CJK 单元格**：`md_extract_tables_json` / `md_extract_table_rows` 对 `概念阶段` 等中文单元格返回空串（最小复现：`| 1 \| 概念阶段 \| X |` → `["1","","X"]`）；我们语料全中文，不可用。提取改 markdown-it-py |

### 9.3 调研后排除

| 项目 | 排除理由 |
|------|----------|
| TableRAG（Google, NeurIPS 2024） | 查时检索**已有**表（schema+cell retrieval 喂 LM），不做文档→表提取，SQL 非权威；其「先检索相关表」思路可在 catalog 很大时参考 |
| RAGFlow（DeepDoc） | 完整 RAG 平台（86k★），采用=换栈；TSR 思路参考 |
| DocETL / LOTUS / Palimpzest | LLM ETL 框架级，两次 LLM 调用手卷即可，不值得引框架 |
| DuckDB `pdf` / `excel` 扩展 | 按格式分派，违反统一 markitdown 原则 |
| flock / llm / open_prompt（DuckDB 内调 LLM） | 监督在灌入时由 host 编排，不需要数据库内调 LLM |

---

## 10. 分阶段交付

### Phase 0 — 提取与真值实证（**已完成**）

| 项 | 状态 |
|----|------|
| `scripts/struct_query_poc/extract_tables.py`：markitdown md → markdown-it-py → rotate_header → DuckDB → fixtures | ✅ **4/4 PASS**（A0 370；A1 验证59/发布30 等六阶段；A2 LPDT-03 表序 first；A3 序号自校验） |
| duckdb_markdown CJK bug 最小复现与否决记录 | ✅ §9.2 |
| Unnamed 列方言与 drop 守卫 | ✅ §3.3 |
| 剩余：PDF 财报样本（分隔行/跨页/合计对账） | ⬜ 排入 P1 初 |

### Phase 1 — pipeline 成型 + 最小产品面

| 项 | 内容 |
|----|------|
| pipeline | ②~⑦ 全链（提取/合并/建表/校验套件/健康报告/入库/证据映射），Python 脚本 → 可嵌入 ingestion worker |
| supervision loop v1 | 6 工具薄 loop + prompts 落盘（§4.5）+ 预算降级 |
| host | `struct_query.rs`（duckdb-rs core 只读 + §6.2 加固 + 校验）；dispatch 注册 |
| bridge/SDK | shim 两方法；eval_bridge 登记 |
| skill | `prompts/clusters/knowledge-base/SKILL.md` 增补 struct 方法表（第三人称，无 golden 实体名） |
| 验收 | `QUESTIONS=86,88,106`（重灌后含表格存储语料）：106-IPD 数字可解释、86 表序、88 阶段计数；无表 doc 返回无表格不 panic |

### Phase 2 — 硬化

- 监督 loop 工具化升级（fetch_slice/run_check 自由调用形态）；PDF 跨页合并鲁棒性
- fts 表内值发现；finetype 数值规整列；增量重灌；telemetry（提取成功率、指令分布、low/quarantine 率）

---

## 11. 验收矩阵

| ID | 检查 | Phase | 状态 |
|----|------|-------|------|
| A0 | IPD `COUNT(*)=370` | P0 | ✅ |
| A1 | 阶段计数 验证59/发布30（全六阶段） | P0 | ✅ |
| A2 | 概念阶段 LPDT 表序 first = LPDT-03 | P0 | ✅ |
| A3 | 序号自校验 max==count | P0 | ✅ |
| A4 | PDF 样本（白药）：分隔行残迹剔除 / 布局网格识别 / 638 banner 捕获 / 无续表触发 | P1a | ✅ |
| A5 | 监督干预成功案例：构造「表头被吃」样本 → rotate_header → 复验过 | P1 | ⬜ |
| B0 | bridge catalog/query 可调 | P1 | ✅（P1d E2E tool_trace：struct_catalog×4 / struct_query×11 全 Ok；P1c shim parity 测试） |
| B1 | 安全锁：`read_csv`/ATTACH/多语句/`;` 拼接被拒 | P1 | ✅（struct_query 测试 10/10：forbidden 矩阵含 DELETE/SET/ATTACH/read_csv/子查询；P1c 探针：READ_ONLY 拒写、外部访问拒、lock_configuration 防撤销） |
| B2 | skill 无 golden 实体名；观察语气 | P1 | ✅（SKILL.md 增补无 golden 实体名；低自由度行已由祈使式改观察式） |
| B3 | 86/88/106 切片（含表格存储语料） | P1 | ◐ 86/88 ✅（correctness=1；88 全绿 PASS）；106 ❌ sticky 未解：末轮停在 codegen 块未出 synthesis（疑轮次/预算耗尽），struct/grep 调用全 Ok、取证方向正确（370 口径 + 638 grep）→ 归 P2 预算/telemetry 观察 |
| B4 | 无表 doc：catalog 空、ok、不 panic | P1 | ✅（P1d E2E 全 10 doc scope 运行无 panic、exit=0；单测 no_relations / 缺文件跳过） |
| C0 | 明细证据 `__chunk_id` 水合；聚合附 scanned_chunks | P1 | ⬜ |

---

## 12. 风险

| 风险 | 缓解 |
|------|------|
| markitdown PDF 缺分隔行 → 提取漏表 | 启发式扫描 funnel + LLM `reparse_region`/`insert_delimiter`；隔离兜底 |
| 跨页表碎片 | 表头签名聚类 + LLM 裁决合并；A4 实测 |
| LLM 错指令 | schema 校验 + 确定性守卫 + SQL 复验三重夹；budget 降级 |
| 单值错误漏网（中部、非锚点） | 物理边界，诚实声明（§5）；confidence 通道 |
| 社区扩展（parser_tools）成熟度 | 仅便利层；兜底 prepare API + 关键词黑名单 |
| duckdb-rs 构建体积/WSL CI | P1 首件事实测构建；不行则 host 旁路 python 进程（下策） |
| 老 doc 无表格存储 | catalog 空=「无表格」非回归；随 markitdown 重灌覆盖 |

---

## 13. 开工顺序

```text
[x] 0. 拍板:v3 架构(统一 markitdown + 灌入 pipeline + DuckDB 核心 + supervision loop)
[x] 1. P0:提取实证(4/4 PASS)+ duckdb_markdown 否决 + 方言记录
[x] 2. P1a:pipeline 全链(提取/合并/校验/健康报告/_meta/重建语义)+ PDF 样本实测(11/11 PASS)
[ ] 2b.P1a 收尾:__src_line → __chunk_id 证据映射(待切块联动);真实会计报表样本验合计对账
[x] 3. P1b:supervision loop v1(6 工具薄 loop + prompts 落盘 + 守卫/终态回归,12/12 确定性 + live 不变量 PASS)
[x] 4. P1c:host struct_catalog/struct_query(duckdb-rs 加固只读)+ bridge + skill 增补
[x] 5. P1d:重灌语料(ipd t0 370 行 high_candidate / 白药 9 网格 needs_diagnosis → storage/struct_store/<doc_id>.duckdb),QUESTIONS=86,88,106 切片:86/88 答对、106 sticky 依旧(详见 §11 B3 与附录 B)
[ ] 6. P2:loop 工具化 / fts / 数值规整 / telemetry
```

---

## 14. 变更纪律与参考

### 纪律

- prompts 全落 `prompts/pipeline/table-supervision/`(system + obs 模板),不进 Rust;observation 第三人称;无 golden 实体名。
- T3:host 执行经 `runtime::tools::dispatch`,无 AppState 旁路。
- LLM 与 parser 均不改单元格原值;指令必须过 schema + 守卫 + 复验。
- 验证:PoC fixture → 目标 `cargo test` → 含表格存储语料的 fail 切片;不默认全量 149。

### 参考

| 类型 | 路径 |
|------|------|
| P0 脚本 | `scripts/struct_query_poc/extract_tables.py`(4/4 PASS) |
| markitdown 产物 | `/tmp/markitdown_out/`(7-29 换血实验) |
| 灌库表结构(部分实现,本功能不用) | `docs/plans/2026-07-28-table-aware-ingestion-design.md` |
| grep 现状/方言实证 | `docs/plans/2026-07-29-markitdown-grep-toolcall-spec.md` |
| 沙箱桥 | `docs/adr/0009-codegen-sandbox-retrieval-bridge.md` |
| duckdb_markdown(CJK bug,已否) | github.com/teaguesterling/duckdb_markdown |
| SQL 校验扩展 | github.com/hotdata-dev/duckdb_extension_parser_tools |
| 加固配方 | github.com/simonw/research(duckdb-security) |
| q106 产物 | `e2e_output/realistic_corpus_full_eval/q106.json` |

## A. P1c 实现证据（2026-07-31 续）

代码落点：
- `contracts/src/tool_call.rs`：`StructCatalogArgs` / `StructQueryArgs`
- `crates/rag-core/src/runtime/tools/struct_query.rs`：host 侧 `struct_catalog` / `struct_query` 实现
- `crates/rag-core/src/runtime/tools/mod.rs`：dispatch 注册
- `crates/rag-core/src/runtime/bridge.rs`：桥接 + `supported_method_names`
- `crates/code-interpreter/src/bridge.rs`：沙箱 shim 方法 + parity 测试
- `crates/agent-loop/src/react_loop/sdk_gate.rs`：`RAG_PRIMITIVES` 白名单扩展
- `crates/rag-core/Cargo.toml`：`duckdb = { version = "1", features = ["bundled"] }`
- `prompts/clusters/knowledge-base/SKILL.md`：技能页新增 `struct_catalog` / `struct_query` 用法与约束

关键行为：
- 只读加固：`access_mode=READ_ONLY` + `SET enable_external_access=false; SET lock_configuration=true;`
- SQL 白名单：仅单条 SELECT；禁 attach/copy/install/load/pragma/set/create/insert/update/delete/drop/alter/export/import/read_csv/read_json/read_parquet 等
- 表名解析：FROM/JOIN 中的 relation 必须当前 scope 的 catalog 可见；v1 禁止跨 doc relation 联合
- 缺失文件 → `relations` 空（「无表格」）而非 error
- 结果硬顶：`MAX_RESULT_ROWS=200`、`MAX_SAMPLE_ROWS=3`、`MAX_CELL_CHARS=300`
- 证据：当前回传 `doc_id` + `row_ord` + `__src_line`；`__chunk_id` 待 2b 与 chunker 联动

测试结果：
- `cargo test -p avrag-rag-core --lib struct_query`：5/5 PASS
- `cargo test -p avrag-rag-core --lib`：86 passed / 0 failed
- `cargo test -p avrag-code-interpreter --lib`：15 passed / 0 failed
- `cargo test -p agent-loop --lib sdk_gate`：6 passed / 0 failed
- 探针验证：duckdb-rs bundled 可读取 Python duckdb 1.5.5 写出的 `/tmp/poc_ipd.duckdb`，且加固 SET 不可被模型撤销
- 构建事实：WSL2 下 `libduckdb-sys` bundled 编译约 5m37s（单 crate）

## B. P1d 验收证据（2026-07-31）

重灌（`scripts/struct_query_poc/pipeline.py`，doc_id 取自 E2E 语料缓存 `realistic_corpus_cache.json`）：
- `avrag-rs/storage/struct_store/693eb189-0b1e-462e-9d72-127339ecacea.duckdb`（ipd）：t0 370 行，checks all_passed，high_candidate
- `avrag-rs/storage/struct_store/b38a4960-bf6b-4108-87b7-63b91b4bbf76.duckdb`（白药）：t0–t8 共 9 张布局网格，全 needs_diagnosis；「638 个业务对象」在正文（fixtures txt:475）不在表中，属 grep 侧事实

切片运行（真实 LLM，约 3m14s；log `/tmp/sac_e2e/fail6_20260731-132409.log`，v2 artifacts `v2_20260731-053509`）：
```
STRUCT_STORE_DIR=$PWD/avrag-rs/storage/struct_store QUESTIONS=86,88,106 \
  bash avrag-rs/scripts/sac-skill-fail6-reg.sh
```

| Q | struct 调用（全 Ok） | v2 结果 | 答案要点 |
|---|----------------------|---------|----------|
| 86 表序 | catalog×1 + query×2 | correctness=1；UNGROUNDED（recall 机制性 0：struct 结果不在 RETRIEVAL_TOOLS，待 2b 登记） | LPDT-03，明示「t0 表行序」 |
| 88 阶段计数 | catalog×2 + query×6 | **PASS**：correctness=1 / faithfulness=1 / recall=1.0 | 验证 59 / 发布 30，逐行口径 + 去重口径并存解释（符合 rubric） |
| 106 双数跨 doc | catalog×2 + query×3 + grep×4 | correctness=0：最终「答案」是 code 块（loop 末轮为 codegen，未出 synthesis） | judge：三个关键点全 miss；取证方向本身正确（370 口径 SQL + 638 grep） |

结论：struct_catalog/struct_query 在真实 agent loop 全链路可用（SDK shim → bridge → host 加固只读），86/88 由 struct 取证答对；106 为既有 sticky 题，失败模式从 half-coverage 变为「code 块当答案」，与 struct 无关，归 P2（预算/telemetry）与 2b（证据映射后登记 RETRIEVAL_TOOLS，recall 机制性 0 随之消除）。
