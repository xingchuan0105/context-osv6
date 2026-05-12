# Minsky 文件 Query → Answer E2E 观测报告：RAG planner / 检索通道 / Agent 机制对比

生成时间：2026-04-30 13:59 CST

## 0. 结论先行

- 本次复用已有 `minsky86.pdf` session/source，完成了从 `/api/v1/chat` query 到 answer 的 RAG E2E。
- 文档实际是 Hyman Minsky 的《Stabilizing an Unstable Economy》，不是 Marvin Minsky 的《Society of Mind》。系统最终回答能够识别“检索证据不足/文档不匹配”，这是正确的 evidence boundary。
- planner 输出的是一个单 semantic query 的 `ExecutePlanRequest`，带 `query_entities`，但没有 `bm25_terms`、`graph_hints`、`placeholder_triplets`。这直接解释了：text dense 和 multimodal dense 有召回；BM25 与 graph-only 都为 0。
- text dense 返回的是 Hyman Minsky/economic agents/plans 等弱语义相关 chunk；multimodal dense 返回经济图表/figure chunk；这些 chunk 与用户 query 只有“同名 Minsky / agents / relationship / mechanism”层面的向量相似，不是 Society of Mind 的真实证据。
- 当前代码的“skill 加载 + 自主规划检索策略”更接近 prompt-envelope 内的静态 behavior skill + LLM 结构化计划，而不是 Hermes/Codex/Claude Code 那种完整工具注册表、权限、循环工具调用、子代理/工作区机制。

## 1. E2E 运行信息

- run artifact: `/home/chuan/context-osv6/avrag-rs/.hermes/runs/e2e-minsky-agent-mech-1777528419-current.json`
- run_name: `e2e-minsky-agent-mech-1777528419-current`
- document_id: `db7bc56e-f840-46ef-91b4-8cbb534b5598`
- notebook_id: `b7774d00-df07-4d9b-8f32-c6f2a60b8f21`
- query: 请基于 minsky86.pdf 中 Minsky 的 Society of Mind 论文，用中文解释：Society of Mind、agents、K-lines、frames 之间是什么关系？这种由许多简单 agent 协作形成智能的机制，与现代软件 agent 的规划和工具/检索机制有什么可类比之处？
- chat http_status: `200`
- SSE event counts: `{"start": 1, "activity": 4, "answer_start": 1, "token": 227, "citations": 1, "done": 1}`
- answer_length: `514`

数据库/解析状态：

- document status: `completed`
- chunk_count: `881`
- chunks_by_type: `{"body": 881, "summary": 1}`
- multimodal_chunks: `45`

最终回答摘要：

> 根据检索到的文档内容，我无法回答您的问题。文档 `minsky86.pdf` 实际收录的是经济学家 Hyman Minsky 的著作 **《Stabilizing an Unstable Economy》**，其中讨论的是金融不稳定性、投资融资、价格形成、代理（agents）在经济学中的决策行为等宏观经济理论。文中出现的 "agent" 指的是经济主体（企业、银行等），而与 Marvin Minsky 在 **《Society of Mind》** 中提出的认知科学概念（如智能体（agents）、K-lines、框架（frames）以及简单智能体协作产生智能的机制）完全无关。 检索到的证据不包含任何关于 Society of Mind 理论体系（agents、K-lines、frames）的内容，因此无法解释这些概念之间的关系，也无法进行与现代软件 agent 规划/工具/检索机制的类比。 建议您： - 确认文档名称或来源是否正确，需要查找 Marvin Minsky 所著的 **《The Society of Mind》**（1986）原文。 - 如果您能提供正确的 PDF 文件，我可以重新检索并为您分析。

## 2. Planner 阶段输出 schema 与本次实际 plan

### 2.1 代码期望的 ExecutePlanRequest schema

```json
{
  "plan_version": "rag-execute-v1",
  "doc_scope": [
    "document-id"
  ],
  "items": [
    {
      "priority": 1.0,
      "query": "semantic retrieval query 或 bm25_terms 二选一"
    }
  ],
  "summary_mode": "none | related | all",
  "budget": {
    "total_candidate_budget": "optional",
    "final_chunk_budget": "optional"
  },
  "channel_budget": {
    "text_dense": "optional",
    "bm25": "optional",
    "multimodal_dense": "optional",
    "graph": "optional"
  },
  "query_entities": [
    {
      "text": "named entity",
      "kind": "optional kind"
    }
  ],
  "graph_hints": [
    {
      "subject": "optional",
      "predicate": "optional",
      "object": "optional"
    }
  ],
  "placeholder_triplets": [
    {
      "subject": "known or ?placeholder",
      "predicate": "relationship",
      "object": "known or ?placeholder"
    }
  ],
  "trace": {
    "request_id": "optional",
    "origin": "optional"
  }
}
```

### 2.2 本次 planner 实际输出

```json
{
  "items": [
    {
      "query": "Society of Mind agents K-lines frames relationship mechanism simple agent cooperation forming intelligence",
      "priority": 1.0
    }
  ],
  "doc_scope": [
    "db7bc56e-f840-46ef-91b4-8cbb534b5598"
  ],
  "plan_version": "rag-execute-v1",
  "summary_mode": "related",
  "query_entities": [
    {
      "kind": "concept",
      "text": "Society of Mind"
    },
    {
      "kind": "concept",
      "text": "agents"
    },
    {
      "kind": "concept",
      "text": "K-lines"
    },
    {
      "kind": "concept",
      "text": "frames"
    },
    {
      "kind": "person",
      "text": "Marvin Minsky"
    }
  ]
}
```

解释：

- `items[0].query` 是英文语义检索 query，把用户中文问题归约为：Society of Mind、agents、K-lines、frames、relationship/mechanism/simple agent cooperation。
- `summary_mode=related` 表示 runtime 可以把相关文档摘要纳入候选上下文，但本次 channel probes 中 summary_chunk_count 为 0。
- `query_entities` 捕获了 Society of Mind、agents、K-lines、frames、Marvin Minsky。
- planner 没给 `bm25_terms`，所以 BM25 only probe 中 `bm25_k=0`，没有 BM25 lexical 检索。
- planner 没给 `graph_hints` / `placeholder_triplets`，所以 graph-only probe 没有关系图检索入口。虽然问题语义上是“关系型问题”，但 planner 没把关系显式编码成 graph hints，这是当前可改进点。

## 3. 通道隔离检索结果

### 3.1 Full captured plan

coverage: `{"requested_doc_count": 1, "matched_doc_count": 1, "retrieved_chunk_count": 12, "summary_chunk_count": 0, "channel_coverage": {"text_dense": 6, "bm25": 0, "multimodal_dense": 6, "graph": 0}}`
channel_trace:
- text_dense: raw=14, hydrated=14, selected=14, latency_ms=195
- bm25: raw=0, hydrated=0, selected=0, latency_ms=0
- multimodal_dense: raw=6, hydrated=6, selected=6, latency_ms=118
- graph: raw=0, hydrated=0, selected=0, latency_ms=8
item_trace:
- kind=query, query=Society of Mind agents K-lines frames relationship mechanism simple agent cooperation forming intelligence, bm25_terms=[], recall_budget=40, bm25_k=0, dense_k=40
- kind=summary, query=, bm25_terms=[], recall_budget=0, bm25_k=0, dense_k=0

| # | channel | type/page | score | chunk_id | 内容/含义 | 与 query 的关系 |
|---|---|---|---:|---|---|---|
| 1 | `milvus_multimodal_dense` | figure/p317 | 0.393507 | `34d7813a` | [Image: f78e13b564f1675a4b4d3068178da4920e96d5b3d3efbcdd5ed7a3f2aa6227a5.jpg] | 见通道说明。 |
| 2 | `milvus_multimodal_dense` | figure/p207 | 0.322808 | `8bc2d8c6` | Price-taking firms react to changes in demand by adjusting output along their marginal cost curve. Even though such firms own and operate capital assets and have debts, they do not have the … | 见通道说明。 |
| 3 | `milvus_multimodal_dense` | figure/p194 | 0.308914 | `d41f13ef` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: 13ffd6c21550f7db29… | 见通道说明。 |
| 4 | `milvus_multimodal_dense` | figure/p194 | 0.308742 | `1ff4be10` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: d6690ff5a995bfe10b… | 见通道说明。 |
| 5 | `milvus_multimodal_dense` | figure/p194 | 0.306279 | `478ef542` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: bf959f38f5c8f6e095… | 见通道说明。 |
| 6 | `milvus_multimodal_dense` | figure/p191 | 0.305845 | `c6ad29d9` | will be extended to include more than the skeletal relations. In the more complete case, the government budget position will be seen to affect relative prices, money prices, and the movement… | 见通道说明。 |
| 7 | `milvus_text_dense` | paragraph/p310 | 0.0166667 | `031a4344` | FrankH.Hahn,MoneyandInflation(Cambridge:MITPress,1983):"Theobjectivesofagentsthatdeterminetheiractionsandplansdonotdependonanynominalmagnitudes.Agentscareonlyaboutrealthings,suchasgoods(prop… | 见通道说明。 |
| 8 | `milvus_text_dense` | paragraph/p9 | 0.0163934 | `0e16ac87` | xiiMINSKY'SSTABILIZINGANUNSTABLEECONOMYmorefrequentandsevere.AsMinskyinsisted,thereisaninherentandfun-damentalinstabilityinoursortofeconomythattendstowardaspeculativeboom,followingJ.M.Keynes… | 见通道说明。 |


full plan 采用默认通道预算，最终 12 个 chunk 由 6 个 text_dense + 6 个 multimodal_dense 组成；BM25/graph 为 0。由于 reranker 配置/多模态图片 URL 存在运行时降级，最终排序基本回退到 pre-rerank/vector 原始顺序。

### 3.2 Text dense only：文本向量

coverage: `{"requested_doc_count": 1, "matched_doc_count": 1, "retrieved_chunk_count": 8, "summary_chunk_count": 0, "channel_coverage": {"text_dense": 8, "bm25": 0, "multimodal_dense": 0, "graph": 0}}`
channel_trace:
- text_dense: raw=40, hydrated=40, selected=40, latency_ms=167
- bm25: raw=0, hydrated=0, selected=0, latency_ms=0
- multimodal_dense: raw=0, hydrated=0, selected=0, latency_ms=0
- graph: raw=0, hydrated=0, selected=0, latency_ms=0
item_trace:
- kind=query, query=Society of Mind agents K-lines frames relationship mechanism simple agent cooperation forming intelligence, bm25_terms=[], recall_budget=40, bm25_k=0, dense_k=40
- kind=summary, query=, bm25_terms=[], recall_budget=0, bm25_k=0, dense_k=0

| # | channel | type/page | score | chunk_id | 内容/含义 | 与 query 的关系 |
|---|---|---|---:|---|---|---|
| 1 | `milvus_text_dense` | paragraph/p310 | 0.0166667 | `031a4344` | FrankH.Hahn,MoneyandInflation(Cambridge:MITPress,1983):"Theobjectivesofagentsthatdeterminetheiractionsandplansdonotdependonanynominalmagnitudes.Agentscareonlyaboutrealthings,suchasgoods(prop… | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |
| 2 | `milvus_text_dense` | paragraph/p9 | 0.0163934 | `0e16ac87` | xiiMINSKY'SSTABILIZINGANUNSTABLEECONOMYmorefrequentandsevere.AsMinskyinsisted,thereisaninherentandfun-damentalinstabilityinoursortofeconomythattendstowardaspeculativeboom,followingJ.M.Keynes… | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |
| 3 | `milvus_text_dense` | paragraph/p146 | 0.016129 | `124508ea` | The"rationalexpectations"schoolholdsthatagentsdonotknowthefuture,buttheyformulatetheirexpectationsonthebasisofasatisfactoryknowledge(i.e.,atheory)ofhowtheeconomyfunctions.Ifweaddpropositions… | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |
| 4 | `milvus_text_dense` | paragraph/p141 | 0.015873 | `7418e44c` | Inasmuchasouraimistoindicatehowwecandobetterthanwehave,andasthebestisoftentheenemyofthegood,wecanforgetabouttheoptimum.Eventhoughatendencytowardcoherenceexistsbecauseoftheprocessesthatdeterm… | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |
| 5 | `milvus_text_dense` | paragraph/p345 | 0.015625 | `5e922121` | understandingisthefoundationoflegitimacyforreform.THEIMPORTANCEOFTHEAGENDATobin'sdefinitionoftheroleofthehouseintellectualmaybedescribedascontrollingtheagenda.Princesandpublicalikedependonin… | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |
| 6 | `milvus_text_dense` | paragraph/p15 | 0.0153846 | `c6313cf9` | xviiiMINSKY'SSTABILIZINGANUNSTABLEECONOMYrobustfinancialstructurecouldopenthedoortoasnowballofdefaultsthatwouldoverwhelmsuchconstraints.Thiswouldbecomemorelikelyiftheinstitutionalconstraints… | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |
| 7 | `milvus_text_dense` | paragraph/p194 | 0.0151515 | `c03f6e82` | $$ \pi = I + D f - B T D f + T ^ {\pi} \text {o r} \tag {16} $$ | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |
| 8 | `milvus_text_dense` | paragraph/p194 | 0.0149254 | `301a6d87` | $$ \pi^ {*} = I + D f - B T D f. \tag {16a} $$ | 语义向量把 query 中的 Minsky/agents/plans/relationship 等词映射到经济学文本里的 Hyman Minsky、economic agents、plans/coherence/agenda；是弱相关/同名误召回，不是 Society of Mind 证据。 |



### 3.3 BM25 only：关键词/稀疏检索

coverage: `{"requested_doc_count": 1, "matched_doc_count": 0, "retrieved_chunk_count": 0, "summary_chunk_count": 0, "channel_coverage": {"text_dense": 0, "bm25": 0, "multimodal_dense": 0, "graph": 0}}`
channel_trace:
- text_dense: raw=0, hydrated=0, selected=0, latency_ms=0
- bm25: raw=0, hydrated=0, selected=0, latency_ms=0
- multimodal_dense: raw=0, hydrated=0, selected=0, latency_ms=0
- graph: raw=0, hydrated=0, selected=0, latency_ms=0
item_trace:
- kind=query, query=Society of Mind agents K-lines frames relationship mechanism simple agent cooperation forming intelligence, bm25_terms=[], recall_budget=40, bm25_k=0, dense_k=40
- kind=summary, query=, bm25_terms=[], recall_budget=0, bm25_k=0, dense_k=0

无返回 chunk。


BM25 返回 0 的直接原因不是数据库没有文本，而是 planner item 没有 `bm25_terms`，runtime item_trace 中 `bm25_terms=[]` 且 `bm25_k=0`。如果要让 BM25 参与，需要 planner 对 rare terms 输出如 `Society of Mind`, `K-lines`, `frames`, `Marvin Minsky` 等 bm25_terms，或 runtime 对 semantic query 做可控词项派生。

### 3.4 Multimodal dense only：多模态向量

coverage: `{"requested_doc_count": 1, "matched_doc_count": 1, "retrieved_chunk_count": 8, "summary_chunk_count": 0, "channel_coverage": {"text_dense": 0, "bm25": 0, "multimodal_dense": 8, "graph": 0}}`
channel_trace:
- text_dense: raw=0, hydrated=0, selected=0, latency_ms=0
- bm25: raw=0, hydrated=0, selected=0, latency_ms=0
- multimodal_dense: raw=40, hydrated=40, selected=40, latency_ms=129
- graph: raw=0, hydrated=0, selected=0, latency_ms=0
item_trace:
- kind=query, query=Society of Mind agents K-lines frames relationship mechanism simple agent cooperation forming intelligence, bm25_terms=[], recall_budget=40, bm25_k=0, dense_k=40
- kind=summary, query=, bm25_terms=[], recall_budget=0, bm25_k=0, dense_k=0

| # | channel | type/page | score | chunk_id | 内容/含义 | 与 query 的关系 |
|---|---|---|---:|---|---|---|
| 1 | `milvus_multimodal_dense` | figure/p317 | 0.393507 | `34d7813a` | [Image: f78e13b564f1675a4b4d3068178da4920e96d5b3d3efbcdd5ed7a3f2aa6227a5.jpg] | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |
| 2 | `milvus_multimodal_dense` | figure/p207 | 0.322808 | `8bc2d8c6` | Price-taking firms react to changes in demand by adjusting output along their marginal cost curve. Even though such firms own and operate capital assets and have debts, they do not have the … | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |
| 3 | `milvus_multimodal_dense` | figure/p194 | 0.308914 | `d41f13ef` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: 13ffd6c21550f7db29… | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |
| 4 | `milvus_multimodal_dense` | figure/p194 | 0.308742 | `1ff4be10` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: d6690ff5a995bfe10b… | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |
| 5 | `milvus_multimodal_dense` | figure/p194 | 0.306279 | `478ef542` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: bf959f38f5c8f6e095… | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |
| 6 | `milvus_multimodal_dense` | figure/p191 | 0.305845 | `c6ad29d9` | will be extended to include more than the skeletal relations. In the more complete case, the government budget position will be seen to affect relative prices, money prices, and the movement… | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |
| 7 | `milvus_multimodal_dense` | figure/p194 | 0.305317 | `61cb46e4` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: eb72219185f66dfad0… | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |
| 8 | `milvus_multimodal_dense` | figure/p194 | 0.304047 | `be68912c` | increases domestic profits and raises prices. The favorable balance of trade sought by mercantilist economic policies helps raise profits even as it raises prices. [Image: 08f1ba78f649962bb6… | 多模态向量在 figure/caption 语义空间中命中经济图表、价格/货币关系图；与“关系/mechanism/agents”有宽泛向量相似，但主题仍是经济学，不是 Marvin Minsky。 |



### 3.5 Graph only：Vector Graph RAG

coverage: `{"requested_doc_count": 1, "matched_doc_count": 0, "retrieved_chunk_count": 0, "summary_chunk_count": 0, "channel_coverage": {"text_dense": 0, "bm25": 0, "multimodal_dense": 0, "graph": 0}}`
channel_trace:
- text_dense: raw=0, hydrated=0, selected=0, latency_ms=0
- bm25: raw=0, hydrated=0, selected=0, latency_ms=0
- multimodal_dense: raw=0, hydrated=0, selected=0, latency_ms=0
- graph: raw=0, hydrated=0, selected=0, latency_ms=4
item_trace:
- kind=query, query=Society of Mind agents K-lines frames relationship mechanism simple agent cooperation forming intelligence, bm25_terms=[], recall_budget=40, bm25_k=0, dense_k=40
- kind=summary, query=, bm25_terms=[], recall_budget=0, bm25_k=0, dense_k=0

无返回 chunk。


Graph 返回 0 的直接原因是 plan 中没有 `graph_hints` 或 `placeholder_triplets`。当前 query 明确问“X、Y、Z 之间是什么关系”，理论上适合生成 placeholder triplets，例如 `(Society of Mind, has_component, agents)`、`(K-lines, related_to, frames)`；但 planner 没输出，graph channel 没有起点。

## 4. 为什么这些 chunk 会返回？

### Text dense

- 文本向量检索按 query embedding 与文本 chunk embedding 的语义相似度取近邻，不要求 exact lexical match。
- 因为文件是 Hyman Minsky 经济学著作，正文里大量出现 `Minsky`，也出现 `agents`、`plans`、`coherence`、`agenda` 等与 query token/语义相近的词。
- 命中 chunk 的 score 很低（约 0.015~0.017），说明不是强相关，只是最接近的候选。
- chunk 的实际含义是经济学中的主体/预期/计划/政策议程，而不是 Marvin Minsky 的认知科学 agents/K-lines/frames。

### Multimodal dense

- 多模态向量检索在 figure/caption/周边文本和 query 之间做向量近邻。
- 返回的大多是经济图表、价格/货币/财政关系图、成本曲线等 figure chunk。
- 它们的相似性来自“relationship/mechanism/agents”等宽泛概念以及视觉/图表文本空间的弱匹配；不是用户所问 Society of Mind。
- 本次多模态 reranker 降级，原因是图片 URL/base64 不符合 reranker API 要求，随后普通 reranker 也因 API key 配置无效降级；因此排序更依赖原始向量相似度。

### BM25

- BM25 依赖明确关键词/词项。当前 planner 没输出 bm25_terms，所以 probe 中没有发起 BM25 搜索。
- 即使加入 `K-lines`、`Society of Mind`、`frames`，在这份经济学文档里也大概率无法命中真正相关内容；这反而有助于判定文档不匹配。

### Graph RAG

- Graph channel 依赖 graph_hints 或 placeholder_triplets 把“关系问题”转成实体-关系线索。
- 本次 planner 只输出 query_entities，没有输出可执行关系 hint，因此 graph raw/hydrated/selected 都是 0。
- 这暴露一个 planner gap：对“X/Y/Z 之间关系”这类问题，应更积极输出 graph_hints/placeholder_triplets。

## 5. 与 Hermes / Codex / Claude Code agent 机制的比较

| 维度 | 当前 Context OS RAG/MainAgent | Hermes Agent | Codex CLI | Claude Code |
|---|---|---|---|---|
| skill/规则来源 | Rust 代码内静态 `MainAgentBehaviorSkill`，写入 prompt envelope 的 `<Behavior Skill>` | Markdown skills registry，可按任务显式/自动加载，并可持久化/更新 | AGENTS.md/系统提示 + CLI 内置行为；没有 Hermes 式长期 skill registry | CLAUDE.md、`.claude/skills`、custom agents、settings/hooks |
| agent loop | RAG 分阶段：plan JSON -> runtime 检索 -> answer；工具不是模型动态调用，而是服务端固定编排 | 通用 ReAct/tool loop：模型可多轮选择工具，工具结果回填上下文 | 面向代码仓库的自主读写/命令执行 loop | 面向代码仓库的自主读写/命令执行 loop，支持 subagents/hooks |
| 工具/权限 | 检索通道是后端 runtime 内部能力，planner 只能输出 plan schema | toolsets + tool schema + 平台权限/审批/调度 | sandbox/approval/full-auto/yolo | permission modes、allowed/disallowed tools、hooks |
| 规划输出 | 强约束 `ExecutePlanRequest`，利于审计、复放、通道隔离 probe | 通常是隐式计划 + tool calls；也可通过 skills 要求结构化输出 | 通常自然语言/内部规划 + shell/file 工具动作 | 通常自然语言/内部规划 + tool actions；也支持 JSON schema print mode |
| 证据边界 | RAG answer prompt 明确“只能基于 retrieval bundle”；本次能拒答错误文档 | 取决于任务 skill/system prompt；工具结果和长期记忆都在上下文中，需要 agent 自律区分 | 代码/命令输出是主要证据，适合软件开发 | 代码/命令输出是主要证据，项目 memory/CLAUDE.md 提供规则 |
| 可复现性 | 高：plan、backend_trace、channel_budget 可复放，适合产品级 RAG 观测 | 中：session/tools 可记录，但模型可自由选择下一步 | 中：会话/patch 可追踪，内部策略不一定结构化 | 中高：JSON/stream-json、session id、hooks 带来较好审计 |
| 自主性 | 中：planner 自主决定 query/entities/hints，但执行拓扑固定 | 高：模型自主选择工具、可写文件、可调度任务 | 高：面向代码任务自主探索/修改/测试 | 高：面向代码任务自主探索/修改/测试/派生 subagents |

关键相同点：

- 都把“行为规则/skills/项目上下文”作为 prompt 的一部分，让模型在当前任务中遵守。
- 都依赖模型先做某种形式的 planning，再调用外部能力或服务端能力。
- 都需要明确 evidence boundary，否则模型会把检索结果、记忆、项目规则混在一起。

关键差异：

- 当前 Context OS 的 skill 是产品内的静态行为模板，不是通用 agent 运行时的可扩展 skill registry。
- 当前 RAG planner 的输出是一个可验证、可重放的检索计划；Hermes/Codex/Claude Code 更偏通用工具执行 loop，计划常隐藏在模型决策过程中。
- 当前代码“自主规划检索策略”的自主性主要体现在 query/bm25_terms/entities/graph_hints 的选择；不具备 Hermes/Codex/Claude Code 那种任意工具选择、文件读写、shell 命令、子代理协调能力。
- 因为执行拓扑固定，Context OS 更适合产品 RAG 的稳定性、审计与可解释；通用 coding agents 更适合开放式开发任务。

## 6. 发现的 gap / 后续建议

1. Planner 对关系型问题没有输出 graph hints：本 query 明确询问 Society of Mind / agents / K-lines / frames 的关系，但 plan 只有 query_entities。建议加单测：关系型 query 必须产生 graph_hints 或 placeholder_triplets，除非 planner 判断文档范围明显无关。
2. BM25 没有 terms：对 rare terms (`K-lines`, `Society of Mind`, `frames`, `Marvin Minsky`) 应该输出 BM25 item，用于快速判定是否 exact miss。
3. 多模态 reranker 图片输入合同不匹配：当前图片 URL/base64 不满足外部 API 要求，导致 mm_reranker 降级。需要明确 reranker 期望：公网 HTTP(S)、签名 URL、还是 base64 data URL。
4. 普通 reranker API key 无效：导致排序回退到 pre-rerank；这会放大弱向量误召回。需要修 key sync 或在报告中显式标注降级。
5. 当前拒答结果是正确的，但用户体验上可以更早提示“minsky86.pdf 是 Hyman Minsky 经济学文档，不是 Marvin Minsky Society of Mind”，这可以由 BM25 exact miss + doc metadata/summary 判断辅助完成。

## 7. 验证命令

```bash
cd /home/chuan/context-osv6/avrag-rs
RUN_NAME=e2e-minsky-agent-mech-$(date +%s)-current \
E2E_EMAIL=e2e-minsky-agent-mech-1777390809@e2e.test \
E2E_PASSWORD=[REDACTED] \
E2E_NOTEBOOK_ID=b7774d00-df07-4d9b-8f32-c6f2a60b8f21 \
E2E_DOCUMENT_ID=db7bc56e-f840-46ef-91b4-8cbb534b5598 \
E2E_ORG_ID=7f743c86-738b-45fe-b426-d1477d3233a4 \
python3 .hermes/scripts/continue_minsky_agent_mechanisms.py
```

