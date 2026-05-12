> **⚠️ 部分过时**：本文档 §11 WebSearchAgent、§12 Prompt 管理体系、§2 semantic memory vectors 等内容已被 `2026-05-12-architecture-baseline.md` 取代。请以基准版为准。

# 当前产品架构：UnifiedAgentService + 三独立 Agent + RAG API + Milvus 检索数据面

> 状态：2026-05-11 更新。三 Agent 拆分已落地，Chat、Search、RAG 生产路径均已走 UnifiedAgentService。GraphFlow 已退场。
> 本文档覆盖 `PRD_RUST.md`、ADR 0002 以及 2026-03 旧实施计划中以 Qdrant/Tantivy 为最终目标的检索描述。

## 1. 核心结论

产品采用 **UnifiedAgentService + 三独立 Agent** 架构：

```text
User
  -> UnifiedAgentService (dispatcher)
      -> ChatAgent         → 直接对话 / 创意写作 / 头脑风暴
      -> WebSearchAgent    → 外部搜索 (Brave LLM Context / Perplexity)
      -> RagAgent          → 检索增强生成 (已生产化，走 UnifiedAgentService + tool-call 范式)

RAG API (检索服务，非自主 agent)
  -> BM25 retrieval
  -> text dense retrieval
  -> multimodal dense retrieval
  -> graph relation retrieval
  -> fusion / rerank / evidence packaging
```

`UnifiedAgentService` 是面向用户的唯一调度层，根据 `AgentRequest.kind` 路由到三个独立 Agent：

| Agent | 职责 | 生产状态 |
|-------|------|----------|
| `ChatAgent` | 直接对话、创意写作、头脑风暴、解释说明 | 已生产化，走 UnifiedAgentService |
| `WebSearchAgent` | 本地 planner → 多子查询并行 → 结果聚合 → 答案合成 | 已生产化，走 UnifiedAgentService |
| `RagAgent` | 检索计划生成 → RAG API 调用 → 答案合成 | 已生产化，走 UnifiedAgentService；tool-call 范式已落地 |

每个 Agent 独立实现 `Agent` trait，通过统一的 `AgentEvent` 事件流与调用方通信。这种拆分避免了单体式 Main Agent 的职责膨胀，使各模式有独立的 skill、prompt 和降级策略。

`RAG API` 不是自主 agent，而是检索服务。它可以调用 LLM 或 reranker 完成有边界的检索子任务，但不负责对话策略、澄清策略、长程规划或最终回答风格。

---

## 2. 存储分工

稳定架构保留产品控制数据与检索数据的分层。

```text
Postgres: 产品控制面
- users
- organizations
- workspaces / notebooks
- auth and sessions
- chat history
- agent memory metadata (session summaries, user profiles)
- ingestion jobs
- audit / usage / billing
- document lifecycle state

Milvus: 检索数据面
- text chunks
- multimodal chunks
- BM25 sparse vectors
- dense text vectors
- multimodal vectors
- kg_entities
- kg_relations
- graph passages / chunk evidence
- semantic memory vectors
```

即使不考虑当前项目已经使用 Postgres 的事实，从功能和场景出发，Postgres 仍然是产品控制面的最佳默认选择：事务、关系约束、schema 演进、JSONB、运维生态和行级安全都更适合用户、权限、会话、任务、审计、计费等产品数据。Milvus 应作为统一检索数据库，而不是唯一产品数据库。

---

## 3. Agent 事件契约

所有 Agent 通过统一的 `AgentEvent` 事件流与调用方通信：

```text
AgentEvent::Activity         → 进度通知 (planning / searching / reading_sources)
AgentEvent::ReasoningSummaryDelta → 推理摘要增量
AgentEvent::MessageDelta     → 答案文本增量
AgentEvent::Citations        → 引用来源
AgentEvent::Usage            → Token 用量 (provider, model, tokens)
AgentEvent::DebugTrace       → 调试信息 (debug flag 控制)
AgentEvent::Done             → 最终完成
AgentEvent::Error            → 终端错误
```

Streaming 路径通过 `ChannelSink` 实时转发到 SSE；非 streaming 路径通过 `CollectingSink` 收集后组装响应。所有 Agent 共享同一事件契约，前端无需感知底层 Agent 差异。

---

## 4. RAG API 边界

### 4.1 输入

`RAG API` 接收结构化检索计划与服务端执行上下文。

建议输入字段：

- `plan_version`
- `doc_scope`
- `items`
- `bm25_keywords`
- `query`
- `query_entities`
- `graph_hints`
- `summary_mode`
- `budget`
- `acl_context`
- `trace_context`

它不依赖原始 session history、用户偏好记忆或上一轮 assistant 失败结论。

### 4.2 输出

`RAG API` 返回 retrieval bundle：

- candidate chunks
- citations
- relation paths
- graph-supported chunks
- summary chunks
- score breakdown
- coverage
- degrade trace
- backend trace

它不返回最终用户回答，也不返回是否澄清的对话级决策。

---

## 5. 模型辅助检索算子

RAG API 可以使用模型调用，但这些调用必须是确定边界的检索算子。

允许：

- 文档入库时的三元组抽取
- query entity extraction
- relation/path candidate filtering
- chunk reranking
- evidence compression

不允许：

- 用户级 clarify 策略
- 自主多步 agent 规划
- 最终回答口吻和会话策略
- 持久化偏好解释

这条边界保留了产品结构：一个 `UnifiedAgentService` 调度层，多个有界检索算子。

---

## 6. 三元组抽取

三元组抽取复用前期 benchmark 脚本策略：

1. 按 token 预算切分文档 batch。
2. 并发处理 batch。
3. 使用 benchmark 中验证过的 Gemini 3.1 Flash 系列 provider/model。
4. 提示词保持不变。
5. provider 支持时关闭 thinking mode。
6. 严格解析 JSON，拒绝 malformed triplets。

JSON 契约与 vector-graph-rag 兼容：

```json
{
  "triplets": [
    ["subject", "predicate", "object"]
  ]
}
```

导入 graph index 时，每个 chunk 转成：

```json
{
  "id": "chunk_id",
  "passage": "chunk text",
  "triplets": [
    ["subject", "predicate", "object"]
  ]
}
```

---

## 7. Vector Graph RAG 路线

Rust 后端应移植核心行为，不把 Python 服务作为运行时依赖。

已检查的上游实现核心为三个逻辑 collection：

- `entities`
- `relations`
- `passages`

图邻接通过 ID 引用表达：

- entity -> relation_ids / passage_ids
- relation -> entity_ids / passage_ids / subject / predicate / object
- passage -> entity_ids / relation_ids

Rust 目标流程：

```text
Ingestion:
chunk -> triplet extraction -> entity normalization -> relation construction
      -> entity/relation/passage embeddings -> Milvus upsert

Query:
query -> query entity extraction
      -> entity vector search + relation vector search
      -> subgraph expansion
      -> fan-out control / eviction
      -> relation/path rerank
      -> supporting chunk hydration
```

原 Python 项目适合作为参考实现，但不能照搬为产品实现。例如它的 `add_documents` 路径会 drop/recreate collections；产品实现需要增量 upsert、delete、ACL filter 和 rebuild 语义。

---

## 8. 检索通道

每次 query 至少包含以下检索通道：

```text
BM25 keyword retrieval
text dense retrieval
multimodal dense retrieval
graph relation retrieval
```

### 8.1 BM25

BM25 迁移到 Milvus，作为统一检索数据面的一部分。

Planner 生成短关键词，第一版采用 OR 风格关键词召回：

```text
bm25_keywords = ["明斯基", "心智社会", "框架"]
```

默认不启用中文滑动 n-gram 检索。滑动检索能提升模糊召回，但也可能命中无关片段、降低精度。若短词 OR 召回不足，再增加单独的 char bigram/trigram 字段并做对比后决定是否启用。

### 8.2 Text Dense

文本 chunk 使用 text embedding collection，负责普通文本语义召回。

### 8.3 Multimodal Dense

多模态 chunk 使用独立 multimodal collection。一次 query 同时搜索 text 与 multimodal collection，再进入统一 rerank 和证据预算。

多模态候选必须保留：

- asset id
- page
- caption
- context text
- source locator
- original document id

### 8.4 Graph Relation Retrieval

图关系检索返回 relation paths 与 supporting chunks。它不是普通 chunk 相似度检索，而是服务于多跳问题的桥接证据召回。

---

## 9. 融合与预算

图关系结果应参与最终排序，但第一版不建议完全无保护混排。

推荐预算：

```text
text dense: 30-35%
BM25: 20-25%
multimodal: 10-15%
graph-supported chunks: 20-30%
```

推荐流程：

```text
1. 并行执行 BM25、text dense、multimodal dense、graph retrieval。
2. 构建带 source label 和 score breakdown 的候选池。
3. 使用 RRF 或 channel-aware normalization 做第一层融合。
4. 对可比较的 chunk candidates 复用现有 reranker。
5. 保留 graph-supported chunks 的最低预算。
6. 在 token budget 内裁剪最终上下文。
```

原因：图检索常找到的是"推理链条中的必要桥"，不一定是词面或语义上最相似的 chunk。完全混排可能把关键桥接证据排掉。

---

## 10. 记忆层架构（三层模型）

Agent 记忆层采用三层架构，已完全取代早期的工作记忆设计：

```text
Layer 1 (短期): chat_messages — 对话原文
  - 用途：指代消解、对话连续性
  - 触发：每轮对话自动加载最近 N 条
  - 衰减：无需衰减，原始记录

Layer 2 (中期): chat_sessions.summary — 结构化 JSON 摘要
  - 用途：跨轮次上下文压缩、session 主题提炼
  - 触发：每 10 轮对话触发一次 LLM 摘要
  - 格式：结构化 JSON（主题、关键信息、未完成事项等）
  - 消费：注入 agent system prompt 作为 continuity context

Layer 3 (长期): user_profiles.structured_profile — 用户结构化画像
  - 用途：跨 session 用户偏好、专业领域、常用表达方式
  - 触发：每日"做梦"（24h 节流）
  - 机制：LLM 输出 delta 建议（add/reinforce/revise/weaken/remove）
  - 合并：运行时确定性合并（含置信度衰减、冲突消解、版本隔离）
  - 固定槽位：expertise_domains / preferred_answer_style / frequently_asked_topics
```

### 10.1 记忆注入规则

所有 LLM 调用点统一遵循以下注入规则：

- **Session summary**：提供对话连续性；不作为事实证据
- **User preferences**：只影响表达风格，不覆盖事实或推理
- **对话原文**：用于指代消解，但不参与事实判断
- **RAG Evidence**：唯一的事实权威来源

### 10.2 淘汰与冲突消解

- Layer 3 冲突消解按时间顺序进行，非固定周期淘汰
- 长期不用用户偏好不会被清空（非固定周期衰减）
- 新增 `session_continuity_hints` 表：跨 session 短期衔接桥梁（FIFO 上限 3 条，7 天过期）

---

## 11. WebSearch Agent 设计

`WebSearchAgent` 实现完整的本地 agent pipeline：

```text
planner (intent recognition + coreference resolution + sub-query generation)
  → multi-query execution → result aggregation → answer synthesis
```

### 11.1 Search Plan

Planner 输出 `SearchPlan` 结构：

- `sub_queries`: 1-3 个子查询，覆盖用户意图
- `intent_summary`: 用户意图摘要
- `needs_clarification`: 是否需要澄清
- `preferred_vertical`: 搜索垂直领域 (`web` | `news`)

### 11.2 Brave LLM Context 路径

- 本地 planner 生成子查询和垂直偏好
- 并行执行多个子查询（支持 vertical 路由到 `/res/v1/news/search`）
- URL 去重 + citation 重新编号
- LLM 合成最终答案（流式或非流式）

### 11.3 Perplexity 路径

- 委托给 provider 的 built-in agentic flow
- 流式透传 provider 的事件

### 11.4 Runtime 参数注入

Brave 搜索支持运行时参数：

- `SEARCH_LANG` → `search_lang`
- `SEARCH_COUNTRY` → `country`
- `SEARCH_FRESHNESS` → `freshness`

---

## 12. Prompt 管理体系

Prompt 从硬编码字符串迁移到 `prompts/` 目录下的外置文件，使用 `include_str!()` 加载：

```text
prompts/
  chat_agent_system.txt           → ChatAgent system prompt
  search_plan_system.txt          → WebSearchAgent planner prompt
  web_search_system.txt           → WebSearchAgent answer synthesis prompt
  rag_plan_system.txt             → RAG planning prompt (GraphFlow)
  rag_answer_system.txt           → RAG answer prompt (GraphFlow)
  session_summary_system.txt      → Layer 2 摘要生成 prompt
  user_profile_extraction_system.txt → Layer 3 "做梦" delta prompt
  triplet_extraction_system.txt   → 三元组抽取 prompt
  *.tmpl                          → 用户消息模板
```

共享加载器：`load_prompt_template()` 支持按目录/版本/名称加载。

---

## 13. i18n 支持

Agent 输出支持中英双语：

- `ChatRequest.language` 字段贯穿调用链
- `activity::planning_title()` / `fallback::no_valid_retrieval_results()` 等 i18n 函数
- Agent name 和 icon 根据语言动态选择

---

## 14. 评测范围

完整 retrieval evaluation 平台暂不作为第一阶段目标。

第一阶段只保留轻量 sanity set：

- 约 20 条人工问题
- 覆盖精确关键词问题
- 覆盖语义问题
- 覆盖多模态问题
- 覆盖少量多跳图关系问题

用途：

- 设计期：确认检索设计是否基本可用。
- 运维期：在 analyzer、prompt、embedding、reranker 改动后发现明显退化。

完整 Recall@k / MRR / answer-faithfulness 评测等检索链路稳定后再补。

---

## 15. 产品级保护约束

目标架构必须保留以下约束：

- 每次 Milvus 查询必须强制带服务端 ACL filter，例如 `org_id`、`workspace_id`、`doc_scope`。
- 每个检索结果必须带 provenance：`doc_id`、`chunk_id`、`page`、`parse_run_id` 和可用的 `source_locator`。
- 文档重解析必须删除或 supersede 旧 chunk、embedding、entity、relation 与 passage。
- 图抽取失败时必须降级到 BM25 + dense + multimodal retrieval。
- BM25 失败时必须降级到 dense + multimodal + graph retrieval。
- 图扩展必须有 fan-out limit、hop limit 与 relation count eviction。
- RAG API trace 必须显示最终上下文由哪些通道贡献。

---

## 16. 后续待定项

仍需单独确认：

- **graph relation rerank**：是只复用现有 reranker，还是增加单独 LLM relation selector（未实现）
- **是否在 Postgres 中保留一份规范化 graph adjacency**，用于更强的产品治理
- **Qdrant/Tantivy 相关代码迁移到 Milvus 的时间表**（已完成：workspace 中已无 Qdrant/Tantivy 代码）
- ~~RagAgent 接线~~ ✅ 已完成：GraphFlow 已退场，RagAgent 已接入 UnifiedAgentService
- ~~GraphFlow 退场~~ ✅ 已完成
