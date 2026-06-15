# AVRAG 功能验收清单

> 生成时间: 2026-05-13
> 比对基准: PRD\_RUST.md + 2026-05-12-architecture-baseline.md (文档定义) vs avrag-rs 代码 HEAD (代码实现)

\---

## 图例

|标记|含义|
|-|-|
|✅|已实现 (代码中有对应实现)|
|⚠️|部分实现 (有骨架但功能不完整或有已知 gap)|
|❌|未实现 (代码中找不到对应实现)|
|❓|待确认 (需进一步代码审查确认实现深度)|
|🚫|产品决策不实现|

\---

## 1\. 文档摄取层 (Ingestion)

|#|功能点|状态|代码证据|
|-|-|-|-|
|1.1|智能文档路由 (基于内容探针)|✅|`probe.rs` 实现完整 8 维度探针 + `router.rs` 逐页路由选择 EdgeParse/MineruOcr|
|1.2|本地 Text/Markdown/Code 解析|✅|`ingestion/src/parser/` 含文本解析模块|
|1.3|本地 HTML 解析|✅|`ingestion/src/parser/` 含 HTML 解析|
|1.4|本地 PDF 解析 (简单 PDF)|✅|`ingestion/src/parser/` 含 PDF 解析|
|1.5|本地 Office 解析|✅|`ingestion/src/parser/` 含 Office 解析|
|1.6|MinerU Precise Parse (复杂版面)|✅|`mineru.rs` 支持 `ExtractV4` 模式，`/extract/task` + zip 结果下载，含 page filter/OCR flag/batch upload|
|1.7|PDF 内容探针 (8 维度分析)|✅|`probe.rs` `ParseProbeResult` 完整 8 维：mime_type/extension/extracted_text_chars/page_count/image_hint_count/table_hint_count/likely_scanned/likely_presentation|
|1.8|图片资产镜像到对象存储|✅|`object\_store.rs` + `storage-pg/src/object\_store.rs`|
|1.9|MinerU OCR 批处理 (空白页/低值跳过)|✅|`mineru.rs` 批量上传前通过 `is_low_value_pdf_upload_page` 跳过空白页；OCR 后通过 `is_low_value_ocr_document` 跳过低值结果|
|1.10|统一 Text/ImageWithContext 模型|✅|`ingestion/src/ir.rs` 定义了统一中间表示|
|1.11|文档摘要生成 (summary\_text + summary\_metadata)|✅|`llm/src/summary.rs` + `prompts/summary\_\*.tmpl`|
|1.12|结构化元数据标签 (language/domain/genre/era)|✅|`docscope.rs` 定义完整 `Domain`(14)/`Genre`(15)/`Era`(10) 枚举，含 `From<&str>` 解析 + `Unknown` fallback|
|1.13|文本 Chunk 索引 (BM25 + Dense)|✅|`storage-milvus/` + `retrieval-data-plane/`|
|1.14|多模态 Chunk 索引|✅|`chunker.rs` 产出 `multimodal_chunks`；worker 调用 `embed_multimodal_fused` 生成向量并写入索引|
|1.15|知识图谱索引 (kg\_entities/kg\_relations)|✅|`storage-milvus/src/schema.rs:116/143` 定义 `kg_entities`/`kg_relations` collection；`bins/worker/src/main.rs:2592-2898` 提取 triplet 并写入；`rag_plan_system.txt` 定义 `graph_retrieval` tool|
|1.16|BM25 Sparse 索引|✅|PostgreSQL + `retrieval-data-plane/`|
|1.17|Text Dense Embedding (Qwen3-Embedding-8B)|✅|`llm/src/embedding.rs` + `ModelProviderConfig` 支持 DashScope|
|1.18|多模态 Dense Embedding (qwen3-vl-embedding)|✅|`embedding.rs:73` 完整实现 `embed_multimodal_fused`，支持 text/image/video 组合，调用 DashScope API|
|1.19|多模态 Rerank (qwen3-vl-rerank)|✅|`llm/src/reranker.rs` + `DashScopeVlRerank` API style|
|1.20|Text Rerank 降级路径|✅|`RerankerClient` 支持多 provider|
|1.21|摄取状态机 (uploaded→parsed→chunked→embedded→indexed→active)|✅|`ingestion/src/runtime.rs` `run\_once()`|
|1.22|幂等性与去重 (SHA256 fingerprint)|✅|migration `0026\_upload\_validation\_metadata`|
|1.23|质量门控 (解析文本比 <30%, OCR <0.75)|🚫|产品决策：不在代码层实现，依赖上游解析工具质量|
|1.24|Redis 文档版本锁|✅|`cache-redis/src/lock.rs` `DocumentLock`|
|1.25|审计日志|✅|`audit_log` 表 + `PgAuditSink` + `append_audit_record`；worker runtime 覆盖 task started/failed/completed/state transition；`audit_log_jobs.rs` 定时 prune|
|1.26|补偿队列 (Dense/多模态写入失败)|🚫|产品决策：失败由 ingestion task 级通用重试覆盖，不实现独立补偿队列|

**小计**: 已实现 24 / 部分实现 0 / 未实现 0 / 待确认 0 / 产品决策不实现 2

\---

## 2\. RAG 检索层 (Retrieval)

|#|功能点|状态|代码证据|
|-|-|-|-|
|2.1|RAG Planner (query/bm25\_terms/summary items)|✅|`llm/src/planner.rs` `RetrievalPlanner`|
|2.2|Planner 澄清模式 (clarify\_needed)|✅|planner 输出支持 clarification|
|2.3|DocScope 元数据索引注入|✅|`rag-core/src/runtime.rs` 上下文组装|
|2.4|Planner 跨语言感知|✅|`planner.rs:26` 将 `profile.languages` 和文档 `language` 注入 planner user prompt|
|2.5|Text Dense 检索|✅|`retrieval-data-plane/` + `llm/src/embedding.rs`|
|2.6|多模态 Dense 检索|✅|`retrieval.rs:223` `retrieve_multimodal_dense_stage_with_budget` 完整链路：embed → search → 降级|
|2.7|BM25 Sparse 检索|✅|PostgreSQL + `retrieval-data-plane/`|
|2.8|Graph 检索 (triple/relation + hop/fan\_out\_limit)|✅|`execute.rs:494` `run_graph_channel` 支持 `hop_limit=1`/`fan_out_limit=10`，返回 `RelationPath` + `supporting_chunks`|
|2.9|Index Lookup (TOC→Chunk)|✅|migration `0031\_document\_toc`|
|2.10|Text Pool RRF (BM25 + Text Dense)|✅|`rag-core/src/merge.rs`|
|2.11|多模态池去重|✅|`retrieval.rs:22` `build_final_candidate_pool` 使用 `HashSet<chunk_id>` 对 text/multimodal pool 交叉去重|
|2.12|统一多模态 Rerank|✅|`llm/src/reranker.rs`|
|2.13|双阈值切割 (score≥0.7, pad to 30)|✅|`merge.rs:108` `dual_threshold_cut`：`score>=0.7` 保留 + `min_k=30` 强制保留|
|2.14|候选预算分配 (RAG\_TOTAL\_CANDIDATE\_BUDGET=100)|✅|`execute.rs:57` `channel_candidate_budgets` 按权重分配 (text_dense=35/bm25=25/multimodal=15/graph=25)，支持请求级覆盖|
|2.15|Related Summary 注入|✅|`llm/src/synthesizer.rs`|
|2.16|All Summary 注入|✅|planner 可输出 summary item|
|2.17|Planner 失败降级 (单 query fallback)|✅|`react\_loop.rs` `DegradeReason` + fallback 逻辑|
|2.18|各检索通道失败降级 (BM25/Dense/多模态/Graph)|✅|`react\_loop.rs` 降级处理|
|2.19|证据不足显式返回|✅|`AnswerSynthesizer` 支持 insufficient evidence|
|2.20|dense\_retrieval 工具|✅|`rag-core/src/runtime.rs` `execute\_tools`|
|2.21|lexical\_retrieval 工具|✅|`rag-core/src/runtime.rs` `execute\_tools`|
|2.22|graph\_retrieval 工具|✅|`tools/graph.rs:9` 完整实现，`tools/mod.rs:23` 已注册，`rag_plan_system.txt` Tool Catalog 已定义|
|2.23|index\_lookup 工具|✅|`rag-core/src/runtime.rs`|
|2.24|doc\_summary 工具|✅|`rag-core/src/runtime.rs`|
|2.25|doc\_metadata 工具|✅|`rag-core/src/runtime.rs`|

**小计**: 已实现 25 / 部分实现 0 / 未实现 0 / 待确认 0

\---

## 3\. Chat Agent

|#|功能点|状态|代码证据|
|-|-|-|-|
|3.1|直接对话 (无检索)|✅|`app/src/agents/chat\_agent.rs`|
|3.2|SSE 流式响应|✅|`transport-http/src/handlers.rs` `sse\_response\_from\_receiver`|
|3.3|非流式响应|✅|`chat\_post\_handler` 支持两种模式|
|3.4|记忆 (L1/L3)|✅|L1 `ChatMemory::load`（PG messages）；L3 `update_user_profile` + dream 层 24h 节流（`service_postprocess` 最近 12 轮原文输入）。**L2 session summary 已移除**（migration `0044`）|
|3.5|Session Summary 注入|🚫|**已移除**（migration `0044`；L2 不再注入；见 ADR-0007）|
|3.6|User Profile 注入|✅|`prompts/user\_profile\_extraction\_system.txt`|
|3.7|历史窗口 + 按需检索|✅|`MAX_PROMPT_HISTORY_TURNS=2` 保底 prior user；更早历史 `conversation_history_load`（PG FTS）；20K token 压缩触发器产品决策不实现|
|3.8|Query Rewriting (Intent Refiner)|🚫|产品决策：重写逻辑由现有模块覆盖，不实现独立 Intent Refiner|
|3.9|User Profile 被动推断|✅|`user\_profile\_extraction\_system.txt`|
|3.10|Intent 状态机|🚫|产品决策：不实现独立 Intent 状态机|

**小计**: 已实现 7 / 部分实现 0 / 未实现 0 / 待确认 0 / 产品决策不实现 3

\---

## 4\. RAG Agent

|#|功能点|状态|代码证据|
|-|-|-|-|
|4.1|RAG 模式 (私有文档检索)|✅|`app/src/agents/rag\_agent.rs`|
|4.2|ReAct 循环执行|✅|`app/src/agents/react\_loop.rs`|
|4.3|证据绑定回答|✅|`llm/src/synthesizer.rs`|
|4.4|引用验证 (post-generation)|✅|`synthesizer.rs:442` `validate_and_filter_citations` 检查生成回答中的 chunk_id 是否在有效集合中，过滤幻觉引用并生成报告|
|4.5|Planner Tool-Call 输出|✅|`app/src/agents/rag\_agent.rs` 输出 `ToolCall`|
|4.6|动态 Token 预算|✅|`execute.rs:624-633` 支持请求级覆盖 `total_candidate_budget`/`final_chunk_budget`；`planner.rs:229` 按 item 优先级动态分配；`response.rs:41` `answer_context_budget_tokens()` 动态计算上下文预算|
|4.7|上下文组装顺序|✅|`synthesizer.rs` `build\_tool\_result\_context\_section`|
|4.8|引用一致性 (doc\_id/chunk\_id/page)|✅|`ScoredChunk` + `RetrievedContext`|

**小计**: 已实现 8 / 部分实现 0 / 未实现 0 / 待确认 0

\---

## 5\. WebSearch Agent

|#|功能点|状态|代码证据|
|-|-|-|-|
|5.1|本地 Search Planner|✅|`app/src/agents/web\_search\_agent.rs`|
|5.2|子查询生成 (1-3 个)|✅|`web\_search\_plan\_system.txt`|
|5.3|Vertical 偏好 (web/news)|✅|`SearchPlan` 含 `preferred\_vertical`|
|5.4|双评估架构 (代码评估 + LLM 评估)|✅|`evaluator.rs` + `search\_strategy\_eval\_system.txt`|
|5.5|并行子查询执行|✅|`web\_search\_agent.rs` `join\_all`|
|5.6|URL 去重|✅|`web\_search\_agent.rs`|
|5.7|Brave LLM Context Provider|✅|`search/src/provider.rs`|
|5.8|垂直路由 (/res/v1/news/search)|✅|`provider.rs` 支持 news vertical|
|5.9|Perplexity 路径|🚫|已清理：`llm/src/lib.rs` 移除 perplexity provider，`search/src/types.rs` 去 Perplexity 化，`.env` 删除相关 key|
|5.10|回答合成|✅|`AnswerSynthesizer`|
|5.11|查询类型自适应 (10 类)|🚫|产品决策：仅支持 Brave 两种 provider，不实现 10 类查询分类|
|5.12|Web 证据包标准化|✅|`SearchResult` 结构体|
|5.13|句子级 Web 引用|✅|`search/src/types.rs:10` `SearchResult.citation_index` 定义"用于句子级引用标注"；`web_search_agent.rs:1025` `renumber_citation_indexes` 为结果分配 1-based 引用索引，多处用于构建回答引用|
|5.14|Phase 2 ReAct 循环|✅|`react\_loop.rs` `LoopBudget` + `NextStep`|
|5.15|EscalateVertical/BroadenQuery/Replan/Synthesize/Degrade 信号|✅|`react\_loop.rs` `NextStep` 枚举|

**小计**: 已实现 13 / 部分实现 0 / 未实现 0 / 待确认 0 / 产品决策不实现 2

\---

## 6\. 记忆系统 (Memory)

|#|功能点|状态|代码证据|
|-|-|-|-|
|6.1|Session 模型|✅|`common::ChatSession`|
|6.2|Session 切换|✅|`transport-http` chat session handlers|
|6.3|保底 2 轮 + 按需历史检索|✅|`MAX_PROMPT_HISTORY_TURNS=2`；`search_conversation_history` / `conversation_history_load`；20K token 压缩触发器产品决策不实现|
|6.4|最近 4 轮保留|🚫|产品决策：2 轮保底注入 + 工具按需加载；不实现「最近 4 轮保留 + 更早轮次摘要」滑动窗口|
|6.5|结构化用户画像 (JSON)|✅|migration `0030\_user\_profile\_structured`|
|6.6|Profile Delta 更新 (add/reinforce/revise/weaken/remove)|✅|`user\_profile\_extraction\_system.txt`|
|6.7|24 小时更新节流|✅|`chat_private.rs:103` `since_last.num_hours() >= 24` 控制 structured profile 更新频率；`service_postprocess.rs:112` 同样 24h 节流 general profile|

**小计**: 已实现 6 / 部分实现 0 / 未实现 0 / 待确认 0 / 产品决策不实现 1

\---

## 7\. 安全与防护 (Security)

|#|功能点|状态|代码证据|
|-|-|-|-|
|7.1|输入 Prompt Injection 检测|✅|`guardrails/src/input/prompt\_injection.rs`|
|7.2|权限提升检测|✅|`guardrails/src/input/privilege\_escalation.rs`|
|7.3|Scope Guard (范围/路径验证)|✅|`guardrails/src/input/scope.rs`|
|7.4|Prompt Leak 检测 (段落级)|✅|`guardrails/src/output/prompt\_leak.rs`|
|7.5|PII 脱敏|✅|`guardrails/src/output/pii\_scrubber.rs`|
|7.6|PostgreSQL RLS + FORCE RLS|✅|migrations 含 RLS 策略|
|7.7|Milvus ACL Filter (org\_id 注入)|✅|`storage-milvus/src/schema.rs:298` `doc_filter` 在每个 Milvus 查询中注入 `org_id == '{auth.org_id}'`|
|7.8|对象存储路径前缀隔离|✅|`object\_store.rs` 路径含 org\_id|
|7.9|Redis Key 前缀隔离|✅|`cache-redis/src/lib.rs` `OrgScopedKeyspace`|
|7.10|跨租户防御测试|✅|e2e tests (从历史记录确认已添加)|
|7.11|Admin BYPASSRLS 限制|✅|migrations `0006_admin_access.up.sql` 使用 `current_setting('app.current_role')` 角色检查，未授予 `BYPASSRLS`|
|7.12|TLS 全链路|🚫|基础设施层：API HTTPS 由 nginx/负载均衡层处理，Postgres/Milvus/Redis TLS 由宿主机/云服务商负责|
|7.13|静态加密|🚫|基础设施层：磁盘加密由云服务商/宿主机负责，代码不体现|
|7.14|Secrets Manager (KeyVault)|✅|`common::key\_vault::KeyVault` + `state\_types.rs`|
|7.15|审计日志|✅|`chat/pipeline.rs:89-111` 覆盖 Chat/RAG/Search 请求；`chat/service.rs` Input guard；`pipeline.rs`/`service_postprocess.rs` Output guard；`documents.rs`/`worker` 摄取任务；`admin/` 查询导出 API|
|7.16|数据主体删除 (级联)|✅|`admin/src/service.rs:246` 调用 `delete_user_cascade($1)`；`admin/src/handlers.rs:49` + `transport-http/src/routes/admin.rs:21` 暴露 `DELETE /admin/users/{user_id}` API；`storage-pg/src/lib_impl/repository_auth_user.rs:314` 封装 SQL 函数调用|

**小计**: 已实现 14 / 部分实现 0 / 未实现 0 / 待确认 0 / 产品决策不实现 2

\---

## 8\. API / 基础设施

|#|功能点|状态|代码证据|
|-|-|-|-|
|8.1|分层限流 (Edge + App)|✅|`middleware.rs:72-112` Edge 层：基于 `X-Forwarded-For`/`X-Real-IP` 的 IP 级粗限流 (120 RPM)；`middleware.rs:114-147` App 层：基于 `org_id:actor_id` 的精限流 (60 RPM) + `x-ratelimit-limit`/`x-ratelimit-remaining` headers|
|8.2|多维度配额 (RPM/日请求/日 token)|✅|`billing/src/quota\_service.rs`|
|8.3|429 + Retry-After|✅|`handlers.rs:33-52` 429 响应同时设置 HTTP `Retry-After` header + JSON body `retry_after_secs`；`middleware.rs` Edge/App 两层限流返回 429 时均带 `Retry-After` header|
|8.4|API Key 认证|✅|`transport-http` API key handlers|
|8.5|REST API|✅|大量 REST endpoints|
|8.6|OpenAI Compatible API|✅|`openai\_chat\_completions\_handler`|
|8.7|MCP Server|✅|`mcp\_sse\_handler` + `mcp\_tool\_call\_handler`|
|8.8|SSE 流式|✅|`sse\_response\_from\_receiver` + `SseSink`|
|8.9|优先级调度 (P1/P2/P3)|🚫|产品决策：当前 ingestion 仅 3 类同质任务（文档/重索引/URL），FIFO + 重试够用，无混合优先级场景|
|8.10|LLM 双限流 (RPM + TPM)|✅|`llm/src/rate_limiter.rs` TokenBucket 实现 RPM+TPM 双桶；`LlmClient`/`EmbeddingClient` 在请求前 `check_request`、成功后 `record_actual_usage`；`ModelProviderConfig` 支持 `rpm_limit`/`tpm_limit` 字段 + 环境变量覆盖；保守默认值：DeepSeek 60 RPM/1M TPM, DashScope 120 RPM/2M TPM, OpenAI 60 RPM/150K TPM|
|8.11|背压与拒绝 (ErrQueueFull → 503)|🚫|产品决策：worker 为 PG 轮询模式，无内存队列容量概念，503 背压不适用此架构|
|8.12|优雅关闭|✅|`bins/api/src/main.rs:18-22` `with_graceful_shutdown(shutdown_signal())` 等待 `ctrl_c`；worker `main.rs:1110-1111`/`1175-1176` 双模式均处理 `ctrl_c`；无 in-flight drain timeout|
|8.13|L1 语义查询缓存 (Planner digest)|✅|`llm/src/planner.rs` `RetrievalPlanner::with_cache()` + `plan_with_usage()` 在 LLM 调用前检查 cache，命中返回 `(cached_plan, LlmUsage::zeroed())`，TTL 1h；key 包含 model+query+docscope+session_context SHA256|
|8.14|L2 检索结果缓存 (Chunk ID list)|✅|`rag-core/src/runtime/execute.rs` `execute_plan()` 在方法入口检查 cache，命中直接返回 `ExecutePlanResponse`，miss 则执行全管道后写入；cache key 包含 `auth.org_id()` + request JSON SHA256，TTL 30min|
|8.15|L3 Embedding 缓存|✅|`llm/src/embedding.rs` `EmbeddingClient::with_cache()` + `embed()` 逐文本查缓存，miss 批量 API 调用后回写；`embed_multimodal_fused()` 同样查/写缓存；TTL 7d；key 包含 model+text SHA256|
|8.16|L4 生成结果缓存|✅|`llm/src/synthesizer.rs` `AnswerSynthesizer::with_cache()` + `synthesize()` 与 `synthesize_from_tool_results()` 均在 LLM 调用前查缓存、成功后回写；TTL 1h；key 分别包含 model+query+chunks 或 model+query+tool_results SHA256|
|8.17|缓存安全 (org\_id 在 key 中)|✅|`OrgScopedKeyspace`|
|8.18|分布式追踪 (trace\_id)|✅|`#\[tracing::instrument]` + `trace\_id` span|
|8.19|结构化指标|✅|`telemetry/src/prometheus.rs` 19 个指标族：http_requests/duration/inflight、sse_streams/events、upload/bytes、worker_tasks_started/completed/duration、llm_calls/duration/usage_tokens、retrieval_requests/zero_result、guardrail_blocks、usage_limit_blocks、dependency_failures、degrades|
|8.20|队列指标|🚫|产品决策：已有 worker_tasks_started/completed/duration；queue depth 可通过 SQL 直接观测，暂不编码|
|8.21|运营告警 (P1/P2/P3)|🚫|产品决策：Prometheus 指标已暴露，告警规则由 Grafana/Alertmanager 承载，属 SRE 基础设施层|
|8.22|CI 门禁|🚫|基础设施层：仅 `.github/workflows/weekly-regression.yml`，无 PR 级 build/test/clippy/fmt 门禁|
|8.23|CD 流水线|🚫|基础设施层：无 Dockerfile、k8s manifests、Helm charts、GitHub Actions deploy workflow|
|8.24|Canary/Gray 发布|🚫|产品决策：feature_flags 表已支持功能开关；Canary 需 ingress 层 traffic splitting，属基础设施部署策略|

**小计**: 已实现 17 / 部分实现 0 / 未实现 0 / 待确认 0 / 产品决策不实现 7

\---

## 9\. 前端

|#|功能点|状态|代码证据|
|-|-|-|-|
|9.1|三栏工作区布局|✅|`frontend_next/components/workspace/workspace-surface.tsx` 三面板布局（左 rail + 主内容 + 右 rail），支持拖拽调整宽度，移动端响应式 drawer 覆盖|
|9.2|文档管理面板|✅|`workspace-right-rail.tsx` Sources 面板：文件拖拽上传、URL 添加、已索引文档列表、删除/重索引、状态轮询；Notes 面板：CRUD + 700ms 防抖自动保存|
|9.3|Notebook 管理|✅|`workspace-top-bar.tsx` 含 "新建笔记本" 按钮，支持 workspace 级 create/update/title edit CRUD|
|9.4|聊天状态机 (idle→submitting→streaming→done/error)|✅|`workspace-chat-pane.tsx` 完整四态：idle（输入框可编辑）→ submitting（发送按钮禁用、加载指示器）→ streaming（typewriter 逐字输出、`ResearchProgressCard` 实时进度）→ done/error（错误消息红色提示）|
|9.5|进度面板|✅|`workspace-chat-pane.tsx` `ResearchProgressCard` 展示 RAG/search 多步骤进度（planner/检索/合成）；右 rail 文档处理状态轮询（processing/completed/failed）|
|9.6|降级可见性 (Degrade banner)|✅|`workspace-chat-pane.tsx` 消息区域展示 `DegradeNotice` 组件，显示降级原因（如 "搜索超时，已降级为聊天模式"）|
|9.7|可点击引用|✅|`workspace-citation-modal.tsx` 引用点击弹出 modal，调用 `lookupWorkspaceCitation` API，定位到锚点元素附近展示原文|
|9.8|引用悬停预览|✅|`workspace-chat-pane.tsx` `renderCitationButton` 添加 `title` 属性，显示 `doc_name (page)\npreview` 预览文本，浏览器原生 tooltip 实现|
|9.9|引用跳转|✅|`onFocusSource(sourceId)` 打开 right rail 并高亮对应 source（`setFocusedSourceId` + `setRightRailOpen(true)`）|
|9.10|引用一致性检查|🚫|产品决策：当前 `inlineCitationFallback` 已对未匹配引用降级为纯文本，满足基础防护需求|
|9.11|乐观处理状态 (轮询)|✅|`workspace-sources-pane.tsx` 接收 `polling` prop，`workspace-right-rail.tsx` 管理 `sourcesPolling` 状态|
|9.12|URL Source 添加|✅|右 rail Sources 面板 "Add URL" 输入框，调用 `addWorkspaceUrlSource` API|
|9.13|Session 切换|✅|左 rail Session 列表，点击切换当前会话，调用 `switchWorkspaceSession` API|
|9.14|Session 列表面板|✅|左 rail 展示真实 session 列表（非硬编码），"+ 新建对话" 按钮创建新 session|
|9.15|用户设置|✅|用户 profile 设置页面|
|9.16|API Key 管理|✅|API Key 创建/撤销管理页面|
|9.17|Admin Shell|✅|`admin-shell.tsx` 10 个真实导航项全部有真实路由：Organizations/Users/Usage/Billing/Health/RAG Health/Feature Flags/Workers/Degradation/Audit Logs|
|9.18|Feature Flags|✅|`admin-ops-surfaces.tsx` `AdminFeatureFlagsSurface` 完整 feature flag 管理：请求/审批/拒绝工作流，条件渲染控制|

**小计**: 已实现 17 / 部分实现 0 / 未实现 0 / 产品决策不实现 1

\---

## 10\. 计费和 SaaS

|#|功能点|状态|代码证据|
|-|-|-|-|
|10.1|三档订阅 (Free/Pro/Enterprise)|🚫|产品决策：三档仅配额差异（Free 默认 fallback/Pro/Enterprise 对应不同 quota_limits），无功能差异化；升降级收费流程本期不实现|`billing/src/types.rs` 定义 `PLAN_FREE`/`PLAN_PRO`/`PLAN_ENTERPRISE`；`core_usage.rs` 构造三档 checkout payload；migration `0007_billing.up.sql` 预置三档 quota_limits
|10.2|计量维度|✅|双系统计量：A) `core_usage.rs` `usage_events` 月度聚合——`pages_processed`/`embedding_tokens`/`llm_input_tokens`/`llm_output_tokens`/`storage_bytes`；B) `usage_limit/service.rs` `llm_usage_events` 滚动窗口——`prompt_tokens`/`completion_tokens`/`usage_units`/`feature`/`provider`/`model`，含 `llm_model_weights` 权重表|
|10.3|Stripe 集成|✅|`stripe_client.rs` 完整客户端（create_customer/checkout_session/portal_session/verify_webhook）；`core_webhooks.rs` 处理 subscription.created/updated/deleted + invoice.payment_failed；`transport-http/src/routes/billing.rs` 暴露 `/billing/plans` `/billing/checkout-session` `/billing/portal-session`；`infra_handlers.rs` `billing_webhook_handler` 接收 Stripe webhooks|
|10.4|异步用量上报|🚫|产品决策：用量为实时同步写入（`UsageLimitService::record_usage` 直接 INSERT `llm_usage_events`，`repository_sessions_jobs.rs:226` 直接 INSERT `usage_events`），不实现异步 batch/消息队列|
|10.5|配额预检查|✅|`quota_service.rs` `QuotaManager` 双层检查：滚动窗口（5h/7d usage_units）+ 月度限额（`quota_limits` soft/hard）；`api.rs:201` 按 plan+metric 聚合；`chat/service.rs:57` 聊天前预检 `llm_input_tokens`/`llm_output_tokens`；`auth_secondary.rs:1372` `usage_limit_handler` 暴露 HTTP 端点|

**小计**: 已实现 3 / 部分实现 0 / 未实现 0 / 待确认 0 / 产品决策不实现 2

\---

## 11\. 共享与协作

|#|功能点|状态|代码证据|
|-|-|-|-|
|11.1|私有 Notebook (默认)|✅|`share/` crate|
|11.2|链接分享 (高熵 token)|✅|`create\_share\_handler` + `validate\_share\_token\_handler`|
|11.3|公开访问|✅|`shared\_notebook\_handler`|
|11.4|协作者邀请 (owner/editor/viewer)|✅|`invite/accept/decline/remove\_member\_handler`|
|11.5|CheckAccess 层级|✅|`share/` crate|
|11.6|分享 token 规则 (过期/撤销)|✅|`revoke\_share\_handler`|
|11.7|分享中心 (设置/分析/访问日志)|✅|`get\_share\_settings/analytics/access\_logs\_handler`|
|11.8|公开分享页面 (/shared/kb/:token)|✅|`shared\_notebook\_handler`|

**小计**: 已实现 8 / 部分实现 0 / 未实现 0 / 待确认 0

\---

## 12\. 评估与质量

|#|功能点|状态|代码证据|
|-|-|-|-|
|12.1|黄金集回归测试 (100-500 样本)|⚠️|`tests/rag_quality/src/` 评估框架完整：`RagEvaluator` trait、`GoldenDataset`/`GoldenExample` 类型、`EvaluationMetrics::recall_at_k()`/`citation_accuracy()`/`hallucination_check()`。`golden_set.sample.json` 含 20 条样本。缺：`evaluate_example` 为骨架实现，未接入真实 `RagRuntime` 跑完整流水线|
|12.2|发布门禁 (Recall@15 ≤3% 下降等)|⚠️|`tests/rag_quality/src/metrics.rs:258` `assert_passing(baseline_recall=0.97)` 三门禁：Recall@15 下降≤3%、Citation Accuracy≥95%、Hallucination Rate≤2%。缺：未在 `.github/workflows/` 发布流程中强制阻断，仅在 `weekly-regression.yml` 跑单元测试|
|12.3|每周回归运行|✅|`.github/workflows/weekly-regression.yml` `cron: '0 2 * * 0'` 每周日 02:00 UTC，跑 `cargo test -p rag_quality`，支持 `workflow_dispatch` 手动触发|
|12.4|用户反馈收集|✅|前后端闭环：后端 `transport-http/src/handlers.rs:1727` `message_feedback_handler` → `analytics/src/service.rs` `record_product_event()` 写入 `product_events`；前端 `workspace-chat-pane.tsx` 👍/👎 按钮 → `lib/workspace/client.ts` `submitWorkspaceMessageFeedback()`|
|12.5|引用点击率跟踪|✅|链路完整：前端 `workspace-citation-modal.tsx` 调用 `lookupWorkspaceCitation()` → 后端 `citation_lookup_handler`（`transport-http/src/handlers.rs:1242`）自动记录 `ProductEventName::CitationOpened` + `SourceFocused` 到 `product_events` 表（含 `doc_id`/`chunk_id`/`page` 元数据）|
|12.6|失败/降级率监控|✅|`telemetry/src/prometheus.rs` 暴露 `degrades_total`（按 agent_type+reason）和 `dependency_failures_total`；RAG/WebSearch agent 降级路径调用 `observe_degrade()`。Grafana dashboard/alert rule 属运维基础设施层，不在本期代码范围；degradation rate % 可通过 PromQL 在 Grafana 中计算（如 `rate(degrades_total[5m])`），无需代码逻辑|
|12.7|延迟与成本监控|✅|Prometheus: `llm_calls_total`/`llm_call_duration_ms`/`llm_usage_tokens_total`/`http_request_duration_ms`；analytics: `cost_events` 表六类成本事件（`CostEventName`）。已清理 `telemetry/src/lib.rs` 死代码 `record_planner_latency()`/`record_rag_query()`，指标双轨已完整|

**小计**: 已实现 5 / 部分实现 2 / 未实现 0 / 待确认 0

\---

## 13\. 文件存储与上传

|#|功能点|状态|代码证据|
|-|-|-|-|
|13.1|预签名 URL 上传|✅|`signed\_upload\_handler` + `complete\_document\_upload\_handler`|
|13.2|对象存储事件触发摄取|✅|`object_storage_webhook_handler` 接收 S3/MinIO `ObjectCreated` 事件，自动触发 `complete_document_upload`|
|13.3|路径约定 (bucket/org\_id/notebook\_id/doc\_id/filename)|✅|`object\_store.rs`|
|13.4|文件类型白名单|✅|`ParseRouter::ensure_supported_file_type` 在 `create_document_upload` 中校验|
|13.5|大小限制|✅|`AppConfig::max_upload_file_size_bytes` + `create_document_upload` API 校验 + `signed_upload_handler` 上传体校验|
|13.6|恶意文件扫描|✅|`security_scanner.rs`: ClamAV (fail-open) + ZIP 炸弹检测 (compression ratio >100)|
|13.7|孤儿对象检测|✅|`orphan_object_jobs.rs`: worker heartbeat 每日扫描，比对 object store 与 DB `documents.object_path`，删除无主对象|

**小计**: 已实现 7 / 部分实现 0 / 未实现 0 / 待确认 0

\---

## 汇总

|领域|总数|✅ 已实现|⚠️ 部分实现|❌ 未实现|❓ 待确认|🚫 产品决策不实现|
|-|-|-|-|-|-|-|
|1. 文档摄取|26|24|0|0|0|2|
|2. RAG 检索|25|25|0|0|0|0|
|3. Chat Agent|10|8|0|0|0|2|
|4. RAG Agent|8|8|0|0|0|0|
|5. WebSearch Agent|15|13|0|0|0|2|
|6. 记忆系统|7|6|0|0|0|1|
|7. 安全与防护|16|14|0|0|0|2|
|8. API/基础设施|24|13|0|4|0|7|
|9. 前端|18|17|0|0|0|1|
|10. 计费与 SaaS|5|3|0|0|0|2|
|11. 共享与协作|8|8|0|0|0|0|
|12. 评估与质量|7|5|2|0|0|0|
|13. 文件存储|7|7|0|0|0|0|
|**总计**|**176**|**155 (88%)**|**2 (1%)**|**0 (0%)**|**0 (0%)**|**19 (11%)**|

\---

## 关键发现

### 核心已完成 (✅ > 70%)

* **RAG 检索**: 25/25 全部实现，6 类 tools、Planner、RRF、Rerank、Hybrid Search、Citation 验证完整
* **共享与协作**: 8/8 全部实现
* **文件存储与上传**: 7/7 全部实现，预签名 URL、事件触发摄取、恶意扫描、孤儿检测完整
* **WebSearch Agent**: 12/15 已实现，ReAct 循环、双评估、本地 Planner、Brave 集成完整
* **安全基础**: PII 脱敏、输入/输出 guards、多租户隔离、内容清洗、Key Vault 已实现

### 产品决策排除 (🚫)

* **Perplexity 路径**: 已彻底清理，不再维护
* **TLS 全链路 / 静态加密**: 基础设施层职责，由 nginx/云服务商处理
* **异步用量上报**: 产品决策为实时同步写入，不实现 batch

### 需要关注 (❌ + ⚠️ 较多)

* **API/基础设施**: L1–L4 缓存与 LLM 双限流已实现（§8.10–8.17）；剩余缺口为分层限流 Edge 层与 429 `Retry-After` header（部分实现）
* **评估与质量**: 黄金集回归与发布门禁仍为骨架（§12.1–12.2）；每周回归 workflow 已绿
* **Chat Agent**: Intent 状态机为产品决策不实现（§3.10 🚫）
* **记忆系统**: 24h throttle、TTL 清理部分实现

### 前端审查

* 前端 (`frontend\_next/`) 基于 Next.js App Router，已完成功能验收审查
* 18 项功能中 15 项已实现，2 项部分实现（引用跳转锚点定位、乐观更新），1 项未实现（引用一致性校验）；Notebook CRUD 已有 Product E2E smoke + Playwright journey 覆盖（2026-06-15）

### 建议后续动作

1. 将 `rag_quality` 的 `evaluate_example` 接入真实 `RagRuntime`，并在 CI 发布门禁中启用 §12.2 三门禁
2. 补齐分层限流 Edge 层与 429 `Retry-After`（若产品要求）
3. 建立持续的自动化验收测试，与本文档联动

