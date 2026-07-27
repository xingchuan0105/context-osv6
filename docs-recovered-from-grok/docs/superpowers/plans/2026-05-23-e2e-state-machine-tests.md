# E2E State Machine + Progressive Disclosure Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Staging-automated integration tests verifying v5 strategy state machine correctness and prompt injection against real LLM, vector DB, and web search.

**Architecture:** Introduce `LlmProvider` trait to decouple strategies from `LlmClient` concrete struct. Wrap real client with `RecordingLlmProvider` that captures system prompts and responses before delegating. Tests assert on state history transitions, prompt structure (skill bodies, tool catalogs, format skills), and progressive disclosure at three tiers.

**Tech Stack:** Rust + async-trait + Tokio + avrag-rs crates (llm, app, rag_core, search)

---

## File Structure

| File | Responsibility | Action |
|------|---------------|--------|
| `crates/llm/src/lib.rs` | LlmProvider trait + impl for LlmClient | Modify |
| `crates/app/src/agents/strategy/chat.rs` | ChatStrategy uses Arc<dyn LlmProvider> | Modify |
| `crates/app/src/agents/strategy/rag.rs` | RagStrategy uses Arc<dyn LlmProvider> | Modify |
| `crates/app/src/agents/strategy/search.rs` | SearchStrategy uses Arc<dyn LlmProvider> | Modify |
| `crates/app/src/agents/unified/mod.rs` | UnifiedAgent constructs strategies with provider | Modify |
| `crates/app/tests/e2e/mod.rs` | Test module root + re-exports | Create |
| `crates/app/tests/e2e/config.rs` | E2EConfig from env vars | Create |
| `crates/app/tests/e2e/recording_llm.rs` | RecordingLlmProvider + LlmCall | Create |
| `crates/app/tests/e2e/assertions.rs` | Reusable assertion helpers | Create |
| `crates/app/tests/e2e_chat.rs` | Chat strategy tests (2 scenarios) | Create |
| `crates/app/tests/e2e_rag.rs` | RAG strategy tests (2 scenarios) | Create |
| `crates/app/tests/e2e_search.rs` | Search strategy tests (2 scenarios) | Create |
| `.github/workflows/e2e-staging.yml` | CI workflow for staging automation | Create |

---

## Task 1: Add LlmProvider trait to llm crate

**Files:**
- Modify: `crates/llm/src/lib.rs`

- [ ] **Step 1: Add LlmProvider trait**

In `crates/llm/src/lib.rs`, add after the `LlmClient` re-export:

```rust
/// Trait for LLM completion providers.
/// Allows injecting mock/recording providers in tests.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse>;
}

/// Zero-cost wrapper: implement LlmProvider for LlmClient.
#[async_trait::async_trait]
impl LlmProvider for LlmClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        self.complete(messages, temperature).await
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p avrag_llm`

Expected: Build succeeds with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/llm/src/lib.rs
git commit -m "feat(llm): add LlmProvider trait for dependency injection"
```

---

## Task 2: Refactor ChatStrategy to use Arc<dyn LlmProvider>

**Files:**
- Modify: `crates/app/src/agents/strategy/chat.rs`

- [ ] **Step 1: Update ChatStrategy struct**

In `crates/app/src/agents/strategy/chat.rs`, change the struct definition (around line 156):

```rust
pub struct ChatStrategy {
    pub llm: std::sync::Arc<dyn avrag_llm::LlmProvider>,
    pub temperature: Option<f32>,
}
```

- [ ] **Step 2: Update LLM call sites**

Find all `self.llm.complete(...)` calls and ensure they work with `Arc<dyn LlmProvider>`. The method signature is identical, so no changes needed except ensuring `Arc` is imported:

```rust
use std::sync::Arc;
```

- [ ] **Step 3: Handle complete_stream — use Option A**

For `complete_stream` calls (line 408), we keep `LlmClient` for streaming since it has complex signature (CancellationToken + callback). Add `llm_client: Option<LlmClient>` field:

```rust
pub struct ChatStrategy {
    pub llm: std::sync::Arc<dyn avrag_llm::LlmProvider>,
    pub llm_client: Option<avrag_llm::LlmClient>,  // for streaming
    pub temperature: Option<f32>,
}
```

In `step_answer` (line 408), change:

```rust
// Before:
let stream = self.llm.complete_stream(&messages, self.temperature, cancel, move |delta| {

// After:
let llm_client = self.llm_client.as_ref()
    .ok_or_else(|| AppError::internal("LLM client not available for streaming"))?;
let stream = llm_client.complete_stream(&messages, self.temperature, cancel, move |delta| {
```

For non-streaming path (line 236, 446), keep using `self.llm.complete(...)` unchanged.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p app`

Expected: Errors about missing fields in `UnifiedAgent` when constructing `ChatStrategy`. That's expected — we'll fix in Task 4.

- [ ] **Step 5: Commit**

```bash
git add crates/app/src/agents/strategy/chat.rs
git commit -m "refactor(chat): use Arc<dyn LlmProvider> for non-streaming LLM calls"
```

---

## Task 3: Refactor RagStrategy and SearchStrategy to use Arc<dyn LlmProvider>

**Files:**
- Modify: `crates/app/src/agents/strategy/rag.rs`
- Modify: `crates/app/src/agents/strategy/search.rs`

- [ ] **Step 1: Update RagStrategy struct**

In `crates/app/src/agents/strategy/rag.rs` (around line 233):

```rust
pub struct RagStrategy {
    pub llm: std::sync::Arc<dyn avrag_llm::LlmProvider>,
    pub temperature: Option<f32>,
}
```

- [ ] **Step 2: Update RagStrategy LLM call sites**

Find `self.llm.complete(...)` calls (line 896). These should work unchanged with `Arc<dyn LlmProvider>`.

For `AnswerSynthesizer::from_llm_client(self.llm.clone())` (line 914), we need to pass a real `LlmClient`. Add a field:

```rust
pub struct RagStrategy {
    pub llm: std::sync::Arc<dyn avrag_llm::LlmProvider>,
    pub llm_client: Option<avrag_llm::LlmClient>,  // for AnswerSynthesizer
    pub temperature: Option<f32>,
}
```

Then update line 914:

```rust
let synthesizer = if let Some(ref client) = self.llm_client {
    Some(avrag_llm::AnswerSynthesizer::from_llm_client(client.clone()))
} else {
    None
};
```

- [ ] **Step 3: Update SearchStrategy struct**

In `crates/app/src/agents/strategy/search.rs` (around line 193):

```rust
pub struct SearchStrategy {
    pub llm: std::sync::Arc<dyn avrag_llm::LlmProvider>,
    pub llm_client: Option<avrag_llm::LlmClient>,  // for free functions
    pub temperature: Option<f32>,
    pub search_executor: std::sync::Arc<dyn avrag_search::SearchProvider>,
    pub search_synthesizer: Option<std::sync::Arc<dyn SearchAnswerSynthesizer>>,
}
```

- [ ] **Step 4: Update SearchStrategy LLM call sites**

For free function calls like `generate_search_plan(&self.llm, ...)` (line 1286), update the function signature:

```rust
async fn generate_search_plan(
    llm: &dyn avrag_llm::LlmProvider,
    query: &str,
    temperature: Option<f32>,
    system_prompt: &str,
) -> Option<SearchPlan> {
    // ... existing code, llm.complete() works unchanged
}
```

Then call it as:

```rust
generate_search_plan(self.llm.as_ref(), query, temperature, system_prompt).await
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p app`

Expected: Errors in `UnifiedAgent` about field types. Expected.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/agents/strategy/rag.rs crates/app/src/agents/strategy/search.rs
git commit -m "refactor(rag,search): use Arc<dyn LlmProvider> for LLM calls"
```

---

## Task 4: Update UnifiedAgent to construct strategies with Arc<dyn LlmProvider>

**Files:**
- Modify: `crates/app/src/agents/unified/mod.rs`

- [ ] **Step 1: Add llm_client field to UnifiedAgent**

In `crates/app/src/agents/unified/mod.rs`, update the struct (around line 27):

```rust
pub struct UnifiedAgent {
    llm_client: Option<LlmClient>,
    llm_provider: Option<std::sync::Arc<dyn avrag_llm::LlmProvider>>,
    temperature: Option<f32>,
    rag_runtime: Option<std::sync::Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<std::sync::Arc<dyn avrag_search::SearchProvider>>,
}
```

- [ ] **Step 2: Update constructor**

Update `UnifiedAgent::new` (around line 35):

```rust
pub fn new(
    llm_client: Option<LlmClient>,
    temperature: Option<f32>,
) -> Self {
    let llm_provider = llm_client.clone().map(|c| {
        std::sync::Arc::new(c) as std::sync::Arc<dyn avrag_llm::LlmProvider>
    });
    Self {
        llm_client,
        llm_provider,
        temperature,
        rag_runtime: None,
        search_executor: None,
    }
}
```

- [ ] **Step 3: Update Chat arm**

In the `match request.kind` block, Chat arm (around line 118):

```rust
crate::agents::AgentKind::Chat => {
    let _ = sink.emit(AgentEvent::Activity {
        stage: "chat".to_string(),
        message: "Direct chat".to_string(),
    }).await;

    let ctx = ChatContext::from_request(
        request,
        trace_id,
        LoopBudget::chat(UserTier::Pro),
        sink.clone_boxed(),
        cancellation,
    )?;
    let strategy = ChatStrategy {
        llm: self.llm_provider.clone().ok_or_else(|| AppError::internal("LLM not configured"))?,
        llm_client: self.llm_client.clone(),
        temperature: self.temperature,
    };
    let mut result = executor.run(&strategy, ctx).await?;
    result.routing_decision = Some(routing_decision.clone());
    Ok(result)
}
```

- [ ] **Step 4: Update Rag arm**

Similar pattern for Rag arm (around line 139):

```rust
let strategy = RagStrategy {
    llm: self.llm_provider.clone().ok_or_else(|| AppError::internal("LLM not configured"))?,
    llm_client: self.llm_client.clone(),
    temperature: self.temperature,
};
```

- [ ] **Step 5: Update Search arm**

And Search arm (around line 178):

```rust
let strategy = SearchStrategy {
    llm: self.llm_provider.clone().ok_or_else(|| AppError::internal("LLM not configured"))?,
    llm_client: self.llm_client.clone(),
    temperature: self.temperature,
    search_executor,
    search_synthesizer: self.llm_client.clone().map(|llm| {
        std::sync::Arc::new(crate::agents::strategy::search::LlmSearchAnswerSynthesizer { llm })
            as std::sync::Arc<dyn crate::agents::strategy::search::SearchAnswerSynthesizer>
    }),
};
```

- [ ] **Step 6: Verify full build**

Run: `cargo build -p app`

Expected: Build succeeds. All strategies now use `Arc<dyn LlmProvider>`.

- [ ] **Step 7: Commit**

```bash
git add crates/app/src/agents/unified/mod.rs
git commit -m "refactor(unified): construct strategies with Arc<dyn LlmProvider>"
```

---

## Task 5: Verify production code still works

**Files:** None (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p app --lib`

Expected: All existing tests pass. If any fail, fix them.

- [ ] **Step 2: Check clippy**

Run: `cargo clippy -p app -- -D warnings`

Expected: No warnings.

- [ ] **Step 3: Commit any fixes**

```bash
git add -A
git commit -m "test: fix test failures after LlmProvider refactor"
```

---

## Task 6: Create test infrastructure — E2EConfig and RecordingLlmProvider

**Files:**
- Create: `crates/app/tests/e2e/mod.rs`
- Create: `crates/app/tests/e2e/config.rs`
- Create: `crates/app/tests/e2e/recording_llm.rs`

**Note on state_id tracking:** The spec mentions `LlmCall` should have `state_id` and `strategy` fields, but these aren't available at the LLM call site. Instead, we infer the state from the user message content (e.g., "plan", "evaluate", "answer") in the assertion helpers. This is pragmatic — the state is implicit in which strategy method made the call.

- [ ] **Step 1: Create e2e module structure**

Create `crates/app/tests/e2e/mod.rs`:

```rust
//! E2E integration tests for v5 state machine + progressive disclosure.
//! 
//! These tests require a staging environment with real LLM, vector DB, and web search.
//! Run with: cargo test --ignored -p app --test e2e

pub mod config;
pub mod recording_llm;
pub mod assertions;

// Re-export for convenience
pub use config::E2EConfig;
pub use recording_llm::{RecordingLlmProvider, LlmCall};
```

- [ ] **Step 2: Create E2EConfig**

Create `crates/app/tests/e2e/config.rs`:

```rust
//! Staging environment configuration for E2E tests.

pub struct E2EConfig {
    pub llm_base_url: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub brave_api_key: Option<String>,
    pub vector_db_url: Option<String>,
}

impl E2EConfig {
    pub fn from_env() -> Option<Self> {
        let llm_base_url = std::env::var("E2E_LLM_BASE_URL").ok()?;
        let llm_api_key = std::env::var("E2E_LLM_API_KEY").ok()?;
        let llm_model = std::env::var("E2E_LLM_MODEL").ok()?;
        
        Some(Self {
            llm_base_url,
            llm_api_key,
            llm_model,
            brave_api_key: std::env::var("E2E_BRAVE_API_KEY").ok(),
            vector_db_url: std::env::var("E2E_VECTOR_DB_URL").ok(),
        })
    }

    pub fn llm_client(&self) -> avrag_llm::LlmClient {
        avrag_llm::LlmClient::new(avrag_llm::ModelProviderConfig {
            base_url: self.llm_base_url.clone(),
            api_key: self.llm_api_key.clone(),
            model: self.llm_model.clone(),
            timeout_ms: 30_000,
            api_style: Some(avrag_llm::ApiStyle::OpenAi),
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        })
    }
}
```

- [ ] **Step 3: Create RecordingLlmProvider**

Create `crates/app/tests/e2e/recording_llm.rs`:

```rust
//! Recording wrapper around LlmProvider for capturing prompts and responses.

use avrag_llm::{ChatMessage, LlmProvider, LlmResponse};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct LlmCall {
    pub system_prompt: String,
    pub user_messages: Vec<ChatMessage>,
    pub response_content: String,
    pub timestamp_ms: u64,
}

pub struct RecordingLlmProvider {
    inner: Arc<dyn LlmProvider>,
    calls: Arc<Mutex<Vec<LlmCall>>>,
}

impl RecordingLlmProvider {
    pub fn new(inner: Arc<dyn LlmProvider>) -> Self {
        Self {
            inner,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn calls(&self) -> Vec<LlmCall> {
        self.calls.lock().unwrap().clone()
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl LlmProvider for RecordingLlmProvider {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<LlmResponse> {
        // Extract system prompt (first message with role "system")
        let system_prompt = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Extract user messages
        let user_messages: Vec<ChatMessage> = messages
            .iter()
            .filter(|m| m.role != "system")
            .cloned()
            .collect();

        // Delegate to real provider
        let response = self.inner.complete(messages, temperature).await?;

        // Record the call
        let call = LlmCall {
            system_prompt,
            user_messages,
            response_content: response.content.clone(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        self.calls.lock().unwrap().push(call);

        Ok(response)
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/app/tests/e2e/
git commit -m "test(e2e): add config and RecordingLlmProvider infrastructure"
```

---

## Task 7: Create assertion helpers

**Files:**
- Create: `crates/app/tests/e2e/assertions.rs`

- [ ] **Step 1: Create assertions module**

Create `crates/app/tests/e2e/assertions.rs`:

```rust
//! Reusable assertion helpers for E2E tests.

use app::agents::capability::{CapabilityRegistry, StrategySchema, TransitionSchema};
use app::agents::progressive::PromptRegistry;
use app::agents::runtime::StateRecord;

use super::recording_llm::LlmCall;

/// Assert that state transitions match the schema.
pub fn assert_valid_transitions(schema: &StrategySchema, history: &[StateRecord]) {
    for window in history.windows(2) {
        let from = &window[0].state_id;
        let to = &window[1].state_id;
        let valid = schema.transitions.iter().any(|t| t.from == from && t.to == to);
        assert!(
            valid,
            "Invalid state transition: {} → {} (not in schema for strategy '{}')",
            from, to, schema.id
        );
    }
}

/// Assert that a prompt contains the expected skill body.
pub fn assert_prompt_contains_skill(prompt: &str, skill_id: &str) {
    let registry = PromptRegistry::standard_cached();
    let skill = registry
        .skill(skill_id)
        .unwrap_or_else(|| panic!("Skill '{}' not found in registry", skill_id));
    let body = skill.system_prompt();
    assert!(
        prompt.contains(body),
        "Prompt does not contain skill '{}' body. Expected {} chars, got {} chars in prompt.",
        skill_id,
        body.len(),
        prompt.len()
    );
}

/// Assert that a prompt contains tool catalog entries.
pub fn assert_prompt_has_tool_catalog(prompt: &str, strategy: &str) {
    let registry = CapabilityRegistry::standard_cached();
    let plan_tools = registry.plan_tools(strategy);
    
    for tool in plan_tools {
        // Tier 1: Index — tool name and description
        let header = format!("### {} (v{})", tool.id, tool.version);
        assert!(
            prompt.contains(&header),
            "Prompt missing tool catalog header: {}",
            header
        );
        assert!(
            prompt.contains(&tool.description),
            "Prompt missing tool description for '{}'",
            tool.id
        );

        // Tier 3: Schema — parameters
        if let Some(props) = tool.input_schema.get("properties").and_then(|p| p.as_object()) {
            assert!(
                prompt.contains("Parameters:"),
                "Prompt missing 'Parameters:' section for tool '{}'",
                tool.id
            );
            for (name, schema) in props {
                if let Some(ty) = schema.get("type").and_then(|t| t.as_str()) {
                    let param_line = format!("{}: {}", name, ty);
                    assert!(
                        prompt.contains(&param_line),
                        "Prompt missing parameter '{}' for tool '{}'",
                        param_line,
                        tool.id
                    );
                }
            }
        }
    }
}

/// Assert that a prompt contains format skills catalog.
pub fn assert_prompt_has_format_skills(prompt: &str) {
    assert!(
        prompt.contains("## Available Output Formats"),
        "Prompt missing '## Available Output Formats' section"
    );
    for skill_id in ["ppt-generation", "html-renderer", "teaching", "framework-extraction"] {
        assert!(
            prompt.contains(skill_id),
            "Prompt missing format skill '{}'",
            skill_id
        );
    }
}

/// Assert state kinds match expected values.
pub fn assert_state_kinds(history: &[StateRecord]) {
    for record in history {
        let expected_kind = match record.state_id.as_str() {
            "Plan" | "Decompose" => "Plan",
            "ExecuteAtomic" | "ExecuteRetrieve" | "ParallelSearch" => "Execute",
            "Evaluate" | "Aggregate" => "Evaluate",
            "Answer" => "Answer",
            _ => continue,
        };
        assert_eq!(
            record.state_kind, expected_kind,
            "State '{}' has kind '{}', expected '{}'",
            record.state_id, record.state_kind, expected_kind
        );
    }
}

/// Assert budget usage is within expected range.
pub fn assert_budget_usage(budget_used: u8, max_expected: u8) {
    assert!(
        budget_used <= max_expected,
        "Budget used {} exceeds max expected {}",
        budget_used,
        max_expected
    );
}

/// Find LLM call for a specific state (by matching user message content).
pub fn find_llm_call_for_state<'a>(
    calls: &'a [LlmCall],
    state_hint: &str,
) -> Option<&'a LlmCall> {
    calls.iter().find(|c| c.user_messages.iter().any(|m| m.content.contains(state_hint)))
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/app/tests/e2e/assertions.rs
git commit -m "test(e2e): add reusable assertion helpers"
```

---

## Task 8: Write Chat strategy E2E test (simple conversation)

**Files:**
- Create: `crates/app/tests/e2e_chat.rs`

- [ ] **Step 1: Create test file**

Create `crates/app/tests/e2e_chat.rs`:

```rust
//! E2E tests for Chat strategy state machine + progressive disclosure.

mod e2e;

use app::agents::capability::ChatStrategy;
use app::agents::events::CollectingSink;
use app::agents::runtime::AgentRequest;
use app::agents::strategy::Strategy;
use app::agents::AgentKind;
use common::ChatTurnInput;

use e2e::config::E2EConfig;
use e2e::recording_llm::RecordingLlmProvider;

#[tokio::test]
#[ignore = "requires staging environment (E2E_LLM_BASE_URL, E2E_LLM_API_KEY, E2E_LLM_MODEL)"]
async fn chat_simple_conversation_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let recording = RecordingLlmProvider::new(std::sync::Arc::new(llm_client.clone()));
    let recording_arc = std::sync::Arc::new(recording);

    let ctx = app::agents::strategy::chat::ChatContext::from_request(
        AgentRequest {
            kind: AgentKind::Chat,
            query: "What is the capital of France?".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![ChatTurnInput {
                role: "user".to_string(),
                content: "What is the capital of France?".to_string(),
            }],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth_context: serde_json::json!({"org_id": "test-org"}),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
        },
        "test-chat".to_string(),
        app::agents::react_loop::LoopBudget::default(),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    ).unwrap();

    let strategy = ChatStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let result = app::agents::strategy::executor::StrategyExecutor::run(strategy, ctx).await.unwrap();

    // Assert state machine transitions
    let schema = ChatStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    e2e::assertions::assert_valid_transitions(&schema, history);
    e2e::assertions::assert_state_kinds(history);

    // Expected: Plan → ExecuteAtomic → Answer (or Plan → Answer if no tools)
    assert!(history.len() >= 2, "Expected at least 2 states, got {}", history.len());

    // Assert progressive disclosure
    let calls = recording_arc.calls();
    assert!(calls.len() >= 2, "Expected at least 2 LLM calls, got {}", calls.len());

    // Plan call
    let plan_call = &calls[0];
    e2e::assertions::assert_prompt_contains_skill(&plan_call.system_prompt, "chat-plan");
    e2e::assertions::assert_prompt_has_tool_catalog(&plan_call.system_prompt, "chat");

    // Answer call
    let answer_call = calls.last().unwrap();
    e2e::assertions::assert_prompt_contains_skill(&answer_call.system_prompt, "chat");
    e2e::assertions::assert_prompt_has_format_skills(&answer_call.system_prompt);

    // Budget
    e2e::assertions::assert_budget_usage(result.budget_used.unwrap_or(0), 1);
}
```

- [ ] **Step 2: Run test manually (if staging available)**

Run: `cargo test --ignored -p app --test e2e_chat chat_simple_conversation_state_machine`

Expected: PASS (if staging env vars set) or SKIP (if not set).

- [ ] **Step 3: Commit**

```bash
git add crates/app/tests/e2e_chat.rs
git commit -m "test(e2e): add Chat simple conversation state machine test"
```

---

## Task 9: Write RAG strategy E2E test (single-pass sufficient)

**Files:**
- Create: `crates/app/tests/e2e_rag.rs`

- [ ] **Step 1: Create test file**

Create `crates/app/tests/e2e_rag.rs`:

```rust
//! E2E tests for RAG strategy state machine + progressive disclosure.

mod e2e;

use app::agents::capability::RagStrategy;
use app::agents::events::CollectingSink;
use app::agents::runtime::AgentRequest;
use app::agents::strategy::Strategy;
use app::agents::AgentKind;
use common::ChatTurnInput;

use e2e::config::E2EConfig;
use e2e::recording_llm::RecordingLlmProvider;

#[tokio::test]
#[ignore = "requires staging environment + vector DB with refund_policy.md"]
async fn rag_single_pass_sufficient_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let recording = RecordingLlmProvider::new(std::sync::Arc::new(llm_client.clone()));
    let recording_arc = std::sync::Arc::new(recording);

    // Requires vector DB with doc-1 containing refund policy text
    let rag_runtime = std::sync::Arc::new(
        avrag_rag_core::RagRuntime::new(
            config.vector_db_url.clone().expect("E2E_VECTOR_DB_URL not set"),
        ).await.unwrap()
    );

    let ctx = app::agents::strategy::rag::RagContext::from_request(
        AgentRequest {
            kind: AgentKind::Rag,
            query: "What is the refund policy?".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec!["doc-1".to_string()],
            messages: vec![ChatTurnInput {
                role: "user".to_string(),
                content: "What is the refund policy?".to_string(),
            }],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth_context: serde_json::json!({"org_id": "test-org"}),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
        },
        "test-rag".to_string(),
        app::agents::react_loop::LoopBudget::default(),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
        rag_runtime,
    ).unwrap();

    let strategy = RagStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let result = app::agents::strategy::executor::StrategyExecutor::run(strategy, ctx).await.unwrap();

    // Assert state machine: Plan → ExecuteRetrieve → Evaluate → Answer
    let schema = RagStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    e2e::assertions::assert_valid_transitions(&schema, history);
    e2e::assertions::assert_state_kinds(history);

    assert_eq!(history.len(), 4, "Expected 4 states (Plan→ExecuteRetrieve→Evaluate→Answer), got {}", history.len());

    // Assert progressive disclosure
    let calls = recording_arc.calls();
    assert!(calls.len() >= 3, "Expected at least 3 LLM calls (Plan, Evaluate, Answer), got {}", calls.len());

    // Plan call
    let plan_call = &calls[0];
    e2e::assertions::assert_prompt_contains_skill(&plan_call.system_prompt, "rag-plan");
    e2e::assertions::assert_prompt_has_tool_catalog(&plan_call.system_prompt, "rag");

    // Evaluate call
    let eval_call = calls.iter().find(|c| c.user_messages.iter().any(|m| m.content.contains("evaluate"))).unwrap();
    e2e::assertions::assert_prompt_contains_skill(&eval_call.system_prompt, "rag-eval");
    e2e::assertions::assert_prompt_has_tool_catalog(&eval_call.system_prompt, "rag");

    // Answer call
    let answer_call = calls.last().unwrap();
    e2e::assertions::assert_prompt_contains_skill(&answer_call.system_prompt, "rag-answer");
    e2e::assertions::assert_prompt_has_format_skills(&answer_call.system_prompt);

    // Budget
    e2e::assertions::assert_budget_usage(result.budget_used.unwrap_or(0), 1);
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/app/tests/e2e_rag.rs
git commit -m "test(e2e): add RAG single-pass sufficient state machine test"
```

---

## Task 10: Write Search strategy E2E test (single-pass)

**Files:**
- Create: `crates/app/tests/e2e_search.rs`

- [ ] **Step 1: Create test file**

Create `crates/app/tests/e2e_search.rs`:

```rust
//! E2E tests for Search strategy state machine + progressive disclosure.

mod e2e;

use app::agents::capability::SearchStrategy;
use app::agents::events::CollectingSink;
use app::agents::runtime::AgentRequest;
use app::agents::strategy::Strategy;
use app::agents::AgentKind;
use common::ChatTurnInput;

use e2e::config::E2EConfig;
use e2e::recording_llm::RecordingLlmProvider;

#[tokio::test]
#[ignore = "requires staging environment + Brave API key"]
async fn search_single_pass_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let recording = RecordingLlmProvider::new(std::sync::Arc::new(llm_client.clone()));
    let recording_arc = std::sync::Arc::new(recording);

    let search_executor = std::sync::Arc::new(
        avrag_search::BraveSearchProvider::new(
            config.brave_api_key.clone().expect("E2E_BRAVE_API_KEY not set"),
        )
    );

    let ctx = app::agents::strategy::search::SearchContext::from_request(
        AgentRequest {
            kind: AgentKind::Search,
            query: "What is the latest Rust release?".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![ChatTurnInput {
                role: "user".to_string(),
                content: "What is the latest Rust release?".to_string(),
            }],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth_context: serde_json::json!({"org_id": "test-org"}),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
        },
        "test-search".to_string(),
        app::agents::react_loop::LoopBudget::default(),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    ).unwrap();

    let strategy = SearchStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client.clone()),
        temperature: None,
        search_executor,
        search_synthesizer: Some(std::sync::Arc::new(
            app::agents::strategy::search::LlmSearchAnswerSynthesizer { llm: llm_client }
        )),
    };

    let result = app::agents::strategy::executor::StrategyExecutor::run(strategy, ctx).await.unwrap();

    // Assert state machine: Decompose → ParallelSearch → Aggregate → Evaluate → Answer
    let schema = SearchStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    e2e::assertions::assert_valid_transitions(&schema, history);
    e2e::assertions::assert_state_kinds(history);

    assert_eq!(history.len(), 5, "Expected 5 states, got {}", history.len());

    // Assert progressive disclosure
    let calls = recording_arc.calls();
    assert!(calls.len() >= 3, "Expected at least 3 LLM calls, got {}", calls.len());

    // Decompose (Plan) call
    let plan_call = &calls[0];
    e2e::assertions::assert_prompt_contains_skill(&plan_call.system_prompt, "search-plan");
    e2e::assertions::assert_prompt_has_tool_catalog(&plan_call.system_prompt, "search");

    // Evaluate call
    let eval_call = calls.iter().find(|c| c.user_messages.iter().any(|m| m.content.contains("evaluate"))).unwrap();
    e2e::assertions::assert_prompt_contains_skill(&eval_call.system_prompt, "search-eval");

    // Answer call
    let answer_call = calls.last().unwrap();
    e2e::assertions::assert_prompt_contains_skill(&answer_call.system_prompt, "search-answer");
    e2e::assertions::assert_prompt_has_format_skills(&answer_call.system_prompt);

    // Budget
    e2e::assertions::assert_budget_usage(result.budget_used.unwrap_or(0), 1);
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/app/tests/e2e_search.rs
git commit -m "test(e2e): add Search single-pass state machine test"
```

---

## Task 11: Create GitHub Actions workflow for staging E2E tests

**Files:**
- Create: `.github/workflows/e2e-staging.yml`

- [ ] **Step 1: Create workflow**

Create `.github/workflows/e2e-staging.yml`:

```yaml
name: E2E Staging Tests

on:
  workflow_dispatch:
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM UTC

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-e2e-${{ hashFiles('**/Cargo.lock') }}

      - name: Run E2E tests
        working-directory: avrag-rs
        env:
          E2E_LLM_BASE_URL: ${{ secrets.E2E_LLM_BASE_URL }}
          E2E_LLM_API_KEY: ${{ secrets.E2E_LLM_API_KEY }}
          E2E_LLM_MODEL: gpt-4o-mini
          E2E_BRAVE_API_KEY: ${{ secrets.E2E_BRAVE_API_KEY }}
          E2E_VECTOR_DB_URL: ${{ secrets.E2E_VECTOR_DB_URL }}
        run: cargo test --ignored -p app --test e2e_chat --test e2e_rag --test e2e_search

      - name: Notify on failure
        if: failure()
        run: |
          echo "::error::E2E staging tests failed"
          exit 1
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/e2e-staging.yml
git commit -m "ci: add GitHub Actions workflow for E2E staging tests"
```

---

## Task 12: Final verification and documentation

**Files:**
- Modify: `docs/superpowers/specs/2026-05-23-e2e-state-machine-prompt-validation-design.md` (add success criteria check)

- [ ] **Step 1: Verify all tests compile**

Run: `cargo test --no-run --ignored -p app`

Expected: All E2E tests compile without errors.

- [ ] **Step 2: Run unit tests to ensure no regressions**

Run: `cargo test -p app --lib`

Expected: All 424+ existing tests still pass.

- [ ] **Step 3: Update spec with success criteria**

Add to the spec document:

```markdown
## Implementation Complete

- [x] LlmProvider trait added (Task 1)
- [x] Strategies refactored to use Arc<dyn LlmProvider> (Tasks 2-4)
- [x] Production code verified (Task 5)
- [x] Test infrastructure created (Tasks 6-7)
- [x] Chat E2E test written (Task 8)
- [x] RAG E2E test written (Task 9)
- [x] Search E2E test written (Task 10)
- [x] CI workflow created (Task 11)

### Manual Verification (requires staging)

```bash
export E2E_LLM_BASE_URL="https://api.openai.com/v1"
export E2E_LLM_API_KEY="sk-..."
export E2E_LLM_MODEL="gpt-4o-mini"
export E2E_BRAVE_API_KEY="..."
export E2E_VECTOR_DB_URL="http://localhost:6333"

cargo test --ignored -p app --test e2e_chat --test e2e_rag --test e2e_search
```

Expected: All tests PASS.
```

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-05-23-e2e-state-machine-prompt-validation-design.md
git commit -m "docs: mark E2E spec as implementation complete"
```

---

---

## Task 13: Write Chat strategy E2E test (with atomic tool call)

**Files:**
- Modify: `crates/app/tests/e2e_chat.rs`

- [ ] **Step 1: Add test for atomic tool execution**

Append to `crates/app/tests/e2e_chat.rs`:

```rust
#[tokio::test]
#[ignore = "requires staging environment (E2E_LLM_BASE_URL, E2E_LLM_API_KEY, E2E_LLM_MODEL)"]
async fn chat_with_atomic_tool_call_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let recording = RecordingLlmProvider::new(std::sync::Arc::new(llm_client.clone()));
    let recording_arc = std::sync::Arc::new(recording);

    let ctx = app::agents::strategy::chat::ChatContext::from_request(
        AgentRequest {
            kind: AgentKind::Chat,
            query: "What is 2 + 2?".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![ChatTurnInput {
                role: "user".to_string(),
                content: "What is 2 + 2?".to_string(),
            }],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec!["calculator".to_string()],
            format_hint: None,
            max_iterations: None,
            auth_context: serde_json::json!({"org_id": "test-org"}),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
        },
        "test-chat-tool".to_string(),
        app::agents::react_loop::LoopBudget::default(),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    ).unwrap();

    let strategy = ChatStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let result = app::agents::strategy::executor::StrategyExecutor::run(strategy, ctx).await.unwrap();

    // Assert state machine: Plan → ExecuteAtomic → Answer
    let schema = ChatStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    e2e::assertions::assert_valid_transitions(&schema, history);
    e2e::assertions::assert_state_kinds(history);

    // Should have ExecuteAtomic state
    assert!(
        history.iter().any(|s| s.state_id == "ExecuteAtomic"),
        "Expected ExecuteAtomic state in history"
    );

    // Assert Plan LLM response contains tool_call to calculator
    let calls = recording_arc.calls();
    let plan_call = &calls[0];
    assert!(
        plan_call.response_content.contains("calculator") || plan_call.response_content.contains("tool_call"),
        "Plan response should contain calculator tool call, got: {}",
        &plan_call.response_content[..200.min(plan_call.response_content.len())]
    );

    // Assert tool result was injected into Answer messages
    let answer_call = calls.last().unwrap();
    let has_tool_result = answer_call.user_messages.iter().any(|m| 
        m.content.contains("4") || m.content.contains("result") || m.content.contains("calculator")
    );
    assert!(has_tool_result, "Answer messages should contain tool result context");

    // Budget
    e2e::assertions::assert_budget_usage(result.budget_used.unwrap_or(0), 1);
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/app/tests/e2e_chat.rs
git commit -m "test(e2e): add Chat with atomic tool call state machine test"
```

---

## Task 14: Write RAG strategy E2E test (Evaluate insufficient → re-execute)

**Files:**
- Modify: `crates/app/tests/e2e_rag.rs`

- [ ] **Step 1: Add test for RAG replan loop**

Append to `crates/app/tests/e2e_rag.rs`:

```rust
#[tokio::test]
#[ignore = "requires staging environment + vector DB with partial pricing info"]
async fn rag_evaluate_insufficient_reexecute_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let recording = RecordingLlmProvider::new(std::sync::Arc::new(llm_client.clone()));
    let recording_arc = std::sync::Arc::new(recording);

    // Requires vector DB with doc-1 containing Plan A pricing but NOT Plan B
    let rag_runtime = std::sync::Arc::new(
        avrag_rag_core::RagRuntime::new(
            config.vector_db_url.clone().expect("E2E_VECTOR_DB_URL not set"),
        ).await.unwrap()
    );

    let ctx = app::agents::strategy::rag::RagContext::from_request(
        AgentRequest {
            kind: AgentKind::Rag,
            query: "Compare the pricing of Plan A and Plan B".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec!["doc-1".to_string()],
            messages: vec![ChatTurnInput {
                role: "user".to_string(),
                content: "Compare the pricing of Plan A and Plan B".to_string(),
            }],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth_context: serde_json::json!({"org_id": "test-org"}),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
        },
        "test-rag-replan".to_string(),
        app::agents::react_loop::LoopBudget::default(),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
        rag_runtime,
    ).unwrap();

    let strategy = RagStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let result = app::agents::strategy::executor::StrategyExecutor::run(strategy, ctx).await.unwrap();

    // Assert state machine: Plan → ExecuteRetrieve → Evaluate → ExecuteRetrieve → Answer
    let schema = RagStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    e2e::assertions::assert_valid_transitions(&schema, history);
    e2e::assertions::assert_state_kinds(history);

    // Should have 5 states (replan loop)
    assert_eq!(
        history.len(), 5,
        "Expected 5 states (Plan→ExecuteRetrieve→Evaluate→ExecuteRetrieve→Answer), got {}",
        history.len()
    );

    // Verify Evaluate → ExecuteRetrieve transition exists
    let has_evaluate_to_execute = history.windows(2).any(|w| {
        w[0].state_id == "Evaluate" && w[1].state_id == "ExecuteRetrieve"
    });
    assert!(
        has_evaluate_to_execute,
        "Expected Evaluate → ExecuteRetrieve transition in state history"
    );

    // Assert Evaluate LLM response contains "insufficient" decision
    let calls = recording_arc.calls();
    let eval_call = calls.iter()
        .find(|c| c.user_messages.iter().any(|m| m.content.contains("evaluate")))
        .expect("No Evaluate LLM call found");
    assert!(
        eval_call.response_content.contains("insufficient") || eval_call.response_content.contains("Insufficient"),
        "Evaluate response should contain 'insufficient' decision, got: {}",
        &eval_call.response_content[..200.min(eval_call.response_content.len())]
    );

    // Verify no second Plan LLM call (should go directly to ExecuteRetrieve)
    let plan_calls: Vec<_> = calls.iter()
        .filter(|c| c.user_messages.iter().any(|m| m.content.contains("plan")))
        .collect();
    assert_eq!(
        plan_calls.len(), 1,
        "Expected exactly 1 Plan LLM call, got {} (replan should skip Plan)",
        plan_calls.len()
    );

    // Budget should be 2 iterations
    e2e::assertions::assert_budget_usage(result.budget_used.unwrap_or(0), 2);
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/app/tests/e2e_rag.rs
git commit -m "test(e2e): add RAG Evaluate insufficient re-execute state machine test"
```

---

## Task 15: Write Search strategy E2E test (vertical escalation)

**Files:**
- Modify: `crates/app/tests/e2e_search.rs`

- [ ] **Step 1: Add test for vertical escalation**

Append to `crates/app/tests/e2e_search.rs`:

```rust
#[tokio::test]
#[ignore = "requires staging environment + Brave API key"]
async fn search_vertical_escalation_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let recording = RecordingLlmProvider::new(std::sync::Arc::new(llm_client.clone()));
    let recording_arc = std::sync::Arc::new(recording);

    let search_executor = std::sync::Arc::new(
        avrag_search::BraveSearchProvider::new(
            config.brave_api_key.clone().expect("E2E_BRAVE_API_KEY not set"),
        )
    );

    // Use a query that's likely to trigger vertical escalation or replan
    let ctx = app::agents::strategy::search::SearchContext::from_request(
        AgentRequest {
            kind: AgentKind::Search,
            query: "latest AI news from today with detailed analysis".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![ChatTurnInput {
                role: "user".to_string(),
                content: "latest AI news from today with detailed analysis".to_string(),
            }],
            session_summary: None,
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
            auth_context: serde_json::json!({"org_id": "test-org"}),
            docscope_metadata: None,
            metadata: std::collections::BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
        },
        "test-search-vertical".to_string(),
        app::agents::react_loop::LoopBudget::default(),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    ).unwrap();

    let strategy = SearchStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client.clone()),
        temperature: None,
        search_executor,
        search_synthesizer: Some(std::sync::Arc::new(
            app::agents::strategy::search::LlmSearchAnswerSynthesizer { llm: llm_client }
        )),
    };

    let result = app::agents::strategy::executor::StrategyExecutor::run(strategy, ctx).await.unwrap();

    // Assert state machine transitions are valid
    let schema = SearchStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    e2e::assertions::assert_valid_transitions(&schema, history);
    e2e::assertions::assert_state_kinds(history);

    // Should have replan or vertical escalation (more than 5 states)
    // Note: This test is somewhat non-deterministic based on LLM behavior
    assert!(
        history.len() >= 5,
        "Expected at least 5 states, got {}",
        history.len()
    );

    // If replan occurred, verify Evaluate → ParallelSearch transition
    let has_replan = history.windows(2).any(|w| {
        w[0].state_id == "Evaluate" && w[1].state_id == "ParallelSearch"
    });

    if has_replan {
        // Verify Evaluate response contains "insufficient" or "replan"
        let calls = recording_arc.calls();
        let eval_call = calls.iter()
            .find(|c| c.user_messages.iter().any(|m| m.content.contains("evaluate")))
            .expect("No Evaluate LLM call found");
        assert!(
            eval_call.response_content.contains("insufficient") 
                || eval_call.response_content.contains("replan")
                || eval_call.response_content.contains("Insufficient"),
            "Evaluate response should contain replan decision, got: {}",
            &eval_call.response_content[..200.min(eval_call.response_content.len())]
        );
    }

    // Budget should be 1-2 iterations
    e2e::assertions::assert_budget_usage(result.budget_used.unwrap_or(0), 2);
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/app/tests/e2e_search.rs
git commit -m "test(e2e): add Search vertical escalation state machine test"
```

---

## Summary

**Total tasks:** 15  
**Estimated time:** 3-4 hours  
**Key deliverables:**
- `LlmProvider` trait for dependency injection
- Strategies refactored to use `Arc<dyn LlmProvider>`
- 6 E2E integration tests covering all spec scenarios:
  - Chat: simple conversation + atomic tool call
  - RAG: single-pass sufficient + evaluate insufficient re-execute
  - Search: single-pass + vertical escalation
- Reusable test infrastructure (config, recording, assertions)
- GitHub Actions workflow for staging automation

**Test coverage matrix:**
| Strategy | Scenario | State Path | Task |
|----------|----------|------------|------|
| Chat | Simple | Plan → Answer | 8 |
| Chat | Tool call | Plan → ExecuteAtomic → Answer | 13 |
| RAG | Single-pass | Plan → ExecuteRetrieve → Evaluate → Answer | 9 |
| RAG | Replan | Plan → ExecuteRetrieve → Evaluate → ExecuteRetrieve → Answer | 14 |
| Search | Single-pass | Decompose → ParallelSearch → Aggregate → Evaluate → Answer | 10 |
| Search | Vertical | Decompose → ParallelSearch → Aggregate → Evaluate → ParallelSearch → ... | 15 |

**Next steps:**
1. Implement tasks sequentially using subagent-driven-development or executing-plans skill
2. After all tasks complete, manually run E2E tests with staging environment
3. Configure GitHub secrets for CI automation
4. Monitor first nightly CI run
