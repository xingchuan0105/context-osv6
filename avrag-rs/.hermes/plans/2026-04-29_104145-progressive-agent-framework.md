# Progressive Agent Framework Design

> Status: design only, no code changes.
> Scope: replace the current `MainAgent`-style mixed mechanism with three mode-specific agents: `rag_agent`, `chat_agent`, `websearch_agent`.
> Principle: keep the trunk stable and small; do not fill implementation details prematurely.

## 0. Sensitive local note

Brave Search API key supplied by user for later local wiring:

```text
BRAVE_SEARCH_API_KEY=BSAE-imJpiRvD5S002bBYPHSsjhwEFJ
```

Do not commit this file or copy the key into source code, migrations, fixtures, test snapshots, or public docs. Later implementation should move it to local env/secrets storage.

## 1. Problem statement

The current `crates/app/src/main_agent/mod.rs` is not really progressive disclosure:

- One `MainAgent` owns mode dispatch, general chat, RAG plan, RAG answer, prompt envelopes, schema parsing, fallback behavior, and response construction.
- The model sees a generic `MainAgent` identity even when it is only planning retrieval or only answering with evidence.
- The `RAG_PLAN_SYSTEM_PROMPT` describes a JSON shape, but the actual accepted schema is `ExecutePlanRequest` in `crates/common/src/rag_execute.rs` with additional fields such as `budget`, `channel_budget`, and `trace`. The prompt also uses examples/rules that can drift from the Rust schema.
- Behavior skill text is embedded in the prompt envelope, not loaded only at the stage that needs it.

The new target is three explicit agents, one per product mode:

1. `rag_agent`
2. `chat_agent`
3. `websearch_agent`

No `main_agent` abstraction remains in the runtime path.

## 2. Architecture target

### 2.1 Top-level mode dispatch

Keep dispatch deterministic and boring:

```text
ChatRequest.agent_type
  "rag"       -> RagAgent
  "chat"      -> ChatAgent
  "search"    -> WebSearchAgent
  default     -> ChatAgent or request validation error, product decision later
```

The dispatcher is not an agent. It only routes to a concrete agent by explicit mode.

Recommended module shape:

```text
crates/app/src/agents/
  mod.rs
  context.rs
  rag.rs
  chat.rs
  websearch.rs
  prompts/
    rag_base.md
    rag_plan_skill.md
    rag_answer_skill.md
    chat_base.md
    websearch_base.md
```

Later, if prompt files are not wanted at runtime, keep the same logical split with Rust constants. The important boundary is not file format; it is stage-specific disclosure.

### 2.2 Shared minimal interfaces

Keep a tiny interface per agent. Do not create a trait unless two real implementations need to vary.

```rust
RagAgent::run(request, context, deps, stream) -> ChatResponse
ChatAgent::run(request, context, deps, stream) -> ChatResponse
WebSearchAgent::run(request, context, deps, stream) -> ChatResponse
```

Shared context should be explicit:

```rust
AgentContext {
  conversation_summary: Option<String>,
  recent_messages: Vec<ChatMessageExcerpt>,
  user_preferences: Vec<String>,
  locale: Option<String>,
  timezone: Option<String>,
}
```

This context is reference context. It can resolve pronouns and choose style, but it is not automatically factual evidence.

## 3. Progressive disclosure model

Progressive disclosure means each LLM call only receives the role, context, skill, schema, and evidence needed for that specific stage.

### 3.1 RAG agent stages

```text
RagAgent
  Stage A: prepare_context
    - Read conversation history for pronoun/coreference resolution.
    - Read user preferences for answer style.
    - Validate doc_scope.
    - No retrieval plan skill yet.

  Stage B: plan
    - System prompt: RAG role + basic function only.
    - Load `rag_plan_skill` only here.
    - Output must be `RagPlanDecision` schema.
    - No answer instructions.
    - No retrieved chunks.

  Stage C: retrieve
    - Deterministic code executes the plan.
    - No LLM call unless later explicitly adding query expansion.

  Stage D: answer
    - System prompt: RAG role + basic function only.
    - Load `rag_answer_skill` only after chunks are available.
    - Provide retrieved chunks/evidence bundle.
    - Output natural-language answer only.
```

This directly fixes the current issue: plan-time prompt does not contain answer behavior; answer-time prompt does not contain plan schema.

### 3.2 Chat agent stages

```text
ChatAgent
  Stage A: prepare_context
    - Read conversation history for pronoun/coreference and concise continuity summary.
    - Read user preferences for style.

  Stage B: answer
    - System prompt: chat role + basic function.
    - No retrieval plan skill.
    - No RAG schema.
    - Output natural-language answer only.
```

Chat mode should not carry RAG-specific fields, retrieval budgets, graph hints, or source grounding contracts.

### 3.3 WebSearch agent stages

```text
WebSearchAgent
  Stage A: prepare_context
    - Read conversation history for pronoun/coreference resolution.
    - Read user preferences for answer style.
    - Decide whether the user needs fresh/public web information.

  Stage B: plan_search
    - Output `WebSearchPlan` schema.
    - Choose Brave endpoint mode: web, news, image, video, llm_context, or mixed.
    - No final answer yet.

  Stage C: execute_search
    - Deterministic code calls Brave Search API.
    - Normalize Brave responses into stable internal evidence items.

  Stage D: answer
    - LLM receives normalized search evidence and answer instructions.
    - Output natural-language answer with citations/source list.
```

For trunk stability, prefer internal synthesis from Brave search/LLM-context evidence over delegating the whole answer to Brave Answers API at first. Brave Answers can remain an optional later adapter.

## 4. RAG agent schema design

Do not let the prompt hand-write a schema that can drift. The schema should have one Rust DTO as source of truth, with prompt examples generated or copied from tests.

### 4.1 Plan decision

```json
{
  "action": "execute",
  "plan": {
    "plan_version": "rag-plan-v2",
    "doc_scope": ["document-id"],
    "items": [
      {
        "kind": "semantic",
        "query": "expanded retrieval query",
        "priority": 1.0,
        "reason": "why this query is needed"
      }
    ],
    "entities": [
      { "text": "Minsky", "kind": "person" }
    ],
    "relations": [
      { "subject": "agents", "predicate": "related_to", "object": "K-lines" }
    ],
    "summary_mode": "none"
  }
}
```

Clarify branch:

```json
{
  "action": "clarify",
  "message": "one concise clarification question"
}
```

### 4.2 Retrieval item kinds

Keep the item union explicit instead of relying on “exactly one of query or bm25_terms”:

```text
semantic      -> text dense + multimodal dense candidates
lexical       -> BM25 terms for exact names, codes, file names, rare terms
relationship  -> graph/vector-graph retrieval hints
summary       -> document summary injection request
```

Minimal JSON shape:

```json
{
  "kind": "semantic|lexical|relationship|summary",
  "priority": 0.0,
  "query": "optional for semantic/relationship",
  "terms": ["optional", "for", "lexical"],
  "subject": "optional relationship subject",
  "predicate": "optional relationship predicate",
  "object": "optional relationship object",
  "reason": "short planner rationale"
}
```

This is easier for the model and easier to log. Execution code can convert it to current `ExecutePlanRequest` during migration.

## 5. Chat agent schema design

Chat agent does not need a plan schema unless product wants debug traces. The stable internal trace can be simple:

```json
{
  "agent": "chat",
  "context_used": {
    "conversation_summary": true,
    "recent_message_count": 6,
    "user_preferences_count": 2
  },
  "output_type": "natural_language"
}
```

The LLM output remains plain text.

## 6. Brave Search API modes and access methods

Base URL:

```text
https://api.search.brave.com/res/v1
```

Authentication header:

```text
X-Subscription-Token: ${BRAVE_SEARCH_API_KEY}
Accept: application/json
Accept-Encoding: gzip  # optional
```

### 6.1 Web search

Endpoint:

```text
GET  /web/search
POST /web/search
```

Use for normal ranked web results with title, URL, snippets, metadata, mixed result ordering, optional news/videos/FAQ/infobox/locations in one response.

Important params:

```text
q              required, 1-400 chars, max 50 words
country        default US, or ALL
search_lang    default en
ui_lang         e.g. en-US
count          1-20
offset         0-9
safesearch     off|moderate|strict
freshness      pd|pw|pm|py|YYYY-MM-DDtoYYYY-MM-DD
result_filter  web,news,videos,discussions,faq,infobox,locations
extra_snippets bool
operators      bool, supports site:, filetype:, intitle:, exact phrases, AND/OR/NOT
```

Connection example:

```bash
curl -s "https://api.search.brave.com/res/v1/web/search" \
  -H "Accept: application/json" \
  -H "X-Subscription-Token: ${BRAVE_SEARCH_API_KEY}" \
  -G --data-urlencode "q=rust async runtime comparison" \
     --data-urlencode "count=10" \
     --data-urlencode "freshness=pm"
```

### 6.2 News search

Endpoint:

```text
GET  /news/search
POST /news/search
```

Use for fresh news, company/product updates, date-bounded news research.

Important params:

```text
q required
country default US or ALL
search_lang default en
count 1-50
safesearch off|moderate|strict
freshness pd|pw|pm|py|YYYY-MM-DDtoYYYY-MM-DD
```

### 6.3 Image search

Endpoint from Brave API reference navigation:

```text
GET /images/search
```

Use when the user asks for visual examples, diagrams, screenshots, logos, or image sources. Keep it out of MVP answer synthesis unless the UI supports image citations.

### 6.4 Video search

Endpoint from Brave API reference navigation:

```text
GET  /videos/search
POST /videos/search
```

Use for videos/tutorials/talks/demos. Normalize result title, url, description, age, thumbnail, duration if present.

### 6.5 LLM Context

Endpoint:

```text
GET  /llm/context
POST /llm/context
```

Use for AI/RAG grounding. It returns pre-extracted page content, text/table/code chunks, and token-budgeted context. This is the best Brave mode for `websearch_agent` answer synthesis.

Important params from docs:

```text
q required
country default US or ALL
search_lang default en
count 1-50
max_tokens / context budget params
context_threshold_mode strict|balanced|lenient
```

### 6.6 Brave Answers API, optional later

Endpoint:

```text
POST /chat/completions
```

OpenAI-compatible, model `brave`, supports standard answer mode and research mode via `enable_research=true`.

Do not make this the first trunk path if the product goal is an inspectable websearch agent. It hides planning/search evidence similarly to Perplexity. Keep it as a future adapter for “fast external answer” or “deep research fallback,” not as the core agent mechanism.

## 7. WebSearch agent schema design

### 7.1 WebSearchPlan

```json
{
  "plan_version": "websearch-plan-v1",
  "query_intent": "fresh_facts|news|how_to|comparison|reference|local|visual|video",
  "answer_strategy": "search_then_synthesize",
  "searches": [
    {
      "id": "s1",
      "mode": "web|news|llm_context|image|video",
      "query": "search query",
      "params": {
        "country": "ALL",
        "search_lang": "en",
        "count": 10,
        "freshness": "pm",
        "safesearch": "moderate",
        "result_filter": ["web", "news"]
      },
      "reason": "why this search is needed"
    }
  ],
  "citation_policy": "url_required",
  "clarify_if": []
}
```

Clarify branch:

```json
{
  "action": "clarify",
  "message": "one concise clarification question"
}
```

### 7.2 Normalized evidence item

```json
{
  "source_id": "s1:r1",
  "mode": "web|news|llm_context|image|video",
  "title": "string",
  "url": "https://...",
  "snippet": "string",
  "published_or_age": "optional",
  "source_name": "optional",
  "content": "optional extracted LLM context text",
  "rank": 1,
  "search_id": "s1"
}
```

### 7.3 WebSearch answer contract

The LLM answer stage receives only:

- user query
- conversation/coreference summary
- user preferences
- `WebSearchPlan`
- normalized evidence items

Output:

```text
natural language answer + compact source list / citations
```

No raw Brave JSON should be pushed directly into the answer prompt unless debugging is enabled.

## 8. Migration plan, intentionally shallow

### Step 1: Create agent module split

Files likely to change:

```text
crates/app/src/agents/mod.rs
crates/app/src/agents/context.rs
crates/app/src/agents/rag.rs
crates/app/src/agents/chat.rs
crates/app/src/agents/websearch.rs
```

Keep current behavior reachable while moving code. Do not redesign retrieval execution yet.

Verification:

```bash
cargo test -p app main_agent
cargo test -p app chat
```

### Step 2: Replace `MainAgent::decide` with deterministic mode dispatch

Target: remove agent-like decision logic from `MainAgent`; the route/session `agent_type` chooses agent directly.

Verification: existing chat routing tests should still pass after expected rename updates.

### Step 3: Introduce `RagAgent` with progressive prompt stages

Move current RAG plan/answer code into `RagAgent`, but split prompt construction:

- base role prompt
- plan skill prompt loaded only in plan stage
- answer skill prompt loaded only in answer stage

Do not change retrieval ranking in this step.

Verification:

```bash
cargo test -p app rag
cargo test -p avrag-storage-pg chat
```

### Step 4: Introduce `ChatAgent`

Move general chat code into `ChatAgent` and strip RAG-specific envelope fields.

Verification:

```bash
cargo test -p app general
```

### Step 5: Add Brave client adapter behind current search executor

Files likely to change:

```text
crates/search/src/config.rs
crates/search/src/executor.rs
crates/search/src/provider.rs   # or split into provider_perplexity.rs / provider_brave.rs
crates/search/src/types.rs
```

Minimal config target:

```rust
SearchConfig {
  provider: "brave",
  brave_api_key: Option<String>,
}
```

Keep Perplexity code only if needed for temporary rollback. Do not expose both as a product-level agent.

Verification:

```bash
cargo test -p avrag-search
```

### Step 6: Introduce `WebSearchAgent`

Wire `WebSearchPlan -> Brave calls -> normalized evidence -> answer synthesis`.

MVP should support:

- web search
- news search
- llm_context

Defer image/video UI details unless product requires them immediately.

Verification:

```bash
cargo test -p avrag-search
cargo test -p app search
```

## 9. Explicit non-goals for this stage

- No new multi-agent orchestration framework.
- No generic tool-call agent loop.
- No hidden autonomous browsing.
- No broad refactor of graph retrieval.
- No prompt micro-optimization before schema boundaries are stable.
- No Brave Answers API as the primary path until inspectability requirements are met.

## 10. Main recommendation

Use three concrete agents and one deterministic dispatcher.

The clean trunk is:

```text
agent_type -> concrete agent -> stage-specific LLM calls -> deterministic executor -> response
```

For RAG, progressive disclosure is the key product improvement:

```text
base role/context -> plan skill/schema -> retrieve -> answer skill/evidence -> answer
```

For web search, Brave should be treated as a search/evidence provider first, not as a replacement black-box answer agent. That keeps the system inspectable and consistent with the RAG agent design.
