# Main Agent + RAG API + Milvus 检索数据面实施计划

> 状态：Phase 0-6 code/docs implemented; live Milvus smoke pending
> 来源：`2026-04-26-current-product-rag-architecture.md` 架构文档、当前代码 review、gitnexus 影响分析。
> 目标：把当前 `Qdrant + Tantivy/Postgres BM25 + partial multimodal` 实现，迁移到 `Postgres 控制面 + Milvus 检索数据面 + graph relation retrieval` 的目标架构。

## 1. Success Criteria

- `/api/v1/chat` 仍是用户入口，由 Main Agent 负责 mode routing、clarify、plan 和最终回答。
- `/api/v1/rag/execute-plan` 仍是 RAG API 深模块接口，只接收检索计划并返回 retrieval bundle，不接管会话策略。
- RAG API v2 能表达 text dense、BM25、multimodal、graph relation retrieval 的输入、输出、trace 和 provenance。
- Milvus 成为默认检索数据面；Qdrant/Tantivy/Postgres BM25 作为 legacy adapter 暂留，直到 sanity set 通过后退场。
- 文档重解析能删除或 supersede 同一文档旧的 text、multimodal、entity、relation、passage 索引数据。
- 图检索失败时能降级到 dense + BM25 + multimodal；BM25 失败时能降级到 dense + multimodal + graph。
- 20 条轻量 sanity set 覆盖关键词、语义、多模态、图关系问题，并作为迁移默认 gate。

## 2. Current State and gitnexus Impact

gitnexus 当前索引状态：`context-osv6` 已在 commit `c08501f` 索引完成，`11746 symbols / 25147 edges / 300 processes`，状态 up-to-date。

### 2.1 高影响链路

- `RagRuntime::execute_plan` 是高风险变更点。gitnexus impact 显示会影响 `rag_execute_plan_handler`、`execute_chat_stream`、`graphflow_tasks_rag::run` 和 app 层 execute wrapper。
- `ExecutePlanResponse` 是高风险 contract 变更点。影响 Main Agent bundle consumption、RAG runtime tests、HTTP handler、chat streaming 和 graphflow RAG task。
- `run_document_pipeline` 上游影响较小，只直接影响 worker `process`，但它是 parse run、chunk、embedding、Qdrant/Tantivy 写入的唯一关键路径。

### 2.2 代码 gap

- `ExecutePlanRequest` 只有 `doc_scope/items/summary_mode/budget/trace`，缺 `query_entities`、`graph_hints`、channel budget、服务端 trace context。
- `RetrievalBundle` 只有 chunks、citations、summary chunks，缺 relation paths、graph-supported chunks、score breakdown、channel coverage、parse_run_id。
- `RagRuntime::execute_plan` 顺序执行 dense -> BM25 -> multimodal -> rerank，没有 graph retrieval，也没有真正的并行 channel runner。
- `graphflow_tasks_rag.rs` 已有细粒度 RAG task 名称和实现，但 `build_chat_graph` 只接入 monolith `RagExecutePlanTask`。
- Worker ingestion 写 Postgres chunks/assets、Tantivy lexical、Qdrant dense/multimodal；没有 triplet extraction、entity/relation schema、Milvus upsert。
- `parse_run_id` 已写入 Qdrant payload 和 Postgres rows，但没有出现在 `RetrievedChunk`、`Citation` 或最终 RAG evidence bundle。
- `frontend_next` 消费 `/api/v1/chat` SSE 和 citations；目前不直接消费 `/rag/execute-plan`，但 citations 字段扩展会影响 TypeScript 类型。

## 3. Target Module Shape

### 3.1 Retrieval Data Plane seam

新增一个真实 seam，因为未来至少有两个 adapter：

- `LegacyRetrievalDataPlane`: 包装现有 Qdrant dense、Tantivy/Postgres BM25、Qdrant multimodal。
- `MilvusRetrievalDataPlane`: 新目标实现，负责 Milvus text、multimodal、BM25 sparse、entity、relation、passage collections。

建议落点：

- 新 crate：`crates/retrieval-data-plane`
- 新 adapter crate：`crates/storage-milvus`
- `crates/rag-core` 依赖 `retrieval-data-plane`，不再直接依赖 `storage-qdrant` 和 `search`。
- worker 依赖 `retrieval-data-plane` + concrete adapter，通过 `replace_document_index` 写检索数据面。

接口只保留实现所需能力：

```rust
#[async_trait]
pub trait RetrievalDataPlane: Send + Sync {
    async fn ensure_schema(&self) -> Result<()>;
    async fn replace_document_index(&self, batch: DocumentIndexBatch) -> Result<IndexWriteReport>;
    async fn delete_document_index(&self, owner_user_id: UserId, document_id: Uuid) -> Result<()>;
    async fn search_text_dense(&self, request: TextDenseSearchRequest) -> Result<Vec<ChannelCandidate>>;
    async fn search_bm25(&self, request: Bm25SearchRequest) -> Result<Vec<ChannelCandidate>>;
    async fn search_multimodal(&self, request: MultimodalSearchRequest) -> Result<Vec<ChannelCandidate>>;
    async fn search_graph(&self, request: GraphSearchRequest) -> Result<GraphSearchOutput>;
}
```

规则：

- Adapter 内部处理 collection 名、Milvus filter expression、Qdrant filter、Tantivy fallback。
- Caller 必须提供 `AuthContext` 或等价 server-side ACL context；客户端请求不得传入 `acl_context`。
- 所有 search output 必须带 `owner_user_id/doc_id/chunk_id/parse_run_id/retrieval_channel/score_breakdown/source_locator`。

### 3.2 RAG API v2 contract

保持 `/api/v1/rag/execute-plan` route 不变，扩展 DTO 为 v2 兼容形态。

Request 增量字段：

- `query_entities: Vec<QueryEntity>`
- `graph_hints: Vec<GraphHint>`
- `channel_budget: Option<ChannelBudget>`
- `trace: ExecutePlanTrace` 扩展 `trace_id`、`origin`、`request_id`

保持：

- `doc_scope`
- `items`
- `summary_mode`
- `budget`
- `ExecutePlanItem` 的 `query` / `bm25_terms` one-of 规则

Response 增量字段：

- `RetrievalBundle.relation_paths`
- `RetrievalBundle.graph_supported_chunks`
- `RetrievedChunk.parse_run_id`
- `RetrievedChunk.score_breakdown`
- `Coverage.channel_coverage`
- `BackendTrace.channel_trace`
- `Citation.parse_run_id`

兼容策略：

- 第一阶段仍默认 `plan_version = "rag-execute-v1"`，但允许 v2 optional fields。
- Main Agent prompt 更新后再把默认常量切到 `rag-execute-v2`。
- TypeScript `frontend_next/lib/workspace/stream.ts` 只补 citations 类型，不改变 SSE event shape。

## 4. Phased Implementation

### Phase 0: Baseline and Contract Guard

目标：先锁当前行为，防止迁移时把 Main Agent/RAG API 边界改坏。

实施：

- 在 `common/tests/rag_execute_contract.rs` 增加 v2 optional fields 的 serde roundtrip 和 legacy unknown-field guard。
- 在 `transport-http/tests/rag_execute_plan_contract.rs` 增加 response provenance 断言，占位期可断言字段为 `None`。
- 在 `app/src/main_agent/mod.rs` tests 增加 bundle 消费顺序：retrieval chunks、graph-supported chunks、summary chunks 的优先级。
- 把 `tests/rag_quality/golden_set.sample.json` 扩成 20 条分类样例：keyword、semantic、multimodal、graph。

影响：

- 主要影响 `common`、`transport-http`、`app` tests。
- 不改 runtime 行为。

验收：

- `cd avrag-rs && cargo test -p common rag_execute_contract`
- `cd avrag-rs && cargo test -p transport-http rag_execute_plan_contract`
- `cd avrag-rs && cargo test -p app main_agent`

### Phase 1: Retrieval Data Plane Interface and Legacy Adapter

目标：先把现有检索栈包进真实 adapter，让 `rag-core` 面向稳定检索接口。

实施：

- 新增 `crates/retrieval-data-plane`，定义 DTO 和 `RetrievalDataPlane` trait。
- 新增 `LegacyRetrievalDataPlane`，内部复用现有 Qdrant、Tantivy/Postgres BM25、多模态 Qdrant 逻辑。
- `RagRuntime` 改为持有 `Arc<dyn RetrievalDataPlane>`，`execute_plan` 通过 trait 调用 channel search。
- 先保持顺序执行，输出应与旧实现一致。
- `AppState::bootstrap` 和 worker 初始化根据配置选择 legacy adapter。

影响：

- 高风险：`rag-core/src/runtime/execute.rs`、`rag-core/src/runtime/retrieval.rs`、`app/src/lib_impl/rag_execute.rs`。
- 中风险：`app/src/lib_impl/chat_streaming.rs`、`app/src/chat/graphflow_tasks_rag.rs`，因为它们消费 `execute_plan`。
- 低风险：`storage-qdrant` 和 `search` 保持现有实现，只从 adapter 调用。

验收：

- Phase 0 测试全部通过。
- `cd avrag-rs && cargo test -p avrag-rag-core`
- `cd avrag-rs && cargo test -p app chat_service_contract`

### Phase 2: Milvus Adapter and Schema

目标：实现 Milvus text dense、BM25 sparse、multimodal dense 的读写能力，但不默认切流量。

实施：

- 新增 `crates/storage-milvus`，使用 Milvus v2 REST API。
- Collection 设计：
  - `rag_text_chunks`: text、text_dense、text_sparse(BM25 function output)、metadata fields。
  - `rag_multimodal_chunks`: multimodal_dense、asset/page/caption/context/source locator。
  - `rag_kg_entities`: entity text、entity_dense、doc/workspace/org metadata。
  - `rag_kg_relations`: subject、predicate、object、relation_dense、passage ids。
  - `rag_graph_passages`: chunk evidence and adjacency metadata。
- 所有 collection 必须有 scalar fields：`owner_user_id`、`workspace_id` nullable、`doc_id`、`chunk_id` nullable、`parse_run_id`、`doc_version`。
- `search_*` 必须生成服务端 ACL filter：至少 `owner_user_id == ...` 且 `doc_id in [...]`。
- BM25 使用 Milvus built-in BM25 function；查询时传原始 query text，不传预计算 sparse vector。
- dense + BM25 + multimodal 可先分别 search 后在 Rust 中 channel-aware merge；Milvus hybrid search 作为后续优化，不作为首个默认路径。

影响：

- 新增 crate 和配置，主要影响 workspace Cargo、`AppConfig`、bootstrap、worker adapter factory。
- 不删除 Qdrant/Tantivy。
- 需要新增 `.env.example` Milvus 配置项，但不能移除 Qdrant 配置。

验收：

- `cd avrag-rs && cargo test -p avrag-storage-milvus`
- Milvus 未配置时，现有 legacy tests 不受影响。
- Milvus 配置存在时，adapter integration test 能 ensure schema、insert/upsert、filter search、delete by doc。

### Phase 3: Ingestion Triplet Extraction and Index Replacement

目标：让 worker 在一次 parse run 内完成文本、多模态、KG 数据写入，并支持重解析 supersede。

实施：

- 在 `run_document_pipeline` 的 chunk plan 生成后，构建 `DocumentIndexBatch`。
- 文本 chunk 写入 batch：content、dense vector、BM25 source text、metadata、parse_run_id。
- 多模态 chunk 写入 batch：asset_id、caption、context_text、multimodal vector、source_locator、parse_run_id。
- 新增 bounded triplet extraction operator：
  - 输入来自 chunk text batch。
  - 按 token budget 切 batch。
  - 并发上限固定为 4。
  - 输出严格解析 `{"triplets":[["subject","predicate","object"]]}`。
  - malformed JSON 或 provider error 只记录 degrade，不阻断 text/multimodal indexing。
- 对 triplet 做最小规范化：trim、空值过滤、同 chunk 内去重；不做复杂 entity linking。
- 调用 `replace_document_index`，由 adapter 负责删除同 `owner_user_id + doc_id` 的旧检索数据后 upsert 当前 `parse_run_id`。
- parse run output 增加 `entity_count/relation_count/graph_passage_count`。

影响：

- 高风险但局部：`bins/worker/src/main.rs` 当前 `run_document_pipeline` 过长，建议只抽私有 helper，不做跨模块大重构。
- 中风险：usage/cost analytics 需要新增 triplet extraction usage event。
- 低风险：Postgres document blocks/chunks/assets 保持 source of truth，不迁到 Milvus。

验收：

- worker unit tests 覆盖：triplet success、malformed JSON degrade、reindex deletes old doc vectors、parse_run_id propagated。
- `cd avrag-rs && cargo test -p worker` 如果 binary package 不支持独立 test，则跑 `cd avrag-rs && cargo test --workspace run_document_pipeline --no-fail-fast`。
- 手动本地 ingestion smoke：上传同一文档两次，Milvus count 只保留最新 doc index。

### Phase 4: Graph Relation Retrieval

目标：让 RAG API 返回 relation paths 和 supporting chunks，先可用，再优化质量。

实施：

- `MainAgent::plan_rag` prompt 增加 `query_entities` 和 `graph_hints` 输出说明。
- fallback planner 不做 LLM entity extraction，只用原 query 走 dense/BM25/multimodal。
- RAG runtime 新增 `search_graph` stage：
  - 有 `query_entities` 时优先用实体检索。
  - 无 `query_entities` 但有 LLM 可用时，执行 bounded query entity extraction。
  - entity search + relation search 后做 1-hop subgraph expansion。
  - 第一版 hop limit 固定 1，relation candidate limit 固定 20，supporting chunk limit 固定 8。
  - relation/path rerank 先复用现有 reranker；无 reranker 时按 channel score 排序。
- `RetrievalBundle` 装配 relation paths 和 graph-supported chunks。
- Graph channel 失败只写 `degrade_trace`，不得让整个 execute-plan 失败。

影响：

- 高风险：`ExecutePlanResponse` consumers。
- 中风险：Main Agent answer prompt，因为 answer evidence 会变多。
- 前端只显示 citations；第一版不新增 relation path UI。

验收：

- `common` contract tests 覆盖 relation path serde。
- `rag-core` tests 覆盖 graph failure degrade、graph-supported chunk minimum budget、parse_run_id provenance。
- `app` tests 覆盖 Main Agent answer prompt 不把 user preferences 当 evidence。

### Phase 5: Parallel Channel Runner and Budgeting

目标：实现架构文档要求的并行 channel retrieval 和预算隔离。

实施：

- 在 `rag-core` 增加 channel runner：
  - text dense、BM25、multimodal、graph 并行执行。
  - 每个 channel 独立 budget、timeout、degrade trace。
  - 默认预算：text dense 35%、BM25 25%、multimodal 15%、graph 25%。
- 合并策略：
  - 先做 channel-aware RRF。
  - graph-supported chunks 保留最低 20% final context budget。
  - reranker 只处理可比较的 chunk candidates；relation path 不直接当普通 chunk 混排。
- `BackendTrace.channel_trace` 输出每个 channel 的 raw count、hydrated count、selected count、latency、degrade reason。

影响：

- 高风险：`RagRuntime::execute_plan` 控制流。
- 中风险：trace/debug UI，如果前端后续展示 `mode_debug`。
- 低风险：HTTP route 不变。

验收：

- runtime tests 覆盖预算分配、部分 channel fail-open、graph minimum budget。
- `cd avrag-rs && cargo test -p avrag-rag-core`
- `cd avrag-rs && cargo test -p transport-http rag_execute_plan_contract`

### Phase 6: Default Cutover and Legacy Retirement

目标：在 sanity set 通过后，把默认检索数据面切到 Milvus，并清理冲突文档。

实施：

- 配置默认从 `legacy` 切到 `milvus`，但保留 `RETRIEVAL_BACKEND=legacy` 回滚开关。
- 更新 `README.md` 和 runbook，明确正式前端是 `frontend_next`，Rust frontend fallback 是 legacy/dev-only。
- Qdrant/Tantivy 代码只在 legacy adapter 内保留；不再让新功能依赖它。
- 删除或禁用未接线的 graphflow fine-grained RAG tasks，除非 Phase 5 决定把它们真正接入主图。
- sanity set 全部通过后，标记 Qdrant/Tantivy/Postgres BM25 为 deprecated。

影响：

- 中风险：local dev 配置和 README。
- 高风险：生产默认 backend 切换，必须有回滚开关。

验收：

- `cd avrag-rs && cargo test --workspace`
- `cd frontend_next && pnpm typecheck`
- 20 条 sanity set：每类至少 4 条，必须全部返回非空 evidence；graph 类允许 graph degrade，但 dense/BM25 fallback 必须返回可解释 trace。

## 5. Rollout Order

1. 合并 Phase 0，建立 contract/sanity guard。
2. 合并 Phase 1，行为应与现状等价。
3. 合并 Phase 2，Milvus adapter behind config，不切默认。
4. 合并 Phase 3，对单一测试 org/workspace 开启 Milvus indexing。
5. 合并 Phase 4，对测试 org/workspace 开启 graph retrieval。
6. 合并 Phase 5，开启 parallel runner。
7. Phase 6 只在 sanity set 和 smoke 测试通过后执行。

切流后默认配置：

```text
RETRIEVAL_BACKEND=milvus
MILVUS_URL=http://127.0.0.1:19530
MILVUS_TOKEN=
MILVUS_DATABASE=default
MILVUS_COLLECTION_PREFIX=avrag
RAG_GRAPH_RETRIEVAL_ENABLED=true
RAG_PARALLEL_CHANNELS_ENABLED=true
```

回滚开关：

```text
RETRIEVAL_BACKEND=legacy
```

## 6. Explicit Non-Goals

- 不把 Postgres 产品控制面迁到 Milvus。
- 不把 Python vector-graph-rag 作为运行时服务依赖。
- 不在第一版实现完整 Recall@k/MRR/faithfulness 平台，只做 20 条 sanity set。
- 不在第一版为 relation paths 做前端可视化；先确保 evidence bundle 和 citations 正确。
- 不删除 legacy Qdrant/Tantivy，直到 Milvus 默认路径通过 sanity set。

## 7. Reference Notes

- Milvus 官方文档确认 BM25 full-text search 使用内置 BM25 function 生成 sparse vector，插入原始文本即可，查询时传原始文本。
- Milvus 支持 scalar filter expression，可用于强制服务端 ACL filter。
- Milvus 支持 insert/upsert/delete；大规模 upsert 有内存成本，worker 应采用按文档 replace/delete-then-insert，而不是全库大批量 upsert。
- Milvus multi-vector hybrid search 支持 RRF/Weighted rerank；本计划第一版仍在 Rust 中做 channel-aware merge，以保留 graph budget 和现有 trace 语义。
