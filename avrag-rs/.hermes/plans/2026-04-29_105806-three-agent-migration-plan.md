# Three-Agent Progressive Migration Plan

> Status: design record. Final executable migration plan has been extracted to `.hermes/plans/2026-04-29_182854-three-agent-rig-migration-implementation-plan.md`.
> Confirmed business decision: frontend already uses explicit user selection for modes. No automatic route guessing and no complex switch suggestion UX.
> Latest migration direction: full switch to the new Rig-backed three-agent runtime. Do not keep legacy/Rig dual paths.

## Confirmed decision 1: mode ownership

Business choice: A — user explicitly selects the mode in the frontend.

Implication:

```text
User-selected mode -> backend deterministic dispatch -> concrete agent
```

Backend should not run a smart `MainAgent` to infer whether the question is RAG, chat, or web search.

Stable routing target:

```text
agent_type = "rag"    -> RagAgent
agent_type = "chat"   -> ChatAgent
agent_type = "search" -> WebSearchAgent
```

## Migration phases

### Phase 0 — Confirm mode semantics

Goal: make product behavior unambiguous before changing code.

Confirmed:
- Mode is selected explicitly by the user in frontend.
- No automatic mode guessing.
- No complex “you may want to switch mode” prompt in this migration.

Still to confirm:
- What each mode is allowed to use as evidence.
- What each mode should do when required evidence is missing.

### Phase 1 — Build three-agent skeleton without changing behavior

Goal: split ownership first, without changing answer quality or retrieval behavior.

Business behavior should remain the same.

Target structure:

```text
RagAgent       owns document-grounded Q&A
ChatAgent      owns normal conversation
WebSearchAgent owns public web search Q&A
```

Main point:
- The dispatcher is not an agent.
- It routes based only on user-selected mode.

### Phase 2 — Make RagAgent progressive

Goal: RAG becomes stage-based instead of one mixed prompt identity.

Stages:

```text
prepare_context -> plan -> retrieve -> answer
```

Business meaning:
- `prepare_context`: understand what “it/this/that” refers to and read user style preference.
- `plan`: decide how to search the selected documents.
- `retrieve`: execute retrieval deterministically.
- `answer`: answer only after evidence chunks are available.

### Phase 3 — Replace RAG plan schema with business-readable v2

Goal: plan output should match the real schema and be understandable in debug output.

Target plan item kinds:

```text
semantic      document meaning search
lexical       exact keyword / BM25 search
relationship  graph / relationship search
summary       document summary support
```

### Phase 4 — Replace Perplexity path with Brave-backed WebSearchAgent

Goal: Brave becomes evidence provider; our agent remains responsible for planning and answering.

MVP modes:

```text
web         normal web results
news        current news / updates
llm_context extracted page context for synthesis
```

Deferred unless needed:

```text
image
video
Brave Answers API as black-box answer provider
```

### Phase 5 — Align frontend progress/debug display

Goal: user sees business-stage progress, debug mode sees schemas/evidence.

RAG display:

```text
理解问题 -> 制定检索计划 -> 检索文档 -> 整理证据 -> 生成回答
```

WebSearch display:

```text
理解问题 -> 制定搜索计划 -> 搜索网页 -> 整理来源 -> 生成回答
```

Chat display:

```text
理解上下文 -> 生成回答
```

### Phase 6 — Remove old MainAgent path

Goal: after three paths are stable, remove old mixed abstraction.

Exit criteria:
- RAG does not call MainAgent plan/answer functions.
- Chat does not call MainAgent general answer functions.
- Search does not call Perplexity agent API.
- Product trace no longer uses `main_agent` as a business concept.

## Confirmed decision 2: strict evidence boundaries

Business choice: A — strict evidence boundaries per mode.

Reason:
- The three modes should feel meaningfully different.
- Strict boundaries maximize each mode's value and make user expectations stable.
- The system should not blur document-grounded answers, normal chat, and web-grounded answers.

Mode boundaries:

```text
RagAgent
  factual evidence = selected documents / retrieved chunks only
  conversation history = pronoun and context resolution only
  user preferences = answer style only
  insufficient document evidence = say evidence is insufficient

ChatAgent
  factual basis = model general knowledge + conversation context
  forbidden = pretending to use uploaded documents or live web search
  if fresh/public evidence is needed = say this belongs in WebSearch mode

WebSearchAgent
  factual evidence = Brave search evidence only
  model role = synthesize, compare, explain, format
  insufficient web evidence = say search evidence is insufficient
```

## Confirmed decision 3: RAG answer and citation experience

Business choice: A/B/C across the three sub-questions.

Rules:

```text
Citation visibility
  Every RAG answer should visibly cite sources in the answer body.
  Source/debug panel can still keep the full evidence chain.

Insufficient evidence
  Do not short-refuse only.
  Explain that evidence is insufficient, include the closest retrieved evidence, and state the missing link clearly.

Answer structure
  Default to direct answer first.
  For complex relationship/comparison/research questions, use a more evidence-analytic structure.
```

Business meaning:
- RAG should feel visibly grounded, not merely “chat with hidden sources.”
- Evidence insufficiency should be useful, showing what was found and what was missing.
- The user gets a fast conclusion first, but deep research questions still get structured analysis.

## Confirmed decision 4: RAG plan visibility and debug granularity

Business choice: 1A / 2B / 3A across the three sub-questions.

Rules:

```text
Normal user plan visibility
  Normal users see high-level business progress only.
  They do not see raw JSON plan by default.

Debug retrieval visibility
  Debug mode exposes retrieval results by channel:
    - semantic / text vector
    - multimodal vector
    - BM25
    - graph / relationship retrieval
  Debug mode should also show final merged/reranked evidence.

Planner item reason
  Planner items do not require human-readable reason in the first migration version.
  Keep the plan schema leaner and reduce model-output noise.
```

Business meaning:
- The product remains clean for normal users.
- Developers/advanced debug can still answer “which retrieval channel returned which chunks.”
- Plan schema stays simpler in the skeleton-first phase; explanatory `reason` can be added later if debug analysis shows it is worth the extra prompt/schema burden.

## Confirmed decision 5: RAG planning responsibility and retrieval channel control

Business choice:

```text
1. The RAG planner does not choose channels on/off.
   It always outputs three fixed content classes in the plan schema:
     - rewrite_query
     - bm25_keywords
     - triplets

   The planner's autonomy is limited to how many entries it outputs for each class.
   It may output zero or more entries per class, within schema limits.

2. Because BM25 keywords and triplets are explicit plan outputs, BM25 and graph retrieval should run with full allocated budget whenever their corresponding plan entries exist.
   They are not low-budget optional probes.

3. Multimodal retrieval should always run.
```

Reason:
- The agent does not see all source content before retrieval and does not reliably get a second reflection pass after seeing evidence.
- Letting the agent freely choose retrieval channels creates too much uncertainty.
- Letting the agent only rewrite query underuses its intelligence.
- A fixed three-part plan gives the agent useful planning work while keeping retrieval topology stable.

Target RAG plan responsibility:

```text
Planner decides:
  - how to rewrite the user question for semantic retrieval
  - which BM25 keywords are worth exact lexical matching
  - which relationship triplets are worth graph retrieval
  - how many entries to emit per class

Planner does not decide:
  - whether semantic retrieval exists as a channel
  - whether multimodal retrieval exists as a channel
  - whether BM25 / graph are available retrieval mechanisms
  - final retrieval budgets outside schema limits
```

Business rule for BM25 keyword language:

```text
BM25 keywords should be selected according to document metadata language.
If the document metadata indicates English, prefer English exact terms.
If the document metadata indicates Chinese, prefer Chinese exact terms.
If mixed/unknown, include the document's original terminology and only add translated terms when useful.
```

Retrieval execution rule:

```text
semantic text vector    always runs from rewrite_query
multimodal vector       always runs
BM25                    runs full allocated budget when bm25_keywords is non-empty
graph / triplets        runs full allocated budget when triplets is non-empty
```

## Confirmed decision 6: RAG plan schema limits and placeholder behavior

Business choice:

```text
1. Do not set separate hard limits per content class.
   Use one total sub-query item budget across all generated plan content.

2. All sub-query content classes may be empty:
   - rewrite_queries can be empty
   - bm25_keywords can be empty
   - triplets can be empty

3. Regardless of sub-query output, the original user query is always sent to embedding retrieval.
   This merges the normal path and fallback path.

4. Triplets may contain up to two placeholders when the planner judges it useful.
   This should follow the T²RAG-style idea of placeholder triplets rather than forcing only one unknown position.
```

Reason:
- A per-class limit is too rigid. The planner should decide how to spend a shared query budget across rewrite queries, BM25 terms, and triplets.
- Some questions genuinely do not need one or more sub-query classes.
- Always embedding-searching the original query makes fallback deterministic and avoids “empty plan means no semantic retrieval.”
- Two-placeholder triplets are useful for exploratory relationship questions where the user names only one anchor entity or asks an open relationship question.

Target execution rule:

```text
Semantic embedding retrieval inputs:
  - original user query, always
  - plus zero or more rewrite_queries from planner

BM25 retrieval inputs:
  - zero or more bm25_keywords from planner
  - if empty, BM25 can be skipped because the original query path is already covered by embedding retrieval

Graph retrieval inputs:
  - zero or more triplets from planner
  - triplets may include 0, 1, or 2 placeholders

Multimodal retrieval inputs:
  - always runs, using the original query and/or semantic query bundle according to implementation detail
```

Schema budget principle:

```text
total_sub_query_budget = shared cap across rewrite_queries + bm25_keywords + triplets
planner decides allocation within the cap
```

The exact numeric total budget remains an implementation tuning decision, not a per-class product rule.

## Confirmed decision 7: WebSearch Agent behavior with Brave

Business choice: 1A / 2A / 3A across the three sub-questions.

Rules:

```text
Answer synthesis
  Brave is an evidence provider, not the final answer agent.
  WebSearchAgent uses Brave Search APIs to collect evidence, then synthesizes the answer with our own answer skill/LLM.
  Do not use Brave Answers API as the primary path in this migration.

Citation visibility
  Normal WebSearch answers must show visible source citations in the answer body, similar to RAG.
  This reinforces that WebSearch is public-web-grounded, not model-memory chat.

MVP Brave modes
  MVP supports:
    - web search
    - news search
    - llm_context
  Image/video are not part of the first executable MVP.
```

Business meaning:
- WebSearch becomes an inspectable, cited, public-web evidence mode.
- It does not repeat the Perplexity black-box answer pattern.
- The first implementation stays focused on text evidence and current information rather than media search UX.

## Confirmed decision 8: WebSearch planning schema and Brave mode selection

Business choice:

```text
Use Brave LLM Context as the default and primary WebSearch evidence path.
Do not run normal web search by default.
The original user query always goes to LLM Context.
There is no web -> llm_context upgrade path in MVP.
The agent may autonomously decide whether to add extra LLM Context queries and whether to use News when the task needs news freshness.
```

Reason:
- Normal Brave web search mostly returns URLs and snippets. To answer well, the system would still need page parsing.
- Brave API is usage/call billed; running both web and llm_context by default is unnecessarily expensive.
- LLM Context already provides extracted content suitable for answer synthesis.
- Therefore, WebSearch MVP should avoid double-searching and avoid a staged web-to-context upgrade.

Target execution rule:

```text
LLM Context:
  always runs with the original user query
  also runs zero or more additional planner-generated context queries

News Search:
  runs only when the planner decides fresh news coverage is needed

Web Search:
  not used in MVP default path
  reserved for future source-discovery or lightweight search UI needs
```

Target WebSearch plan shape:

```json
{
  "plan_version": "websearch-plan-v1",
  "context_queries": ["optional additional LLM Context query"],
  "news_queries": ["optional news query"],
  "freshness": "optional pd|pw|pm|py|date-range",
  "country": "ALL",
  "search_lang": "en"
}
```

Fallback rule:

```text
Even if context_queries and news_queries are empty, the original user query still goes to LLM Context.
```

## Confirmed decision 9: ChatAgent behavior and memory boundary

Business choice: 1B / 2A / 3A across the three sub-questions.

Rules:

```text
Context and memory
  ChatAgent uses current conversation context plus long-term user preference/memory summaries.
  Memory is used for continuity, style, format preferences, and personalization.
  Memory is not treated as external factual evidence.

Citations
  ChatAgent does not show citations.
  Citations are reserved for evidence-grounded modes: RAG and WebSearch.

Boundary handling
  If a ChatAgent request clearly requires uploaded document evidence, answer with a boundary statement and tell the user this belongs in RAG mode.
  If it clearly requires fresh/public web evidence, answer with a boundary statement and tell the user this belongs in WebSearch mode.
  Do not provide a generic factual answer with a warning, because that blurs mode boundaries.
```

Business meaning:
- Chat remains a personal conversational assistant.
- It can remember how the user likes answers, but it does not pretend to have read documents or searched the web.
- Strict boundary statements preserve the value of the three explicit modes.

## Confirmed decision 10: streaming/progress events and debug retention

Business choice: A with log-based retention instead of permanent database storage.

Rules:

```text
User-visible progress stages

RagAgent:
  理解问题 -> 制定检索计划 -> 检索文档 -> 整理证据 -> 生成回答

WebSearchAgent:
  理解问题 -> 制定搜索计划 -> 搜索网页内容 -> 整理来源 -> 生成回答

ChatAgent:
  理解上下文 -> 生成回答

Debug streaming
  Debug payloads can travel in the same SSE stream as normal progress.
  They are emitted only when a debug flag is enabled.

Debug persistence
  Do not permanently store all plan/retrieval/source debug payloads in primary business tables.
  Persist debug artifacts to logs or log-like run artifacts with retention/cleanup.
  This avoids unbounded database growth from chunks, retrieval traces, and source evidence.
```

Business meaning:
- Normal users see clean business progress.
- Debug users can inspect plan schema, channel-level retrieval, evidence, sources, and degrade trace during a run.
- E2E and quality analysis remain possible through logs/artifacts.
- Storage remains bounded through periodic cleanup.

Retention implication:

```text
Debug artifacts should be treated as operational observability data, not durable product data.
Primary product data keeps only final answer, citations/source references, and minimal trace IDs.
Detailed plan/retrieval/source payloads live in logs/artifacts and expire by retention policy.
```

## Confirmed decision 11: migration order, gates, and cross-cutting impact assessment

Business choice: A — skeleton first, migrate ChatAgent -> WebSearchAgent -> RagAgent, with tests + API E2E + frontend-visible QA gates.

Additional requirement:
- Before implementation, explicitly assess impacts on billing/usage, auth, worker/queue, database structure, frontend wiring, and existing Rust GraphFlow usage.

### Migration order

```text
Phase 1: Create three-agent skeleton while keeping current behavior
Phase 2: Migrate ChatAgent first
Phase 3: Migrate WebSearchAgent second
Phase 4: Migrate RagAgent last
Phase 5: Remove old MainAgent path after all three modes pass gates
```

Reason:
- ChatAgent has the smallest dependency surface.
- WebSearchAgent can replace Perplexity independently of document ingestion and Milvus.
- RagAgent has the largest blast radius: planner schema, retrieval, multimodal, BM25, graph, citations, and debug traces.

### Release gates per phase

```text
Unit / contract tests
  schema parsing, routing, citation behavior, boundary behavior

Local API E2E
  /api/v1/chat non-stream and stream paths per agent_type

Frontend-visible manual QA
  explicit mode selection, progress stages, citations, source panels, debug flag behavior
```

### Billing / usage impact

Current baseline:
- `execute_chat_preflight` checks coarse token quota before chat execution.
- `record_usage_for_execution` records LLM usage when `execution.llm_usage` exists.
- Current graphflow cost event source is `graphflow`.
- Search currently relies on Perplexity-like `llm_usage` if provider returns it.

Migration impact:

```text
ChatAgent
  mostly same as current general chat usage

WebSearchAgent
  adds Brave LLM Context API calls charged per request/call, not necessarily token-metered LLM usage
  must record Brave API call count/cost separately from answer LLM token usage
  should not log the Brave API key

RagAgent
  plan LLM + answer LLM remain billable
  always-running multimodal retrieval may increase embedding/rerank/API costs if runtime performs fresh calls per query
  BM25/graph full budget affects CPU/query latency more than external LLM billing unless graph expansion calls external models
```

Design rule:
- Do not block migration on a full billing rewrite.
- Add explicit metering points for `websearch.llm_context`, `websearch.news`, `rag.plan`, `rag.answer`, and later retrieval-cost counters.
- If the billing system cannot yet price Brave calls precisely, record request counts and provider metadata first.

### Auth / access control impact

Current baseline:
- `AuthContext` provides org/user scope.
- Preflight runs quota checks and input guard.
- RAG relies on server-side `doc_scope` / org-scoped repository access.
- Share mode already requires sign-in for asking questions.

Migration impact:

```text
All agents
  must keep the same AuthContext and preflight guard path
  must not let model-produced schema override org_id/user_id/doc_scope

RagAgent
  doc_scope remains server-controlled and server-validated
  planner can read document metadata but cannot expand scope beyond selected documents

WebSearchAgent
  Brave key is a server-side secret only
  user never supplies provider credentials
  evidence is public-web evidence but request still belongs to the user's org/user for quota and audit

ChatAgent
  memory/prefs are scoped to the authenticated user/session
```

### Worker / queue impact

Current baseline:
- Worker is primarily for ingestion/document cleanup/background jobs.
- Chat execution is request/response, not worker-queued.

Migration impact:

```text
ChatAgent
  no worker impact

WebSearchAgent
  no worker queue needed for MVP; keep it request/response
  debug artifact cleanup can be handled by log retention or a lightweight cleanup job, not the chat worker path

RagAgent
  no new ingestion queue requirement for the chat migration itself
  but always-running multimodal and graph retrieval assumes ingestion has already produced multimodal chunks and graph/triplet indexes where available
```

Design rule:
- Do not introduce a new chat execution queue in this migration.
- Keep worker changes out of Phase 1-3 unless needed for debug artifact retention or graph index availability.

### Database structure impact

Current baseline:
- `chat_sessions.agent_type` is plain text, so new `chat` value likely does not need a schema migration.
- Existing data/tests use `general`; new product term is `chat`.
- Debug payloads can be large.

Migration impact:

```text
Agent type naming
  support `chat` as the new public mode
  keep `general` as a backward-compatible alias during migration

Debug artifacts
  do not store full plan/retrieval/source payloads permanently in primary business tables
  store minimal trace_id/run_id with final messages if needed
  write detailed artifacts to logs/run files with retention cleanup

Search config
  add Brave config/env support without persisting API keys in DB
```

Design rule:
- Avoid DB schema changes in the skeleton phase.
- If later needed, add only small durable fields such as `trace_id` or `debug_artifact_id`, not raw chunk/source blobs.

### Frontend wiring impact

Current baseline:
- Frontend already uses explicit mode selection.
- Search and RAG citation rendering exists in tests.
- Current tests and some payloads still use `general` for chat-like mode.

Migration impact:

```text
Mode values
  frontend should send `chat` for ChatAgent
  backend should accept both `chat` and legacy `general` until old sessions/tests are migrated

Progress UI
  map activity phases to the confirmed business stage names per agent

Citations
  RAG and WebSearch answers must include visible citations in answer blocks/body
  ChatAgent must not show citations

Debug mode
  add/confirm debug flag plumbing for SSE debug payloads
  normal users should only see high-level progress
```

### Rust GraphFlow assessment

Current state from code inspection:
- Non-stream chat path uses `execute_chat_graphflow` and `build_chat_graph`.
- GraphFlow currently models shared rails well: preflight, session resolution, mode routing, output guard, persist, usage, notifications, build response.
- RAG has existing graphflow tasks: prepare planner input, call planner, normalize plan, execute plan, synthesize answer, validate citations.
- Streaming path currently bypasses GraphFlow orchestration in important places and still calls `MainAgent::decide`, creating a dual-path risk.
- Existing `ModeSelectTask` still delegates to `MainAgent::decide`, which conflicts with the new deterministic dispatcher goal.

Assessment:

```text
Keep GraphFlow as an orchestration kernel.
Do not build a new workflow framework.
Do not force every agent internals into graph nodes in Phase 1.
Replace MainAgent-dependent nodes with deterministic dispatch + concrete agent services.
Unify stream and non-stream paths so they do not drift.
```

Recommended GraphFlow role in the new architecture:

```text
GraphFlow owns shared execution rails:
  preflight -> session -> deterministic dispatch -> agent run -> output guard -> persist -> usage -> notify -> response

Concrete agents own mode-specific logic:
  ChatAgent.run
  WebSearchAgent.run
  RagAgent.run
```

Why this avoids wheel-reinvention:
- Existing GraphFlow already provides task sequencing and context passing.
- Existing postprocess tasks already cover persist/usage/notify.
- The migration needs better agent boundaries, not a brand-new workflow runtime.

Main caveat:
- GraphFlow must either become stream-aware through a stream/debug sink in context, or both streaming and non-streaming paths must call the same agent services under GraphFlow-compatible wrappers. The current split path should not survive the migration.
- Do not collapse the streaming endpoint into a buffered non-stream path. The current code has real SSE plumbing: the HTTP handler returns an Axum `Sse` stream backed by an `mpsc` receiver; the worker task sends `ChatEvent` values as they happen; the LLM client calls `/chat/completions` with `stream=true` and forwards provider delta chunks. The migration must preserve this event sink/callback behavior.
- Sharing agent service means sharing business logic, not sharing output transport. The agent service should accept an optional stream/debug sink. In streaming mode it emits progress/debug/token events immediately; in non-stream mode it can collect or ignore those events and return the final `ChatResponse`.
- Verification gate: for each agent type, test both `/api/v1/chat` non-stream and stream. Stream must prove true SSE behavior by observing `start/activity/answer_start/token/done` events before the final payload and provider deltas when the selected model supports streaming. Falling back to `chunk_text_for_stream` is allowed only as explicit degrade/fallback, not as the primary streaming implementation.

## Confirmed decision 12: GraphFlow strategy and SSE safety

Business choice: A confirmed.

Confirmed direction:
1. Keep GraphFlow as the shared orchestration kernel and replace only the MainAgent-dependent nodes.
2. Require streaming and non-streaming chat to share the same concrete agent service implementations before deep RAG changes.
3. Avoid adding graph nodes for every internal sub-step in Phase 1; add nodes only when they provide real observability, error isolation, or branch-control value.
4. Preserve true SSE as a first-class requirement. The shared service design must be stream-sink based, not buffer-first.

## Confirmed decision 13: semi-true streaming with reasoning summary

Business clarification:
- Desired UX is semi-true streaming: backend receives true model/tool/event stream, coalesces/buffers deltas, and frontend renders a smooth answer stream plus a reasoning-summary/progress panel.
- The reasoning panel should not expose raw hidden chain-of-thought. It should show model-visible planning summaries, reasoning summaries when provider-supported, retrieval/tool/status events, and debug traces when enabled.
- Use one SSE transport from backend to frontend. Do not create a second independent channel for reasoning/progress.

Current GraphFlow fit:
- `graph-flow = 0.4.0` supports task sequencing, context passing, conditional branching, step/batch execution, persistence, and fanout.
- It does not provide a built-in `Stream<Item = AgentEvent>` runtime, built-in SSE transport, or built-in token/reasoning-summary coalescer.
- It can still support the target UX if we pass an event sink through GraphFlow context or graph-compatible wrappers. Tasks/agents emit events to that sink while GraphFlow continues to orchestrate the shared rails.

Implementation rule:
```text
GraphFlow remains the workflow/orchestration layer.
Axum SSE remains the HTTP transport layer.
Agent services emit typed AgentEvents into a shared sink.
A coalescer batches token/reasoning-summary deltas before converting them into ChatEvent SSE frames.
```

Recommended event categories:
```text
activity/status            -> normal user progress panel
reasoning_summary_delta    -> visible reasoning-summary panel, not raw CoT
message_delta              -> answer bubble
trace/debug                -> gated plan schema, retrieval chunks, backend trace
citations                  -> source panel / citation rendering
done/error                 -> lifecycle completion
usage                      -> final metering/debug payload where safe
```

Framework assessment:
- Reframe Rig from "future optional adapter" to "serious target for the model/agent streaming layer". The current custom `avrag_llm::LlmClient` only handles basic chat completion text deltas and usage; Rig 0.36 exposes typed streaming items for message deltas, tool-call deltas, reasoning/reasoning deltas, final response, and message id.
- Do not replace GraphFlow with Rig in Phase 1. Keep GraphFlow for product rails already wired into auth/session/persist/usage/notify. Use Rig inside concrete agents or an `avrag-agent-runtime`/`avrag-llm` adapter layer.
- Do not enable GraphFlow's optional `rig` feature blindly: `graph-flow = 0.4.0` declares an optional `rig-core = 0.19.0`, while current Rig is `rig-core = 0.36.0`. Prefer adding current `rig-core` directly to the app/LLM adapter layer and keep GraphFlow decoupled.
- Swiftide is worth evaluating only if we want to replace larger parts of the RAG/query/indexing layer, which is out of scope for the three-agent migration.
- ADK-Rust / AutoAgents are candidates only if we decide to rebuild the workflow runtime around multi-agent graphs. That is not recommended now because it duplicates current GraphFlow/product rails.
- Low-level stream crates can be useful for coalescing/debouncing/event-envelope helpers, but they should not own product routing or evidence boundaries.

Rig adoption recommendation:
```text
Use Rig as the model/agent streaming engine.
Keep GraphFlow as the request orchestration kernel.
Keep Axum SSE as the single frontend transport.
Keep RAG retrieval/search executors deterministic unless a product mode explicitly allows autonomous tool choice.
```

Rig adoption phases:
1. Spike: add a small isolated Rig adapter for one answer-generation call, map Rig stream items into internal `AgentEvent` values, and verify message/reasoning/tool/final events compile and stream.
2. ChatAgent first: replace current `MainAgent::answer_general_stream` / `LlmClient::complete_stream` usage with Rig streaming while preserving existing prompts, memory context, usage recording, and SSE event contract.
3. WebSearchAgent second: keep Brave LLM Context as deterministic evidence retrieval; use Rig for synthesis streaming and optional visible tool/status event modeling.
4. RagAgent last: keep planner/retrieval/execution evidence boundaries deterministic; use Rig for plan/answer LLM calls where it improves structured output, reasoning-summary events, and typed streaming.
5. Only after these pass gates, decide whether Rig tools should become real executable tool adapters for retrieval/search. Do not let Rig's agent autonomy override explicit user-selected modes or server-validated doc_scope.

Verification gate:
- Streaming API must keep one SSE request pending while events arrive.
- Token/reasoning-summary deltas may be batched by time or size, but the first visible progress event and first answer delta must arrive before final completion.
- Buffered fallback must be labeled in `degrade_trace` and must not be the default path.
- Frontend QA must distinguish answer stream, reasoning-summary stream, progress/status stream, and debug trace stream even though all travel over the same SSE connection.

## Remaining open decisions before implementation

These are the remaining business/architecture clarifications before writing code:

1. Rig adoption scope
   - Recommended default: use Rig for model/agent streaming in the new concrete agents, but keep GraphFlow as product orchestration.
   - Open detail: whether Phase 1 spike should be mandatory before ChatAgent migration, or bundled into ChatAgent migration.

2. Reasoning-summary product contract
   - Recommended default: show visible reasoning summaries and progress/tool/retrieval status, never raw hidden chain-of-thought.
   - Open detail: whether reasoning summary is shown for all users by default or only in an expandable panel.

3. SSE event contract
   - Recommended default: keep one SSE connection and add explicit event types for reasoning summary rather than overloading debug trace.
   - Open detail: whether to add `reasoning_summary_delta` as a first-class `ChatEvent` immediately or first tunnel it through existing `trace` events for migration speed.

4. Rig tool autonomy boundary
   - Recommended default: do not let Rig autonomously choose product mode, doc scope, or retrieval topology in MVP.
   - Open detail: whether WebSearch should later expose Brave as a real Rig tool or remain deterministic forever.

5. Debug artifact retention
   - Recommended default: logs/run artifacts with retention, primary database stores only minimal trace/artifact IDs.
   - Open detail: retention period and whether local/dev artifacts should be downloadable from UI.

6. Backward compatibility and rollout
   - Decision: do not keep dual legacy/Rig runtime switches. The project is still in development, the previous agent architecture has not shipped as a stable business baseline, and preserving two implementations would create more complexity than it removes.
   - Implement the new Rig-backed path as the single target runtime for the three-agent migration. Keep compatibility only at public API/protocol boundaries where needed, such as accepting legacy `general` as an alias for `chat` during data/test migration.
   - Do not implement both a non-Rig semi-true streaming path and a Rig semi-true streaming path. The semi-true streaming design should be built once on top of Rig stream events, internal `AgentEvent`, coalescing, and the existing Axum SSE transport.
