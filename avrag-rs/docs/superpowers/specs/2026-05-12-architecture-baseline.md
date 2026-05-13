# avrag-rs 产品架构基准版（2026-05-12）

> **状态**：已确认定稿。取代所有更早日期文档中的冲突内容。
> **文档优先级**：本基准版 > 2026-05-10 > 2026-05-09 > 2026-04-26 > 2026-04-27 > 更早文档。
> **生效日期**：2026-05-12。后续修改需更新日期并说明变更原因。
> **生成方法**：以最新日期文档为准，合并冲突点，标注与代码实际状态的 gap。
>
> **修订记录**：
> - **2026-05-12（当日修订）**：修正 §13、§14、§18 中关于 MainAgent 和 RagAgent 的过时描述。MainAgent 模块已从代码库完全删除，RagAgent 已独立承载 RAG production path。
> - **2026-05-13（次日修订）**：同步定稿后 5 次提交的代码变更：
>   - §1.2 / §7：W1（本地 planner 未实现）+ W2（跳过 planner）**已修复**，落地双评估架构（commit `88532b1`、`b2e4f1f`）
>   - §14：AGENT_LLM 模型由 `deepseek-v4-flash` 改为 `deepseek-v4-pro`（代码与 .env.example 实际值）；SEARCH_LLM 独立行已删除（统一为 AGENT_LLM_\*）；Perplexity/SiliconFlow 主路径已删但代码层残留待清理
>   - §18.1：`search_plan_system_legacy.txt` 已被重命名为 `web_search_plan_system.txt`，成为当前生产 prompt（commit `ce1fa96`）；`web_search_plan_system_new.txt` 为死文件
>   - §18.3：`SEARCH_PLANNER_ENABLED` 死配置已彻底清理
>   - §9.3 / §18 新增第 13 项 Cleanup backlog：`canary.rs` / `sysvec.rs` 模块和 `canary_token` 字段属结构性残留，未装到 pipeline

---

## 1. 核心架构

### 1.1 调度层

产品采用 **UnifiedAgentService + 三独立 Agent** 架构。

```text
User
  -> UnifiedAgentService (dispatcher)
      -> ChatAgent         -> 直接对话 / 创意写作 / 头脑风暴
      -> WebSearchAgent    -> 外部搜索 (Brave LLM Context / Perplexity)
      -> RagAgent          -> 检索增强生成 (tool-call 范式)

RAG API (检索服务，非自主 agent)
  -> BM25 / text dense / multimodal dense / graph relation retrieval
  -> fusion / rerank / evidence packaging
```

`UnifiedAgentService` 是面向用户的唯一调度层，根据 `AgentRequest.kind` 路由到三个独立 Agent。每个 Agent 独立实现 `Agent` trait，通过统一的 `AgentEvent` 事件流与调用方通信。

**历史变更**：旧架构使用单体式 "Main Agent"，已拆分为三独立 Agent（2026-04-23 → 2026-04-26）。

### 1.2 Agent 生产状态

| Agent | 职责 | 生产状态 |
|-------|------|----------|
| `ChatAgent` | 直接对话、创意写作、头脑风暴、解释说明 | 已生产化，走 UnifiedAgentService |
| `WebSearchAgent` | 本地 planner → 多子查询并行 → 双评估 → 答案合成 | 已生产化，走 UnifiedAgentService；本地 planner + 双评估（code + LLM）架构已落地 |
| `RagAgent` | 检索计划生成 → RAG API 调用 → 答案合成 | 已生产化，走 UnifiedAgentService；tool-call 范式已落地；独立 ReAct 循环执行完整 RAG 流程 |

> **Gap W1（已修复 2026-05-12 commit `88532b1`）**：`WebSearchAgent` 本地 planner 已实现。
> - `web_search_agent.rs:218-275` 的 `plan_search()` 调用 AGENT_LLM 生成 `SearchPlan`（sub_queries / intent_summary / needs_clarification / preferred_vertical）
> - `web_search_agent.rs:288-475` 在 ReAct 之前先跑 Phase 1 Local Planner：并行执行所有 sub_queries（`futures::future::join_all`），按 URL 去重累积结果
> - 加载 prompt `prompts/web_search_plan_system.txt`（内容由 `search_plan_system_legacy.txt` 直接重命名而来，commit `ce1fa96`）
>
> **Gap W3**：`WebSearchAgent` streaming callback 已修复为 channel bridge（2026-04-30），但缺少 live Brave LLM Context E2E smoke 验证。

---

## 2. 存储分工

```text
Postgres: 产品控制面
- users, organizations, workspaces / notebooks
- auth and sessions, chat history
- agent memory metadata (session summaries, user profiles)
- ingestion jobs, audit / usage / billing
- document lifecycle state

Milvus: 检索数据面
- text chunks, multimodal chunks
- BM25 sparse vectors, dense text vectors, multimodal vectors
- kg_entities, kg_relations, graph passages / chunk evidence

Redis: 短时辅助（注意分工差异）
- bins/worker:   `avrag-cache-redis::DocumentLock` —— 分布式文档锁
- crates/app:    `adapters/redis_rate_limiter.rs` 直接使用 `redis` crate —— API 限流
- 注：`avrag-cache-redis` 不在 API 请求路径上；app 与 worker 共享 Redis 实例但各走一套客户端
```

> **Note**：`semantic memory vectors` 在 `2026-04-26 §10` 中列出，但 `2026-05-10` 明确标记为 P3-1 "不修复（长期画像全存 md 文档）"。当前代码未实现。

> **2026-05-13 修订**：补充 Redis 分工说明（cache-redis 仅 worker / app 走 redis 直连），与 §17 文档冲突清单第 14 项配套。

---

## 3. Agent 事件契约

所有 Agent 通过统一的 `AgentEvent` 事件流通信：

```text
AgentEvent::Activity              -> 进度通知
AgentEvent::ReasoningSummaryDelta -> 推理摘要增量
AgentEvent::MessageDelta          -> 答案文本增量
AgentEvent::Citations             -> 引用来源
AgentEvent::Usage                 -> Token 用量
AgentEvent::DebugTrace            -> 调试信息 (debug flag 控制)
AgentEvent::Done                  -> 最终完成
AgentEvent::Error                 -> 终端错误
```

Streaming 路径通过 `ChannelSink` 实时转发到 SSE；非 streaming 路径通过 `CollectingSink` 收集后组装。

---

## 4. RAG API 边界

### 4.1 定位

RAG API 不是自主 agent，而是检索服务。它可以调用 LLM 完成**有边界的检索算子**（三元组抽取、query entity extraction、relation/path rerank、chunk rerank、evidence compression），但不负责对话策略、澄清策略、长程规划或最终回答风格。

### 4.2 输入

- `plan_version`, `doc_scope`, `items`, `bm25_keywords`
- `query`, `query_entities`, `graph_hints`, `summary_mode`
- `budget`, `acl_context`, `trace_context`

不接收：session history、session summary、clarify 语义、agent memory。

### 4.3 输出

- candidate chunks, citations, relation paths
- graph-supported chunks, summary chunks
- score breakdown, coverage, degrade trace, backend trace

不输出：最终用户回答、是否澄清的对话级决策。

---

## 5. RAG Runtime 工具目录（Tool Catalog v1）

2026-05-09 引入的工具分发架构，从 monolithic `ExecutePlanRequest` 重构为工具目录 + 分发执行模式。

| 工具名 | 职责 | 状态 |
|--------|------|------|
| `dense_retrieval` | 向量检索（文本 + 多模态融合） | ✅ 生产 |
| `lexical_retrieval` | BM25 精确字面检索 | ✅ 生产 |
| `graph_retrieval` | 三元组/关系检索 | ✅ 生产，后端就绪 |
| `index_lookup` | TOC → chunk_id 直取 | ✅ 生产 |
| `doc_summary` | 读取预生成摘要 | ✅ 生产 |
| `doc_metadata` | 读取文档元信息 | ✅ 生产 |

Planner 输出 `Vec<ToolCall>` + `next_step`（第一版固定为 `"answer"`）。Runtime dispatcher 并行执行、收集 `Vec<ToolResult>`。Synthesizer 接收 `Vec<ToolResult>` 生成答案。

---

## 6. 记忆层架构（三层模型）

已完全取代早期工作记忆设计。

```text
Layer 1 (短期): chat_messages — 对话原文
  - 用途：指代消解、对话连续性
  - 衰减：无需衰减，原始记录

Layer 2 (中期): chat_sessions.summary — 结构化 JSON 摘要
  - 触发：每 10 轮对话触发一次 LLM 摘要
  - 消费：注入 agent system prompt 作为 continuity context

Layer 3 (长期): user_profiles.structured_profile — 用户结构化画像
  - 触发：会话后被动触发（`persist_chat_execution` 中检查），24h 节流
  - 机制：LLM 输出 delta 建议（add/reinforce/revise/weaken/remove）
  - 设计意图：手动触发优于定时任务（长尾用户空转问题）
```

**注入规则**：
- Session summary：提供对话连续性；不作为事实证据
- User preferences：只影响表达风格，不覆盖事实或推理
- RAG Evidence：唯一的事实权威来源

> **历史变更**：旧设计 "工作记忆层"（`AgentRequest.working_memory`、`DialogueStateRow` 等）已完全移除（`2026-04-27`）。

---

## 7. WebSearch Agent 设计

### 7.1 目标架构

```text
planner (intent recognition + coreference resolution + sub-query generation)
  -> multi-query execution -> result aggregation -> answer synthesis
```

Planner 输出 `SearchPlan`：
- `sub_queries`: 1-3 个子查询
- `intent_summary`: 用户意图摘要
- `needs_clarification`: 是否需要澄清
- `preferred_vertical`: `web` | `news`

### 7.2 Brave LLM Context 路径（设计目标）

- 本地 planner 生成子查询和垂直偏好
- 并行执行多个子查询（支持 vertical 路由到 `/res/v1/news/search`）
- URL 去重 + citation 重新编号
- LLM 合成最终答案（流式或非流式）

### 7.3 Perplexity 路径

- 委托给 provider 的 built-in agentic flow
- 流式透传 provider 的事件

### 7.4 当前代码实际状态

> **状态**：本地 planner 已实现并生产化（2026-05-12 commit `88532b1`、`b2e4f1f`）。
>
> **Gap W2（已修复）**：Brave 路径不再跳过本地 planner。`web_search_agent.rs::run_react_loop()` 分两阶段：
>
> **Phase 1 — Local Planner**（`web_search_agent.rs:294-472`）：
> - `plan_search()` 调用 AGENT_LLM 生成 `SearchPlan`
> - 解析输出（`parse_search_plan`）：`sub_queries`（1-3 条）+ `intent_summary` + `needs_clarification` + `preferred_vertical`（"general"/"news"）
> - 并行执行所有 sub_queries（`futures::future::join_all`），按 URL 去重累积
> - **双评估架构**：
>   - Code 评估：`EvaluationSignals::compute_term_coverage`（快速,基于 recall/term coverage）
>   - LLM 评估：`evaluate_search_strategy`（深度,输出 `SearchStrategyEvaluation` + `suggested_followup_queries`）
>   - LLM 评估可用时优先采纳，code 评估作为兜底
> - 评估结果若为 `Synthesize` 则直接进入答案合成；否则透传 `all_sub_queries` 进入 Phase 2
>
> **Phase 2 — ReAct Loop**（回退/精化，`web_search_agent.rs:474-737`）：
> - 在 `LoopBudget::search(UserTier::Pro)`（b=2）预算内执行
> - 每轮按对象信号路由：`EscalateVertical`(general→news)、`BroadenQuery`(去末尾 token)、`Replan`、`Synthesize`、`Degrade`
> - LLM 给出的 `suggested_followup_queries` 优先于机械式 broaden
>
> Brave provider 入口 `executor.execute_search(query, vertical)`（`search/src/provider.rs`）只负责单次 API 调用，不再承担 sub_query 拆分。Perplexity 路径已在 commit `4dd71a1` 中从主路径移除。

---

## 8. Prompt 管理体系

### 8.1 当前状态

所有系统提示词外置到 `prompts/` 目录，使用 `include_str!()` 编译时加载。

```text
prompts/
  chat_agent_system.txt           -> ChatAgent system prompt
  rag_answer_system.txt           -> RAG Synthesizer system prompt
  web_search_system.txt           -> WebSearchAgent synthesis prompt
  rag_plan_system.txt             -> RagAgent planner prompt (tool catalog 格式)
  rag_planner_system.txt          -> Legacy RetrievalPlanner prompt（已标记为 legacy）
  rag_strategy_eval_system.txt    -> RAG strategy evaluation
  search_strategy_eval_system.txt -> Search strategy evaluation
  session_summary_system.txt      -> Layer 2 摘要
  user_profile_extraction_system.txt -> Layer 3 "做梦"
  triplet_extraction_system.txt   -> 三元组抽取
  summary_generation.v1.tmpl      -> 文档摘要
  summary_generation_finalize.v1.tmpl -> 文档摘要 finalize
  *.tmpl                          -> 用户消息模板
```

### 8.2 历史变更

旧设计提到 `load_prompt_template()` 共享加载器（`2026-04-27` 声称"已建立"）。**实际代码中没有这个函数**，prompt 直接使用 `include_str!`。

> **Gap P1**：Prompt 管理 infra 缺失：无 DB schema + CRUD API、无版本历史、无 A/B testing、无灰度回滚。`2026-05-10` 明确标记为 P2-2 "不修复（等上线，prompt 不频繁更新）"。

---

## 9. Guard Pipeline

### 9.1 当前状态（已确认设计目标）

两层 Guard 架构：

```text
Input Guards:
  - PromptInjectionGuard    (关键字正则)
  - PrivilegeEscalationGuard (关键字正则)
  - ScopeGuard              (范围/路径正则)

Output Guards:
  - PromptLeakGuard         (段落级系统提示词泄露检测)
  - PiiScrubberGuard        (PII 脱敏)
```

### 9.2 历史变更

- `2026-04-27 P1-7`："GuardPipeline 是壳，默认返回 pass"
- `2026-05-10 P1-1`：提到 OutputGuardPipeline 有 `citation_provability` + `pii_scrubber` + `harmful_content`
- **当前代码**：已移除 `citation_provability`、`harmful_content`、`canary_leak` 三个 guard，只保留 `prompt_leak` + `pii_scrubber`

> **确认**：`prompt_leak` + `pii_scrubber` 为设计目标。G1（semantic guard）❌ 不修复（沙盒环境足够）。G2（canary token / SysVec）已取消，不再推进。

### 9.3 结构性残留（已知）

虽然 G2 已取消，以下死代码仍在仓库中，未装入 `GuardPipeline`（参见 §18 第 13 项 Cleanup backlog）：

- `crates/guardrails/src/canary.rs`、`crates/guardrails/src/sysvec.rs` 文件存在
- `crates/guardrails/src/lib.rs:10,13`：`pub mod canary; pub mod sysvec;` 模块声明
- `crates/app/src/chat/pipeline.rs:40-42`：`canary_token: Option<String>` 字段
- `crates/app/src/chat/service_modes.rs:39,140,199`：3 处 `canary_token: None` 塞入

这些残留不影响运行时行为（pipeline 不调用），属于 cleanup backlog。

### 9.4 Output guard 按模式启用矩阵（2026-05-13）

`check_output` 是同步纯函数（无 LLM/无 I/O），由 `BuildChatExecutionParams.apply_output_guard` 字段在 `pipeline_steps.rs` 按模式切换：

| 模式 | apply_output_guard | 理由 |
|------|-------------------|------|
| Chat (general) | `false` | 仅用户 query + 自有 L2/L3 memory 进入 LLM，无外部数据。开启会 scrub 用户自己输入的联系方式，弊大于利 |
| Search | **`true`**（2026-05-13 起）| 外部网页 snippet 直接进 LLM 上下文（`web_search_agent.rs:1192`），prompt 注入和 PII 泄露是真实风险 |
| RAG | `true` | 文档内容进 LLM 上下文，与 Search 同理 |

> **说明**：`PromptLeakGuard` 的 `PROMPT_SOURCES` 已 `include_str!` 收录 chat/search/rag 三套 prompt 指纹（`prompt_leak.rs:16-64`）；`PiiScrubberGuard` 只匹配 SSN/信用卡/邮箱/美式电话/MRN/护照/驾照标准格式，不会误伤人名地名。Chat 模式不开 guard 是 ROI 取舍，不是安全等级降低。

---

## 10. GraphFlow 退场

**状态**：✅ 已完成。

- Chat 模式：已走 `UnifiedAgentService → ChatAgent`
- Search 模式：已走 `UnifiedAgentService → WebSearchAgent`
- RAG 模式：已走 `UnifiedAgentService → RagAgent`（tool-call 范式）

旧 GraphFlow 文件已从代码中删除。`2026-04-26` 文档 §16 已更新确认。

> **历史说明**：GraphFlow 在迁移期间仍负责共享编排 rails（preflight、session、deterministic dispatch、output guard、persist、usage、notify、response），但不直接参与 agent 内部逻辑。`2026-04-29` 三 agent 迁移计划明确保留 GraphFlow 作为编排内核，不替换为新的 workflow 框架。

---

## 11. 产品级保护约束

每次 Milvus 查询必须强制带服务端 ACL filter（`org_id`、`workspace_id`、`doc_scope`）。

每个检索结果必须带 provenance：`doc_id`、`chunk_id`、`page`、`parse_run_id`、`source_locator`。

图检索失败时降级到 BM25 + dense + multimodal；BM25 失败时降级到 dense + multimodal + graph。

图扩展必须有 `fan_out_limit`、`hop_limit`、`relation_count` eviction。

---

## 12. Agent Harness 升级方向（已回撤）

> **状态：已回撤** (commit `f8407c1`)
> **原因**：产品定位调整，核心聚焦知识库检索+网络检索，Agent 协作由用户自选的 Claude Code/Hermes 提供。

原设计的三项升级（真 tool-use 循环、滑动窗口、Skill 按需加载）曾记录于 `2026-05-12-agent-harness-upgrades.md`，现已废弃。

**保留的参考结论**：
- Skill 两层加载机制（目录+按需）可用于优化当前 system prompt 体积
- 三层滑动窗口可用于替换当前 `session_summary` 单点摘要
- 共享 context 块缓存策略可用于减少 Plan+Answer 模式的 token 重复

**废弃的内容**：AgentLoop、AgentToolRegistry、多 Agent 协作、后台任务管理。

---

## 13. 迁移验收状态（2026-04-30 Kilo 报告）

综合评分：**84/100**。合并就绪度：**76/100**。不建议原样合并。

| 模块 | 评分 | 关键完成项 | 剩余 Gap |
|------|------|-----------|----------|
| Chat/general 主路径 | 92/100 | streaming/non-streaming 均走 UnifiedAgentService；`general` → `chat` alias；真 stream | `TASK_MEMORY_MODE` 仍需 cleanup |
| WebSearch Brave 主路径 | 83/100 | 默认 provider 改 Brave LLM Context；callback await 修成 channel bridge；answer synthesis streaming | 缺 live Brave LLM Context smoke；fallback 语义需明确标识 |
| RAG plan v2 core | 86/100 | schema guard、original query text dense、graph triplets、两 placeholder 已落地；Minsky E2E artifact | `RagAgent` 已完全承载 RAG production path |
| 前后端 contract / streaming | 90/100 | SSE contract、stream event order、frontend progress panel 均通过 | — |
| 安全与 diff hygiene | 72/100 | 无 hardcoded secret；`git diff --check` 通过 | diff 范围过大（44 tracked files、2466+ insertions）；`mineru.rs` 单文件 1028 行改动超出主线范围 |

**建议修复顺序**（来自 `2026-04-30` review plan）：
1. P0：定 auth error contract（`login_required` vs `unauthorized`）
2. P1：拆分 merge 范围（Agent / WebSearch / RAG contract / Frontend / MinerU 五组）
3. P1：✅ `RagAgent` 已完全承载 RAG answer，`main_agent` 模块已删除
4. P2：live Brave smoke 或明确记录为已知 gap
5. P2：最终全量验证 + secret scan

---

## 14. 模型 Provider 矩阵（2026-05-13 修订）

| 用途 | Provider | Model | 配置前缀 | 状态 |
|------|----------|-------|----------|------|
| Agent LLM（Chat / RAG / WebSearch plan+answer+eval） | DeepSeek | `deepseek-v4-pro` | `AGENT_LLM_*` | ✅ 主 agentic 推理 |
| Memory LLM（session summary / user profile） | DeepSeek | `deepseek-v4-flash` | `MEMORY_LLM_*` | ✅ 快速廉价 |
| Ingestion LLM（文档摘要 / 三元组抽取） | DMXAPI | `gemini-3.1-flash-lite-preview` | `INGESTION_LLM_*` | ✅ 超快 |
| Legacy retrieval planner | DashScope | `qwen3.5-flash` | `INTENT_LLM_*` | ⚠️ 仅低层 RAG runtime 使用 |
| 文本 Embedding | DashScope | `text-embedding-v4` | `EMBEDDING_*` | ✅ 1024 维 |
| 多模态 Embedding | DashScope | `qwen3-vl-embedding` | `MM_EMBEDDING_*` | ✅ 1024 维对齐 |
| 多模态 Rerank | DashScope | `qwen3-vl-rerank` | `MM_RERANK_*` | ✅ |
| 文本 Rerank | DashScope | `qwen3-vl-rerank` | `RERANK_*` | ✅ DashScope 化（commit `4dd71a1`） |
| WebSearch provider | Brave | LLM Context | `SEARCH_API_KEY` | ✅ 唯一生产 provider |

> **2026-05-13 变更**：
> - Agent LLM 模型由 `deepseek-v4-flash` 改为 `deepseek-v4-pro`（`config.rs:171` + `.env.example:76`）
> - SEARCH_LLM 独立行已删除：`.env.example:103` 显式标注 "SEARCH_LLM is deprecated — all search-agent LLM calls now use AGENT_LLM_*"
> - 文本 Rerank 由 SiliconFlow `Qwen3-Reranker-8B` 切到 DashScope `qwen3-vl-rerank`（DashScope-only profile）
> - Perplexity 已不再在 `.env.example` 出现，`SEARCH_PROVIDER=brave_llm_context` 锁定
>
> **残留代码（不影响主路径）**：
> - `crates/llm/src/lib.rs:81-84`：`provider name` 推断 match 仍含 `siliconflow` / `perplexity` 分支
> - `crates/search/src/{types.rs,tests_impl.rs}`：注释和测试中提及 Perplexity
> 建议跟随后续清理一并删除。

---

## 15. 前端状态

### 15.1 当前前端（frontend_next）

- Next.js/React/TypeScript 栈
- SSE streaming parser 支持 `activity` / `trace` / `token` / `reasoning_summary_delta`
- Progress panel：RAG/Search 模式下显示业务阶段进度，Chat 模式隐藏
- Citation 渲染：RAG 和 WebSearch 显示正文可见引用；Chat 不显示
- `general` → `chat` alias 已收敛

### 15.2 V6 Frontend Dev Plan（已存档）

`DEV_PLAN_V6_FRONTEND.md` 规划从 React/Next.js 迁移到 Rust (Leptos)。**已确认：Leptos 方案不再推进**，仅作存档保留。

当前生产前端 **Next.js/React** 为设计目标，无迁移计划。

---

## 16. E2E 验证状态

### 16.1 Minsky86 RAG E2E（2026-04-28）

**状态**：✅ 完整链路通过。

- 样本：`/mnt/e/Download/minsky86.pdf`（5.6MB，407 页）
- 文档实际内容：Hyman Minsky《Stabilizing an Unstable Economy》（经济学著作）
- 用户 query：询问 Marvin Minsky《Society of Mind》概念关系
- 系统正确给出 **evidence-boundary 拒答**：识别文档不匹配，说明无法回答

**链路验证**：
- Upload → MinerU OCR batch → IR chunk plan → summary + triplet extraction → text/multimodal/KG indexing → RAG SSE answer → citations
- `text_chunks=881`，`multimodal_chunks=45`，`kg_entities=894`，`kg_relations=640`
- SSE 事件：`start=1, activity=4, answer_start=1, token=183, citations=1, done=1`

**关键发现**：
- Planner 只输出单 semantic query + `query_entities`，无 `bm25_terms` / `graph_hints` / `placeholder_triplets`
- 导致 text dense + multimodal dense 有召回，BM25 + graph-only 为 0
- 这是 planner gap：对关系型问题应更积极输出 graph hints

### 16.2 已知阻塞项

| 阻塞项 | 状态 | 说明 |
|--------|------|------|
| Milvus 本地启动 | ✅ Docker | `scripts/dev-services-up.sh` 不启动 Milvus；实际使用 Docker 启动（已确认） |
| Playwright E2E | ⚠️ | `@playwright/test` 依赖未在 `avrag-rs` 内安装；当前用 API + SSE 手动链路验证替代 |
| Live Brave smoke | ⚠️ | 需要真实 `SEARCH_API_KEY`；unit/fake 测试已通过 |

---

## 17. 文档冲突清单（已解决）

| # | 冲突点 | 旧文档说法 | 新文档说法 | 基准版采用 |
|---|--------|-----------|-----------|-----------|
| 1 | WebSearchAgent 本地 planner | `2026-04-26 §11` 描述完整实现 | `2026-04-27` 标记"完成" | ✅ 已实现（2026-05-12 commit `88532b1`、`b2e4f1f`），Phase 1 Planner + Phase 2 ReAct + 双评估 |
| 2 | GuardPipeline 能力 | `2026-04-27 P1-7` "是壳" | `2026-05-10 P1-1" 含 citation_provability + harmful_content | 以当前代码为准：prompt_leak + pii_scrubber |
| 3 | Prompt 共享加载器 | `2026-04-27` 声称"已建立" | 无后续文档提及 | 以代码为准：`include_str!` 直接加载，**Gap P1** |
| 4 | RagAgent 状态 | `2026-04-27 P0-1` "未接线" | `2026-05-10` "已完成" | ✅ 已完成；`RagAgent` 已独立承载 RAG production path，`main_agent` 模块已删除 |
| 5 | GraphFlow 退场 | `2026-04-27 P2-1` "退场计划缺失" | `2026-05-10` "已完成" | ✅ 已完成 |
| 6 | Guard 语义层 | `2026-05-10 P1-1` 需要 semantic guard | 底部标记"❌ 不修复" | ❌ 不修复（沙盒环境） |
| 7 | 记忆层 | `2026-04-25` 描述工作记忆 | `2026-04-27` "已移除" | 三层模型（L1/L2/L3） |
| 8 | 调度层名称 | `2026-04-23` "Main Agent" | `2026-04-26` "UnifiedAgentService" | UnifiedAgentService |
| 9 | RAG API 边界 | `2026-04-23` "纯工具 backend" | `2026-04-26` "bounded retrieval operators" | 后者（更准确） |
| 10 | Skill 机制 | `2026-04-29 progressive framework` 描述动态 skill registry | `2026-04-28 report` 确认当前是静态 prompt-envelope skill | 当前为静态 `MainAgentBehaviorSkill`；动态 skill registry 是 **Agent Harness 升级 3**（P2） |
| 11 | 三 agent 迁移顺序 | `2026-04-29` 计划 Phase 0→7 严格顺序 | `2026-04-30` Kilo 报告实际并行推进多模块 | 以 Kilo 实际验收状态为准（Chat 92、WebSearch 83、RAG 86） |
| 12 | GraphFlow 角色 | `2026-04-29` 计划明确保留 GraphFlow 作为编排内核 | `2026-04-30` agent gap 文档确认 GraphFlow 负责 rails | ✅ 保留，已写入 §10 |
| 13 | Milvus 检索数据面 | `2026-04-26` 目标架构 | `2026-04-26-rag-milvus-graph-plan` Phase 0-6 已代码实现 | ✅ 已实现，live smoke pending |
| 14 | Redis 分工（2026-05-13） | 既往文档把 `cache-redis` 列在 app 路径 | 实际：`cache-redis::DocumentLock` 仅 worker 使用，app 用 `redis` crate 直连做限流 | 已在 §2 补充分工说明 |
| 15 | LoopBudget 数值（2026-05-13） | `agents/react_loop.rs:6` 注释写 RAG=3/Search=2 | 实际：Free RAG=2/Search=1/Chat=1；Pro/Enterprise RAG=4/Search=3/Chat=3 | 注释已修正 commit b67c803 后续修订 |
| 16 | Search output guard（2026-05-13） | search 模式 `apply_output_guard:false`，外网 snippet 进 LLM 不过 guard | search 模式启用 `prompt_leak + pii_scrubber`；chat 模式保留 false（无外部数据） | 已在 §9.4 写入矩阵 |

---

## 18. 已确认项

1. **WebSearchAgent 本地 planner**：**已实现**（2026-05-12 commit `88532b1`、`b2e4f1f`）。`prompts/web_search_plan_system.txt` 为当前生产 prompt，内容由 `search_plan_system_legacy.txt` 直接重命名而来（commit `ce1fa96`，两文件 byte-identical）。设计架构：Phase 1 Local Planner（LLM 生成 sub_queries + 双评估）→ Phase 2 ReAct loop（兜底）。
2. **`rag_planner_system.txt`**：`RetrievalPlanner` 组件已标记为 legacy，由 `RagAgent` 取代。提示词文件和对应代码保留到 RagAgent 完全稳定后删除。注：`crates/guardrails/src/output/prompt_leak.rs:62` 通过 `include_str!` 引用此文件作为系统提示词泄露检测的指纹，删除前需迁移该引用。
3. **`SEARCH_PLANNER_ENABLED` 配置**：**已清理 ✅**。`grep -rn "SEARCH_PLANNER_ENABLED"` 在仓库内零命中。
4. **Guard 语义层**：❌ 不修复。沙盒环境规则层已足够。
5. **Prompt 管理 infra**：❌ 不修复。等上线后根据实际更新频率再评估。
6. **`load_prompt_template()` 共享加载器**：不存在于代码中。`include_str!` 为当前实际加载方式。
7. **Semantic memory vectors**：❌ 不修复。长期画像全存结构化 JSON，不向量化。
8. **MainAgent 已删除**：`main_agent` 模块已从代码库完全移除（grep 零命中）。Chat/general 主路径和 RAG production answer 分别由 `ChatAgent` 和 `RagAgent` 独立承载。
9. **MinerU OCR batching**：`2026-04-28` 已实现 batch upload + blank page skip + low-value skip。不是 agent migration 主线，建议单独提交。
10. **Brave LLM Context 主路径**：Search provider 默认已切 Brave，`perplexity` 作为 legacy provider 保留但非默认。
11. **Auth error contract**：`login_required` vs `unauthorized` 语义边界待核验（middleware nest 路径匹配失败时可能混淆）。
12. **三 agent 迁移不做 legacy/Rig 双路径**：`2026-04-29` 计划明确不做 feature flag 灰度，直接切新架构。唯一兼容：`general` → `chat` alias。
13. **Cleanup backlog（2026-05-13 新增）**：以下为已知死代码 / 死文件，不影响运行时行为，待统一清理：
    - `crates/guardrails/src/canary.rs`、`crates/guardrails/src/sysvec.rs`（G2 已取消，文件和 `pub mod` 声明残留）
    - `crates/app/src/chat/pipeline.rs:42` 的 `canary_token: Option<String>` 字段及 `service_modes.rs` 3 处 `None` 塞入
    - `prompts/web_search_plan_system_new.txt`（24 行短版本，无代码引用，commit `88532b1` 实施期间的中间产物）
    - `crates/llm/src/lib.rs:81-84`：`provider name` URL 推断 match 中残留 `siliconflow` / `perplexity` 分支
    - `crates/search/src/types.rs:8` 注释 + `crates/search/src/tests_impl.rs:36-93` 测试中对 Perplexity 的提及

---

## 19. 参考文档索引

### 当前架构真相源（按优先级）

| 文档 | 状态 | 说明 |
|------|------|------|
| `2026-05-12-architecture-baseline.md` | ✅ 当前 | 本文档，取代所有更早文档 |
| `2026-05-12-agent-harness-upgrades.md` | 📝 草案待审 | Agent 层三项升级设计 |
| `2026-04-26-current-product-rag-architecture.md` | ⚠️ 部分过时 | 架构方向仍有效，术语和细节被基准版取代 |

### 历史文档（已加 deprecation banner）

| 文档 | 状态 | 取代原因 |
|------|------|----------|
| `2026-04-27-codebase-gap-review.md` | ❌ 已删除 | 被 `2026-05-10` + 基准版取代，已清理 |
| `2026-04-25-main-agent-memory-and-context-design.md` | ⚠️ 部分过时 | 工作记忆层已移除，被三层模型取代 |
| `2026-04-23-main-agent-and-rag-tool-backend-design.md` | ⚠️ 术语过时 | "Main Agent" 术语被 "UnifiedAgentService" 取代 |
| `2026-04-23-rag-tool-backend-and-agent-control-discussion.md` | ⚠️ 已被取代 | 讨论结论已整合进 `2026-04-26` §4 |

### 实施计划（历史参考）

| 文档 | 状态 | 说明 |
|------|------|------|
| `2026-04-29-three-agent-rig-migration-implementation-plan.md` | 📝 设计记录 | 8 阶段迁移计划，子任务在 `.hermes/plans/2026-04-29_184541-three-agent-rig-subtasks/` |
| `2026-04-29-three-agent-migration-plan.md` | 📝 设计记录 | 13 项已确认业务决策 |
| `2026-04-29-progressive-agent-framework.md` | 📝 设计记录 | 三 agent 渐进式设计框架 |
| `2026-04-26-rag-milvus-graph-implementation-plan.md` | 📝 Phase 0-6 已实现 | Milvus + graph 迁移，live smoke pending |
| `2026-04-23-main-agent-and-rag-tool-backend-implementation-plan.md` | 📝 大部分已实现 | 5 阶段边界收缩计划 |

### 审计/验收报告

| 文档 | 状态 | 说明 |
|------|------|------|
| `2026-04-30-kilo-agent-migration-acceptance.md` | ✅ 验收记录 | 评分 84/100，合并就绪度 76/100 |
| `2026-04-30-minsky-rag-e2e-agent-mechanisms.md` | ✅ E2E 记录 | Minsky 完整链路通过，evidence boundary 正确 |
| `2026-04-28-rag-e2e-minsky86-live-run.md` | ✅ E2E 记录 | Live run 详细时间线 |
| `2026-04-28-model-api-streaming-audit.md` | ✅ 审计记录 | DeepSeek 切换 + streaming 诊断 |
| `2026-04-28-main-agent-prompts-skills-flow.md` | ✅ 代码审计 | 当前 MainAgent skill 是静态 prompt-envelope 机制 |

### PRD / 开发计划

| 文档 | 状态 | 说明 |
|------|------|------|
| `DEV_PLAN.md` | ⚠️ 历史快照 | Phases 0-3 ✅，Phase 4 前端在 context-osv5 |
| `DEV_PLAN_V6_FRONTEND.md` | ❌ 已删除 | Leptos 方案不再推进，仅存档后已清理 |
| `GAP_ANALYSIS.md` | ⚠️ 历史快照 | 2026-03-20 PRD 对照，~95% 完成 |

---

> 本文档取代以下文档中的冲突内容：
> - `2026-04-27-codebase-gap-review.md`（全部，已过期）
> - `2026-04-26-current-product-rag-architecture.md`（部分章节需更新）
> - `2026-04-25-main-agent-memory-and-context-design.md`（工作记忆部分已过时）
> - `2026-04-23-main-agent-and-rag-tool-backend-design.md`（Main Agent 术语已过时）
> - `.hermes/plans/2026-04-29_*` 中的架构方向（已整合为基准版 §1、§7、§10、§13）
> - `.hermes/reports/2026-04-28_*` 和 `2026-04-30_*` 中的验收结论（已整合为基准版 §13-§16）
