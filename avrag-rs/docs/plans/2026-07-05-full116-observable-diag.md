# Full116 Observable 批跑离线诊断（2026-07-05）

> Run：`e2e_20260704-172230_local_edb73279542c4f86a634086cf1f6eaa2`  
> 配置：`RETRIEVAL_GRAPH_AUGMENT=on`，复用 7 篇 realistic 语料，零灌库  
> 模式：只跑不评（observable 产物 + 事后 `e2e-analyzer rag-diag`）  
> 日志：`crates/app/tests/e2e_output/realistic_full_observable_batch_20260705.log`

---

## 1. 跑批结果

| 范围 | 题数 | 产物 | 失败 |
|------|------|------|------|
| realistic | 107 | `realistic_q1`…`q107` | 0（probe 级） |
| graph | 9 | `graph_q1`…`q9` | 0 |
| **合计** | **116** | 282MB | **0** |

产物根目录：

`crates/app/tests/e2e_output/rag_quality_smoke_v5/e2e_20260704-172230_local_edb73279542c4f86a634086cf1f6eaa2/`

离线报告：

- `crates/app/tests/e2e_output/rag_diag_realistic_20260705.md`
- `crates/app/tests/e2e_output/rag_diag_graph_20260705.md`
- `crates/app/tests/e2e_output/rag_diag_full116_summary.json`

---

## 2. 自动评分总览（rag-diag）

| 范围 | PASS | 非 PASS | 通过率 |
|------|------|---------|--------|
| realistic（107 题） | 96 | **11** | 89.7% |
| graph（9 题） | 9 | 0 | 100% |

11 题自动标签均为 `RETRIEVAL_MISS`，但人工核对后**多数不是检索失败**（见 §3）。

Graph 9 题 `must_include` 全过，但通道侧仍有隐患（**D 类，非答案挂题**，深析 §10）：

- negative_controls **4/4** 误触 `graph_retrieval`（augment 侧车计入 legacy 指标）
- `graph_cited` **0/9**（合成 cite 正文，不 cite `graph_relation`）
- 大量 `graph_augment` degrade_trace

---

## 3. 11 挂题归纳（按根因）

### A. 真产品问题（1 题）

| 题号 | subset | 现象 |
|------|--------|------|
| Q79 | ipd_table | `doc_scan` 统计开发 93 vs 原文 92（`llm_judge=pipeline_fault`） |

> **Q17** 已从 A 类移除——深析见 §4，实为 golden「获取/获得」子串误伤。  
> **Q19** 已从 A 类移除——深析见 §5，实为 **golden/query 语义错位**（agent 答 6.1.2 更切题）。  
> **Q88** 已从 A 类移除——深析见 §7；**full116 数值 59/30 正确**，挂因是 `must_include` 整串格式（`llm_judge` 里 45/24 为旧 run）。

### B. Golden / 评分误伤（8 题）

| 题号 | 说明 |
|------|------|
| **Q17** | 四条全写出；仅「获取」写成「获得」导致 `must_include` 1/4 未命中 | ✅ **已修** must_include 拆锚点 |
| **Q19** | ~~golden 与 query 错位~~ **已修 golden**（§6.1.2 三项）；本次 run 答案应 PASS |
| **Q88** | 验证 59 / 发布 30 全对；`must_include` 要整串 | ✅ **已修** `59个`/`30个` 锚点 |
| Q33 | 答案 2000万吨/60kg+ 正确 | ✅ **已修** `2000`/`60千克` |
| Q47 | 答案 native tool calling 正确 | ✅ **已修** 锚定 `native tool calling` |
| Q49 | 答案 `parse_rag_plan_decision` 正确 | ✅ **已修** 锚定标识符 |
| Q60 | 答案 `run_auto_fallback` 正确 | ✅ **已修** 锚定标识符 |
| Q90 | 三阶段日期全对 | ✅ **已修** 分阶段日期锚点 |

### C. 跨文档综合表述差异（2 题）

| 题号 | 说明 | 深析 |
|------|------|------|
| **Q101** | 答「管理主义」框架；golden 要「生产者/销售导向 vs 客户价值导向」整串 | §8 |
| **Q104** | 列出 4 份文档角度，与 golden 四件套句式/篇目不完全一致 | §9 |

> full116 下两题 **均未拒答**、`recall=100%`；`manual_override` / `llm_judge` 仍按旧 run（Q101 流式截断、Q104 拒答）标注，**已过时**。

### D. Graph 通道评测错位（9 题，非答案挂题）

| 现象 | full116（`RETRIEVAL_GRAPH_AUGMENT=on`） | 说明 |
|------|----------------------------------------|------|
| `must_include` | **9/9 PASS** | 答案全对，rag-diag 标签 PASS |
| `negative_controls` 误触 graph | **4/4**（100%） | 评测把 `graph_augment` 侧车记成 `graph_retrieval_called` |
| `graph_cited` | **0/9** | 合成只 cite 正文 chunk，不 cite `graph_relation` |
| `source_recall` | **0/9** | 已知评测盲区（done 事件剥 `tool_results.data`） |

> **不是产品答错题**，而是 **augment 开启后 legacy 指标全面失真**。深析见 §10。

### 按 subset 分布

```
cross_document   2   (Q101, Q104)
thesis_synthesis 2   (Q17, Q19)
adr_factual      2   (Q47, Q49)
ipd_table        2   (Q79, Q88)
thesis_numeric   1   (Q33)
cross_adr        1   (Q60)
baiyao_pdf       1   (Q90)
```

---

## 4. 逐题深析：Q17

**产物**：`…/realistic_q17/`（`response.json`、`reasoning_summary.txt`、`trace_reasoning.jsonl`、`sse_events.jsonl`）

### 4.1 Query 问了什么

```
论文通过对营销人员的调研，关于营销活动中遇到了哪些问题，得出了哪些结论？
```

拆解：

1. **范围**：营销人员调研（§3.2.3），不是管理层/竞争对手调研
2. **问题**：营销活动中遇到了哪些问题
3. **结论**：得出了哪些结论

Golden 期望（`golden_set_realistic.json`）：

- `must_include` 四条（§3.2.3 编号列表）：
  - 获取客户看重数量轻视质量
  - 不重视渠道和推广
  - 设计团队过于主观
  - 过于追求短期利益

### 4.2 LLM 如何推理（5 轮 agent loop）

| 轮次 | 决策 | 行为 |
|------|------|------|
| iter0 | `code_gen` | dense + lexical 首轮检索；命中论文结构 chunk（seq 12/122）等 |
| iter1 | （无 evaluation 记录） | 继续检索规划 |
| iter2 | `code_gen` | 补搜「第四章 / 营销问题」类 query |
| iter3 | `code_gen` | 再次 dense/lexical |
| iter4 | `budget_exhausted` → `synthesized` | 预算耗尽，强制合成（注入 `rag-answer`） |

推理要点（`reasoning_summary.txt`）：

1. **首轮判断**：只调 `dense_search` + `lexical_search`；图谱由 bridge `graph_augment` 确定性附带（无需 codegen 调 `graph_search`）
2. **证据定位**：最终锁定 `a935e963` = §3.2.3 营销人员调研总结（含完整四条）
3. **范围扩张**：除 §3.2.3 外，还纳入 3.2.5 综述、4R 框架四章问题、论文结论章——**超出 query 字面范围**
4. **cite 修复**：合成前意识到短 id 无效，改用完整 UUID

工具调用（15 次）：dense×3、lexical×5、graph×7（均为 augment 附带）

### 4.3 如何回答

**第一段（营销人员四类问题）**——对应 query 核心：

> 一是公司**获得**客户看重数量轻视质量…  
> 二是不重视渠道和推广…  
> 三是设计团队过于主观…  
> 四是公司过于追求短期利益…

引用：`a935e963`（§3.2.3 全文块）、`fe26091d`（第 4 条所在邻块）

**第二段（综合调研 + 4R 框架）**——超出「仅营销人员调研」范围

**第三段（论文结论）**——对应 query「得出了哪些结论」

### 4.4 挂题根因（重新定性）

| 维度 | 结果 |
|------|------|
| 检索 | ✅ 命中 `a935e963`（§3.2.3 完整四条） |
| 合成 | ✅ 四条问题全部写入答案 |
| `must_include` | ❌ **3/4 字面命中** |

唯一未命中项：

| golden token | 答案写法 | 原因 |
|--------------|----------|------|
| `获取客户看重数量轻视质量` | `公司**获得**客户看重数量轻视质量` | **获取→获得** 同义改写，子串不匹配 |

原文 §3.2.3（1）：`获取客户看重数量轻视质量`  
答案用「获得」而非照抄「获取」→ `contains` 判 FAIL。

**结论：Q17 应归为 B 类（golden 措辞误伤），不是合成漏项或检索失败。**

建议修复：

- `must_include` 拆为 `["获取", "数量轻视质量"]` 或加 `manual_override`
- 或评分层对「获取/获得」做同义归一

### 4.5 次要观察（非挂题主因）

- **回答过宽**：用户只问营销人员调研，agent 额外写了 4R 框架 + 全文结论（~1500 字）
- **预算耗尽**：4 轮检索后才合成，iter1 无 evaluation 记录值得查
- **graph augment 噪声**：单跳 factual 题仍触发 7 次 graph degrade

---

## 5. 逐题深析：Q19

**产物**：`…/realistic_q19/`（`response.json`、`reasoning_summary.txt`、`trace_reasoning.jsonl`）

### 5.1 Query 问了什么

```
论文提出Y冷冻设备公司应该从哪三个方面进行能力建设？
```

字面拆解：

1. **主体**：Y 冷冻设备公司
2. **动作**：进行「能力建设」
3. **形式**：哪**三个方面**

Golden 期望（`golden_set_realistic.json`，description 已注明「原 golden 误标为 6.1 子节」）：

| 字段 | 内容 |
|------|------|
| `expected_answer` | 公司能力建设、公司管理制度调整、营销活动量化管理 |
| `must_include` | 公司能力建设 / 管理制度调整 / 营销活动量化管理 |
| `source_chunks` | 第六章开篇总述句（非 6.1.2 子项） |

语料原文（`thesis_y_refrigeration.txt` L48）：

> 第六章…提出**公司能力建设、公司管理制度调整、营销活动量化管理**这 3 个方面的 6 项措施…

第六章实际结构：

```
6   保障方案（总述：上述三个方面）
├── 6.1 公司能力建设
│   ├── 6.1.1 信息系统建设
│   └── 6.1.2 研发能力建设 ← 内部又有「三个内容」
├── 6.2 管理制度调整
└── 6.3 营销活动量化管理
```

**歧义点**：query 写「能力建设」易指向 §6.1；但 golden 要的是**第六章总述的三个保障方面**（6.1 + 6.2 + 6.3 并列），其中第一项叫「公司能力建设」。

### 5.2 LLM 如何推理（4 轮检索 + 合成）

| 轮次 | 决策 | 行为 |
|------|------|------|
| iter0 | `code_gen` | dense + lexical 并行；**首轮即命中** `a2fee26d`（第六章总述）与 `7c5a8f74`（6.1.2） |
| iter1 | `code_gen` | 继续搜「三个方面」；两 chunk 仍在结果集 |
| iter2 | `code_gen` | lexical 精搜；锁定 `7c5a8f74` 的 context_after 含 (2)(3) |
| iter3 | `synthesized` | 未再检索，直接合成 |

推理要点（`reasoning_summary.txt`）：

1. **正确 chunk 已见**：iter0 就读到 `a2fee26d` 的「公司能力建设、公司管理制度调整、营销活动量化管理」
2. **主动排除 golden**：推理写明「那是保障方案的三个方面，不是能力建设的三个方面」
3. **层级误判**：发现 6.1 仅 6.1.1/6.1.2 两个子节，转而采信 6.1.2 内「研发能力的建设包括三个内容」
4. **未用结构导航**：未调 `doc_profile` 看 6.1/6.2/6.3 并列大纲

工具调用（12 次）：dense×3、lexical×4、graph augment×5。

### 5.3 如何回答

引用单一 chunk `7c5a8f74`（§6.1.2 研发能力建设），列出：

1. 研发硬件设施的建设
2. 依靠大数据指导研发
3. 增强研发团队能力

开篇写「第六章…之『能力建设』部分…研发能力建设」——**文档层级定位在 6.1.2 子节**。

`faith=100%`（rag-diag）：答案忠实于所 cite 的 chunk，但 cite 错了层级。

### 5.4 挂题根因（重新定性）

| 维度 | 结果 |
|------|------|
| 检索 | ✅ iter0 已召回 `a2fee26d` 与 `7c5a8f74` |
| 合成/消歧 | ✅ 在两层「三个方面」之间选了**更贴 query 字面**的 6.1.2 |
| Agent 答案 | ✅ 研发硬件 / 大数据 / 研发团队——忠实于 `7c5a8f74`，`faith=100%` |
| `must_include` | ❌ 0/3（golden 要的是另一层答案） |
| 自动标签 `RETRIEVAL_MISS` | **误标** |

**结论：Q19 应归为 B 类（golden/query 错位），不是产品 bug。**

### 5.4.1 为何 agent 答法更切题

Query：

> 论文提出 Y 冷冻设备公司应该从**哪三个方面进行能力建设**？

语法上「哪三个方面」修饰的是「**进行能力建设**」，不是「保障方案有哪三个方面」。

| 候选答案 | 与 query 的贴合度 |
|----------|-------------------|
| **6.1.2 三项**（agent） | ✅ 均在「能力建设」范畴内；原文写「研发能力的建设包括**三个内容**」 |
| **第六章总述三项**（golden） | ❌ 后两项「管理制度调整」「营销活动量化管理」属于 6.2/6.3，**不是能力建设** |

Golden 三项实为**第六章保障方案**的并列大纲（6.1+6.2+6.3），只有第一项叫「公司能力建设」；与 query 字面不对齐。Golden `description` 写「原 golden 误标为 6.1 子节」——该次修正方向可能**反了**。

### 5.5 修复方向（改 golden，不改 agent）

| 优先级 | 方向 |
|--------|------|
| P0 | ~~**改 golden `must_include`**~~ ✅ 已对齐 §6.1.2：`研发硬件设施` / `大数据指导研发` / `研发团队` |
| P0 | ~~**`expected_answer`**~~ ✅ 已更新 |
| 可选 | 另起一题评第六章总述：「论文第六章保障方案从哪三个方面提出措施？」 |

---

## 6. 逐题深析：Q79

**产物**：`…/realistic_q79/`

### 6.1 Query 问了什么

```
华为IPD流程中开发阶段有多少个活动？哪个阶段活动最多？
```

Golden（`manual_override` + `llm_judge=pipeline_fault`）：

- 开发阶段 **92** 个活动，是所有阶段中最多的
- 原文实核：活动编号 168–259 = 开发段，共 92 条（`ipd_activity_count_trace.md`）

### 6.2 LLM 如何推理

| 轮次 | 行为 |
|------|------|
| iter0 | dense + lexical 命中 IPD 活动表 chunk |
| iter1 | `doc_chunks()` 全量 + 沙箱 Python 按行统计 → **开发 93**，其余五阶段与 golden 一致 |
| iter2 | 合成 |

要点：五阶段副计数全对，仅开发 **+1** → 遍历完整，差在**解析/去重**，不是漏召回（`recall=100%`）。

### 6.3 如何回答

> 开发阶段 **93** 个活动，最多。概念 81 / 计划 86 / 验证 59 / 发布 30 / 生命周期 22。

「哪个阶段最多」仍正确；开发数比 golden 多 1。

### 6.4 挂题根因（A 类 — 真产品问题）

| 维度 | 结果 |
|------|------|
| 检索 | ✅ `recall=100%`，`doc_chunks` 拉满 102 chunk |
| 统计 | ❌ 沙箱按行 `split('\t')[1]=='开发阶段'` 计数，**未按活动编号去重** |
| `RETRIEVAL_MISS` 标签 | **误标**（实为 `pipeline_fault`） |

**精确 +1 机理：活动 #188 折行 + chunk 切块边界重复一行。**

1. **原文**（`huawei_ipd_370_activities.txt` L742–743）活动 188 名称跨行，第二行以 `BOM` 续写，不是新活动：

   ```
   188	开发阶段	开始进行EC管理，发布初始
   BOM	SE-130	…
   ```

   原文按行 `split('\t')[1]=='开发阶段'` → **92**（与 golden / `ipd_activity_count_trace.md` 一致）。

2. **灌库切块**（E2E PG `chunks` cursor 55/56）同一活动头被切成两块，**尾部与首部各保留一行 `188 … 开发阶段 …`**：

   | chunk | 边界内容 |
   |-------|----------|
   | cur=55 末行 | `188  开发阶段  开始进行EC管理，发布初始` |
   | cur=56 首行 | `188  开发阶段  开始进行EC管理，发布初始` + `BOM …` 续行 |

3. **`doc_chunks()` 拼接 102 chunk 后**，joined 文本出现连续重复（离线复现）：

   ```
   188\t开发阶段\t开始进行EC管理，发布初始
   188\t开发阶段\t开始进行EC管理，发布初始
   BOM\tSE-130\t…
   ```

   对 joined 文本做与沙箱相同的行级 tab 分列计数 → **开发阶段 93**；其余五阶段仍与 golden 一致（81/86/59/30/22）。SSE `context_after`（chunk `c45653ea`）可见同一重复，与 dense 邻块窗口一致。

4. **排除项**：非漏 chunk、非多文档 scope、非检索 top-K 截断；五阶段副计数全对，说明遍历完整，仅 **#188 边界重复行** 导致开发阶段 +1。

### 6.5 修复方向

| 优先级 | 方向 |
|--------|------|
| P0 | 服务端 `table_stats` / line-grep：`^\d+\t{stage}\t` + `set(活动编号)` 去重，少让 LLM 沙箱手搓行计数 |
| P1 | codegen 示例强调：**跨 chunk 拼接后同一编号只计一次**；折行活动（续行非 `\d+\t` 开头）勿当新行 |
| P2 | 切块：尽量避免在活动行中间切断，或在 ingestion 归一化折行表项 |
| P3 | `doc_chunks(doc_ids=[ipd])` 收窄 scope（不治本，但减噪声） |

---

## 7. 逐题深析：Q88

**产物**：`…/realistic_q88/`（`response.json`、`reasoning_summary.txt`、`trace_reasoning.jsonl`）

### 7.1 Query 问了什么

```
华为IPD流程中验证阶段有多少个活动？发布阶段有多少个活动？
```

两层计数（同文档、同表）：

| 子问题 | 原文实核（`ipd_activity_count_trace.md`） |
|--------|------------------------------------------|
| 验证阶段 | 编号 **260–318**，共 **59** |
| 发布阶段 | 编号 **319–348**，共 **30** |

Golden：

| 字段 | 内容 |
|------|------|
| `expected_answer` | 验证阶段59个，发布阶段30个 |
| `must_include` | **整串** `验证阶段59个，发布阶段30个`（单项） |
| `manual_override` | `answer_correct=false`（依据旧 run 45/24，**与 full116 不符**） |

### 7.2 LLM 如何推理

| 轮次 | 行为 |
|------|------|
| iter0 | `dense_search` + `lexical_search` 并行，命中验证/发布活动表片段 |
| iter1 | `doc_chunks(doc_ids=[ipd])` + 沙箱统计 → stdout：`验证阶段活动数量: 59` / `发布阶段活动数量: 30` |
| iter2 | 合成 |

路径与 Q79 同类（先搜后全量 `doc_chunks`），但本次计数**与原文一致**。

### 7.3 如何回答（full116）

> 根据IPD流程文档，验证阶段共有**59**个活动，发布阶段共有**30**个活动。

数值正确；cite 了含表行的 chunk（`157a6a40…`、`175206eb…`）。

### 7.4 挂题根因（B 类 — golden 格式误伤）

| 维度 | 结果 |
|------|------|
| 检索 | ✅ `recall=100%` |
| 计数 | ✅ 59 / 30（与 golden / 原文一致） |
| `must_include` | ❌ 整串未命中 |
| `RETRIEVAL_MISS` 标签 | **误标** |

**为何 `must_include` 失败**：golden 要连续子串 `验证阶段59个，发布阶段30个`；agent 写成 `验证阶段**共有**59个**活动**`（两阶段各插入「共有」「活动」）。`contains_loose` 在「段」与「5」、「发布阶段」与「3」之间累计 gap 超限 → 整串 FAIL。

拆项检测（同答案）：

| token | 命中？ |
|-------|--------|
| `59个` | ✅ |
| `30个` | ✅ |
| `验证阶段` / `发布阶段` | ✅ |
| `验证阶段59个，发布阶段30个`（整串） | ❌ |

### 7.5 与旧 run（45/24）的关系

`golden_set` 中 `llm_judge=pipeline_fault` 记录的是 **full107 旧 run**（验证 45 / 发布 24，少计 -14/-6）。本地已无该产物；机理推测为 **未走全量 `doc_chunks`**、仅据 dense top-K 片段估数，或行计数未覆盖 260–318 / 319–348 全段。

**full116 已数对**，故本题在最新 run 下**不是** Q79 式真产品挂题，而是 **eval 口径 + 旧 override 未刷新**。

### 7.6 修复方向（改 golden，不改 agent）

| 优先级 | 方向 |
|--------|------|
| P0 | `must_include` 拆项：`["59个", "30个"]`（或加 `验证阶段`/`发布阶段` 防串库）；对齐同 subset Q81 的 `81个` 写法 |
| P0 | 更新 / 移除 `manual_override.answer_correct=false` 与过时的 `llm_judge` 45/24 备注 |
| 观测 | 可再跑 2–3 次 Q88，确认 `doc_chunks` 计数稳定（旧 run 方差仍可能存在） |

---

## 8. 逐题深析：Q101（C 类）

**产物**：`…/realistic_q101/`

### 8.1 Query

```
论文中Y冷冻设备公司的营销问题和咨询文章中国内企业软件市场的问题，有什么共同的根源？
```

跨 **2 篇语料**（`thesis_y_refrigeration.txt` + `consulting_platform_network_effects.txt`）的主题综合。

Golden：`must_include` 为 **整段** expected_answer（生产者/销售导向 vs 客户价值导向 + 两侧例证）。

### 8.2 full116 表现

| 维度 | 结果 |
|------|------|
| 检索 | ✅ `recall=100%`；iter0–1 命中论文 chunk + 咨询文 `d2174a64` 等 |
| 拒答 | ❌ 无（旧 `llm_judge`「流式截断只剩 `{`」**不适用于本 run**） |
| 答案框架 | **管理主义（Managerialism）** + 手段与目的背离 |
| 两侧例证 | ✅ 以产定销 / 照搬营销策略（论文）；✅ 项目贩子 / 重获客轻产品（咨询文） |
| `must_include` | ❌ 整串未命中 |

### 8.3 与 golden 的差异（为何挂）

| golden 要的表述 | agent 实际表述 | 语料中有无字面 |
|----------------|----------------|----------------|
| 生产者导向 / 销售导向 | 管理主义、短期指标、照搬模式 | 「销售导向」仅见于咨询文章节标题；**无「生产者导向」「客户价值导向」字面** |
| 短期签约金额 / 全生命周期价值 | 短期利益导向、获取客户而非产品优化 | 咨询文有「短期签约金额」「全生命周期」语义，agent 未复述 golden 句式 |
| 以产定销 | ✅ 明确写出 | 论文有 |
| 客户价值导向 | 写「客户价值」但未套 golden 框架 | 多篇有「客户价值」语义 |

**结论**：agent 选了咨询文里的 **管理主义** 作为上位概念（`6cbf3aa9`、`d2174a64` 有据），与 golden 的 **营销理论框架**（生产者 vs 客户价值导向）是同一问题的两种合法综合口径，**不是检索失败，也不是事实性胡编**。

### 8.4 归类与标签

| 类 | 判定 |
|----|------|
| **C 类** | ✅ 跨文档综合的**框架/措辞**与 golden 不一致 |
| A 类 | ❌ 非数错、非漏召回 |
| B 类 | 部分重叠（整串 `must_include` 过死），但核心差异是**合成角度**而非 mere 标点 |

`RETRIEVAL_MISS`：**误标**。

### 8.5 修复方向

| 优先级 | 方向 |
|--------|------|
| P0 | `must_include` 拆语义锚点：`以产定销`、`项目贩子`、`客户价值`（或 `短期`+`客户`），勿绑整段 paraphrase |
| P0 | 接受 **管理主义** 为等价正确答案（`manual_override.answer_correct=true` 或双框架 keywords） |
| P1 | 刷新过时 `llm_judge` / `manual_override`（流式截断备注） |
| 可选 | 合成 prompt：跨文档「共同根源」须**显式点两侧**各一例，再归纳 |

---

## 9. 逐题深析：Q104（C 类）

**产物**：`…/realistic_q104/`

### 9.1 Query

```
在这7份文档中，哪些文档提到了'客户'的重要性？分别从什么角度？
```

跨 **7 篇语料** 的枚举 + 分文档角度说明。

Golden：4 条 **定制句式** `must_include`（Y冷冻/智遥咨询/薪酬解构/云南白药各一条）。

### 9.2 full116 表现

| 维度 | 结果 |
|------|------|
| 检索 | ✅ `recall=100%`（旧 judge「33% + 拒答」**不适用**） |
| 工具 | dense + lexical + `doc_chunks` + `doc_profile` + `doc_summary` |
| 答案结构 | 列出 **4 份文档** + 各自角度 + cite |

**Agent 四件套**：

| # | 文档 | agent 角度 |
|---|------|------------|
| 1 | `thesis_y_refrigeration.txt` | STP/4R、客户需求导向、客户价值 |
| 2 | `baiyao_it_planning.txt` | 客户优先原则、以客户价值为中心 |
| 3 | `consulting_platform_network_effects.txt` | 用户价值、合同金额 vs 业务改进 |
| 4 | `huawei_ipd_370_activities.txt` | 客户访谈、BETA 测试、需求融入流程 |

**Golden 四件套**：

| # | 文档 | golden 角度 |
|---|------|-------------|
| 1 | Y冷冻论文 | 4R — 关联和**回应**客户需求 |
| 2 | 智遥咨询 | 平台效应 — **用户生态价值** |
| 3 | **薪酬解构** | 薪酬**一致性原则** — 岗位职责与薪酬匹配 |
| 4 | 云南白药 | IT 规划 — 以客户价值为中心 |

### 9.3 差异矩阵

| 对比项 | 结果 |
|--------|------|
| 云南白药 | ✅ 语义对齐（客户优先 / 客户价值为中心） |
| Y冷冻论文 | △ 都提 4R/客户，agent 未写 golden 句式「关联和回应客户需求」 |
| 智遥咨询 | △ 都提平台/用户价值，agent 未写「用户生态价值」字面 |
| 薪酬解构 | ❌ agent **未列**；该文仅案例里出现「客户招待」，与「客户重要性」关联弱，golden 角度偏 **薪酬原则** 而非客户主题 |
| 华为 IPD | agent **多列**；活动表大量客户验证活动，**合理解读**，但 golden 未纳入 |

四条 `must_include` **整句命中率 0/4**；零散词命中：`4R`、`平台效应`、`客户价值为中心`、`客户优先` 等。

### 9.4 归类

| 类 | 判定 |
|----|------|
| **C 类** | ✅ 已完成跨文档综合，挂因是 **golden 定制句式 + 期望篇目** 与 agent 枚举不一致 |
| 检索弱项 | ❌ full116 未复现（旧备注失效） |
| 评测设计 | △ 薪酬篇入 golden 较牵强（query 问「客户」重要性） |

`RETRIEVAL_MISS`：**误标**。

### 9.5 修复方向

| 优先级 | 方向 |
|--------|------|
| P0 | `must_include` 改为 **分文档关键词**（如 `4R` + `平台效应` + `一致性原则` + `客户价值为中心`），勿绑四整句 |
| P1 | 复核 golden 是否应包含 **薪酬解构**（客户关联弱）或改为 **ADR / IPD** 等客户证据更充分的篇目 |
| P1 | 刷新 `manual_override` / `llm_judge`（拒答、recall 33%） |
| 可选 | 合成 prompt：枚举题须 **doc 文件名 + 角度一句**，便于评测拆词 |

---

## 10. D 类深析：Graph 通道（9 题）

**Golden**：`tests/rag_quality/golden_set_graph.json` v2（4 `negative_controls` + 5 `multi_hop`）  
**产物**：`…/graph_q1`…`graph_q9`（full116 observable 批跑）  
**对照 eval**：`crates/app/tests/e2e_output/realistic_graph_eval_v5_on.log` / `v5_off.log`

### 10.1 总览：答案对、指标挂

| 维度 | augment **ON**（full116 / v5_on） | augment **OFF**（v5_off） |
|------|-----------------------------------|---------------------------|
| `must_include` / 答案 | **9/9** | **9/9** |
| `graph_context_hit` | **9/9**（augment 每题有命中） | — |
| `graph_retrieval_called`（trace 任意） | **9/9** | **2/9** |
| NC `graph_false_positive` | **4/4** | **0/4** |
| MH `graph_called` | **5/5** | **2/5** |
| `graph_cited` | **0/9** | **0/9** |
| `source_recall` | **0/9** | **0/9** |

**结论**：Graph 9 题在 full116 下**没有一道答案挂题**；挂的是 **通道评测口径** 与 **augment 侧车行为** 的错位。

### 10.2 NC 4/4「误触 graph」——评测口径问题，非 agent 违规路由

**现象**：`negative_controls` 4 题 `graph_retrieval_called=true`，但 `must_include` 全 PASS。

**逐题 agent 路由（reasoning_summary）**：

| 题 | subset | 模型首轮策略 | 显式 `client.graph_search`？ |
|----|--------|--------------|------------------------------|
| graph_q1 | NC | 单跳属性 → **只用 lexical** | ❌ |
| graph_q2 | NC | 活动号查找 → **lexical/dense** | ❌ |
| graph_q3 | NC | ADR 日期 → **lexical/dense** | ❌ |
| graph_q4 | NC | 同节状态列表 → **不用 graph** | ❌ |

以 **graph_q1** 为例（`…/graph_q1/reasoning_summary.txt`）：

> 「单跳属性用 lexical_search 或 dense_search，**不用 graph_search**」

实际 `tool_results` 仍出现 `graph_retrieval`（`degrade_reason: graph_augment`，`raw_hit_count: 5`），`metadata.json` 记 `stage=graph_retrieval, reason=graph_augment`。

**机理**（`crates/rag-core/src/runtime/bridge.rs` + `graph_augment.rs`）：

1. Agent 只调 `dense_search` / `lexical_search`（codegen 沙箱）。
2. Bridge 在 `RETRIEVAL_GRAPH_AUGMENT=on` 时**并发**跑 `graph_augment`，用 primary 命中的 chunk 作种子做 BFS。
3. Augment 产出写入 `graph_context` + 遥测 `ToolResult{ tool: "graph_retrieval", degrade_reason: "graph_augment" }`。
4. Eval 的 `graph_called` = trace 里**任意** `graph_retrieval`（`rag_quality_prod.rs:2285`）→ NC 4 题全计为 false positive。

**对照**：`v5_off` 下同样 4 题 NC **`graph_false_positive=0/4`**，说明 v2 prompt 收窄后 agent **已不再误调 graph_search**；full116 的 4/4 是 **augment 侧车** 触发的统计假象。

### 10.3 `graph_cited=0/9`——合成策略 + 评测定义

**定义**（`rag_quality_prod.rs:2293-2305`）：最终 `citations[].chunk_id` 是否落在 `graph_retrieval` 返回的 `graph_relation` chunk_id 集合中。

**实测**（9 题一致）：

- `graph_context_hit=true`：augment 或显式 `graph_search` 都把关系块送进了 observation。
- 合成 cite 的 `layer` 全是 `lexical_retrieval` / `dense_retrieval` 正文块（如 graph_q5 cite `1afc8f65`、`46d6a94c`）。
- 即使 iter0 沙箱 `graph_search` 返回 `chunk_type: graph_relation`（graph_q5 SSE `code_execution_result`），最终答案仍 cite ADR 段落，**不 cite 关系块 id**。

**原因链**：

1. `graph_augment` 给关系块打 `source_channel: graph_relation` + `retrieval_hint: 主体答案优先依据 chunks`。
2. `rag-answer` / 合成 prompt 要求事实 cite 正文 chunk；模型把 graph 当**辅助上下文**而非引用源。
3. 关系块文本是「A 映射到 B」短句，ADR 表/段落已含完整映射表 → 模型理性选正文 cite。

**含义**：`graph_cited=0` 不代表 graph 无贡献（可能帮模型定位到正确段落），只代表**未把关系块当正式引用**——对 multi_hop 题的「图通道价值」评测仍偏弱。

### 10.4 multi_hop 5 题：dense/lexical 仍够，显式 graph 不稳定

| 题 | 显式 `graph_search`（codegen） | 答案 | 主要证据来源 |
|----|-------------------------------|------|--------------|
| graph_q5 MH1 | ✅ iter0 三路并行 | PASS | 正文 chunk + 表 4.2 |
| graph_q6 MH2 | ❌ dense/lexical | PASS | ADR-0009 §1/§4 段落 |
| graph_q7 MH3 | ❌ lexical 为主 | PASS | `4915e333` RuntimeBridge 块 |
| graph_q8 MH4 | ❌ lexical 分文档搜 | PASS | EvidenceGate / run_auto_fallback 块 |
| graph_q9 MH6 | ✅ iter0 三路并行 | PASS | complete_with_tools + dispatch 段落 |

- augment **OFF**：仅 q5、q9 显式 `graph_search`（2/5）；其余 3 题 lexical 单跳即够。
- augment **ON**：trace 每题都有 `graph_retrieval`（含多轮 augment），但**不改变答案正确性**。
- 与 `2026-07-04-graph-channel-analysis.md` v3（显式 graph 3/5）相比：prompt 已教路由，但 **ADR 语料 BM25/dense 单 chunk 已能拼全链**，graph 对 must_include 非必要条件。

### 10.5 `source_recall=0/9`（评测盲区，非检索失败）

`chat_streaming.rs` done 事件剥 `tool_results[].data` → `extract_retrieved_chunks` 恒空 → rag-diag / graph eval 的 `recall@final=0%`。SSE trace 里 chunk 完整（单条 ~100KB+）。**与 Graph 通道产品能力无关**，但污染 graph 通道报告的可读性。

### 10.6 D 类归类

| 类 | 判定 |
|----|------|
| **D 类** | ✅ Graph 9 题答案全对；挂因是 **augment 与 legacy 指标冲突** + **cite 策略** + **source_recall 盲区** |
| A 类（产品答错） | ❌ |
| B 类（golden） | ❌（graph golden `must_include` 全过） |

**与 realistic 107 题的关系**：realistic 批跑同样 `RETRIEVAL_GRAPH_AUGMENT=on`，单跳题（如 Q17）trace 也会出现 augment 附带 `graph_retrieval`（§4.5），属同一类**通道噪声**，非挂题主因。

### 10.7 修复方向（未实现）

| 优先级 | 方向 | 预期效果 |
|--------|------|----------|
| **P0** | Eval 拆分 **`graph_explicit_called`** vs **`graph_augment_hit`**；NC `graph_false_positive` 只看 explicit | ✅ 已实现（`rag_quality_prod.rs`） |
| **P1** | multi_hop 评测增 **`graph_context_used`**（推理或答案提及关系链）或放宽 **`graph_cited`**（cite `supporting_chunk_ids` 也算） | 能量化图对合成的实际贡献 |
| **P1** | augment 降噪：primary lexical 高置信单跳时 **skip augment**（NC 类 query 特征） | 减 trace 噪声与 Milvus BFS 成本 |
| **P2** | `source_recall` 改读 SSE trace / citations | graph 报告 recall 不再恒 0 |
| **P2** | 种子小写化 bug（`graph-channel-analysis.md` §3.1） | 提升 augment/显式 graph 命中率与关系质量 |

### 10.8 命令备忘

```bash
# Graph 通道 eval（对比 augment 开关）
RETRIEVAL_GRAPH_AUGMENT=1 E2E_MODE=nightly cargo test -p app --test product_e2e \
  realistic_graph_eval --features product-e2e -- --ignored --nocapture --test-threads=1

RETRIEVAL_GRAPH_AUGMENT=0 E2E_MODE=nightly cargo test -p app --test product_e2e \
  realistic_graph_eval --features product-e2e -- --ignored --nocapture --test-threads=1

# 单题 observable
RAG_GRAPH_QUERIES=1 E2E_MODE=nightly cargo test -p app --test product_e2e \
  realistic_graph_observable_probe --features product-e2e -- --ignored --nocapture --test-threads=1
```

---

## 11. 待分析队列

按优先级：

1. ~~Q17~~ ✅（§4）
2. ~~Q19~~ ✅（§5，golden 已修）
3. ~~Q79~~ ✅（§6）
4. ~~Q88~~ ✅（§7）
5. ~~Q101~~ ✅（§8）
6. ~~Q104~~ ✅（§9）
7. ~~Q33/47/49/60/90~~ ✅（golden 已修）；~~Q17/Q88~~ ✅
8. ~~Graph 通道~~ ✅（§10）

---

## 12. 命令备忘

```bash
# 离线诊断（realistic）
cargo run -p e2e-analyzer -- rag-diag \
  --run crates/app/tests/e2e_output/rag_quality_smoke_v5/e2e_20260704-172230_local_edb73279542c4f86a634086cf1f6eaa2 \
  --golden tests/rag_quality/golden_set_realistic.json \
  --output crates/app/tests/e2e_output/rag_diag_realistic_20260705.md

# 单题 observable 复现
RAG_REALISTIC_QUERIES=17 E2E_MODE=nightly cargo test -p app --test product_e2e \
  realistic_observable_probe --features product-e2e -- --ignored --nocapture --test-threads=1
```
