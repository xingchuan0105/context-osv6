# 当前产品架构：Main Agent + RAG API + Milvus 检索数据面

> 状态：2026-04-26 当前目标架构。
> 本文档覆盖 `PRD_RUST.md`、ADR 0002 以及 2026-03 旧实施计划中以 Qdrant/Tantivy 为最终目标的检索描述。

## 1. 核心结论

产品采用 `Main Agent + RAG API` 架构：

```text
User
  -> Main Agent
      -> 可选 clarify / chat
      -> 生成 RAG tool plan schema
          -> RAG API
              -> BM25 retrieval
              -> text dense retrieval
              -> multimodal dense retrieval
              -> graph relation retrieval
              -> fusion / rerank / evidence packaging
      -> 生成最终有证据约束的回答
```

`Main Agent` 是唯一面向用户的 agent，负责用户交互、mode routing、workspace 指代消解、记忆使用、工具规划和最终回答。

`RAG API` 不是自主 agent，而是检索服务。它可以调用 LLM 或 reranker 完成有边界的检索子任务，但不负责对话策略、澄清策略、长程规划或最终回答风格。

## 2. 存储分工

稳定架构保留产品控制数据与检索数据的分层。

```text
Postgres: 产品控制面
- users
- organizations
- workspaces / notebooks
- auth and sessions
- chat history
- agent memory metadata
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

## 3. RAG API 边界

### 3.1 输入

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

### 3.2 输出

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

## 4. 模型辅助检索算子

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

这条边界保留了产品结构：一个 `Main Agent`，多个有界检索算子。

## 5. 三元组抽取

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

## 6. Vector Graph RAG 路线

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

## 7. 检索通道

每次 query 至少包含以下检索通道：

```text
BM25 keyword retrieval
text dense retrieval
multimodal dense retrieval
graph relation retrieval
```

### 7.1 BM25

BM25 迁移到 Milvus，作为统一检索数据面的一部分。

Planner 生成短关键词，第一版采用 OR 风格关键词召回：

```text
bm25_keywords = ["明斯基", "心智社会", "框架"]
```

默认不启用中文滑动 n-gram 检索。滑动检索能提升模糊召回，但也可能命中无关片段、降低精度。若短词 OR 召回不足，再增加单独的 char bigram/trigram 字段并做对比后决定是否启用。

### 7.2 Text Dense

文本 chunk 使用 text embedding collection，负责普通文本语义召回。

### 7.3 Multimodal Dense

多模态 chunk 使用独立 multimodal collection。一次 query 同时搜索 text 与 multimodal collection，再进入统一 rerank 和证据预算。

多模态候选必须保留：

- asset id
- page
- caption
- context text
- source locator
- original document id

### 7.4 Graph Relation Retrieval

图关系检索返回 relation paths 与 supporting chunks。它不是普通 chunk 相似度检索，而是服务于多跳问题的桥接证据召回。

## 8. 融合与预算

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

原因：图检索常找到的是“推理链条中的必要桥”，不一定是词面或语义上最相似的 chunk。完全混排可能把关键桥接证据排掉。

## 9. Main Agent 记忆层

Main Agent 记忆既包含产品数据，也包含检索数据，取决于具体层级。

```text
短期工作记忆:
- current session references
- current topic
- recent explicit document/entity mentions
- runtime or Postgres-backed session state

长期用户偏好:
- language preference
- answer length
- formatting style
- technical depth
- Postgres as source of truth

Workspace 记忆:
- document metadata
- entity aliases
- project/workspace state
- Postgres for structured truth
- Milvus for semantic lookup copies

语义记忆:
- embeddable memory snippets
- Milvus vectors

操作/审计记忆:
- user actions
- ingestion events
- tool calls
- Postgres
```

原则：需要权限、删除、审计、事务一致性的记忆放 Postgres；需要语义召回的记忆可以同步到 Milvus。

## 10. 评测范围

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

## 11. 产品级保护约束

目标架构必须保留以下约束：

- 每次 Milvus 查询必须强制带服务端 ACL filter，例如 `org_id`、`workspace_id`、`doc_scope`。
- 每个检索结果必须带 provenance：`doc_id`、`chunk_id`、`page`、`parse_run_id` 和可用的 `source_locator`。
- 文档重解析必须删除或 supersede 旧 chunk、embedding、entity、relation 与 passage。
- 图抽取失败时必须降级到 BM25 + dense + multimodal retrieval。
- BM25 失败时必须降级到 dense + multimodal + graph retrieval。
- 图扩展必须有 fan-out limit、hop limit 与 relation count eviction。
- RAG API trace 必须显示最终上下文由哪些通道贡献。

## 12. 后续待定项

仍需单独确认：

- Milvus schema 与字段名。
- Milvus 中文 analyzer 配置。
- graph relation rerank 是只复用现有 reranker，还是增加单独 LLM relation selector。
- 是否在 Postgres 中保留一份规范化 graph adjacency，用于更强的产品治理。
- Qdrant/Tantivy 相关代码迁移到 Milvus 的时间表。
