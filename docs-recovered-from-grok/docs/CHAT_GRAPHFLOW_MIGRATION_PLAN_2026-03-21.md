# context-osv6 Chat Graphflow Migration Plan

> Date: 2026-03-21
> Scope: only migrate `chat` orchestration to graphflow
> Non-goal: do not migrate auth, notebook CRUD, admin CRUD, billing query, ingestion worker
> Historical plan. Graphflow describes the March 2026 implementation migration. Current product boundary is `Main Agent -> RAG API -> Main Agent answer`; see [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## 1. Decision

This project should migrate **only the chat orchestration layer** into a graph-style workflow runtime.

The target is:

- keep HTTP handlers unchanged
- keep repository/storage code unchanged
- keep existing DTOs and API responses unchanged
- move the multi-step chat orchestration out of large hand-written functions

The graphflow layer should orchestrate:

- input guard
- session bootstrap
- mode branching
- mode execution
- output guard
- persistence
- usage accounting
- notifications
- final response assembly

It should **not** own:

- Axum routing
- PostgreSQL repositories
- object storage
- auth middleware
- worker polling
- billing storage

## 2. Why only Chat

`chat` is already an orchestration problem.

Current hand-written flow is concentrated in:

- [app.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L1942)
- [app.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L2130)
- [app.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L2363)

It mixes:

- validation
- branching
- LLM / RAG execution
- guardrails
- persistence
- metering
- notifications

By contrast, ingestion already has a decent workflow boundary:

- [ingestion runtime](/home/chuan/context-osv6/avrag-rs/crates/ingestion/src/lib.rs#L219)
- [worker processor](/home/chuan/context-osv6/avrag-rs/bins/worker/src/main.rs#L151)

So chat has the highest graphflow ROI.

## 3. Library Recommendation

### 3.1 Recommendation

Adopt a graph-style workflow library **behind an internal orchestration trait**.

Given the current requirement and fit to chat/agent workflows:

- preferred pilot choice: `graph-flow`

Rationale:

- graph-shaped orchestration fits chat branching
- better conceptual alignment with agent / tool / multi-step flows
- easier to evolve into richer planner/synthesizer/tool execution later

### 3.2 Important reality check

Rust-native graph/workflow libraries are not equally mature.

Practical conclusion:

- use `graph-flow` as the orchestration engine for chat
- isolate it behind an internal interface
- do not let graph library types leak into handlers or repositories

This keeps migration risk controlled if the library later needs to be replaced.

### 3.3 Adapter rule

Create an internal trait like:

```rust
#[async_trait::async_trait]
pub trait ChatOrchestrator: Send + Sync {
    async fn execute(
        &self,
        ctx: ChatExecutionContext,
    ) -> Result<common::ChatResponse, common::AppError>;
}
```

`AppState` should depend on this trait, not on the graph library directly.

## 4. Target Architecture

## 4.1 Keep the outer boundary

Keep these layers:

- `transport-http`
- auth / request context
- request DTO parsing
- response rendering

Current handler shape can stay:

- [transport-http chat handler](/home/chuan/context-osv6/avrag-rs/crates/transport-http/src/lib.rs#L1416)

Only change:

- handler calls `chat_orchestrator.execute(...)`

## 4.2 Add chat orchestration crate/module

Introduce a new crate or module:

- suggested crate: `crates/chat-orchestration`

Responsibilities:

- define chat graph nodes
- define graph context state
- execute mode branch and collect outputs
- convert graph result into existing `ChatResponse`

Non-responsibilities:

- no SQL
- no Axum
- no JWT/auth middleware
- no SSE formatting

## 4.3 AppState as dependency provider

`AppState` should still build concrete dependencies:

- guard pipeline
- rag runtime
- search executor
- llm client
- chatmemory
- repositories
- notifier / usage recorder

But it should inject them into chat orchestration as dependencies.

## 5. Graph Context Design

Create a single context object for one chat request:

```rust
pub struct ChatExecutionContext {
    pub auth: avrag_auth::AuthContext,
    pub request: common::ChatRequest,
    pub trace_id: String,
    pub user_id: uuid::Uuid,
    pub notebook_id: Option<uuid::Uuid>,
}
```

Create mutable graph state:

```rust
pub struct ChatGraphState {
    pub session: Option<common::ChatSession>,
    pub input_guard: Option<common::GuardResult>,
    pub selected_mode: Option<String>,
    pub rag_response: Option<common::ChatResponse>,
    pub general_response: Option<common::ChatResponse>,
    pub search_response: Option<common::ChatResponse>,
    pub output_guard_report: Option<serde_json::Value>,
    pub final_response: Option<common::ChatResponse>,
    pub degrade_trace: Vec<common::DegradeTraceItem>,
    pub notifications: Vec<serde_json::Value>,
    pub usage_records: Vec<(String, i64, String)>,
}
```

Graph state must carry only orchestration state, not infrastructure handles.

## 6. Node Boundaries

## 6.1 Pre-processing nodes

1. `resolve_request_context`
- parse notebook id if present
- derive user id
- initialize trace id

2. `input_guard`
- run `guard_pipeline.check_input`
- stop graph on block

3. `load_or_create_session`
- existing behavior preserved
- if `session_id` present, load session
- otherwise create session

4. `select_mode`
- normalize `rag | general | search`
- store selected mode in graph state

## 6.2 Branch nodes

5. `run_rag`
- wrap current RAG path from [app.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L2006)
- no behavior change in first migration

6. `run_general`
- wrap current general mode path from [app.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L2130)

7. `run_search`
- wrap current search mode path from [app.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L2363)

Only one of these should execute in a given run.

## 6.3 Post-processing nodes

8. `output_guard`
- run output guard on generated answer
- merge degrade trace

9. `persist_turn`
- append chat turn
- save citations

10. `record_usage`
- record token usage / metrics

11. `emit_notifications`
- emit degrade or completion notifications where current behavior already does so

12. `build_response`
- produce the exact current `ChatResponse`

## 7. Minimal Migration Strategy

## Phase 1: Encapsulate without changing logic

Goal:

- move orchestration shape first
- keep inner business logic intact

Tasks:

- create `ChatOrchestrator` trait
- move current mode-specific functions into a `chat-orchestration` boundary
- keep nodes as thin wrappers around current code

Success criteria:

- HTTP output remains byte-for-byte compatible enough for existing frontend
- tests and existing behavior still pass

## Phase 2: Graph execution

Goal:

- replace hand-written top-level orchestration with graph execution

Tasks:

- represent node sequence and mode branch in graph definition
- wire short-circuit on input guard failure
- wire branch selection for `rag/general/search`

Success criteria:

- behavior unchanged
- traces can indicate which nodes ran

## Phase 3: Improve observability

Goal:

- get value from graph model, not just relocation

Tasks:

- node-level tracing
- node durations
- node failure cause logging
- optional replay/debug serialization

Success criteria:

- chat failures are attributable to a specific node

## 8. What must remain unchanged

The first migration must preserve:

- existing REST contract
- existing SSE contract
- existing `ChatResponse` shape
- existing auth checks
- existing audit / usage / notification side effects
- existing mode semantics

Specifically preserve these public endpoints:

- `/api/v1/chat`
- `/api/v1/chat/sessions`
- `/api/v1/chat/citations/lookup`
- `/v1/notebooks/{notebook_id}/chat/completions`
- `/mcp/notebooks/{notebook_id}`

## 9. What not to graphify

Do not migrate these now:

- `create_workspace`
- `update_workspace`
- `delete_workspace`
- `create_document_upload`
- `complete_document_upload`
- admin list/detail endpoints
- auth/register/login/reset
- billing plan/subscription queries

Reason:

- these are short request/transaction handlers
- graphflow would add more complexity than value here

## 10. Testing requirements

Add or update tests at these levels:

### Unit

- node-level tests for:
  - input guard node
  - mode selection node
  - output guard merge node

### Integration

- graph execution preserves:
  - rag response shape
  - search response shape
  - general response shape

### E2E

- `/api/v1/chat` still works for:
  - registered user
  - notebook with uploaded document
  - search mode query
  - general mode query

## 11. Risk controls

To reduce migration risk:

1. Put graphflow behind feature flag or config switch:
- `CHAT_ORCHESTRATOR=legacy|graph`

2. Keep legacy path during rollout.

3. Add comparison logging:
- legacy result vs graph result on shadow runs where practical

4. Roll out mode by mode:
- first `general`
- then `search`
- then `rag`

This is safer than moving all three modes at once.

## 12. Recommended rollout order

1. Introduce `ChatOrchestrator` trait
2. Move current hand-written orchestration behind trait
3. Implement graphflow orchestrator for `general`
4. Validate output parity
5. Implement `search`
6. Implement `rag`
7. Switch default orchestrator to graph
8. Remove legacy orchestrator only after confidence is high

## 13. Bottom line

This project should:

- use graphflow for `chat orchestration`
- keep data access and HTTP layers outside the graph
- migrate in phases
- preserve current API behavior during the first migration

That gives the best tradeoff between:

- architectural improvement
- delivery risk
- future extensibility
