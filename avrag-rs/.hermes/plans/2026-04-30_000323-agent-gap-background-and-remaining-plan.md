# Agent 主路径剩余 GAP 背景与修复计划

> **For Hermes:** 本文是当前修复工作的 handoff 文档。后续执行时先读取本文，再按 task 顺序推进；如需要并行实现，使用 `subagent-driven-development`，但不得让多个 worker 同时写同一模块边界。

**Goal:** 把当前 Agent 架构收敛到单一生产主路径：Chat/general 走 `ChatAgent`，WebSearch 走 Brave LLM Context 主路径，RAG v2 拓扑按 schema/检索通道边界落地，并完成 focused + 回归验证。

**Architecture:** 当前方向不是“保留 legacy 与新架构双轨灰度”，而是开发阶段直接切新架构。GraphFlow 仍负责业务编排，Rig/LLM client/Agent 只放在模型与 agent streaming layer；共享的是业务逻辑与 `AgentEventSink`，不是把完整答案切块伪装成流式。

**Tech Stack:** Rust modular monolith (`avrag-rs`), GraphFlow, `common` contracts, `app` unified agents, `transport-http` SSE, `search` crate, `rag-core`, Milvus/PostgreSQL；前端为 `frontend_next` 的 Next.js/React/TypeScript。

---

## 1. 当前背景

### 1.1 本轮已经完成的基线

已经完成并通过局部验证的方向：

1. 前端 P0 日志泄漏已修：
   - `frontend_next/lib/workspace/stream.ts` 不再把 token/trace 原文打印到 console。
   - `frontend_next/tests/workspace/stream.test.ts` 已改成防泄漏回归测试。

2. canonical chat mode 已从 `general` 迁移到 `chat`：
   - `general` 只作为兼容 alias。
   - 前端 UI/store/test 已朝 `chat` 收敛。

3. 前端已做 `reasoning_summary_delta` 最小可见处理：
   - 不展示原始 CoT。
   - 作为 progress/activity 一类的 reasoning summary 进入可见状态。

4. `SseSink` 已迁移到 app agent 层：
   - 新位置：`crates/app/src/agents/sse_sink.rs`
   - `transport-http` 不再保留独立 `sse_sink` 源。
   - `SseSink` 实现 `AgentEventSink`。
   - 首个 token delta 前必须先发 `AnswerStart`。
   - streaming 主路径可用 `.without_done_event()`，避免 agent 简化 Done 与最终完整 `ChatResponse` Done 重复。

5. Chat streaming 最小闭环已接到 UnifiedAgentService：
   - `execute_chat_stream(...)` 中 `AgentKind::Chat | None` canonical 到 `chat`。
   - `execute_general_chat_stream(...)` 通过 `UnifiedAgentService` 调 `ChatAgent`。
   - `ChatAgent` 在 `request.stream = true` 时调用 LLM `complete_stream(...)`，不是 buffered fake streaming。

6. 非 streaming `execute_general_mode_core(...)` 已开始切到 ChatAgent：
   - `crates/app/src/chat/service_modes.rs` 中 `execute_general_mode_core` 使用 `agent_service.run(... AgentKind::Chat ...)`。
   - `mode` / `response.agent_type` / `trace.mode` 输出为 `chat`。
   - 已新增但还未在工具额度内跑完的新测试：`general_mode_core_routes_through_unified_chat_agent`。

### 1.2 当前 active todo

当前剩余项：

- `[in_progress] chat-agent-main-path`：让 Chat/general 请求生产路径走 ChatAgent，停止 general 路径依赖 MainAgent。
- `[pending] websearch-brave`：替换 search provider 为 Brave LLM Context 主路径，保留 Perplexity 旧配置为非主路径或移除生产依赖。
- `[pending] rag-plan-v2-core`：修 RAG 关键拓扑：rag-plan-v2/schema guard、original query 永远 text dense、graph 只按 triplets、两 placeholder。
- `[pending] verify`：运行 focused tests、typecheck、cargo tests、diff/secrets 检查并汇总。

### 1.3 必须遵守的架构约束

1. 真 SSE 不能退化成假流式。
   - 允许 final `Done` 带完整 `ChatResponse`。
   - 不允许主回答先 buffered 完再切 chunk 当 token。

2. 不保留 legacy/Rig 双路径。
   - 开发阶段直接切新架构。
   - 兼容 alias 可以存在，但生产主路径不能继续走旧 `MainAgent` general path。

3. GraphFlow 不被 Agent/Rig 替代。
   - GraphFlow 负责业务编排、guard、持久化、usage、citation 校验。
   - Agent 负责模式内模型调用、stream event、检索/生成动作。

4. 证据边界必须清楚。
   - Chat 不伪装成 RAG。
   - WebSearch 只引用 web search 证据。
   - RAG 只引用已检索到的文档 chunks/graph 支撑内容。

5. 不保留敏感信息。
   - 文档、日志、测试输出中如出现 key/token/connection string，必须用 `[REDACTED]`。

---

## 2. 必要架构背景

### 2.1 AgentKind 与模式边界

目前目标模式：

- `chat`：普通对话，不主动检索文档，不使用 WebSearch。
- `search`：联网搜索，走 SearchAgent / SearchExecutor，最终应该以 Brave LLM Context 为主路径。
- `rag`：知识库检索，必须有 `doc_scope`，走 RAG plan + retrieval + answer synthesis。
- `general`：历史兼容 alias，不应作为新的 canonical mode。

相关文件：

- `crates/app/src/agents/runtime.rs`
- `crates/app/src/agents/service.rs`
- `crates/app/src/agents/chat_agent.rs`
- `crates/app/src/agents/web_search_agent.rs`
- `crates/app/src/agents/rag_agent.rs`
- `crates/app/src/lib_impl/chat_streaming.rs`
- `crates/app/src/chat/service_modes.rs`

### 2.2 UnifiedAgentService 的目标职责

`UnifiedAgentService` 应该是模式到 agent 的统一路由层：

- Chat/general -> `ChatAgent`
- Search -> `WebSearchAgent`
- RAG -> `RagAgent`

它不应承载 GraphFlow 的业务编排，也不应把所有路径降级成一次性字符串返回。

### 2.3 MainAgent 的当前角色

`MainAgent` 是旧路径，仍然有价值的部分主要是：

- 已有 prompt/envelope 经验。
- RAG planner 与 answer synthesis 里仍有旧逻辑。
- 历史 contract/test 可能依赖部分输出结构。

但目标不是让 `MainAgent` 继续作为 chat/general 生产主路径。剩余 MainAgent 引用应该只在 RAG 迁移完成前作为过渡对象存在，且需要被明确收敛。

当前仍可见的 MainAgent 相关残留：

- `crates/app/src/chat/graphflow_tasks_rag.rs`
  - `plan_rag_with_main_agent(...)`
  - `answer_rag_with_main_agent(...)`
  - `MainAgent::build_rag_chat_response(...)`
- `crates/app/src/main_agent/mod.rs`
  - 旧 planner、answer builder、RAG response builder。
- `crates/app/src/agents/rag_agent.rs`
  - 注释仍写着 “plans retrieval (via MainAgent planning logic)”
  - 目前主体仍是 placeholder。

### 2.4 WebSearch 当前背景

当前 search crate 仍是 Perplexity 主路径：

- `crates/search/src/config.rs`
  - default provider 为 `perplexity`。
- `crates/search/src/executor.rs`
  - `execute(...)` 调 `provider::execute_perplexity_agent(...)`。
  - `execute_stream(...)` 调 `provider::stream_perplexity_agent(...)`。
  - `ensure_supported()` 只支持 `perplexity`。
- `crates/search/src/provider.rs`
  - Perplexity endpoint 与 stream parsing。
- `crates/app/src/lib_impl/config.rs`
  - app search default provider 仍是 `perplexity`。
- `.env.example`
  - 已出现 `SEARCH_MODE=llm_tools`、`SEARCH_LLM_*` 等配置。
  - 但 `SEARCH_PROVIDER=perplexity` 仍是示例主配置。

`WebSearchAgent` 当前还存在一个重要实现问题：

- `crates/app/src/agents/web_search_agent.rs`
- `execute_stream` callback 是 sync callback，但内部用了：
  - `let _ = sink.emit(...)`
- `sink.emit(...)` 返回 future；这里没有 `.await`，事件可能不会真正发送。
- ChatAgent streaming 已经用 channel bridge 解决类似问题，WebSearchAgent 应复用同类模式。

### 2.5 RAG v2 当前背景

RAG 现状是 “GraphFlow RAG 主路径 + 旧 MainAgent planner/answer + 部分 ExecutePlan v2 contract”。关键点：

- `crates/common/src/rag_execute.rs`
  - 已有 `ExecutePlanRequest`、`ExecutePlanItem`、`ExecutePlanSummaryMode`、`GraphHint`、`PlaceholderTriplet`、`ExecutePlanValidationError`。
  - 已有 `validate()`。
  - 已有 legacy `RagPlan` 互转。
- `crates/common/tests/rag_execute_contract.rs`
  - 已有 ExecutePlan contract 测试，包括 graph/placeholder 相关字段。
- `crates/app/src/chat/graphflow_tasks_rag.rs`
  - `RagCallPlannerTask` 调 `plan_rag_with_main_agent(...)`。
  - `RagNormalizePlanTask` 目前只是 `to_rag_plan_compat()` 后写回，schema guard 不够强。
  - `RagExecutePlanTask` 调 `execute_rag_execute_plan(...)`。
  - `RagAnswerSynthesizeTask` 仍调 `answer_rag_with_main_agent(...)` 和 `MainAgent::build_rag_chat_response(...)`。
- `crates/app/src/agents/rag_agent.rs`
  - 当前是 placeholder，不是真 RAG 生产 agent。

用户明确提到的 RAG v2 关键拓扑：

1. `rag-plan-v2/schema guard`
2. original query 永远 text dense
3. graph 只按 triplets
4. 两 placeholder

这里的核心含义是：planner 输出必须符合 v2 schema，retrieval 执行时各通道职责不能混：

- text dense：必须包含原始 query，保证用户原始问题一定进入语义向量召回。
- BM25：只负责关键词/稀疏召回。
- multimodal dense：只在需要图像/版面/视觉相关证据时用多模态 embedding。
- graph：不能从自然语言 query 自由扩散；只能由明确 triplets / placeholder triplets 驱动。
- placeholder triplets：需要限定最多两个 placeholder，避免图谱检索变成不可控的模糊搜索。

---

## 3. 剩余 GAP 与修复计划

## GAP A: Chat/general 生产主路径仍需收尾

### 背景

Chat streaming 已经走 `UnifiedAgentService -> ChatAgent -> SseSink`，非 streaming `execute_general_mode_core` 也已改向 `ChatAgent`。但还有两个未收口点：

1. 新增测试 `general_mode_core_routes_through_unified_chat_agent` 尚未运行验证。
2. memory-only fallback `execute_memory_chat_compat(...)` 仍可能绕开 ChatAgent。

相关文件：

- `crates/app/src/chat/service_modes.rs`
- `crates/app/src/lib_impl/chat_streaming.rs`
- `crates/app/src/chat/graphflow_tasks_core.rs`
- `crates/app/src/lib_impl/tests.rs`
- `crates/app/src/lib_impl/chat_private.rs`

### GAP

- 如果 `execute_memory_chat_compat(...)` 仍在 production-like path 被调用，那么 “Chat/general 停止依赖旧兼容逻辑” 仍不完整。
- `general` alias 需要只保留入口兼容，不能在 trace/mode/response agent_type 中继续扩散。

### 修复任务

#### Task A1: 跑新增 non-streaming ChatAgent 路由测试

命令：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app general_mode_core_routes_through_unified_chat_agent -- --nocapture
```

预期：

- 测试通过。
- 如失败，优先修正测试暴露的 `ScriptedAgent`、`AppState::new`、`agent_service` 或 type mismatch 问题。

#### Task A2: 查清 `execute_memory_chat_compat(...)` 调用点

命令：

```bash
cd /home/chuan/context-osv6/avrag-rs
rg "execute_memory_chat_compat|TASK_MEMORY_MODE|ModeSelectTask|GeneralModeTask" crates/app/src
```

判断标准：

- 如果只在 isolated memory tests 使用，可以显式注释为 test/memory fallback，并确保 production bootstrap 有 `pg` 时不会走到。
- 如果实际生产路径可能触发，必须改为通过 `execute_general_mode_core(...)` 或 agent service。

#### Task A3: 消除 `general` 输出残留

检查：

```bash
cd /home/chuan/context-osv6/avrag-rs
rg 'agent_type.*general|mode.*general|trace.*general|"general"' crates/app/src frontend_next/tests frontend_next/components frontend_next/lib
```

允许存在：

- alias parse / backward compatibility。
- 测试中显式验证 `general` 输入 canonical 成 `chat`。

不允许存在：

- 新生产响应输出 `agent_type = "general"`。
- 新 trace mode 输出 `general`。
- Chat/general 生产 answer 仍由 `MainAgent::answer_general*` 生成。

#### Task A4: focused regression

命令：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app chat_stream_routes_chat_through_unified_agent_service -- --nocapture
cargo test -p app general_mode_core_routes_through_unified_chat_agent -- --nocapture
cargo test -p app agents -- --nocapture
cargo test -p transport-http --test chat_stream_contract -- --nocapture
```

验收：

- streaming 与 non-streaming Chat 都走 unified agent service。
- SSE contract 不回退。
- `AnswerStart` 仍在首 token 前。

---

## GAP B: WebSearch 仍是 Perplexity 主路径，且 WebSearchAgent streaming callback 不安全

### 背景

目标是 Brave LLM Context 主路径。当前 SearchExecutor 仍 hard-code Perplexity：

- default provider = `perplexity`
- `ensure_supported()` 只支持 perplexity
- provider 实现只有 Perplexity agent endpoint

`.env.example` 已出现新的 LLM tools 搜索配置雏形：

```env
SEARCH_MODE=llm_tools
SEARCH_ENABLE_THINKING=true
SEARCH_TOOLS=web_search,web_extractor,code_interpreter
SEARCH_LLM_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
SEARCH_LLM_API_KEY=
SEARCH_LLM_API_STYLE=auto
SEARCH_LLM_MODEL=qwen3.5-plus
SEARCH_LLM_TIMEOUT_MS=30000
SEARCH_PROVIDER=perplexity
```

这说明已有“LLM tools / search LLM”方向的配置，但 provider 主路径仍未切换。

### GAP

1. `SearchExecutor` 只支持 Perplexity。
2. app config 默认仍是 Perplexity。
3. `WebSearchAgent` 的 stream callback 里 `sink.emit(...)` 未 await，事件可能不会发出。
4. Search streaming 与 Chat streaming 的 event sink 语义未完全一致。

相关文件：

- `crates/search/src/config.rs`
- `crates/search/src/executor.rs`
- `crates/search/src/provider.rs`
- `crates/search/src/tests_impl.rs`
- `crates/app/src/lib_impl/config.rs`
- `crates/app/src/lib_impl/state_methods.rs`
- `crates/app/src/agents/web_search_agent.rs`
- `.env.example`

### 修复任务

#### Task B1: 明确 SearchConfig 的 provider enum / string contract

最小方案：

- 保持 `provider: String`，但 default 改成 `brave_llm_context` 或项目约定名。
- `ensure_supported()` 支持：
  - `brave_llm_context` 主路径
  - `perplexity` legacy path，如果决定保留。

更干净方案：

- 引入 enum：`SearchProvider::{BraveLlmContext, Perplexity}`。
- 但这会触及 config serde/env 边界，改动更大。除非当前 config 类型已经适合，否则先用 string contract 更符合 YAGNI。

#### Task B2: 新增 Brave LLM Context provider

可能文件：

- 新增或扩展：`crates/search/src/provider.rs`
- 如果 provider 文件太大，建议拆：
  - `crates/search/src/provider/mod.rs`
  - `crates/search/src/provider/perplexity.rs`
  - `crates/search/src/provider/brave_llm_context.rs`

验收语义：

- `execute(...)` 返回 `SearchResponse`：
  - `synthesized_answer`
  - `results`
  - `llm_usage`
- `execute_stream(...)` 能发出：
  - `Searching { queries }`
  - `SourcesCollected { results }`
  - `TextDelta { delta }`

注意：不要把 Brave 搜索结果和 LLM 生成回答混成不可追踪纯文本。必须保留 sources/results/citations。

#### Task B3: 修 WebSearchAgent callback await 问题

当前问题片段在：

- `crates/app/src/agents/web_search_agent.rs`

当前模式类似：

```rust
let _ = sink.emit(AgentEvent::Activity { ... });
```

这会创建 future 但不 await。

修复方向：

- 参考 `ChatAgent` 的 channel bridge。
- 在 callback 中只发送 lightweight event 到 channel。
- 在外层 async task/loop 中 await `sink.emit(...)`。
- 保证 `answer.push_str(delta)` 不与 callback 生命周期冲突；如需要，把 answer 累积也移到 channel consumer 侧。

#### Task B4: app config 与 env example 切主路径

涉及：

- `crates/app/src/lib_impl/config.rs`
- `.env.example`
- `crates/app/src/lib_impl/state_methods.rs`

目标：

- 默认 provider 不再是 `perplexity`。
- Perplexity key/model 可以保留为 legacy 配置，但不能是主路径要求。
- Brave LLM Context 所需配置命名清楚，不泄漏真实 key。

#### Task B5: 测试

建议新增/修改：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p avrag-search
cargo test -p app web_search_agent -- --nocapture
cargo test -p app agents -- --nocapture
```

应覆盖：

- unsupported provider 报错信息更新。
- default provider 是 Brave LLM Context。
- missing key/config 的错误是可诊断的，不 panic。
- stream update 能真正进入 `CollectingSink`。

---

## GAP C: RAG v2 拓扑未落地，RagAgent 仍是 placeholder

### 背景

RAG 目前仍由 GraphFlow RAG tasks 串起旧 MainAgent planner/answer。`common` 中已经有 ExecutePlan v2 contract，但 GraphFlow 的 normalize/execute 还没有强制执行用户要求的关键拓扑。

核心文件：

- `crates/common/src/rag_execute.rs`
- `crates/common/tests/rag_execute_contract.rs`
- `crates/app/src/chat/graphflow_tasks_rag.rs`
- `crates/app/src/lib_impl/chat_private.rs`
- `crates/app/src/agents/rag_agent.rs`
- `crates/rag-core/src/*`

### GAP

1. `rag-plan-v2/schema guard` 不够硬。
   - Planner 输出应被严格解析为 `ExecutePlanRequest`。
   - 无效 schema 应 clarify/degrade，而不是静默回 legacy fallback。

2. original query 没被强制进入 text dense。
   - 用户原始 query 必须始终进入 text dense channel。
   - Planner 可以增加改写 query，但不能替代 original query。

3. graph 检索边界不清。
   - graph 只能按 triplets / placeholder triplets 执行。
   - 不能从自由文本 query 直接做 graph expansion。

4. placeholder 数量与语义需要 guard。
   - 用户要求“两 placeholder”。这里应落实为：每个 placeholder triplet 最多两个 placeholder；超过则 reject/clarify/degrade。
   - `PlaceholderTriplet::classification()` 已区分 fuzzy/traceable/resolved，但 validate 需要按目标策略加强。

5. `RagAgent` 仍是 placeholder。
   - 当前仅 emit planning/retrieving/drafting activity，然后返回 `RAG answer placeholder for query: ...`。
   - 这不能作为生产 RAG agent。

### 修复任务

#### Task C1: 加强 ExecutePlanRequest validate

文件：

- `crates/common/src/rag_execute.rs`
- `crates/common/tests/rag_execute_contract.rs`

新增/确认规则：

1. `doc_scope` 非空。
2. `items` 非空且数量受限。
3. 每个 `ExecutePlanItem` 至少且至多一种主要 payload：
   - query
   - bm25_terms
   - multimodal / graph budget metadata 如当前 schema 有对应字段则按 schema 执行
4. `placeholder_triplets` 中每条最多两个 placeholder。
5. graph hints / placeholder triplets 字段必须结构化，不能由自由文本 query 暗示 graph 检索。
6. budget 中 graph 数量如果 > 0，必须有 graph_hints 或 placeholder_triplets。

测试建议：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p common --test rag_execute_contract -- --nocapture
```

#### Task C2: 在 planner normalize 阶段强制 schema guard

文件：

- `crates/app/src/chat/graphflow_tasks_rag.rs`
- 可能涉及 `crates/app/src/lib_impl/chat_private.rs`
- 可能涉及 `crates/app/src/main_agent/mod.rs`

目标：

- `RagCallPlannerTask` 得到 planner output 后，不再只做 legacy `to_rag_plan_compat()`。
- `RagNormalizePlanTask` 必须调用 `ExecutePlanRequest::validate()`。
- validate 失败时：
  - 如果是用户输入缺失，走 clarify。
  - 如果是 planner schema 错，记录 degrade_trace，并用 safe fallback plan；但 fallback 也必须符合 v2 schema。

#### Task C3: original query 永远进入 text dense

文件需先定位：

```bash
cd /home/chuan/context-osv6/avrag-rs
rg "execute_rag_execute_plan|text dense|dense|bm25|multimodal|graph_supported_chunks|backend_trace" crates/app/src crates/rag-core/src crates/storage-milvus/src
```

目标：

- 无论 planner items 如何，执行层都保证原始 `ChatRequest.query` 作为 text dense query 的一部分。
- planner 生成的 query rewrite 可以作为额外 dense query，不得覆盖原始 query。
- trace/debug 里能看到 original query 被放入 text dense。

验收：

- 新增单测：planner 只给 bm25 或 graph 时，最终 execute request/trace 仍包含 original query 的 text dense channel。

#### Task C4: graph 只按 triplets

目标：

- 如果 `graph` budget > 0 但没有 `graph_hints` / `placeholder_triplets`，则不执行 graph channel，并记录 degrade_trace。
- graph channel 只消费结构化 triplet，不消费 text query。

建议测试：

- 有 `graph_hints` 时 graph channel 执行。
- 只有 query + graph budget 时 graph channel 不执行，并降级说明。
- placeholder 超过两个时报 validation error。

#### Task C5: RagAgent 从 placeholder 变成 GraphFlow/RagRuntime 入口

两种可选收敛方式：

方案 1：RAG 继续由 GraphFlow 主路径执行，`RagAgent` 暂不作为生产 RAG。

- 优点：改动小。
- 缺点：UnifiedAgentService 对 RagAgent 的存在意义弱，仍有双入口感。

方案 2：`RagAgent` 封装 RAG plan/execute/synthesize，GraphFlow 调 RagAgent。

- 优点：三种模式都统一进 agent service。
- 缺点：需要小心不要让 Agent 替代 GraphFlow 的 guard/persist/usage/citation validation。

建议：先做方案 1 的安全收敛：

- `RagAgent` 不再返回 placeholder answer。
- 在没有完整接入前，遇到配置不足应返回明确错误，不产生伪答案。
- GraphFlow RAG 主路径先完成 v2 schema/topology 修复。
- 后续再把 RagAgent 作为 GraphFlow 内部可调用 adapter。

#### Task C6: RAG focused tests

建议命令：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p common --test rag_execute_contract -- --nocapture
cargo test -p app rag -- --nocapture
cargo test -p app execute_plan -- --nocapture
cargo test -p app agents -- --nocapture
```

---

## GAP D: 最终验证未完成

### 背景

当前已经跑过的 focused tests 包括：

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app agents --no-run
cargo test -p app sse_sink -- --nocapture
cargo test -p app agents -- --nocapture
cargo test -p app chat_stream_routes_chat_through_unified_agent_service -- --nocapture
cargo test -p transport-http chat_stream_contract --no-run
cargo test -p transport-http --test chat_stream_contract -- --nocapture
```

但最终全量/关键回归还没做。

### GAP

- 新增 non-streaming 测试没跑。
- WebSearch/RAG 修复后还需要 crate-level 回归。
- 前端 typecheck 还未在最终状态下跑。
- diff/secrets 检查还未最终跑。

### 验证计划

#### Task D1: 后端 focused tests

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app general_mode_core_routes_through_unified_chat_agent -- --nocapture
cargo test -p app chat_stream_routes_chat_through_unified_agent_service -- --nocapture
cargo test -p app agents -- --nocapture
cargo test -p transport-http --test chat_stream_contract -- --nocapture
cargo test -p common --test rag_execute_contract -- --nocapture
cargo test -p avrag-search
```

#### Task D2: 后端 broader tests

```bash
cd /home/chuan/context-osv6/avrag-rs
cargo test -p app
cargo test -p transport-http
```

如果时间不足，至少跑被修改 crate 的 focused tests，并在汇总里明确未跑全量。

#### Task D3: 前端 tests/typecheck

```bash
cd /home/chuan/context-osv6/frontend_next
pnpm test tests/workspace/stream.test.ts
pnpm test tests/workspace/ui-store.test.ts tests/workspace/workspace-chat-pane.test.tsx tests/workspace/workspace-right-rail.test.tsx
pnpm typecheck
```

#### Task D4: diff / secrets / log 泄漏检查

```bash
cd /home/chuan/context-osv6
git diff --check
git status --short
rg -n "console\.(log|info|debug)|println!|dbg!|PERPLEXITY_API_KEY=.+|SEARCH_LLM_API_KEY=.+|API_KEY=.+|Bearer [A-Za-z0-9._-]+|sk-[A-Za-z0-9]" avrag-rs frontend_next contracts
```

注意：

- `.env.example` 中空 key 是允许的。
- 如果命中真实 token/key，必须立即脱敏为 `[REDACTED]`。
- 前端不允许 token/trace payload 原文 console 输出。

---

## 4. 推荐执行顺序

1. 先收尾 `chat-agent-main-path`：
   - 跑新增 test。
   - 查 memory fallback。
   - 清理 `general` 输出残留。

2. 再做 `websearch-brave`：
   - SearchConfig/provider 主路径切换。
   - 新增 Brave LLM Context provider。
   - 修 WebSearchAgent stream callback await 问题。
   - 更新 `.env.example`。

3. 再做 `rag-plan-v2-core`：
   - 先强化 common contract/test。
   - 再接 GraphFlow normalize/schema guard。
   - 再修 retrieval topology。
   - 最后处理 RagAgent placeholder。

4. 最后做完整验证：
   - focused backend tests。
   - frontend tests/typecheck。
   - diff/secrets/log scan。
   - 汇总已跑/未跑项。

---

## 5. 风险与取舍

### 风险 1: 过早删除 MainAgent 会扩大爆炸半径

`MainAgent` 仍承载 RAG planner/answer 的旧逻辑。Chat/general 可以先完全脱离 MainAgent，但 RAG 迁移应按 contract -> normalize -> execute -> answer 的顺序推进，不建议一次性删除整个 `main_agent` module。

### 风险 2: WebSearch provider 切换可能影响线上配置

如果保留 Perplexity legacy provider，需要明确它不是 default。若完全移除，需要同步清理 `.env.example`、config、tests 与错误文案。

### 风险 3: RAG graph channel 容易变成“自由文本图搜索”

必须通过 schema guard 限定 graph 只按 triplets/placeholder_triplets，否则图检索会污染证据边界，解释不了为什么返回那些 graph chunks。

### 风险 4: placeholder 规则需要写进 contract

“两 placeholder”不能只写在 prompt 里，必须在 `ExecutePlanRequest::validate()` 或执行入口强制，否则 planner 一旦跑偏仍会进入执行层。

---

## 6. 完成定义

这批剩余工作完成时，应满足：

1. Chat/general：
   - 输入 `general` 也 canonical 到 `chat`。
   - streaming 与 non-streaming 均走 `UnifiedAgentService -> ChatAgent`。
   - 输出 mode/agent_type/trace 为 `chat`。
   - 不再调用 `MainAgent::answer_general*` 作为生产路径。

2. Search：
   - 默认 search provider 是 Brave LLM Context 主路径。
   - Perplexity 不是生产默认依赖。
   - WebSearchAgent stream event 真正 await/发送。
   - sources/citations 保留可追踪边界。

3. RAG：
   - Planner 输出被 v2 schema guard。
   - original query 始终进入 text dense。
   - graph 只按 triplets/placeholder_triplets 执行。
   - 每条 placeholder triplet 最多两个 placeholder。
   - RagAgent 不再产出 placeholder answer 作为看似成功的结果。

4. 验证：
   - focused tests 通过。
   - 前端相关 tests/typecheck 通过或明确记录未跑原因。
   - `git diff --check` 通过。
   - 无 token/trace payload console 泄漏。
   - 无明文密钥进入 diff。
