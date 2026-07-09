# ADR-0010: 移除服务端 Query Normalization，指代消解回归 LLM

| 项目 | 内容 |
|---|---|
| 状态 | **已采纳** |
| 决策日期 | 2026-06-30 |
| 取代 | ADR-0008 §3（Query Normalization / Pre-Loop 服务端消解） |
| 关联 | ADR-0008（部分废止）、ADR-0007（RAG codegen 唯一检索入口） |

---

## 1. 背景

ADR-0008 §3 引入了 Pre-Loop 服务端 query normalization：在 ReAct loop 启动前，服务端用启发式 + LLM fallback 把含代词/省略的 follow-up query 消解成自包含的 `resolved_query`，写入 PG `chat_messages.resolved_query` 列，下游检索/fallback 读 `resolved_query`，messages 仍展示原话。

实现包括：
- `query_normalize.rs`：classify_self_contained / resolve_with_heuristic / resolve_with_llm
- `AgentRequest.resolved_query` + `query_resolution: Option<QueryResolutionMeta>`
- `AgentRunResult.query_resolution`
- `modes/rag.yaml` 的 `query_normalization.{enabled, max_prior_turns, llm_fallback}` 配置
- `AgentEvent::QueryResolved` SSE 事件
- PG `chat_messages.resolved_query` 列 + `turn_metadata.query_resolution`
- `ChatTurnInput.resolved_query` / `ChatMessage.resolved_query` 字段

## 2. 决策

**移除服务端 query normalization。LLM 自己负责指代消解。**

具体改动：
1. 删除 `query_normalize.rs` 模块、`QueryResolutionMeta` 类型、`NormalizeResult` 类型
2. 删除 `AgentRequest.resolved_query` / `query_resolution` 字段及 `effective_query()` / `with_resolved_query()` / `ensure_resolved_query_defaults()` 方法
3. 删除 `AgentRunResult.query_resolution` 字段
4. 删除 `modes/rag.yaml` 的 `query_normalization` 配置块和 `QueryNormalizationConfig` 类型
5. 删除 `AgentEvent::QueryResolved` 事件
6. `disclosure_plan` 的 `RetrievalQuery` slice 改用 `request.query`（原话）
7. `auto_fallback` 改用 `request.query`
8. `memory/SKILL.md` + `reference/anaphora.md` 重写：明确告诉 LLM "你负责指代消解，看到代词/省略主动调 `conversation_history_load`"
9. `rag-system.md` 更新：`Retrieval query:` = 用户原话，不再有服务端消解

**保留（向后兼容）**：
- PG `chat_messages.resolved_query` 列：保留可读老数据，但代码 insert 时传 NULL；后续单独 migration 删列
- `ChatTurnInput.resolved_query` / `ChatMessage.resolved_query` 字段：`#[serde(default)]`，老客户端/老数据反序列化兼容
- `build_user_message_search_tokens(content, resolved_query: Option<&str>)` 签名保留，调用方传 `None`

## 3. 动机

### 3.1 架构简化

Pre-Loop normalization 是 ReAct loop 之外的一层额外 LLM 调用（`llm_fallback` 路径），增加了：
- 一次额外 LLM 调用成本（每个 follow-up turn）
- 一个独立的状态机分支（clarify_answer 提前终止）
- 一套独立的配置（`max_prior_turns`、`llm_fallback`）
- PG schema 一等列 + metadata 双写

LLM 现在已经能在 ReAct loop 内通过 `memory` 簇的 `conversation_history_load` 主动拉取更早历史做消解，Pre-Loop 这层"代消解"成了冗余。

### 3.2 灵活性

Pre-Loop 固定看 6 条 prior turn。LLM 自己调 history 可以按需决定拉多少条、用什么 query 词，对长对话场景更灵活。

### 3.3 职责归位

"理解用户指代"本质是 LLM 的语言理解能力，不该由服务端启发式 + 一次性 LLM 调用预判。把它交给 ReAct loop 内的 LLM，让指代消解与检索决策在同一上下文里完成，更内聚。

## 4. 风险与缓解

### 4.1 LLM 偷懒不调 memory

**风险**：LLM 可能不调 `conversation_history_load` 直接用代词 query 检索，导致多轮退化回单轮（ADR-0008 §1.1 当年要堵的漏洞）。

**缓解**：
- `memory/SKILL.md` 明确指令"看到代词/省略主动调"
- `rag-system.md` 注明"服务端不再消解，你负责"
- `llm_real/multi_turn.rs` 保留"turn2 答案包含 taleb"断言作为回归门
- nightly 跑 realistic_corpus 监控多轮质量

### 4.2 mock LLM 测试无法模拟主动调 memory

**风险**：依赖 mock LLM 的多轮测试可能因 mock 不调 memory 而失败。

**缓解**：现有 `memory_multiturn_smoke` 用 `set_mock_emit_memory_tool(Some("conversation_history_load"))` 强制注入 memory 调用，不依赖 LLM 自主决策。已删除的 `multiturn_anaphora_writes_resolved_query_to_db` 测试是测已删功能，删除合理。

### 4.3 PG 老数据兼容

**风险**：删列会导致老数据丢失。

**缓解**：保留 `resolved_query` 列为 nullable，代码不再写但 select 仍读（`errors_and_mappers.rs` 仍映射到 `ChatMessage.resolved_query` 字段）。后续单独 migration 删列，与代码改动解耦。

## 5. 实施清单

- [x] 删 `query_normalize.rs` + `QueryResolutionMeta` + `NormalizeResult`
- [x] 删 `AgentRequest.{resolved_query, query_resolution}` + 相关方法
- [x] 删 `AgentRunResult.query_resolution`
- [x] 删 `QueryNormalizationConfig` + `modes/rag.yaml` 配置块
- [x] 删 `AgentEvent::QueryResolved` + sse_sink 分支
- [x] `disclosure_plan` RetrievalQuery 用 `request.query`
- [x] `run_prepare` 删 norm 参数，`loop_user_query` 用 `request.query`
- [x] `service_postprocess` user_resolved_query / user_turn_metadata 改 None
- [x] `ChatExecution.query_resolution` 字段删除
- [x] `agent_runtime` ChatTurnInput 构造传 `resolved_query: None`
- [x] `memory/SKILL.md` + `reference/anaphora.md` 重写
- [x] `rag-system.md` 更新 Retrieval query 说明
- [x] 删 `multiturn_anaphora_writes_resolved_query_to_db` 测试
- [x] 改 `multi_turn.rs` llm_real 测试（删 resolved_query DB 断言，保留 taleb 断言）
- [x] ADR-0008 标部分废止
- [ ] PG migration 删 `chat_messages.resolved_query` 列（后续单独 PR）

## 6. 验证

- `cargo check --workspace --tests`：通过
- `cargo test -p app-chat --lib loop::`：95 个单测全过
- `cargo check -p app --features product-e2e --tests`：通过
