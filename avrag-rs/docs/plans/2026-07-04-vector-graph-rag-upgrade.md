# Vector Graph RAG 升级方案：抽取质量 + 图增强双路检索（2026-07-04）

前置文档：`2026-07-04-graph-channel-analysis.md`（P0/P1 已完成：种子大小写解析、
json 围栏、语义种子接线；本文承接其 P2 并引入新的检索架构升级）。

参考依据：
- **arXiv:2507.03608v1**（Leeds，ORAN 三管线基准）：Hybrid GraphRAG 事实准确性 0.58，
  高于纯向量 0.48（+21%）与纯图 0.50（+16%）；代价是上下文相关性最低（0.04 vs 0.10/0.11），
  冗余是 Hybrid 的主要副作用。拼接方式：**向量内容在前、图内容在后**，prompt 明确
  「向量为主体答案来源、图为结构/关系补充」。简单题纯向量最优（0.61），中/难题 Hybrid 最优
- **arXiv:2605.18490**（谓词爆炸）：约束 + 归一化 + 对齐三层治理；谓词长度硬约束、
  闭集白名单、抽取后归一化、原始谓词保留为边属性
- **zilliztech/vector-graph-rag**：纯 Milvus 实现图推理（实体/关系向量化 + 子图扩展 +
  单次重排），MuSiQue/HotpotQA/2Wiki 平均 Recall@5 87.8%——验证无图数据库的可行性

---

## 0. 架构选型结论：保持 Milvus 纯向量图谱，不迁移

最佳实践的三种架构中，本系统**已经是**「纯向量图谱」模式（Zilliz 同款）：
实体/关系向量化存 Milvus（`kg_entities.entity_dense` / `kg_relations.relation_dense`），
BFS 子图扩展在查询侧按实体名 join。**不引入 Neo4j / pgvector+AGE**：
- 双库方案的同步成本与运维复杂度不适用于当前规模
- 查询时按名 join 天然增量友好（新文档灌入即连通，零回填），这是本架构相对
  显式边存储的核心优势，迁移反而丢失
- Zilliz 项目已验证该模式在多跳基准上的召回上限足够

现状短板不在存储架构，在两处：**图数据质量**（§1）与**图通道触达率**（§2）。

---

## 1. 升级一：三元组抽取质量（谓词治理 + 实体规范 + 链式抽取）

### 1.1 实测问题（2026-07-04 基线）

| 指标 | 实测值 | 问题 |
|------|--------|------|
| 谓词爆炸 | 383 条关系 / **177 个谓词**，84% 谓词出现 <3 次 | 图碎片化，BFS 沿谓词无法聚合 |
| 语言分裂 | adr-0004 全英文谓词（`implements`/`extends with`），adr-0009 全中文（`映射到`/`关联`） | 同语料跨文档谓词永不对齐 |
| 实体词汇割裂 | 跨文档共享实体名 **0 个**（5 篇有关系文档两两比对） | 跨文档 BFS 无路可走 |
| 链断裂 | adr-0004 抽了「parse_rag_plan_decision is deprecated」但没抽「被 structured tool parsing 替代」 | 多跳链在图里不存在 |
| 覆盖缺口 | 7 篇文档 2 篇关系数为 0 | 图通道对这些文档失明 |

当前 `prompts/pipeline/triplet-extraction.system.md` 对照五策略：
长度约束 ⚠️ 有（G3）但无语言/时态规则；闭集 ❌ 仅 2 个规范谓词（`标识为`/`属于`）；
抽取后归一化 ❌（`merge_extracted_triplets` 仅小写精确去重）；本体对齐 N/A；两阶段 ❌。

### 1.2 提示词改造（`prompts/pipeline/triplet-extraction.system.md`）

> **2026-07-05 修订**：谓词治理经架构重估后**收缩**。本系统 BFS 按实体名 join（谓词
> 不参与遍历）、关系匹配走 relation_text 向量语义，谓词变体的代价仅为同义冗余边与
> 跨语言向量折损，不构成检索质量瓶颈。§1.2 的「闭集清单 + 强制中文」与 §1.3 的
> registry + LLM 归一化均**不实施**（详见 `2026-07-04-predicate-normalization-design.md`，
> 已标搁置）。落地保留：提示词卫生规则（谓词遵循文档原语言、动词原形、禁模糊词）+
> parse 时静态表归一化（`predicate_normalize.rs`，已实现）。
> §1.4 中「唯一谓词数 <60 / 长尾 <50%」降为观察项；**跨文档共享实体 >10 与
> adr-0004 替代链边存在**为主要验收。

采用**混合模式**（闭集管高频 + 开放补长尾，2605.18490 推荐）：

**新增「谓词规则」节：**
1. **语言统一**：谓词一律中文，无论 chunk 语言（消灭 adr-0004/0009 语言分裂）
2. **核心闭集**（基于实测频率 + eval 需求，约 14 个）：

   ```
   属于 · 标识为 · 撰写 · 包含 · 被替代为 · 依赖 · 实现 · 映射到
   调用 · 派发至 · 定义于 · 负责 · 使用 · 评审
   ```

   规则：优先从闭集选；语义确实不匹配才允许自创（2–4 字中文动词，动词原形，
   禁时态/语态变体）
3. 保留现有 G3 长度约束与 `标识为` 的 G1 门控（表格 catalog 映射语义不变）

**新增「实体命名规则」节（吸收 analysis 文档 P2）：**
4. 代码标识符/专有名词**照抄原文**（反引号内容原样）：`parse_rag_plan_decision`
   不得译作「解析函数」；`RuntimeBridge` 不得写成「RuntimeBridge 结构体」（禁后缀：
   结构体/机制/方法/trait）
5. 同一概念全文档统一同名（chunk 间自洽）

**新增「链式关系强制」节：**
6. 废弃/替代句式必须抽完整链：`(X, 被替代为, Y)`，若同句给出依赖则再抽 `(Y, 依赖, Z)`
7. 实现/定义句式：`(X, 实现, Y)` + 位置信息存在时 `(X, 定义于, 文件路径)`

提示词变更自动作废 completion cache（`TRIPLET_PROMPT_VERSION_HASH` 机制现成）。

### 1.3 代码层归一化兜底（第二道过滤）

**文件**：`bins/worker/src/pipeline/triplet_extraction.rs`

- `parse_triplet_item` 后、`merge_one_triplet` 去重前，过一张**谓词同义映射表**
  （纯规则，零 LLM 成本）。初版表来自实测谓词分布：

  | 变体 | 规范 |
  |------|------|
  | 隶属于 / 归属于 / 是…的一部分 / part of | 属于 |
  | 对应 / 对应于 / maps to | 映射到 |
  | is deprecated / 已废弃 / 被废弃 | 被替代为* 或 已废弃 |
  | implements / 实现于 | 实现 |
  | calls / invokes | 调用 |
  | executes | 执行 |
  | 包括 / 含有 / includes / contains | 包含 |
  | adds method / adds test / 新增 | 新增 |
  | transitions to | 转换至 |
  | 编写 / 著 / authored | 撰写 |

- **可追溯性**（2605.18490 建议）：归一化发生时把原始谓词写进
  `RelationIndexRecord.metadata`（现有字段，`graph_index.rs:133` 处追加
  `"original_predicate": "..."`)
- LLM 辅助归一化 / 嵌入聚类暂缓——语料规模下规则表够用，作为周期性审计手段
  （谓词频率统计脚本见 §4）备用

### 1.4 验收标准（重灌后）

```bash
# 重灌（显式一次性操作）
RAG_QUALITY_REALISTIC_TRIPLET_ENABLED=1 E2E_MODE=nightly cargo test -p app \
  --test product_e2e realistic_graph_reindex --features product-e2e -- \
  --ignored --nocapture --test-threads=1
```

| 指标 | 基线 | 目标 |
|------|------|------|
| 唯一谓词数 | 177 | **< 60** |
| 谓词长尾占比（<3 次） | 84% | < 50% |
| 跨文档共享实体名 | 0 | **> 10**（重点看两篇 ADR） |
| adr-0004 替代链边 | 缺失 | `(parse_rag_plan_decision, 被替代为, structured tool parsing)` 存在 |
| 0 关系文档数 | 2 | ≤ 1（观察项，不阻塞） |

量化脚本沿用 analysis 文档 §6 的重叠度检查 + 本文 §4 的谓词统计。

---

## 2. 升级二：图增强双路检索（graph 挂钩 dense/lexical）

### 2.1 设计动机（实测 + 论文双重支撑）

实测（analysis 文档 §4.2–4.3 + v3/v4 对比）：
- 显式 `graph_search` 工具的 LLM 路由**随机性大**（同题 v3 调 v4 不调，60%→20%）
- 模型对 graph 先验弱（预训练语料 dense/BM25 模式占绝对多数），会话内又因
  稀疏图返回 1 条而获得负反馈——**靠 prompt 拉不平**
- Q6 轨迹证明：模型**会读** `chunk_type: "graph_relation"` 标注并引用进答案——
  「标注 → 综合判断」通路已通

论文（2507.03608）：Hybrid 双路并行 + 分层拼接在中/难题上事实准确性最优；
但冗余失控会拖垮上下文相关性（0.04）——**增强必须带上限**。

结论：把图检索从「LLM 显式选择的工具」降级为「dense/lexical 的自动增强通道」，
路由随机性归零，负反馈反转为正反馈，负对照保护整套退役。

### 2.2 数据流

```
client.dense_search(query=subquery)          # 或 lexical_search
  → RuntimeBridge::call("dense_search")
      ├─ tools::dispatch(dense_retrieval)     ──┐ tokio::join! 并发
      └─ graph_augment(subquery)              ──┘
           1. embed(subquery)                  # lexical 路径需新 embed；~100ms 被并发掩盖
           2. entity_dense ANN → 种子实体      # 阈值 + 上限（见 2.4）
           3. BFS 2 跳（hop_limit=2）
           4. top-N 关系 → scored_relation_chunk（chunk_type=graph_relation）
  → observation:
     { "chunks": [dense 结果…],               # 向量内容在前（论文拼接顺序）
       "graph_context": [关系条目…] }         # 图内容在后，独立小节
```

### 2.3 实现位置与改动清单

| 文件 | 改动 |
|------|------|
| `crates/rag-core/src/runtime/bridge.rs` | `RuntimeBridge::call`：method 为 `dense_search`/`lexical_search` 且开关开启时，并发执行 `graph_augment`；结果以 `graph_context` 键并入 bridge data（不混入 `chunks` 数组，保持分层） |
| `crates/rag-core/src/runtime/tools/graph.rs` | 抽出可复用的 `graph_augment(runtime, auth, subquery, doc_scope) -> Vec<Value>`：embed → `search_graph`（`query_entity_vectors` 路径，P1 已接线）→ present |
| `crates/code-interpreter/src/bridge.rs` | Python shim 无需改（observation 是透明 JSON）；`graph_context` 随 chunks 返回 |
| `prompts/clusters/codegen/SKILL.md` | 返回值节新增一行说明：`graph_context` 条目来自知识图谱关系遍历（`chunk_type=graph_relation`），用于**补充结构/关系细节**，主体答案优先依据 `chunks`（论文 prompt 引导原则）；其 `chunk_id`（=relation_id）可正常 `[[cite:]]` |
| `crates/app-core/src/config.rs` 或 env | `RETRIEVAL_GRAPH_AUGMENT`（默认 off，A/B 用）、`GRAPH_AUGMENT_MAX_RELATIONS`（默认 5）、`GRAPH_AUGMENT_SEED_LIMIT`（默认 5）、`GRAPH_AUGMENT_HOPS`(默认 2) |

选 bridge 层的理由：codegen 是检索唯一入口（ADR-0009），所有 subquery 都经
`RuntimeBridge::call`；工具实现保持纯粹，增强策略集中在一个 seam。
native tool-call 路径暂不覆盖（可后续按需扩展）。

### 2.4 冗余控制（论文 Hybrid 弱点的针对性设计）

| 手段 | 参数 | 依据 |
|------|------|------|
| 种子相似度阈值 | 实体 ANN cosine ≥ 0.45（初值，A/B 调） | 整句 subquery vs 短实体名的向量对齐噪声大，宁漏勿噪 |
| 种子上限 | top 5 实体 | 控制 BFS 扇出 |
| 结果上限 | top 5 关系条目 | 关系是单行文本，5 条 ≈ 200 token，上下文膨胀可忽略 |
| 去重 | 关系的 `supporting_chunk_ids` 与本次 `chunks` 已含的 chunk_id 重叠时不重复给正文 | 避免同一证据双份 |
| 分层拼接 | `graph_context` 独立键、排在 `chunks` 后 | 论文 III.C；固定分 0.85 不参与 dense 排序，杜绝插位污染 |

### 2.5 与显式 graph_search 工具的关系

**保留**显式工具。分工：
- **增强通道**（本节）：兜底触达，每次 dense/lexical 自动带图上下文，覆盖
  「模型没想起来调」的缺口
- **显式工具**：深度关系题（用户点名两端实体、需要 3 跳、需要定向种子）时模型
  主动调用；SKILL.md 路由表保持，但触发信号可放宽回自然描述（负对照压力消失，
  因为增强通道的成本不再是「浪费一轮」）

---

## 3. 评测口径升级

### 3.1 指标切换

增强通道下 `graph_called` 永真，失去意义。新口径：

| 指标 | 定义 | 替代 |
|------|------|------|
| `graph_cited` | 最终 citations 中 chunk_id ∈ relation_id 集合 | 替代 graph_called 成为主指标（Q6 型真贡献） |
| `graph_context_hit` | graph_context 非空的检索轮占比 | 通道触达率 |
| `answer_correct` | must_include 全中 | 不变 |
| latency delta | 增强开/关的 P50 耗时差 | 新增，验证并发掩盖假设 |
| context size delta | 增强开/关的 observation token 差 | 新增，验证冗余控制 |

`golden_set_graph.json` 的 `graph_must_not_call` 字段与 negative_controls 子集
的图误报统计**退役**（保留题目本身测答案正确性）。

### 3.2 与 RAGAS 四维的映射（不新建基础设施）

| RAGAS（论文口径） | 本系统现有等价物 |
|------|------|
| Faithfulness | hallucination_check（harness 已有） |
| Factual Correctness | must_include / llm_judge |
| Context Relevance | selection_precision（已有）——增强开启后重点盯这个，对应论文 0.04 风险 |
| Answer Relevance | llm_judge 相关性维度 |

### 3.3 A/B 方案

```bash
# v5-off（基线）
RETRIEVAL_GRAPH_AUGMENT=0 E2E_MODE=nightly cargo test -p app --test product_e2e \
  realistic_graph_eval --features product-e2e -- --ignored --nocapture --test-threads=1
# v5-on
RETRIEVAL_GRAPH_AUGMENT=1 ...同上
```

在 §1 重灌完成后跑（图密度先上来，否则增强端上来的还是稀汤）。
对比维度：graph_cited、answer_correct、selection_precision、耗时。
可选加跑 realistic 107 题子集看无回归。

---

## 4. 审计脚本（谓词分布，周期性跑）

```bash
python3 - <<'EOF'
from pymilvus import MilvusClient
from collections import Counter
c = MilvusClient(uri='http://127.0.0.1:19530')
org = "org_id == '00000000-0000-0000-0000-000000000001'"
rows = c.query(collection_name='avrag_e2e_00000000_rag_kg_relations', filter=org,
               output_fields=['predicate','doc_id'], limit=1000)
cnt = Counter(r['predicate'] for r in rows)
tail = [p for p, n in cnt.items() if n < 3]
print(f'relations={len(rows)} unique_predicates={len(cnt)} '
      f'longtail={len(tail)}/{len(cnt)}={len(tail)/max(len(cnt),1):.0%}')
for p, n in cnt.most_common(20):
    print(f'{n:4d}  {p}')
EOF
```

长尾谓词（<3 次）触发合并审查 → 增补 §1.3 同义表。

---

## 5. 实施顺序

| 阶段 | 内容 | 依赖 | 验收 | 状态（2026-07-05） |
|------|------|------|------|-------------------|
| **A** | §1 提示词改造 + 谓词同义表 + metadata 保留 | 无 | 单测（normalize 纯函数）+ cargo test -p worker | ✅ 已完成：抽取提示词卫生规则 + `predicate_normalize.rs` 静态表 + `original_predicate` 元数据；registry / LLM 归一化**搁置**（见 `2026-07-04-predicate-normalization-design.md`） |
| **B** | 重灌 realistic 语料 | A | §1.4 五项指标 | ⏸ **待用户显式操作**（`realistic_graph_reindex`，见 §1.4） |
| **C** | §2 bridge 增强钩子 + 冗余控制 + env 开关 + SKILL.md 说明 | 无（与 A 并行开发） | 单测（bridge stub 验证 graph_context 键、上限、开关）| ✅ 已完成：`graph_augment.rs` + `RuntimeBridge::call` 并发钩子 + `RETRIEVAL_GRAPH_AUGMENT` 等 env + SKILL.md |
| **D** | §3 eval 指标切换（graph_cited 等） | 无 | eval 编译通过 | ✅ 已完成：`realistic_graph_eval` 主报 `graph_cited`；`graph_context_hit` / latency / context-size delta 留 §3.3 A/B 观测 |
| **E** | A/B：v5-off vs v5-on | B + C + D | graph_cited > 0；answer_correct 无回归；selection_precision 降幅 < 20%；P50 耗时增幅 < 15% | ⏸ **待用户显式操作**（B 重灌后跑 §3.3 对比） |

A 与 C 可并行派发；E 是总闸门。

---

## 6. 明确不做（本轮）

| 项 | 理由 |
|----|------|
| Neo4j / pgvector+AGE 迁移 | 已是 Milvus 统一存储（§0）；检索资料的迁移建议针对从零选型场景 |
| 社区检测（Leiden）+ 社区摘要 | 7 文档规模无社区结构；`doc_summary` 已覆盖全局问题入口 |
| 本体对齐（FIBO 等） | 企业级词表治理，当前规模规则表够用 |
| chunk 重构为 1024 token 无重叠 | 论文该配置是控制变量非普适最优；现有 chunking 与 seq/邻居窗口机制耦合，动它属独立课题 |
| 单次 LLM rerank 替代 agent 循环 | 服务端已有 cross-encoder rerank + 关系 rerank；agent 循环是产品形态不是待优化项 |
| LLM 辅助谓词归一化 / 嵌入聚类 | 规则表先行，审计脚本（§4）发现规则覆盖不住再升级 |

---

## 7. 风险与回退

- **种子噪声**（整句→实体名 ANN 空间不对齐）：阈值起步偏严（0.45），A/B 中观察
  graph_context 空置率；过高则降阈值，过噪则升
- **selection_precision 下滑超预期**（论文 Hybrid 的 0.04 教训）：先降
  `GRAPH_AUGMENT_MAX_RELATIONS` 5→3；仍差则 graph_context 仅在 dense 空手/低分时注入
  （条件增强，退化为智能兜底）
- **回退**：`RETRIEVAL_GRAPH_AUGMENT=0` 一键关闭，通路全部走回现状
- **重灌风险**：语料重灌是显式操作（analysis 文档 §7 禁忌不变）；提示词 hash
  自动废缓存，无脏读
