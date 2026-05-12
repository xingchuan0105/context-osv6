# Three-Agent Rig Migration Implementation Plan

> **For Hermes:** 本计划是迁移实施计划，不在本步骤写业务代码。后续执行时按阶段拆任务，并在涉及共享/导出符号或跨模块行为时先做影响检查。

**Goal:** 把当前混合式 `MainAgent` 架构迁移为 `ChatAgent` / `WebSearchAgent` / `RagAgent` 三个显式模式 agent，并用 Rig 作为唯一新的 model/agent streaming runtime，支撑半真流式、可见 reasoning summary、progress、debug trace、citations 和 usage 事件。

**Architecture:** 保留 Rust GraphFlow 作为产品编排 kernel；不重造 workflow 框架。GraphFlow 负责 preflight/session/deterministic dispatch/agent run/output guard/persist/usage/notify/response。Rig 放在 model/agent streaming layer。Axum SSE 继续是唯一 HTTP 传输层。三 agent 共享一套 sink-aware agent service，不保留 legacy/Rig 双路径，不实现无 Rig 的第二套半真流式。

**Tech Stack:** Rust, Axum SSE, graph-flow, rig-core 0.36.x, PostgreSQL, Milvus, BM25, multimodal retrieval, Brave LLM Context, Next.js/React/TypeScript frontend.

---

## Subtask plans

This implementation plan has been split into bounded execution subtask plans under:

`/home/chuan/context-osv6/avrag-rs/.hermes/plans/2026-04-29_184541-three-agent-rig-subtasks/`

Start with `README.md`. Each subtask includes scope, file boundaries, verification commands, and explicit legacy-code anti-contamination rules so implementers do not mistake old `MainAgent` / Perplexity / buffered streaming code for target design.

---

## 0. 已确认业务决策

1. 模式选择：用户在前端显式选择 `rag` / `chat` / `search`；后端不自动猜测模式。
2. 证据边界：
   - `RagAgent` 只使用选中文档和检索 chunk 作为事实证据。
   - `ChatAgent` 使用模型通用知识、会话上下文和用户偏好，不伪装成读过文档或搜过网页。
   - `WebSearchAgent` 只使用 Brave evidence 作为外部事实证据。
3. RAG 引用：RAG 正文必须有可见引用；证据不足时说明不足、给最接近证据和缺口。
4. RAG plan：planner 不决定检索通道开关，只输出 `rewrite_queries` / `bm25_keywords` / `triplets`；原始 query 永远进入 embedding 检索。
5. RAG 检索：text vector 和 multimodal vector 固定运行；BM25 和 graph 在对应 plan 项非空时按预算运行。
6. WebSearch：Brave LLM Context 是主路径；Brave 是 evidence provider，不用 Brave Answers 黑盒；不默认跑普通 web search。
7. Streaming：单一 SSE 连接承载 answer、reasoning summary、progress/status、debug trace、citations、done/error。
8. Debug：计划、检索通道 chunk、source payload 等详细 debug artifact 不长期落业务库，写日志/run artifact，并有清理策略。
9. Runtime：全切新架构。不开 legacy/Rig feature flag，不保留两套 runtime，不重复实现无 Rig 半真流式。
10. 兼容边界：只保留必要 API/协议兼容，例如短期接受 `general` 作为 `chat` alias，方便旧测试/旧数据迁移。

---

## 1. 成功标准

### 1.1 产品成功标准

- 前端用户明确选择三种模式之一，后端确定性分发到对应 agent。
- Chat 模式不显示 citations；RAG 和 WebSearch 模式显示正文可见 citations。
- RAG debug 能展示：plan schema、text vector chunks、multimodal chunks、BM25 chunks、graph chunks、merged/reranked evidence。
- WebSearch debug 能展示：Brave LLM Context 查询、返回 source、最终引用映射。
- Streaming 页面能看到：高层 progress、可折叠 reasoning summary、答案增量、引用、完成事件。
- Non-stream 和 stream 调用同一套 agent service，不再各自维护业务路径。

### 1.2 工程成功标准

- `MainAgent` 不再是业务路由/plan/answer 的中心概念。
- `ModeSelectTask` 不再调用 `MainAgent::decide`。
- `RagAgent` / `ChatAgent` / `WebSearchAgent` 有清晰接口和测试边界。
- Rig stream items 被统一映射成内部 `AgentEvent`，再由 coalescer 转成 `ChatEvent` SSE frame。
- 现有 GraphFlow rails 继续承接 preflight、session、persist、usage、notify。
- 没有 legacy/Rig runtime switch；没有两套半真流式实现。

### 1.3 验证成功标准

每个 agent 都至少通过：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app chat_agent
cargo test -p app websearch_agent
cargo test -p app rag_agent
cargo test -p transport-http chat_stream
cargo test -p app graphflow
```

前端至少通过：

```bash
cd /home/chuan/context-osv6/frontend_next
pnpm typecheck
pnpm test
```

E2E 至少覆盖：

```text
/api/v1/chat           non-stream: chat / search / rag
/api/v1/chat/stream    stream:     chat / search / rag
RAG Minsky 文件 query -> plan -> retrieval chunks -> answer citations
WebSearch query -> Brave LLM Context evidence -> cited answer
Chat boundary query -> boundary statement, no fake citation
```

---

## 2. 非目标

本轮不做：

- 不重写 GraphFlow，不引入新的 workflow runtime 替代它。
- 不让 Rig 自主选择产品模式、doc scope 或 retrieval topology。
- 不把 RAG retrieval/search executor 暴露成 Rig 自由工具。
- 不做双路径灰度、legacy/Rig runtime switch。
- 不在业务库长期保存完整 plan/retrieval/source debug blob。
- 不重做 ingestion pipeline、worker queue 或 Milvus schema，除非 RagAgent 迁移中发现已有索引能力缺失。
- 不默认跑 Brave web search + LLM Context 双请求。

---

## 3. 目标模块边界

```text
HTTP layer
  crates/transport-http/src/handlers.rs
    - 只负责请求解析、auth/context 注入、SSE response 包装
    - 不包含 agent 业务判断

Application orchestration layer
  crates/app/src/chat/graphflow.rs
  crates/app/src/chat/service.rs
    - 保留 GraphFlow rails
    - deterministic dispatch: agent_type -> concrete agent
    - stream/non-stream 都调用统一 agent service

Agent runtime layer
  crates/app/src/agents/mod.rs                  new
  crates/app/src/agents/chat.rs                 new
  crates/app/src/agents/websearch.rs            new
  crates/app/src/agents/rag.rs                  new
  crates/app/src/agents/events.rs               new
  crates/app/src/agents/rig_adapter.rs          new or under llm crate
    - 三 agent 的统一接口
    - Rig stream item -> AgentEvent 映射
    - progress/reasoning/message/debug/citation/usage 事件

LLM / model provider layer
  crates/llm/src/client.rs
  crates/llm/src/rig_client.rs                  new, if kept in llm crate
    - Rig-backed model calls
    - structured output helpers
    - streaming helpers

RAG layer
  crates/common/src/rag_execute.rs
  crates/app/src/lib_impl/rag_execute.rs
  crates/app/src/chat/graphflow_tasks_rag.rs
  crates/rag-core/src/runtime/retrieval.rs
  crates/rag-core/src/runtime/execute.rs
    - plan schema v2
    - deterministic retrieval execution
    - debug chunk capture by channel

Search layer
  crates/search/src/config.rs
  crates/search/src/executor.rs
    - Brave LLM Context as primary WebSearch evidence path
    - Perplexity path retired from agent execution

Contracts
  contracts/src/chat.rs
    - ChatEvent 增加 reasoning_summary_delta / debug payload 类型
    - 保持 Start/Activity/AnswerStart/Token/Citations/Done/Error 基础语义

Frontend
  frontend_next
    - mode value `chat` / `rag` / `search`
    - progress panel
    - reasoning summary panel
    - citations/source panel
    - debug trace panel
```

---

## 4. 目标事件模型

### 4.1 内部 AgentEvent

建议先定义内部事件，不直接把 Rig 类型泄漏到 HTTP contract：

```text
AgentEvent::Activity { stage, message }
AgentEvent::ReasoningSummaryDelta { text }
AgentEvent::MessageDelta { text }
AgentEvent::DebugTrace { kind, payload }
AgentEvent::Citations { citations }
AgentEvent::Usage { provider, model, tokens, request_count, metadata }
AgentEvent::Done { final_message, usage }
AgentEvent::Error { code, message }
```

### 4.2 SSE ChatEvent

`AgentEvent` 经 coalescer 映射到现有 `ChatEvent`：

```text
Start
Activity
AnswerStart
ReasoningSummaryDelta   new first-class event
Token                   or MessageDelta, depending current contract naming
Trace                   debug-gated only
Citations
Done
Error
```

规则：

- `reasoning_summary_delta` 做一等事件，不长期塞进 `trace`。
- `trace/debug` 只在 debug flag 开启时发。
- coalescer 可以按时间/字符数合并 token 和 reasoning summary，但不能等 final 完成后再切块。
- 如果 provider 不支持真 streaming，必须在 `degrade_trace` 标注 buffered fallback。

---

## 5. 阶段实施计划

### Phase 0: Contract and architecture prep

**Objective:** 先把公共契约、事件类型、agent 接口和文件边界定下来，不改业务行为。

**Files:**
- Modify: `contracts/src/chat.rs`
- Create: `crates/app/src/agents/mod.rs`
- Create: `crates/app/src/agents/events.rs`
- Create: `crates/app/src/agents/runtime.rs`
- Modify: `crates/app/src/lib.rs` or module export file, according to current crate layout

**Tasks:**

1. 定义 `AgentKind`：`Chat` / `Rag` / `Search`。
2. 增加解析规则：`general` 暂时 alias 到 `Chat`，但内部新代码统一用 `Chat`。
3. 定义 `AgentRequest`，字段包含 auth/session/query/messages/doc_scope/debug/stream sink。
4. 定义 `AgentRunResult`，字段包含 final answer/citations/usage/debug summary/degrade trace。
5. 定义 `AgentEvent` 和 `AgentEventSink`。
6. 在 `contracts/src/chat.rs` 增加 `reasoning_summary_delta` 对应事件。
7. 添加 contract tests：事件序列可以序列化/反序列化，旧事件不破坏。

**Verification:**

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p contracts chat
cargo test -p app agents
```

Expected:
- `general` -> `Chat` alias 测试通过。
- 新 SSE event contract 测试通过。

---

### Phase 1: Rig adapter as the only model streaming runtime

**Objective:** 建 Rig adapter，把 Rig typed stream 映射成内部 `AgentEvent`，作为唯一新 streaming runtime。

**Files:**
- Create: `crates/llm/src/rig_client.rs` or `crates/app/src/agents/rig_adapter.rs`
- Modify: `crates/llm/src/lib.rs`
- Modify: `crates/app/src/lib_impl/config.rs`
- Modify: `Cargo.toml` / relevant crate `Cargo.toml`

**Tasks:**

1. 引入当前版本 `rig-core`，不要启用 `graph-flow` 的旧 `rig` feature。
2. 实现最小 `RigModelClient`：
   - non-stream complete
   - stream complete
   - structured JSON output helper, if Rig supports the provider path cleanly
3. 实现 Rig stream item 到 `AgentEvent` 的映射：
   - message delta -> `MessageDelta`
   - reasoning delta/summary -> `ReasoningSummaryDelta`
   - tool status/delta -> `DebugTrace` or `Activity`, depending product visibility
   - final response -> `Done`
   - usage -> `Usage`
4. 保留现有 provider env/config 名称，避免配置扩散。
5. 写 fake Rig stream 单元测试，不依赖真实 API。

**Verification:**

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p llm rig
cargo test -p app rig_adapter
```

Expected:
- fake stream 能按顺序输出 activity/message/reasoning/done。
- provider 不支持 reasoning 时不会失败，只是不发 reasoning event。

---

### Phase 2: Unified stream/non-stream agent service

**Objective:** 先解决最关键的双路径问题：stream 和 non-stream 调用同一套 agent service，只是 sink 不同。

**Files:**
- Modify: `crates/app/src/chat/service.rs`
- Modify: `crates/app/src/lib_impl/chat_streaming.rs`
- Modify: `crates/app/src/chat/graphflow.rs`
- Modify: `crates/app/src/chat/graphflow_tasks_core.rs`
- Modify: `crates/transport-http/src/handlers.rs`

**Tasks:**

1. 建立 `run_agent(request, sink)` 统一入口。
2. Non-stream path 使用 collecting sink：收集/忽略中间事件，返回 final response。
3. Stream path 使用 SSE sink：即时发送 `ChatEvent`。
4. GraphFlow 的 agent run 节点只调用统一入口，不直接调用 `MainAgent`。
5. 移除或隔离 `chunk_text_for_stream` 主路径；只保留为明确 degraded fallback。
6. 给 stream path 加事件顺序测试：`start -> activity -> answer_start -> token/message_delta -> done`。

**Verification:**

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app chat_service
cargo test -p transport-http chat_stream_contract
```

Expected:
- stream/non-stream 结果一致。
- stream 不等待 final 才发 token。
- fallback 被标注为 degrade，不是默认路径。

---

### Phase 3: ChatAgent migration

**Objective:** 最小风险先迁 ChatAgent，替代当前 `MainAgent::answer_general_stream` / general chat 路径。

**Files:**
- Create: `crates/app/src/agents/chat.rs`
- Modify: `crates/app/src/main_agent/mod.rs` only to remove/stop usage after callers are migrated
- Modify: `crates/app/src/chat/service_modes.rs`
- Modify: `crates/app/src/chat/graphflow.rs`
- Tests: add/modify `crates/app/tests/*chat*` or existing app test module

**Tasks:**

1. 实现 `ChatAgent` 接口。
2. 复用当前 prompt/memory/session context，但输出通过 Rig adapter。
3. 加边界判断：明显需要文档证据 -> 提示切 RAG；明显需要实时外网 -> 提示切 WebSearch。
4. ChatAgent 不生成 citations。
5. 更新 routing：`agent_type=chat|general` -> ChatAgent。
6. 删除 Chat 路径对 `MainAgent::decide` 的依赖。
7. 单测覆盖：普通 chat、doc-boundary、web-boundary、no citations、stream events。

**Verification:**

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app chat_agent
cargo test -p app graphflow_chat
cargo test -p transport-http chat_stream_contract
```

Manual API:

```text
POST /api/v1/chat agent_type=chat
POST /api/v1/chat/stream agent_type=chat
```

Expected:
- 无 citation。
- 有 progress + answer stream。
- boundary query 不泛答、不假装检索。

---

### Phase 4: WebSearchAgent migration with Brave LLM Context

**Objective:** 用 WebSearchAgent 替代 Perplexity/旧 search agent 路径，Brave LLM Context 做主 evidence path，Rig 做 synthesis streaming。

**Files:**
- Create: `crates/app/src/agents/websearch.rs`
- Modify: `crates/search/src/config.rs`
- Modify: `crates/search/src/executor.rs`
- Modify: `crates/app/src/lib_impl/config.rs`
- Modify: `crates/app/src/lib_impl/state_methods.rs`
- Modify: `crates/app/src/chat/service_modes.rs`
- Tests: search executor + websearch agent tests

**Tasks:**

1. 增加/确认 Brave LLM Context config，API key 只在 server env。
2. 定义 `websearch-plan-v1`：
   - `context_queries`
   - `news_queries`
   - `freshness`
   - `country`
   - `search_lang`
3. 执行规则：原始 query 永远进入 Brave LLM Context。
4. 只有 planner 判断需要新闻时才跑 news。
5. 不默认跑 web search。
6. 将 Brave evidence 标准化为 source/citation model。
7. 用 Rig synthesis 生成带可见引用的答案。
8. 记录 usage：Brave request count + answer LLM token usage。
9. Debug trace 包含 context queries、source ids、source snippets/context metadata，但不泄露 API key。

**Verification:**

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p search brave
cargo test -p app websearch_agent
cargo test -p transport-http chat_stream_contract
```

Manual API:

```text
POST /api/v1/chat agent_type=search
POST /api/v1/chat/stream agent_type=search
```

Expected:
- 答案正文有来源引用。
- Debug 能看到 Brave LLM Context evidence。
- 没有 Perplexity 黑盒答案路径。
- 普通 web search 不被默认调用。

---

### Phase 5: RagAgent plan schema v2 and deterministic retrieval

**Objective:** 最后迁 RAG，因为它影响最大：plan schema、text vector、multimodal、BM25、graph、citation validation、debug artifact。

**Files:**
- Create: `crates/app/src/agents/rag.rs`
- Modify: `crates/common/src/rag_execute.rs`
- Modify: `crates/app/src/lib_impl/rag_execute.rs`
- Modify: `crates/app/src/chat/graphflow_tasks_rag.rs`
- Modify: `crates/rag-core/src/runtime/retrieval.rs`
- Modify: `crates/rag-core/src/runtime/execute.rs`
- Modify: `crates/app/src/chat/service_modes.rs`
- Tests: rag execute / retrieval / citation / Minsky E2E scripts

**Tasks:**

1. 定义 `rag-plan-v2`：

```json
{
  "plan_version": "rag-plan-v2",
  "query_language": "en|zh|mixed|unknown",
  "doc_scope": ["server-controlled-document-id"],
  "rewrite_queries": ["optional semantic rewrite"],
  "bm25_keywords": ["optional lexical term"],
  "triplets": [
    {
      "subject": "entity or placeholder",
      "predicate": "relationship or placeholder",
      "object": "entity or placeholder"
    }
  ],
  "summary_mode": "none|document|section"
}
```

2. 强制 server-side doc scope：planner 输出不能扩大 doc scope。
3. Semantic retrieval inputs：原始 query always + rewrite queries optional。
4. Multimodal retrieval always runs。
5. BM25：`bm25_keywords` 非空时运行，按文档语言选择关键词。
6. Graph：`triplets` 非空时运行，允许最多两个 placeholder。
7. Debug trace 按通道记录：
   - text vector returned chunks
   - multimodal returned chunks
   - BM25 returned chunks
   - graph returned chunks
   - merged/reranked chunks
8. Answer synthesis 使用 Rig，正文引用必须可见。
9. Citation validation 仍由服务端验证，防止引用不存在 chunk。
10. 证据不足时输出：不足说明 + 最接近证据 + 缺口。

**Verification:**

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app rag_agent
cargo test -p app rag_execute
cargo test -p app citation
cargo test -p avrag-storage-pg ingestion
```

Minsky E2E:

```bash
cd /home/chuan/context-osv6/avrag-rs
python .hermes/scripts/continue_minsky_agent_mechanisms.py
```

Expected:
- 输出 plan schema v2。
- 能分通道记录 text vector / multimodal / BM25 / graph chunks。
- 能解释各 chunk 与 query 的关系。
- 答案有 citation。
- 无证据时不硬编。

---

### Phase 6: Remove old MainAgent path

**Objective:** 三个 agent 均通过 gates 后，删除或降级旧 `MainAgent` 混合职责，避免双路径继续存在。

**Files:**
- Modify/delete: `crates/app/src/main_agent/mod.rs`
- Modify: all callers found by source search
- Modify: tests that still assert `general` as primary mode

**Tasks:**

1. 搜索所有 `MainAgent` 调用点。
2. 删除 `MainAgent::decide` 的业务调用。
3. 删除 general/search/rag answer 函数的直接使用。
4. 如果某些 schema/prompt 类型仍有复用价值，移动到对应 concrete agent 模块，避免保留 `main_agent` 概念。
5. 更新 trace/source names：不要再把 `main_agent` 作为业务事件来源。
6. 保留 `general` alias 解析测试，但新测试统一使用 `chat`。

**Verification:**

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app
cargo test -p transport-http
```

Additional check:

```text
search MainAgent references: only historical comments or removed entirely
search main_agent trace source: no product trace depends on it
```

---

### Phase 7: Frontend wiring and QA

**Objective:** 前端把新事件和三模式体验接起来。

**Files:**
- Modify: `frontend_next` chat API client files
- Modify: `frontend_next` SSE event parser files
- Modify: `frontend_next` chat/progress/citation/debug components
- Modify: relevant frontend tests

**Tasks:**

1. Mode values：新 UI 默认发送 `chat` / `rag` / `search`。
2. SSE parser 支持 `reasoning_summary_delta`。
3. Reasoning summary 默认折叠，可展开。
4. Progress panel 映射三 agent 的业务阶段：
   - RAG: 理解问题 -> 制定检索计划 -> 检索文档 -> 整理证据 -> 生成回答
   - WebSearch: 理解问题 -> 制定搜索计划 -> 搜索网页内容 -> 整理来源 -> 生成回答
   - Chat: 理解上下文 -> 生成回答
5. RAG/WebSearch 正文 citation 可见；Chat 不渲染 citation 区块。
6. Debug flag 开启时展示 plan/retrieval/source trace；默认用户不展示 raw debug。

**Verification:**

```bash
cd /home/chuan/context-osv6/frontend_next
pnpm typecheck
pnpm test
```

Manual QA:

```text
chat mode: streaming answer + no citations
rag mode: progress + citations + debug retrieval channels
search mode: Brave sources + citations + reasoning summary
```

---

## 6. 测试矩阵

### Backend unit / contract

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p contracts chat
cargo test -p llm rig
cargo test -p app agents
cargo test -p app graphflow
cargo test -p app rag_execute
cargo test -p search brave
cargo test -p transport-http chat_stream_contract
```

### Backend integration / E2E

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app
cargo test -p transport-http
python .hermes/scripts/continue_minsky_agent_mechanisms.py
```

### Frontend

```bash
cd /home/chuan/context-osv6/frontend_next
pnpm typecheck
pnpm test
pnpm build
```

### Manual checks

1. ChatAgent:
   - 普通问答正常。
   - 要求读取上传文档时，边界声明。
   - 要求实时网页证据时，边界声明。
   - 无 citations。
2. WebSearchAgent:
   - Brave LLM Context 被调用。
   - 来源可见引用。
   - debug 中能看到 source/evidence。
   - 不走 Perplexity 黑盒。
3. RagAgent:
   - Minsky 文档 E2E 成功。
   - plan schema v2 落 debug。
   - text/multimodal/BM25/graph chunks 分通道可见。
   - answer citations 可见且可验证。
4. Streaming:
   - 首个 progress 早于 final。
   - 首个 answer delta 早于 final。
   - reasoning summary 可折叠显示。
   - debug trace gated。

---

## 7. 风险与处理

### Risk 1: Rig provider capability mismatch

如果某 provider 不支持 reasoning delta 或 typed usage：

- 不失败。
- 不发对应 event。
- 在 debug/degrade trace 中记录 capability missing。
- message streaming 仍必须正常。

### Risk 2: Stream/non-stream 统一时引入生命周期复杂度

处理：

- 统一业务 service，但 sink 可替换。
- Stream sink 即时发送。
- Non-stream collecting sink 收集 final data。
- 不把 stream 简化成“先完整执行再切块”。

### Risk 3: RAG plan schema 变化导致检索质量波动

处理：

- 先用 Minsky E2E 固定 query 做回归。
- Debug artifact 必须记录每通道 chunks。
- 不让 planner 决定通道开关。
- 原始 query 永远进入 embedding retrieval。

### Risk 4: Debug artifact 过大

处理：

- 默认只保留必要 preview、chunk_id、page、score、channel、source metadata。
- 完整 payload 写 run artifact，有 retention。
- 业务库只存 trace_id/artifact_id。

### Risk 5: 删除 MainAgent 过早

处理：

- 不保留运行时双路径，但删除分两步：
  1. callers 全部迁走。
  2. 测试通过后再删旧模块或降为纯类型/prompt 迁移来源。

---

## 8. 执行顺序建议

严格按下面顺序执行，不并行写同一模块：

```text
1. Contract + AgentEvent + Agent interface
2. Rig adapter + fake stream tests
3. Unified stream/non-stream service sink
4. ChatAgent
5. WebSearchAgent + Brave LLM Context
6. RagAgent plan schema v2 + retrieval debug
7. Remove MainAgent callers and stale traces
8. Frontend event/progress/debug/citation wiring
9. Full backend + frontend + Minsky E2E validation
```

如果要用子代理并行：

- 可以并行只读探索。
- 可以并行写 frontend 和 backend search adapter，前提是 contract 已冻结。
- 不要并行改 `contracts/src/chat.rs`、`chat_streaming.rs`、`graphflow.rs`、agent runtime 核心文件。

---

## 9. Definition of Done

迁移完成时必须满足：

- [ ] 后端 deterministic dispatch：`chat` / `rag` / `search` -> concrete agent。
- [ ] `general` 只作为兼容 alias，不是新代码主路径。
- [ ] `MainAgent::decide` 不再参与 chat request execution。
- [ ] Rig 是唯一新的 model/agent streaming runtime。
- [ ] 没有 legacy/Rig runtime switch。
- [ ] 没有无 Rig 半真流式第二套实现。
- [ ] GraphFlow 仍负责共享 rails。
- [ ] Axum SSE 仍负责 HTTP streaming transport。
- [ ] `reasoning_summary_delta` 是一等事件。
- [ ] ChatAgent 无 citations。
- [ ] WebSearchAgent 有 Brave evidence citations。
- [ ] RagAgent 有 visible citations 和通道级 retrieval debug。
- [ ] Minsky E2E 能复现 query -> plan -> retrieval chunks -> answer。
- [ ] `cargo test` 相关目标通过。
- [ ] `pnpm typecheck/test/build` 通过。

---

## 10. 下一步

下一步不是继续讨论 feature flag，而是从 Phase 0 开始落地：先冻结 event contract 和 agent interface。这个阶段最小、风险最低，但会决定后续所有 agent 和前端的接线形状。
