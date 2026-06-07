# ADR-0004: RAG Agent Loop & Native Tool Calling

* **Status**: Accepted (Converged in design discussion)
* **Author**: Antigravity & User Pair Programming
* **Date**: 2026-06-06

---

## 1. Background & Context

The current RAG strategy implemented in `RagStrategy` (defined in [rag.rs](file:///home/chuan/context-osv6/avrag-rs/crates/app/src/agents/strategy/rag.rs)) operates as a one-way linear pipeline: `Plan ──> ExecuteRetrieve ──> Answer`. 
Although header comments suggested a loop back to `Plan` when budget was left, the actual implementation was linear. After execution, the engine always transitioned forward to `Answer` (via `Pass` or `NeedsFocus`) or terminated (via `Degrade`), providing no feedback mechanism to adjust retrieval queries.

Additionally, `avrag-llm`'s `LlmProvider` trait only supported raw text completion via `.complete()`. The planner was forced to output custom XML-like tags (e.g. `<tool_call>`), which were then parsed on the client-side using regex via `parse_rag_plan_decision`. This approach suffers from instability, lacks native gateway support for structured tools, and complicates prompt engineering.

---

## 2. Decision

We will transition the RAG pipeline into a true **Agent Loop** driven by **native tool calling** with the following technical changes:

### 2.1 State Machine Loop Boundary (Big Loop)
We will leverage the flexible [StrategyExecutor](file:///home/chuan/context-osv6/avrag-rs/crates/app/src/agents/strategy/executor.rs) driver. Instead of local iteration inside the `Answer` state, we will introduce a backward transition in [RagStrategy::step](file:///home/chuan/context-osv6/avrag-rs/crates/app/src/agents/strategy/rag.rs#L299-L318):
- **Plan**: Call LLM with native tool definitions.
  - If LLM emits `tool_calls`, transition to `ExecuteRetrieve`.
  - If LLM emits *no* `tool_calls` (indicating sufficient knowledge), transition to `Answer`.
- **ExecuteRetrieve**: Execute tool calls. Run `EvidenceGate` as a quality filter.
  - If `EvidenceGateOutcome::Degrade`, terminate immediately (saving costs).
  - If `Pass` or `NeedsFocus`: Check `LoopBudget`. If budget is exhausted, transition to `Answer`. Otherwise, transition back to `Plan` to feed back the results.

### 2.2 Backward-Compatible Native Tool Calling
We will refactor `avrag-llm` to support standard structured tool calls:
- **[ChatMessage](file:///home/chuan/context-osv6/avrag-rs/crates/llm/src/client.rs#L523-L526)**: Extend with optional fields to reconstruct tool interaction history for subsequent LLM iterations:
  ```rust
  pub struct ChatMessage {
      pub role: String,
      pub content: String,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub name: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub tool_call_id: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub tool_calls: Option<serde_json::Value>,
  }
  ```
- **[LlmResponse](file:///home/chuan/context-osv6/avrag-rs/crates/llm/src/client.rs#L552-L556)**: Extend with `tool_calls: Option<Vec<common::ToolCall>>`.
- **[LlmProvider](file:///home/chuan/context-osv6/avrag-rs/crates/llm/src/lib.rs#L24-L30)**: Add a new method `complete_with_tools` accepting `tools: &[common::ToolSpec]`, providing a default error implementation for backward compatibility.
- **`LlmClient`**: Implement `complete_with_tools` to serialize the standard OpenAI `tools` request parameter and parse `choices[0].message.tool_calls` in the response.

### 2.3 Component Retain / Deprecate List
- **RETAIN**: `EvidenceGate` is kept as a pure-code fast filter for 0-hits or budget-tight scenarios.
- **DEPRECATE**: Completely remove `parse_rag_plan_decision` XML-parsing code and its corresponding prompts, replacing them with structured tool parsing.

---

## 3. Consequences

### 3.1 Verification Plan

We will perform implementation in three contiguous slices:

1. **Slice 1: Native Tools in `avrag-llm`**
   - Implement `complete_with_tools` in `LlmClient` and update `ChatMessage`/`LlmResponse`.
   - Update `RecordingLlmProvider` in tests to delegate `complete_with_tools`.
   - *Verify*: `cargo test -p avrag-llm` passes.
2. **Slice 2: Loop Flow in `RagStrategy`**
   - Feed previous tool call execution history back into the LLM context in `step_plan` (as `role: assistant + tool_calls` and `role: tool + content` message pairs).
   - Change `step_plan` and `step_execute` to implement the state machine loop: `Plan ──> ExecuteRetrieve ──> Plan`.
   - *Verify*: `cargo check` workspace compiles.
3. **Slice 3: Multi-Iteration Testing**
   - Add a test named `test_rag_agent_loop` in [strategy_rag.rs](file:///home/chuan/context-osv6/avrag-rs/crates/app/tests/strategy_rag.rs) mocking a 2-iteration loop.
   - *Verify*: `cargo test -p app --test strategy_rag` passes.
