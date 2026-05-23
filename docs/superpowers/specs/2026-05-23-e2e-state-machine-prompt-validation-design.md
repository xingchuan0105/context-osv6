# E2E State Machine + Progressive Disclosure Validation Design

> Staging-automated end-to-end test that verifies v5 strategy state machine
> correctness and progressive-disclosure prompt injection against real LLM,
> real vector DB, and real web search.

## Goal

Verify that all three strategies (Chat/RAG/Search) traverse the correct state
machine paths, and that the system prompt sent to the LLM at each state
contains the right skill body, tool catalog (with input_schema parameters),
and format skill catalog — exactly as specified by the v5 architecture.

## Scope

- Three strategies: Chat, RAG (single-pass + replan loop), Search (single-pass + vertical escalation)
- Real external dependencies: LLM API, vector database, Brave web search
- Prompt structural correctness (not LLM-as-judge output quality)
- Staging CI automation via `cargo test --ignored`

Out of scope:
- Output quality evaluation (RAGAS, LLM-as-judge)
- Performance benchmarks
- Frontend integration

---

## Architecture

### RecordingLlmProvider

Wrap the real `LlmClient` with a trait-based provider that records every LLM
call (system prompt, messages, response) before delegating to the real client.

```
trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse>;
}

// Production: thin wrapper around LlmClient
struct RealLlmProvider { client: LlmClient }

// Test: records calls, delegates to real provider
struct RecordingLlmProvider {
    inner: Arc<dyn LlmProvider>,
    calls: Arc<Mutex<Vec<LlmCall>>>,
}
```

### LlmCall Capture

```rust
struct LlmCall {
    state_id: String,           // "Plan", "Evaluate", "Answer", etc.
    strategy: String,           // "chat", "rag", "search"
    system_prompt: String,      // Full system prompt (extracted from messages[0])
    user_messages: Vec<ChatMessage>,
    response_summary: String,   // First 500 chars of response text
    tool_calls: Vec<ToolCall>,  // Tool calls in response (if any)
    timestamp_ms: u64,
}
```

The system prompt is extracted from `messages[0]` when `role == System`.
All data is captured atomically — recording happens before the real call,
so even if the call fails, we still have the prompt.

### Test Execution Flow

```
1. Build RecordingLlmProvider wrapping real LlmClient
2. Construct UnifiedAgent with RecordingLlmProvider
3. Build AgentRequest (kind, query, doc_scope)
4. Run agent.run(request, &sink)
5. Collect:
   - calls: Vec<LlmCall> from RecordingLlmProvider
   - events: Vec<AgentEvent> from CollectingSink
   - state_history: Vec<StateRecord> from AgentRunResult
6. Assert on all three
```

---

## Test Matrix

### 1. Chat — Simple Conversation

**Input**: `kind=Chat, query="What is the capital of France?"`

**State sequence**: `Plan → ExecuteAtomic → Answer` (or `Plan → Answer` if no tools)

**Prompt assertions**:
- Plan system prompt contains `chat-plan` skill body (from `PromptRegistry::skill("chat-plan").system_prompt()`)
- Plan system prompt contains `## Available Tools` section with tool catalog from `CapabilityRegistry::plan_tools("chat")`
- Each tool entry has `### tool_name (v1.0)`, description, and `Parameters:` block with type/required/enum/description
- Answer system prompt contains `chat` skill body (from `PromptRegistry::skill("chat").system_prompt()`)
- Answer system prompt contains `## Available Output Formats` listing 4 format skills: ppt-generation, html-renderer, teaching, framework-extraction

**State history assertions**:
- `state_history` matches `ChatStrategy::schema().transitions`
- Each state has correct `state_kind` (Plan→Plan, ExecuteAtomic→Execute, Answer→Answer)

### 2. Chat — With Atomic Tool Call

**Input**: `kind=Chat, query="What is 2 + 2?"` with `preferred_tools=["calculator"]`

**State sequence**: `Plan → ExecuteAtomic → Answer`

**Additional prompt assertions**:
- Plan LLM response contains a `tool_call` to `calculator`
- After ExecuteAtomic, the tool result is injected into Answer messages
- `UntrustedInputProcessor` sanitization is applied to tool result (verify via `content_guard_trace` in degrade_trace)

### 3. RAG — Single-Pass Sufficient

**Input**: `kind=Rag, query="What is the refund policy?"`, `doc_scope=["doc-1"]`
**Prerequisite**: Vector DB contains a document with refund policy text that scores high enough for `Sufficient` evaluation.

**State sequence**: `Plan → ExecuteRetrieve → Evaluate → Answer`

**Prompt assertions**:
- Plan system prompt contains `rag-plan` skill body + 7 retrieval tools (dense_retrieval, lexical_retrieval, graph_retrieval, etc.) with input_schema parameters
- Evaluate system prompt contains `rag-eval` skill body + tool catalog (for replan suggestions)
- Answer system prompt contains `rag-answer` skill body + format skills catalog

**State history assertions**:
- 4 states, no replan loop
- Budget used: 1 iteration

### 4. RAG — Evaluate Insufficient → Re-execute

**Input**: `kind=Rag, query="Compare the pricing of Plan A and Plan B"`, `doc_scope=["doc-1"]`
**Prerequisite**: Vector DB contains Plan A info but not Plan B, forcing `Insufficient` evaluation.

**State sequence**: `Plan → ExecuteRetrieve → Evaluate → ExecuteRetrieve → Answer`

**Key assertions**:
- Evaluate LLM response contains `"decision": "insufficient"` + `next_actions` array
- Second ExecuteRetrieve uses `current_plan_calls` from evaluation (not a new Plan LLM call)
- Total LLM calls: Plan(1) + Evaluate(1) + Answer(1) = 3 (no second Plan)
- Budget used: 2 iterations

**Schema validation**:
- `Evaluate → ExecuteRetrieve` transition exists in `RagStrategy::schema().transitions`

### 5. Search — Single-Pass

**Input**: `kind=Search, query="What is the latest Rust release?"`
**Prerequisite**: Brave API returns relevant results.

**State sequence**: `Decompose → ParallelSearch → Aggregate → Evaluate → Answer`

**Prompt assertions**:
- Decompose (Plan) system prompt contains `search-plan` skill body + `web_search` tool with input_schema
- Evaluate system prompt contains `search-eval` skill body + tool catalog
- Answer system prompt contains `search-answer` skill body + format skills catalog

### 6. Search — Vertical Escalation

**Input**: `kind=Search, query="latest AI news from today"`, no specific vertical
**Prerequisite**: Results trigger `EscalateVertical` in evaluation.

**State sequence**: includes `Evaluate → next_vertical` or `Evaluate → ParallelSearch` replan

**Key assertions**:
- Evaluate LLM response contains `"decision": "insufficient"`
- `map_search_strategy_to_advice` produces `EscalateVertical` or `Replan`
- State history reflects the chosen path

---

## Progressive Disclosure Verification

Each `LlmCall.system_prompt` is verified at three tiers:

### Tier 1: Index (Tool Catalog — ~50 tokens per tool)

```rust
// For each tool in CapabilityRegistry::plan_tools(strategy):
assert!(system_prompt.contains(&format!("### {} (v{})", tool.id, tool.version)));
assert!(system_prompt.contains(&tool.description));
```

### Tier 2: Load (Skill Body — ~500-5000 tokens)

```rust
// Plan prompt contains the planner skill full text
let planner_body = PromptRegistry::skill(planner_skill_id).system_prompt();
assert!(system_prompt.contains(&planner_body));
```

### Tier 3: Schema (Tool Parameters)

```rust
// Each tool has a Parameters: block with input_schema properties
for tool in plan_tools {
    if let Some(props) = tool.input_schema.get("properties") {
        assert!(system_prompt.contains("Parameters:"));
        for (name, schema) in props.as_object().unwrap() {
            let ty = schema.get("type").unwrap().as_str().unwrap();
            assert!(system_prompt.contains(&format!("{}: {}", name, ty)));
        }
    }
}
```

### Format Skills Catalog (Answer Phase)

```rust
let answer_call = calls.iter().find(|c| c.state_id == "Answer").unwrap();
assert!(answer_call.system_prompt.contains("## Available Output Formats"));
for skill_id in ["ppt-generation", "html-renderer", "teaching", "framework-extraction"] {
    assert!(answer_call.system_prompt.contains(skill_id));
}
```

### Strategy Isolation

```rust
// rag-plan only appears in RAG Plan calls
for call in &calls {
    if call.strategy != "rag" {
        assert!(!call.system_prompt.contains(
            &PromptRegistry::skill("rag-plan").unwrap().system_prompt()
        ));
    }
}
```

---

## State Machine Verification

### Transition Validity

```rust
fn assert_valid_transitions(
    schema: &StrategySchema,
    state_history: &[StateRecord],
) {
    for window in state_history.windows(2) {
        let from = &window[0].state_id;
        let to = &window[1].state_id;
        let valid = schema.transitions.iter().any(|t| t.from == from && t.to == to);
        assert!(valid, "invalid transition: {} → {}", from, to);
    }
}
```

### Budget Tracking

```rust
// RAG replan scenario
assert_eq!(result.budget_used.unwrap().current, 2);
assert!(result.budget_used.unwrap().current <= result.budget_used.unwrap().max);
```

### State Kind Correctness

```rust
for record in &state_history {
    match record.state_id.as_str() {
        "Plan" | "Decompose" => assert_eq!(record.state_kind, "Plan"),
        "ExecuteAtomic" | "ExecuteRetrieve" | "ParallelSearch" => assert_eq!(record.state_kind, "Execute"),
        "Evaluate" | "Aggregate" => assert_eq!(record.state_kind, "Evaluate"),
        "Answer" => assert_eq!(record.state_kind, "Answer"),
        _ => {}
    }
}
```

---

## File Structure

```
crates/app/tests/e2e/
├── mod.rs                        // Shared fixtures, RecordingLlmProvider, helpers
├── chat_state_machine.rs         // Chat strategy tests (2 scenarios)
├── rag_state_machine.rs          // RAG strategy tests (2 scenarios)
└── search_state_machine.rs       // Search strategy tests (2 scenarios)
```

### mod.rs — Shared Infrastructure

```rust
pub mod recording_llm;
pub mod assertions;

// Environment config
pub struct E2EConfig {
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub brave_api_key: Option<String>,
    pub vector_db_url: Option<String>,
}

impl E2EConfig {
    pub fn from_env() -> Self { ... }
    pub fn is_available(&self) -> bool { ... }
}
```

### recording_llm.rs — LlmProvider Trait + Recording Wrapper

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse>;
}

pub struct RecordingLlmProvider {
    inner: Arc<dyn LlmProvider>,
    calls: Arc<Mutex<Vec<LlmCall>>>,
}

impl RecordingLlmProvider {
    pub fn calls(&self) -> Vec<LlmCall> { ... }
    pub fn calls_by_state(&self, state_id: &str) -> Vec<LlmCall> { ... }
}
```

### assertions.rs — Reusable Assertion Helpers

```rust
pub fn assert_valid_transitions(schema: &StrategySchema, history: &[StateRecord]);
pub fn assert_prompt_contains_skill(prompt: &str, skill_id: &str);
pub fn assert_prompt_has_tool_catalog(prompt: &str, tools: &[&ToolMetadata]);
pub fn assert_prompt_has_format_skills(prompt: &str);
pub fn assert_state_kinds(history: &[StateRecord]);
pub fn assert_budget_usage(result: &AgentRunResult, max_expected: u32);
```

---

## Staging Configuration

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `E2E_LLM_BASE_URL` | Yes | LLM API base URL |
| `E2E_LLM_API_KEY` | Yes | LLM API key |
| `E2E_LLM_MODEL` | Yes | Model name (e.g. `gpt-4o-mini`) |
| `E2E_BRAVE_API_KEY` | For Search | Brave web search API key |
| `E2E_VECTOR_DB_URL` | For RAG | Vector database connection URL |

### Test Attributes

All tests marked with:
```rust
#[tokio::test]
#[ignore = "requires staging environment (LLM + external services)"]
async fn test_name() { ... }
```

### CI Trigger

```yaml
# GitHub Actions
staging-e2e:
  if: github.event_name == 'workflow_dispatch' || github.ref == 'refs/heads/main'
  steps:
    - run: cargo test --ignored -p app --test e2e
      env:
        E2E_LLM_BASE_URL: ${{ secrets.E2E_LLM_BASE_URL }}
        E2E_LLM_API_KEY: ${{ secrets.E2E_LLM_API_KEY }}
        E2E_LLM_MODEL: gpt-4o-mini
        E2E_BRAVE_API_KEY: ${{ secrets.E2E_BRAVE_API_KEY }}
```

---

## Prerequisites

### LlmClient Trait Extraction

`LlmClient` is currently a concrete struct with `#[derive(Clone)]`. To inject
`RecordingLlmProvider`, we need a trait boundary.

**Approach**: Add `LlmProvider` trait in `crates/llm/src/lib.rs` with
`complete()` method. Implement it for `LlmClient` (zero-cost wrapper).
Strategies already hold `LlmClient` — change to `Arc<dyn LlmProvider>`.

**Impact**:
- `ChatStrategy.llm`, `RagStrategy.llm`, `SearchStrategy.llm` change from `LlmClient` to `Arc<dyn LlmProvider>`
- `UnifiedAgent` constructor accepts `Option<Arc<dyn LlmProvider>>`
- Production code wraps `LlmClient` in `RealLlmProvider`

This is a small, mechanical refactor affecting ~10 call sites.

### Vector DB Fixture

RAG tests need a vector DB with known documents. Options:
1. Use the existing staging vector DB with pre-seeded documents
2. Create a test namespace/index per test run, seed fixture documents, tear down after

**Recommended**: Option 2 — each test creates an isolated namespace, uploads
2-3 fixture documents (e.g. `refund_policy.md`, `pricing_plan_a.md`), runs
the agent, then deletes the namespace. This keeps tests self-contained.

### Brave Search Determinism

Brave API results change over time. Tests should:
1. Use very specific queries that reliably return relevant results
2. Assert on structural properties (non-empty results, citations present) rather than specific content
3. Accept that some tests may be flaky — use retry logic in CI

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| LLM returns unexpected format → test fails | Medium | Low | Assertions are structural (contains), not exact match |
| Brave API rate limit in CI | Low | Medium | Rate limit between test runs; use cache |
| LlmClient trait refactor breaks production | Low | High | Trait impl for LlmClient is zero-cost; production wraps existing client |
| Staging env secrets leak | Low | Critical | Use GitHub secrets; never log API keys |
| Vector DB fixture setup fails | Medium | Medium | Skip RAG tests if fixture setup fails with clear error message |
| LLM latency makes tests slow | Low | Low | Use small model (gpt-4o-mini); 6 tests × ~3 LLM calls = ~18 calls, ~10s total |

---

## Success Criteria

1. All 6 scenarios pass against staging environment
2. Every state transition in `state_history` matches `StrategySchema.transitions`
3. Every LLM call's system prompt contains the correct skill body and tool catalog
4. RAG replan scenario demonstrates `Evaluate → ExecuteRetrieve` (no second Plan LLM call)
5. Progressive disclosure verified at all three tiers (Index, Load, Schema)
6. Tests runnable via `cargo test --ignored -p app --test e2e`
7. CI pipeline triggers on nightly + manual dispatch
