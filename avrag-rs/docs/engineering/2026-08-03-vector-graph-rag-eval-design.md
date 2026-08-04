# Vector Graph RAG 评测体系设计（graph81 + 六基线）

| 项目 | 内容 |
|---|---|
| 状态 | **2026-08-04：产品路径 D1（VGRAG）正式验收**（graph81 **78/81**）；`DENSE_BACKEND=vgrag` 默认；B0–B4 降为可选对照。后续见 `2026-08-04-vgrag-accept-and-skillopt-merged-plan.md` |
| 日期 | 2026-08-03 |
| 源跑次 | full149 `v2_20260803-090356` + `realistic_corpus_full_eval/qNNN.json` |
| 题单 | `tests/rag_quality/fixtures/graph81_question_ids.json` |
| core 标注 | `tests/rag_quality/fixtures/graph81_core_annotation.json`（启发式预标，待人工确认） |
| 关联 | `2026-08-03-full149-bge-m3-behavior-and-fixes-handover.md`；`docs/plans/2026-07-23-lexical-graph-augment-scoring-design.md` |
| 文献锚 | Zilliz Vector Graph RAG（dense multi-way 种子 + expand + relation rerank） |

---

## 0. 目标

在 **固定语料 / 固定 graph81（81 题）** 上，用 **六条基线** 回答：

1. **纯现状**（已跑归档）表现如何？  
2. **仅把图 chunk 做 L-eval RRF**（触发方式不变）相对纯现状 / 关图是否有增益？  
3. **关图** 掉多少？  
4. **题卡强制 explicit graph 槽 + 独立图调用 + L-eval RRF** 是否更好？  
5. **lexical 侧车但 hop=3 + L-eval RRF** 是增益还是噪声？（仅评测开 3 跳，生产默认仍 1）  
6. **dense multi-way 种子强制管线（VGRAG 向）+ L-eval RRF** 是否优于绑 BM25 侧车？

**非目标（本波）**：完整复刻 Zilliz 的 **LLM relation 单次 rerank**（可记 B4 的可选增强，默认先不做）；替换整页 Agent；每臂全量 149。

---

## 1. 已拍板决策（2026-08-03）

| # | 议题 | 决定 |
|---|------|------|
| 1 | B2 graph 槽满足条件 | **仅 explicit** `graph_retrieval`（`degrade_reason=graph_augment` 的侧车 **不算** 满足题卡） |
| 2 | 「进 RRF」落地深度 | **L-eval**：评测/harness 或 bridge 后处理把图支撑 chunk 与 dense/BM25 列表做 `global_rrf_merge`，结果写入 artifact **并**作为模型可见上下文；不先改全产品 Agent 单次 fused API |
| 3 | graph81-core | **要标注**（见 §3.2 + `graph81_core_annotation.json`） |
| 4 | B3 hop=3 | **仅评测**；生产默认仍 `GRAPH_AUGMENT_HOPS=1` |
| 5 | **B4** | **本波一起跑**——dense multi-way 种子强制管线 + expand + L-eval RRF（见 §2.6） |
| — | 基线编号 | **B_frozen** = 已跑纯现状；**B0** = 现状触发 + L-eval RRF |

---

## 2. 六条基线

统一约束：同一 corpus digests、同一 embedding（bge-m3）、同一 judge；graph81 的 `E2E_QUESTIONS`；**默认不** `E2E_FORCE_INGEST`（除非 digests 变）。

### 2.1 总表

| ID | 名称 | 触发 | hop | 图 chunk 融合 | 数据来源 |
|----|------|------|-----|---------------|----------|
| **B_frozen** | **pure_current（归档）** | lexical 强制 1 跳 side-car | 1 | **不进** RRF（现状） | **已跑** `v2_20260803-090356`，**不再重跑**除非 digests 变 |
| **B0** | **current_rrf** | 同 B_frozen（lexical 侧车） | 1 | **L-eval 进 RRF** | 新跑 |
| **B1** | **graph_off** | 无图 | — | — | 新跑 `RETRIEVAL_GRAPH_AUGMENT=0` |
| **B2** | **forced_slot_rrf** | **题卡 `graph` 槽 + SDK explicit graph** | 1（建议） | **L-eval 进 RRF** | 新跑（需工程） |
| **B3** | **hop3_rrf** | lexical 侧车（同 B0 触发） | **3（仅评测）** | **L-eval 进 RRF** | 新跑（需 hop 开关 + RRF） |
| **B4** | **dense_seed_rrf** | **系统强制 dense multi-way 种子**（entity+relation 向量）→ expand | **1**（默认；与 B3 正交） | **L-eval 进 RRF** | 新跑（需种子管线工程） |

### 2.2 各自变量（正交说明）

| 对比 | 主要回答 |
|------|----------|
| B_frozen vs B0 | **仅**「图证据进 L-eval RRF」的边际（触发不变） |
| B0 vs B1 | 在「会触发 lexical/图侧车」的题上，**有图+RRF** vs **无图** |
| B_frozen vs B1 | 纯现状 side-car vs 关图 |
| B0 vs B2 | side-car 触发 vs **强制 explicit 槽**（融合同为 L-eval RRF） |
| B0 vs B3 | hop1 vs hop3（触发与 RRF 同族） |
| **B0 vs B4** | **绑 BM25 侧车** vs **dense multi-way 强制种子**（融合同 RRF；本波核心文献对照） |
| B2 vs B4 | Agent 显式 graph 槽 vs 系统强制种子管线（都不靠「碰巧 lexical」） |
| B4 vs B1 | dense 种子图 vs 完全无图 |

注意：B2 / B4 都改了触发机制，不要把与 B0 的差异全归因于 RRF。

### 2.3 L-eval RRF 精确定义（B0 / B2 / B3 / B4）

```
dense_lists   = dense_retrieval Ok 的 chunks
sparse_lists  = lexical 的 bm25_chunks（优先）或非 graph 的 chunks
graph_lists   = explicit graph_retrieval chunks
              + lexical/telemetry graph_context.evidence_chunks
fused         = RRF(dense, bm25, graph) k=60
context_for_model = codegen observation 在 GRAPH_L_EVAL_RRF=1 时
                    **优先** 输出 fused（即使模型 print 了局部结果）
```

**权重默认：** 三路等权。

**B_frozen：** 不做融合。

**实现落点（2026-08-03 fix）：**

| 层 | 行为 |
|----|------|
| `lexical` | 可选本地 bm25∪graph RRF；并写 `bm25_chunks` 供三路用 |
| `codegen_bridge` observation | **三路 RRF** dense∪bm25∪graph |
| B2 脚本 | `RETRIEVAL_GRAPH_AUGMENT=0` + 强制 explicit graph |
| hop>1 | 仅 `GRAPH_EVAL_MODE` / L-eval / baseline env 时生效 |

**B4 范围说明：** 仍挂在 lexical 触发路径上启用 dense ANN 种子（非完全脱离 Agent 的独立管线）；对比 B0 的是 **种子方式**（terms vs entity ANN），不是「是否必须调 lexical」。

### 2.4 B2 题卡契约（钉死）

- 动作 id：`graph`（与 runtime 名统一，实现时写死一张表）。  
- **满足条件：仅 explicit Ok**（`tool=graph_retrieval` 且 **不是** `degrade_reason=graph_augment`）。  
- lexical 侧车 **不够**。  
- 结构门：缺 Ok → `required_action_missing_continue`。

### 2.5 B3 hop

- 评测：`GRAPH_AUGMENT_HOPS=3`（或等价）**仅实验 env**。  
- 生产默认：**保持 1**。  
- 触发仍为 lexical 侧车，**不是** B2/B4。

### 2.6 B4：dense multi-way 种子（本波必跑）

对齐 Zilliz Vector Graph RAG 的 **索引/查询前半段**（种子 + 扩图），在 **Agent 外壳** 内评测：

```
query
  → (评测管线强制，不依赖模型写 code)
      entity_extract(query)          # 可用现有 query-card / 轻量 LLM / 规则+NER，实现时钉一种
      seed_entities = dense_search(entity_collection, entities)
      seed_relations = dense_search(relation_collection, entities∪query_text)
      subgraph = expand(seeds, hop=1)   # metadata ID 扩 1 跳；与 B3 的 hop3 分开
      graph_chunks = hydrate(supporting_chunk_ids)
  → 与本题 dense/BM25 工具结果一起 L-eval RRF
  → 上下文交给后续 Agent 生成（或同轮合成，实现选一种并固定）
```

| 维度 | B0 | B4 |
|------|----|----|
| 图触发 | Agent **lexical** → 侧车 | **系统强制** dense multi-way 种子 |
| 依赖模型调 graph/lexical | 要会调 lexical | **不要** |
| 种子信号 | 词面 terms | **entity/relation 向量语义** |
| hop | 1 | **1**（本波；加深用 B3 对照，不叠在 B4） |
| 融合 | L-eval RRF | **同** L-eval RRF |
| relation LLM rerank | 无 | **本波默认无**（减变量；若加记 B4+rerank 为扩展） |

**实现前置（B4）：**

| 项 | 说明 |
|----|------|
| entity / relation 向量集 | 已有 `rag_kg_entities` / `rag_kg_relations`（e2e 已灌） |
| dense 检索入口 | storage 层 entity/relation ANN；评测模式可走内部 API，不必先暴露 `client.graph` |
| expand | 复用 `graph_augment` / `search_graph` 的 1 跳逻辑，但 **种子来自 dense 命中实体**，不是 BM25 terms-only |
| 开关 | 评测 env 如 `E2E_GRAPH_BASELINE=B4` 或 `RETRIEVAL_GRAPH_SEED=dense_multiway`（实现时定名） |
| 与 Agent | 最小：在 retrieve 阶段 **注入** fused 上下文；避免要求模型自己会 multi-way |

**B4 成功判读：** 相对 B0，若 core 子集 PASS/correctness 明显升 → dense 种子优于绑 BM25；若仅 fire 高、分不升 → 种子对了但融合/生成未用上。

---

## 3. 评测集

### 3.1 graph81

| 字段 | 定义 |
|------|------|
| 入选 | 源跑 `tool_trace` 含 ≥1 次 `graph_retrieval` |
| N | **81** |
| ID 文件 | `tests/rag_quality/fixtures/graph81_question_ids.json` |
| E2E_QUESTIONS | 见该 JSON 的 `e2e_questions` |
| 源跑 v2 | PASS 78 / PARTIAL 1 / SELECTION_MISS 1 / INFRA_ERROR 1（= **B_frozen** 分数） |

选择偏差：只含「B_frozen 下已亮图」的题 → 报告必写。

### 3.2 graph81-core（标注）

| 字段 | 定义 |
|------|------|
| 目的 | 答案 **依赖实体—实体关系或多跳结构** 的子集，降低「图触发了但题是单段事实」的污染 |
| 文件 | `tests/rag_quality/fixtures/graph81_core_annotation.json` |
| 流程 | 启发式预标 `core` / `borderline` / `unlikely` → **人工改 `human_label`**（`core` \| `not_core` \| `unsure`） |
| 报告 | 全 graph81 + **仅 core** 两套表 |

**core 入选标准（人工用）：**

- 需要连接 **≥2 个实体/文档对象** 的关系、对照、映射、链路；或  
- 标准答案自然落在 **关系三元组/跨块结构** 而非单句数字事实。  

**不入 core：** 纯单点事实、纯表行计数、对抗拒答、仅列举多数字但无结构边。

启发式预标（**非最终**）：约 13 题 `tier=core` 候选（见 annotation 文件）；人工确认后以 `human_label=core` 为准。

### 3.3 可选 graph-miss 探针（第二波）

从 149 中 **未** 触发 graph 的题抽 10–15 道专名/lexical 题，可跑 **B2 与 B4** 看强制路径是否「救回」。不进主六基线矩阵的必跑集，作附录探针。

---

## 4. 指标

### 4.1 主指标

| 指标 | 说明 |
|------|------|
| v2 PASS 率 | /81 与 /\|core\| |
| mean correctness / faithfulness | judge-ok |
| retrieval recall@15 | 有 gold 的题 |

### 4.2 图指标

| 指标 | 说明 |
|------|------|
| graph_fire_rate | 题级是否图相关成功（侧车 / explicit / **B4 种子管线**） |
| graph_augment_rate / graph_explicit_rate / **graph_dense_seed_rate** | 三种触发分记 |
| seed_entity_n / seed_relation_n | **B4**：dense 种子条数 |
| graph_context_len | 需 harness 落盘 |
| graph_chunk_in_top15 | L-eval RRF 后 channel=graph 占比（B0/B2/B3/**B4**） |
| selected_uses_graph | cite 是否落在 graph 支撑 chunk（能算则算） |

### 4.3 效率

mean iters / tools / 墙钟；code_gen_error 率。

### 4.4 判读阈值（建议）

| 对比 | 倾向 |
|------|------|
| B0 − B_frozen 且 PASS/c 升 | **L-eval RRF 有用**（触发不变） |
| B0 − B1 大 | 图内容有用 |
| B0 ≈ B1 且 fire 高 | 触发了未用上 → 盯 RRF/选用 |
| B2 ≫ B0 | 强制 explicit 槽值得产品化 |
| B3 ≤ B0 | 3 跳有害，评测可停在 1 |
| **B4 ≫ B0（尤其 core）** | **dense 种子优于绑 BM25 侧车** |
| B4 ≈ B0 | 种子通道非瓶颈 → 盯 triple/融合/生成 |
| B4 ≫ B2 | 系统强制种子优于靠模型调 graph 工具 |

---

## 5. 实现与运行

### 5.1 立刻可做

| 臂 | 动作 |
|----|------|
| **B_frozen** | **不重跑**；从 `v2_20260803-090356` + tool_trace 汇总 graph81 切片表 |
| **B1** | `RETRIEVAL_GRAPH_AUGMENT=0 E2E_QUESTIONS=... bash scripts/test-full149.sh` |
| **B0 / B2 / B3 / B4** | 均需工程后再跑 |

### 5.2 工程切片

注意：**B0 也不再是零代码**（依赖 L-eval RRF）。

1. **L-eval RRF + 遥测**（B0/B2/B3/**B4** 共用）  
2. **B1 跑数**（可与 1 并行）  
3. **B0 跑数**  
4. **并行**：B2（题卡+explicit）∥ **B4（dense multi-way 种子管线 + 注入）**  
5. **B3**（评测 hop=3 + 同 L-eval RRF）  
6. 汇总六基线报告  

### 5.3 成本粗算

单臂 graph81 ≈ 20–25min @c=8。  
新跑 **B1+B0+B2+B3+B4 ≈ 5 臂** → 墙钟约 **2–3h**（不含 B4 开发）。

---

## 6. 交付物

| # | 交付 | 状态 |
|---|------|------|
| D1 | graph81 ID JSON | **已有** |
| D2 | 本文（**六基线** + 拍板） | **本版** |
| D3 | graph81-core 标注 JSON | **启发式已写，待人工** |
| D4 | B_frozen 切片报告脚本/表 | **已有** `scripts/report-graph81-bfrozen.py` → `_reports/graph81_b_frozen.tsv` |
| D5 | L-eval RRF | **已接线** `GRAPH_L_EVAL_RRF=1`：lexical 内 bm25∪graph；codegen observation **三路** dense∪bm25∪graph |
| D6 | B1 跑 + 报告 | 待：`bash scripts/run-graph81-baseline.sh B1` |
| D7 | B0 跑 + 报告 | 待：`… B0` |
| D8 | B2 产品 + 跑 | **SDK graph + 强制卡已接线**；待跑 `… B2` |
| D9 | B3 产品 + 跑 | **hop env 已接线**；待跑 `… B3` |
| **D10** | **B4 dense 种子** | **`RETRIEVAL_GRAPH_SEED=dense_multiway` 已接线**；待跑 `… B4` |

### 实现 env 速查

| env | 作用 |
|-----|------|
| `RETRIEVAL_GRAPH_AUGMENT=0\|1` | lexical 侧车（**产品默认 off**；图扩邻在 dense=VGRAG 内） |
| `GRAPH_L_EVAL_RRF=1` | 评测用 observation 三路 RRF（产品默认勿开） |
| `RETRIEVAL_GRAPH_SEED=dense_multiway` | 旧 B4；现 dense 路径内建 multi-way seed |
| `GRAPH_AUGMENT_HOPS=1..3` | 仅 lexical 侧车；**dense VGRAG 固定 hop=2** |
| ~~`GRAPH_EVAL_FORCE_REQUIRED_GRAPH`~~ | **已移除**（`client.graph` 槽位下线） |

### 图质量：本体论封闭关系（2026-08-03）

- 范式：**知识本体之间的基础关系**（非业务动词穷举、非 BPMN 过程连接件表）。  
- **Closed set（6）**：`类型`（is-a）· `部分`（mereology, part→whole）· `参与`（continuant→process）· `依赖` · `位于` · `标识`（code→name denotation）。  
- 领域含义在 **节点**；边只表达可组合的本体构造。  
- Host `predicate_normalize`：同义折叠进 6 类；默认 **strict 丢弃未知**（`TRIPLET_PREDICATE_STRICT=0` 软保留）。  
- Prompt：`prompts/pipeline/triplet-extraction.system.md`。  
- **存量语料须重抽 KG** 后 D0/D1 才可对照图质量。

### 产品拍板（2026-08-03 晚，graph81 后）

- **`client.dense` = VGRAG**（默认）：**大池**（≤24）dense multi-way seed → hop=2 expand → 内部 RRF → **单次** final cut（≤12；k 由 dense 池 adaptive 形状 + graph boost，**不用 RRF 分做 adaptive_k**）。  
- **`DENSE_BACKEND=ann`**：纯向量 dense（对照 / 回滚）。  
- **lexical 侧车默认关**；**无 `client.graph` / 题卡 graph**。  
- cite：图证据仅真 UUID+doc 并入 dense 列表；与 lexical/grep **顺序 alias**。  
- 评测优先：**`D0`（ann）vs `D1`（vgrag）**；`B0–B4` 为 legacy，默认勿当产品验收。

跑法：`bash scripts/run-graph81-baseline.sh B0|B1|B2|B3|B4`

---

## 7. 推进顺序（更新）

```
1) 人工确认 graph81-core（改 human_label）
2) 导出 B_frozen 的 graph81 指标表（零跑）
3) 实现 L-eval RRF + 遥测
4) 跑 B1 → 对比 B_frozen
5) 跑 B0 → 对比 B_frozen / B1
6) 并行：B2 工程 ∥ B4 dense 种子工程
7) 跑 B2、B4 → 对比 B0（侧车 vs 槽位 vs 种子）
8) 跑 B3 → 对比 B0（hop）
9) 汇总产品化：RRF / 强制槽 / 默认 hop / 是否改种子为 dense multi-way
```

---

## 8. 附录：graph81 E2E_QUESTIONS

```
1,3,6,9,10,11,12,13,14,15,16,17,18,20,21,23,24,27,31,33,37,38,39,41,42,43,44,45,51,52,53,54,56,57,59,60,61,62,63,64,65,66,67,68,71,74,76,77,78,79,81,82,83,84,85,88,95,96,99,100,101,102,104,106,107,114,118,119,120,121,126,127,130,131,133,134,138,139,140,141,143
```

### B_frozen 内 nonPASS（graph81）

| n | label | subset |
|---|-------|--------|
| 53 | PARTIAL | adr_factual |
| 77 | INFRA_ERROR | ipd_table |
| 78 | SELECTION_MISS | ipd_table |

*完。*
