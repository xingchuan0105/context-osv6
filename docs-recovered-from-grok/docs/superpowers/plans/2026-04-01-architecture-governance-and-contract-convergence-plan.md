# Architecture Governance And Contract Convergence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved architecture-governance RFC by converging the chat contract, shrinking HTTP and application-layer responsibilities, isolating `memory mode` as a dev/test adapter, and hardening CI without adding new product features.

**Architecture:** Land a shared Rust `contracts` crate first and route all chat transport through it. Once the contract is stable, align frontend SDK/UI behavior, split `transport-http` into responsibility-based modules, extract application services from `AppState`, then move storage/runtime branching behind ports and Redis-backed adapters. Keep `rag-core` and ingestion internals intact unless required to satisfy the new boundaries.

**Tech Stack:** Rust (`axum`, `tokio`, `serde`, `sqlx`, `redis`, `async-trait`), Leptos, Playwright, Cargo path dependencies, shell-based CI checks.

---

## File Map

**New shared contract crate**
- Create: `contracts/Cargo.toml`
- Create: `contracts/src/lib.rs`
- Create: `contracts/src/chat.rs`
- Create: `contracts/src/auth.rs`
- Create: `contracts/src/notebooks.rs`
- Create: `contracts/src/documents.rs`
- Test: `contracts/tests/chat_contract.rs`

**Backend transport and application**
- Modify: `avrag-rs/crates/common/Cargo.toml`
- Modify: `avrag-rs/crates/common/src/lib.rs`
- Modify: `avrag-rs/crates/transport-http/Cargo.toml`
- Modify: `avrag-rs/crates/transport-http/src/lib.rs`
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
- Create: `avrag-rs/crates/transport-http/src/middleware.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/mod.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/chat.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/notebooks.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/auth.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/infra.rs`
- Test: `avrag-rs/crates/transport-http/tests/chat_stream_contract.rs`
- Test: `avrag-rs/crates/transport-http/tests/router_surface.rs`

**Application service and adapter boundary**
- Modify: `avrag-rs/crates/app/src/lib.rs`
- Modify: `avrag-rs/crates/app/src/chat/mod.rs`
- Modify: `avrag-rs/crates/app/src/chat/service.rs`
- Create: `avrag-rs/crates/app/src/services/mod.rs`
- Create: `avrag-rs/crates/app/src/services/chat_service.rs`
- Create: `avrag-rs/crates/app/src/services/notebook_service.rs`
- Create: `avrag-rs/crates/app/src/ports/mod.rs`
- Create: `avrag-rs/crates/app/src/ports/chat_store.rs`
- Create: `avrag-rs/crates/app/src/ports/workspace_store.rs`
- Create: `avrag-rs/crates/app/src/ports/document_store.rs`
- Create: `avrag-rs/crates/app/src/ports/rate_limiter.rs`
- Create: `avrag-rs/crates/app/src/adapters/mod.rs`
- Create: `avrag-rs/crates/app/src/adapters/memory.rs`
- Create: `avrag-rs/crates/app/src/adapters/pg.rs`
- Create: `avrag-rs/crates/app/src/adapters/redis_rate_limiter.rs`
- Test: `avrag-rs/crates/app/tests/chat_service_contract.rs`
- Test: `avrag-rs/crates/app/tests/runtime_adapters.rs`
- Test: `avrag-rs/crates/app/tests/redis_rate_limiter.rs`

**Storage facades**
- Modify: `avrag-rs/crates/storage-pg/src/lib.rs`
- Create: `avrag-rs/crates/storage-pg/src/chat.rs`
- Create: `avrag-rs/crates/storage-pg/src/notebooks.rs`
- Create: `avrag-rs/crates/storage-pg/src/documents.rs`

**Frontend SDK and UI**
- Modify: `frontend_rust/crates/web-sdk/Cargo.toml`
- Modify: `frontend_rust/crates/web-sdk/src/lib.rs`
- Modify: `frontend_rust/crates/web-sdk/src/chat.rs`
- Modify: `frontend_rust/crates/web-sdk/src/sse.rs`
- Modify: `frontend_rust/crates/web-sdk/src/auth.rs`
- Modify: `frontend_rust/crates/web-sdk/src/notebooks.rs`
- Modify: `frontend_rust/crates/web-sdk/src/documents.rs`
- Create: `frontend_rust/crates/web-sdk/tests/contracts_reexports.rs`
- Modify: `frontend_rust/crates/web-ui/src/lib.rs`
- Modify: `frontend_rust/crates/web-ui/src/app.rs`
- Modify: `frontend_rust/crates/web-ui/src/platform.rs`
- Create: `frontend_rust/crates/web-ui/src/platform/capabilities.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/auth.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/settings.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/dashboard.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/shared.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/document/mod.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_trace_panel.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/common/mod.rs`
- Create: `frontend_rust/crates/web-ui/src/components/common/unavailable_feature_card.rs`

**E2E and CI**
- Modify: `avrag-rs/e2e/helpers.ts`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`
- Modify: `avrag-rs/Cargo.toml`
- Create: `scripts/check_contract_governance.sh`
- Create: `.github/workflows/context-osv6-contract-governance.yml`

---

### Task 1: Seed The Shared Contracts Crate

**Files:**
- Create: `contracts/Cargo.toml`
- Create: `contracts/src/lib.rs`
- Create: `contracts/src/chat.rs`
- Test: `contracts/tests/chat_contract.rs`
- Modify: `avrag-rs/crates/common/Cargo.toml`
- Modify: `avrag-rs/crates/common/src/lib.rs`
- Modify: `avrag-rs/crates/transport-http/Cargo.toml`
- Modify: `frontend_rust/crates/web-sdk/Cargo.toml`

- [ ] **Step 1: Write the failing chat contract test**

Create `contracts/tests/chat_contract.rs`:

```rust
use contracts::chat::{ChatDonePayload, ChatEvent, ChatRequest, ChatResponse, ChatTurnInput};

#[test]
fn chat_event_json_tags_are_stable() {
    let event = ChatEvent::Start {
        request_id: "req-1".to_string(),
        session_id: "sess-1".to_string(),
    };

    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["event"], "start");
    assert_eq!(value["request_id"], "req-1");
    assert_eq!(value["session_id"], "sess-1");
}

#[test]
fn chat_request_round_trips_without_transport_headers() {
    let request = ChatRequest {
        query: "hello".to_string(),
        notebook_id: Some("nb-1".to_string()),
        session_id: None,
        agent_type: "general".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: vec!["doc-1".to_string()],
        messages: vec![ChatTurnInput {
            role: "user".to_string(),
            content: "hello".to_string(),
        }],
        stream: true,
    };

    let json = serde_json::to_string(&request).unwrap();
    let decoded: ChatRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.query, "hello");
    assert!(decoded.stream);
}

#[test]
fn done_payload_contains_terminal_fields() {
    let payload = ChatDonePayload {
        request_id: "req-1".to_string(),
        session_id: "sess-1".to_string(),
        message_id: 9,
        response: ChatResponse {
            answer: "ok".to_string(),
            answer_blocks: Vec::new(),
            session_id: "sess-1".to_string(),
            agent_type: "general".to_string(),
            sources: Vec::new(),
            citations: Vec::new(),
            trace: None,
            degrade_trace: Vec::new(),
            planner_output: None,
            mode_debug: None,
            message_id: Some(9),
            guard_report: None,
        },
    };

    let value = serde_json::to_value(payload).unwrap();
    assert_eq!(value["request_id"], "req-1");
    assert_eq!(value["message_id"], 9);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test --manifest-path contracts/Cargo.toml
```

Expected:
- Fail because `contracts/` does not exist yet.

- [ ] **Step 3: Write the minimal shared contract crate**

Create `contracts/Cargo.toml`:

```toml
[package]
name = "contracts"
version = "0.1.0"
edition = "2024"
license = "MIT"

[dependencies]
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
```

Create `contracts/src/lib.rs`:

```rust
pub mod auth;
pub mod chat;
pub mod documents;
pub mod workspaces;
```

Create `contracts/src/chat.rs`:

```rust
use serde::{Deserialize, Serialize};

fn default_agent_type() -> String {
    "rag".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatTurnInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Citation {
    pub citation_id: i64,
    pub doc_id: String,
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    pub doc_name: String,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRef {
    pub doc_id: String,
    pub doc_name: String,
    pub chunk_id: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnswerBlock {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatRequest {
    pub query: String,
    #[serde(default)]
    pub notebook_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "default_agent_type")]
    pub agent_type: String,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub source_token: Option<String>,
    #[serde(default)]
    pub doc_scope: Vec<String>,
    #[serde(default)]
    pub messages: Vec<ChatTurnInput>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatResponse {
    pub answer: String,
    #[serde(default)]
    pub answer_blocks: Vec<AnswerBlock>,
    pub session_id: String,
    pub agent_type: String,
    #[serde(default)]
    pub sources: Vec<SourceRef>,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub trace: Option<serde_json::Value>,
    #[serde(default)]
    pub degrade_trace: Vec<serde_json::Value>,
    #[serde(default)]
    pub planner_output: Option<serde_json::Value>,
    #[serde(default)]
    pub mode_debug: Option<serde_json::Value>,
    #[serde(default)]
    pub message_id: Option<i64>,
    #[serde(default)]
    pub guard_report: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatDonePayload {
    pub request_id: String,
    pub session_id: String,
    pub message_id: i64,
    pub response: ChatResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ChatEvent {
    Start { request_id: String, session_id: String },
    Trace {
        request_id: String,
        stage: String,
        status: String,
        #[serde(default)]
        detail: Option<serde_json::Value>,
    },
    Token {
        request_id: String,
        message_id: i64,
        content: String,
    },
    Citations {
        request_id: String,
        message_id: i64,
        citations: Vec<Citation>,
    },
    Done(ChatDonePayload),
    Error {
        request_id: String,
        code: String,
        message: String,
    },
}
```

Update `avrag-rs/crates/common/Cargo.toml` and `frontend_rust/crates/web-sdk/Cargo.toml` with:

```toml
contracts = { path = "../../../contracts" }
```

Update `avrag-rs/crates/common/src/lib.rs` to re-export the chat transport types:

```rust
pub use contracts::chat::{
    AnswerBlock, ChatDonePayload, ChatEvent, ChatRequest, ChatResponse, ChatTurnInput, Citation,
    SourceRef,
};
```

- [ ] **Step 4: Run the contract tests and verify they pass**

Run:

```bash
cargo test --manifest-path contracts/Cargo.toml
cargo check --manifest-path avrag-rs/Cargo.toml -p common
cargo check --manifest-path frontend_rust/Cargo.toml -p frontend-web-sdk
```

Expected:
- `contracts` tests pass
- backend `common` compiles with the new path dependency
- frontend SDK resolves the shared crate

- [ ] **Step 5: Commit**

```bash
git add contracts/Cargo.toml contracts/src contracts/tests avrag-rs/crates/common/Cargo.toml avrag-rs/crates/common/src/lib.rs frontend_rust/crates/web-sdk/Cargo.toml avrag-rs/crates/transport-http/Cargo.toml
git commit -m "refactor: add shared contracts crate for chat transport"
```

---

### Task 2: Unify POST Chat Streaming And Official SSE Events

**Files:**
- Create: `avrag-rs/crates/transport-http/tests/chat_stream_contract.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib.rs`
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
- Modify: `frontend_rust/crates/web-sdk/src/chat.rs`
- Modify: `frontend_rust/crates/web-sdk/src/sse.rs`
- Modify: `avrag-rs/e2e/helpers.ts`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`

- [ ] **Step 1: Write the failing stream contract test**

Create `avrag-rs/crates/transport-http/tests/chat_stream_contract.rs`:

```rust
use app::{AppConfig, AppState};
use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn post_chat_stream_returns_official_event_sequence() {
    let app = transport_http::build_router(AppState::new(AppConfig::default()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/chat")
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .body(Body::from(
            r#"{"query":"hello","agent_type":"general","stream":true}"#,
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();

    assert!(text.contains("event: start"));
    assert!(text.contains("event: done") || text.contains("event: error"));
    assert!(!text.contains("event: answer"));
}
```

- [ ] **Step 2: Run the targeted test and verify it fails**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --test chat_stream_contract
```

Expected:
- Fail because `POST /api/v1/chat` still returns JSON and the legacy SSE path still emits `answer`.

- [ ] **Step 3: Implement the unified POST streaming path**

Update `avrag-rs/crates/transport-http/src/lib.rs` so chat only exposes `POST`:

```rust
.route("/api/v1/chat", post(handlers::chat_post_handler))
```

Update `avrag-rs/crates/transport-http/src/handlers.rs`:

```rust
use axum::http::HeaderMap;
use contracts::chat::{ChatDonePayload, ChatEvent, ChatRequest};

fn accepts_sse(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/event-stream"))
}

pub(crate) async fn chat_post_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    if req.stream || accepts_sse(&headers) {
        let request_id = headers
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("generated-request-id")
            .to_string();

        return stream_chat_response(state, req, request_id).await;
    }

    match state.execute_chat(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => app_error_response(error),
    }
}

async fn stream_chat_response(
    state: app::AppState,
    req: ChatRequest,
    request_id: String,
) -> Response {
    match state.execute_chat(req).await {
        Ok(response) => {
            let session_id = response.session_id.clone();
            let message_id = response.message_id.unwrap_or_default();
            let events = vec![
                ChatEvent::Start { request_id: request_id.clone(), session_id: session_id.clone() },
                ChatEvent::Token {
                    request_id: request_id.clone(),
                    message_id,
                    content: response.answer.clone(),
                },
                ChatEvent::Citations {
                    request_id: request_id.clone(),
                    message_id,
                    citations: response.citations.clone(),
                },
                ChatEvent::Done(ChatDonePayload {
                    request_id,
                    session_id,
                    message_id,
                    response,
                }),
            ];

            let stream = async_stream::stream! {
                for event in events {
                    let name = match &event {
                        ChatEvent::Start { .. } => "start",
                        ChatEvent::Trace { .. } => "trace",
                        ChatEvent::Token { .. } => "token",
                        ChatEvent::Citations { .. } => "citations",
                        ChatEvent::Done(_) => "done",
                        ChatEvent::Error { .. } => "error",
                    };
                    yield Ok::<_, std::convert::Infallible>(
                        axum::response::sse::Event::default()
                            .event(name)
                            .data(serde_json::to_string(&event).unwrap())
                    );
                }
            };

            axum::response::Sse::new(stream).into_response()
        }
        Err(error) => {
            let stream = async_stream::stream! {
                yield Ok::<_, std::convert::Infallible>(
                    axum::response::sse::Event::default()
                        .event("error")
                        .data(serde_json::to_string(&ChatEvent::Error {
                            request_id,
                            code: error.code().to_string(),
                            message: error.message().to_string(),
                        }).unwrap())
                );
            };
            axum::response::Sse::new(stream).into_response()
        }
    }
}
```

- [ ] **Step 4: Align SDK and E2E callers to the new route**

Update `frontend_rust/crates/web-sdk/src/chat.rs`:

```rust
pub async fn chat(
    &self,
    req: &contracts::chat::ChatRequest,
    request_id: Option<&str>,
) -> anyhow::Result<contracts::chat::ChatResponse> {
    let url = format!("{}{}", self.base_url, "/api/v1/chat");
    let mut builder = self.client.post(&url).json(req);
    if let Some(token) = &self.auth_token {
        builder = builder.header("Authorization", format!("Bearer {}", token));
    }
    if let Some(request_id) = request_id {
        builder = builder.header("x-request-id", request_id);
    }
    Ok(builder.send().await?.error_for_status()?.json().await?)
}
```

Update `frontend_rust/crates/web-sdk/src/sse.rs` and `avrag-rs/e2e/helpers.ts` so both callers send:

```text
POST /api/v1/chat
Accept: text/event-stream
x-request-id: <value>
```

and never call `POST /api/v1/chat?stream=true`.

- [ ] **Step 5: Run verification and commit**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --test chat_stream_contract
cargo check --manifest-path frontend_rust/Cargo.toml -p frontend-web-sdk
cd avrag-rs && npx playwright test e2e/rust-frontend-e2e.spec.ts --grep "T07"
git add avrag-rs/crates/transport-http/src/lib.rs avrag-rs/crates/transport-http/src/handlers.rs avrag-rs/crates/transport-http/tests/chat_stream_contract.rs frontend_rust/crates/web-sdk/src/chat.rs frontend_rust/crates/web-sdk/src/sse.rs avrag-rs/e2e/helpers.ts avrag-rs/e2e/rust-frontend-e2e.spec.ts
git commit -m "refactor: unify chat streaming on post contract"
```

Expected:
- transport-http stream contract test passes
- frontend SDK compiles against the new chat path
- Playwright chat streaming test passes through the official POST contract

---

### Task 3: Extend Contracts To Active Auth, Workspace, And Document DTOs

**Files:**
- Create: `contracts/src/auth.rs`
- Create: `contracts/src/notebooks.rs`
- Create: `contracts/src/documents.rs`
- Modify: `contracts/src/lib.rs`
- Modify: `avrag-rs/crates/common/src/lib.rs`
- Modify: `frontend_rust/crates/web-sdk/src/lib.rs`
- Modify: `frontend_rust/crates/web-sdk/src/auth.rs`
- Modify: `frontend_rust/crates/web-sdk/src/notebooks.rs`
- Modify: `frontend_rust/crates/web-sdk/src/documents.rs`
- Test: `frontend_rust/crates/web-sdk/tests/contracts_reexports.rs`

- [ ] **Step 1: Write the failing SDK re-export test**

Create `frontend_rust/crates/web-sdk/tests/contracts_reexports.rs`:

```rust
use contracts::{auth::AuthEnvelope, documents::DocumentStatusResponse, notebooks::WorkspaceResponse};

#[test]
fn web_sdk_compiles_against_shared_contract_types() {
    let auth = serde_json::from_str::<AuthEnvelope>(r#"{"success":true,"data":null,"error":null}"#).unwrap();
    assert!(auth.success);

    let notebook = serde_json::from_str::<WorkspaceResponse>(
        r#"{"notebook":{"id":"nb-1","org_id":"org-1","owner_id":"user-1","name":"n","title":"n","description":"","created_at":"now","updated_at":"now","document_count":0,"status_summary":{},"shared":false}}"#,
    ).unwrap();
    assert_eq!(notebook.notebook.id, "nb-1");

    let status = serde_json::from_str::<DocumentStatusResponse>(r#"{"status":"queued"}"#).unwrap();
    assert_eq!(status.status, "queued");
}
```

- [ ] **Step 2: Run the targeted test and verify it fails**

Run:

```bash
cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-sdk --test contracts_reexports
```

Expected:
- Fail because the shared auth/notebook/document modules do not exist yet.

- [ ] **Step 3: Add the shared transport modules**

Create `contracts/src/auth.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthUserDto {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthPayload {
    pub token: String,
    pub user: AuthUserDto,
    #[serde(default)]
    pub reset_ticket: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthEnvelope {
    pub success: bool,
    #[serde(default)]
    pub data: Option<AuthPayload>,
    #[serde(default)]
    pub error: Option<String>,
}
```

Create `contracts/src/notebooks.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub org_id: String,
    pub owner_id: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub document_count: i64,
    pub status_summary: HashMap<String, i64>,
    pub shared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceListResponse {
    pub notebooks: Vec<Workspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceResponse {
    pub notebook: Workspace,
}
```

Create `contracts/src/documents.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentStatusResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateDocumentUploadResponse {
    pub document_id: String,
    pub upload_url: String,
    pub status: String,
}
```

Update `frontend_rust/crates/web-sdk/src/lib.rs` to turn `dtos` into re-exports:

```rust
pub mod dtos {
    pub use contracts::auth::*;
    pub use contracts::chat::*;
    pub use contracts::documents::*;
    pub use contracts::workspaces::*;
}
```

Update `frontend_rust/crates/web-sdk/src/auth.rs`, `notebooks.rs`, and `documents.rs` to import `contracts`-backed `dtos::*` only.

- [ ] **Step 4: Run verification**

Run:

```bash
cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-sdk --test contracts_reexports
cargo check --manifest-path frontend_rust/Cargo.toml -p frontend-web-sdk -p frontend-web-ui
cargo check --manifest-path avrag-rs/Cargo.toml -p transport-http -p app
```

Expected:
- SDK re-export test passes
- frontend and backend both compile against the expanded contract modules

- [ ] **Step 5: Commit**

```bash
git add contracts/src/auth.rs contracts/src/notebooks.rs contracts/src/documents.rs contracts/src/lib.rs avrag-rs/crates/common/src/lib.rs frontend_rust/crates/web-sdk/src/lib.rs frontend_rust/crates/web-sdk/src/auth.rs frontend_rust/crates/web-sdk/src/notebooks.rs frontend_rust/crates/web-sdk/src/documents.rs frontend_rust/crates/web-sdk/tests/contracts_reexports.rs
git commit -m "refactor: move active transport DTOs into shared contracts"
```

---

### Task 4: Hide Unsupported UI Surfaces Behind Fixed Capabilities

**Files:**
- Create: `frontend_rust/crates/web-ui/src/platform/capabilities.rs`
- Modify: `frontend_rust/crates/web-ui/src/platform.rs`
- Create: `frontend_rust/crates/web-ui/src/components/common/unavailable_feature_card.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/common/mod.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/auth.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/settings.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/dashboard.rs`
- Modify: `frontend_rust/crates/web-ui/src/routes/shared.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/document/mod.rs`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`

- [ ] **Step 1: Write the failing visible-surface smoke test**

Extend `avrag-rs/e2e/rust-frontend-e2e.spec.ts` with:

```ts
test("T11: unsupported controls are hidden during architecture cleanup", async ({ page, request }) => {
  const auth = await registerTestUser(request);
  await seedBrowserAuth(page, request, auth.token);

  await page.goto("/settings");
  await expect(page.getByRole("button", { name: /资料已更新|Profile updated|Save profile/i })).toHaveCount(0);
  await expect(page.getByRole("link", { name: /重置密码|Reset password/i })).toHaveCount(0);

  await page.goto("/dashboard");
  await expect(page.getByRole("button", { name: /上传|Upload/i })).toHaveCount(0);

  await page.goto("/shared/kb/demo-token");
  await expect(page.getByText(/Unavailable during architecture cleanup/i)).toBeVisible();
});
```

- [ ] **Step 2: Run the targeted E2E and verify it fails**

Run:

```bash
cd avrag-rs && npx playwright test e2e/rust-frontend-e2e.spec.ts --grep "T11"
```

Expected:
- Fail because the current UI still renders unsupported flows or directly calls unsupported routes.

- [ ] **Step 3: Add a fixed capability map and unavailable card**

Create `frontend_rust/crates/web-ui/src/platform/capabilities.rs`:

```rust
#[derive(Clone, Copy)]
pub struct UiCapabilities {
    pub profile_edit: bool,
    pub password_reset: bool,
    pub shared_kb: bool,
    pub document_upload: bool,
}

pub const UI_CAPABILITIES: UiCapabilities = UiCapabilities {
    profile_edit: false,
    password_reset: false,
    shared_kb: false,
    document_upload: false,
};

pub fn ui_capabilities() -> UiCapabilities {
    UI_CAPABILITIES
}
```

Create `frontend_rust/crates/web-ui/src/components/common/unavailable_feature_card.rs`:

```rust
use leptos::prelude::*;

#[component]
pub fn UnavailableFeatureCard() -> impl IntoView {
    view! {
        <div class="app-surface-card">
            <h2 class="app-page-title">{"Unavailable during architecture cleanup"}</h2>
            <p class="app-page-subtitle">
                {"This feature is intentionally hidden until the backend contract is finalized."}
            </p>
        </div>
    }
}
```

Use the capability map so:
- `routes/auth.rs` hides reset-password links and renders `UnavailableFeatureCard` for reset routes
- `routes/settings.rs` hides profile-save and password-reset actions
- `routes/shared.rs` renders only `UnavailableFeatureCard`
- `components/document/mod.rs` hides upload controls
- `routes/dashboard.rs` removes visible share/upload affordances

- [ ] **Step 4: Run verification**

Run:

```bash
cargo check --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui
cd avrag-rs && npx playwright test e2e/rust-frontend-e2e.spec.ts --grep "T11"
```

Expected:
- frontend compiles
- visible unsupported controls are absent and the shared route no longer dead-ends into backend stubs

- [ ] **Step 5: Commit**

```bash
git add frontend_rust/crates/web-ui/src/platform/capabilities.rs frontend_rust/crates/web-ui/src/platform.rs frontend_rust/crates/web-ui/src/components/common/unavailable_feature_card.rs frontend_rust/crates/web-ui/src/components/common/mod.rs frontend_rust/crates/web-ui/src/routes/auth.rs frontend_rust/crates/web-ui/src/routes/settings.rs frontend_rust/crates/web-ui/src/routes/dashboard.rs frontend_rust/crates/web-ui/src/routes/shared.rs frontend_rust/crates/web-ui/src/components/document/mod.rs avrag-rs/e2e/rust-frontend-e2e.spec.ts
git commit -m "chore: hide unsupported ui flows during contract cleanup"
```

---

### Task 5: Split Transport-HTTP By Responsibility

**Files:**
- Create: `avrag-rs/crates/transport-http/src/middleware.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/mod.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/chat.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/notebooks.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/auth.rs`
- Create: `avrag-rs/crates/transport-http/src/routes/infra.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib.rs`
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
- Test: `avrag-rs/crates/transport-http/tests/router_surface.rs`

- [ ] **Step 1: Write the failing router surface test**

Create `avrag-rs/crates/transport-http/tests/router_surface.rs`:

```rust
use app::{AppConfig, AppState};
use axum::{body::Body, http::{Method, Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn router_exposes_only_post_chat_contract() {
    let app = transport_http::build_router(AppState::new(AppConfig::default()));

    let post_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/chat")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"query":"ping","agent_type":"general","stream":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(post_response.status(), StatusCode::METHOD_NOT_ALLOWED);

    let get_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/chat")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
```

- [ ] **Step 2: Run the router test and verify failure**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --test router_surface
```

Expected:
- Fail until the chat GET route is removed and the router is split cleanly.

- [ ] **Step 3: Move routing and middleware into modules**

Create `avrag-rs/crates/transport-http/src/routes/mod.rs`:

```rust
pub mod auth;
pub mod chat;
pub mod infra;
pub mod workspaces;
```

Create `avrag-rs/crates/transport-http/src/lib.rs` in this shape:

```rust
mod handlers;
mod middleware;
mod routes;

pub fn build_router(state: app::AppState) -> axum::Router {
    axum::Router::new()
        .merge(routes::infra::router())
        .nest("/api/auth", routes::auth::router())
        .nest("/api/v1", routes::workspaces::router().merge(routes::chat::router()))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::request_context_middleware,
        ))
        .with_state(state)
}
```

Move the current request-context logic from `lib.rs` into `middleware.rs` without changing behavior.

- [ ] **Step 4: Run verification**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --test router_surface
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --lib
```

Expected:
- router surface test passes
- existing transport-http tests still pass after module extraction

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/transport-http/src/lib.rs avrag-rs/crates/transport-http/src/handlers.rs avrag-rs/crates/transport-http/src/middleware.rs avrag-rs/crates/transport-http/src/routes avrag-rs/crates/transport-http/tests/router_surface.rs
git commit -m "refactor: split transport-http by responsibility"
```

---

### Task 6: Extract Chat And Workspace Services From AppState

**Files:**
- Create: `avrag-rs/crates/app/src/services/mod.rs`
- Create: `avrag-rs/crates/app/src/services/chat_service.rs`
- Create: `avrag-rs/crates/app/src/services/notebook_service.rs`
- Modify: `avrag-rs/crates/app/src/lib.rs`
- Modify: `avrag-rs/crates/app/src/chat/mod.rs`
- Modify: `avrag-rs/crates/app/src/chat/service.rs`
- Test: `avrag-rs/crates/app/tests/chat_service_contract.rs`

- [ ] **Step 1: Write the failing service-level test**

Create `avrag-rs/crates/app/tests/chat_service_contract.rs`:

```rust
use app::services::chat_service::ChatService;
use contracts::chat::ChatRequest;

#[tokio::test]
async fn chat_service_executes_general_chat_with_test_dependencies() {
    let service = ChatService::for_tests();

    let response = service
        .execute(ChatRequest {
            query: "say hello".to_string(),
            notebook_id: None,
            session_id: None,
            agent_type: "general".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: Vec::new(),
            messages: Vec::new(),
            stream: false,
        })
        .await
        .unwrap();

    assert!(!response.answer.is_empty());
    assert_eq!(response.agent_type, "general");
}
```

- [ ] **Step 2: Run the targeted test and verify failure**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p app --test chat_service_contract
```

Expected:
- Fail because `ChatService` does not exist as a standalone service yet.

- [ ] **Step 3: Create the service container and delegate AppState**

Create `avrag-rs/crates/app/src/services/mod.rs`:

```rust
pub mod chat_service;
pub mod notebook_service;
```

Create `avrag-rs/crates/app/src/services/chat_service.rs`:

```rust
use crate::AppState;
use common::AppError;
use contracts::chat::{ChatRequest, ChatResponse};

#[derive(Clone)]
pub struct ChatService {
    state: AppState,
}

impl ChatService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn for_tests() -> Self {
        Self::new(AppState::new(crate::AppConfig::default()))
    }

    pub async fn execute(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        self.state.execute_chat_graphflow(req).await
    }
}
```

Update `avrag-rs/crates/app/src/lib.rs`:

```rust
pub struct ServiceContainer {
    pub chat: std::sync::Arc<crate::services::chat_service::ChatService>,
    pub notebooks: std::sync::Arc<crate::services::notebook_service::WorkspaceService>,
}

pub struct AppState {
    // existing fields...
    services: std::sync::Arc<ServiceContainer>,
}

pub async fn execute_chat(&self, req: contracts::chat::ChatRequest) -> Result<contracts::chat::ChatResponse, common::AppError> {
    self.services.chat.execute(req).await
}
```

- [ ] **Step 4: Run verification**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p app --test chat_service_contract
cargo test --manifest-path avrag-rs/Cargo.toml -p app --lib
```

Expected:
- service-level test passes
- existing app tests still pass with `AppState` delegating through the new service boundary

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/app/src/services avrag-rs/crates/app/src/lib.rs avrag-rs/crates/app/src/chat/mod.rs avrag-rs/crates/app/src/chat/service.rs avrag-rs/crates/app/tests/chat_service_contract.rs
git commit -m "refactor: extract chat and notebook services from app state"
```

---

### Task 7: Isolate Memory Mode Behind Ports And Adapters

**Files:**
- Create: `avrag-rs/crates/app/src/ports/mod.rs`
- Create: `avrag-rs/crates/app/src/ports/chat_store.rs`
- Create: `avrag-rs/crates/app/src/ports/workspace_store.rs`
- Create: `avrag-rs/crates/app/src/ports/document_store.rs`
- Create: `avrag-rs/crates/app/src/ports/rate_limiter.rs`
- Create: `avrag-rs/crates/app/src/adapters/mod.rs`
- Create: `avrag-rs/crates/app/src/adapters/memory.rs`
- Create: `avrag-rs/crates/app/src/adapters/pg.rs`
- Modify: `avrag-rs/crates/app/src/lib.rs`
- Modify: `avrag-rs/crates/app/src/services/chat_service.rs`
- Modify: `avrag-rs/crates/app/src/services/notebook_service.rs`
- Test: `avrag-rs/crates/app/tests/runtime_adapters.rs`

- [ ] **Step 1: Write the failing runtime-adapter test**

Create `avrag-rs/crates/app/tests/runtime_adapters.rs`:

```rust
use app::{AppConfig, AppState};

#[tokio::test]
async fn bootstrap_without_database_url_uses_memory_adapters() {
    let state = AppState::bootstrap(AppConfig {
        database_url: None,
        ..AppConfig::default()
    })
    .await
    .unwrap();

    assert_eq!(state.runtime_mode(), "memory");
    assert!(state.uses_memory_adapters());
}
```

- [ ] **Step 2: Run the targeted test and verify failure**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p app --test runtime_adapters
```

Expected:
- Fail because adapter ownership is still implicit inside `AppState`.

- [ ] **Step 3: Introduce explicit ports and wire adapters**

Create `avrag-rs/crates/app/src/ports/workspace_store.rs`:

```rust
use async_trait::async_trait;
use common::{AppError, CreateWorkspaceRequest, Workspace};

#[async_trait]
pub trait WorkspaceStore: Send + Sync {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, AppError>;
    async fn create_workspace(&self, req: CreateWorkspaceRequest) -> Result<Workspace, AppError>;
}
```

Create `avrag-rs/crates/app/src/adapters/memory.rs`:

```rust
use crate::ports::workspace_store::WorkspaceStore;
use async_trait::async_trait;
use common::{AppError, CreateWorkspaceRequest, Workspace, now_rfc3339};

#[derive(Default, Clone)]
pub struct MemoryWorkspaceStore;

#[async_trait]
impl WorkspaceStore for MemoryWorkspaceStore {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, AppError> {
        Ok(Vec::new())
    }

    async fn create_workspace(&self, req: CreateWorkspaceRequest) -> Result<Workspace, AppError> {
        Ok(Workspace {
            id: common::new_id(),
            org_id: common::default_org_id(),
            owner_id: common::default_user_id(),
            name: req.name.clone(),
            title: req.name,
            description: req.description,
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
        })
    }
}
```

Create `avrag-rs/crates/app/src/adapters/pg.rs`:

```rust
use crate::ports::workspace_store::WorkspaceStore;
use async_trait::async_trait;
use common::{AppError, CreateWorkspaceRequest, Workspace};
use std::sync::Arc;

#[derive(Clone)]
pub struct PgWorkspaceStore {
    pub repo: Arc<avrag_storage_pg::PgAppRepository>,
    pub auth: avrag_auth::AuthContext,
}

#[async_trait]
impl WorkspaceStore for PgWorkspaceStore {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, AppError> {
        self.repo.list_workspaces(&self.auth).await.map_err(crate::map_pg_error)
    }

    async fn create_workspace(&self, req: CreateWorkspaceRequest) -> Result<Workspace, AppError> {
        self.repo
            .create_workspace(&self.auth, req.name.trim(), req.description.trim())
            .await
            .map_err(crate::map_pg_error)
    }
}
```

Update `AppState` bootstrap to choose adapters and expose:

```rust
pub fn uses_memory_adapters(&self) -> bool {
    self.runtime_mode() == "memory"
}
```

- [ ] **Step 4: Run verification**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p app --test runtime_adapters
cargo check --manifest-path avrag-rs/Cargo.toml -p app
```

Expected:
- runtime-adapter test passes
- app compiles with explicit adapter selection

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/app/src/ports avrag-rs/crates/app/src/adapters avrag-rs/crates/app/src/lib.rs avrag-rs/crates/app/src/services/chat_service.rs avrag-rs/crates/app/src/services/notebook_service.rs avrag-rs/crates/app/tests/runtime_adapters.rs
git commit -m "refactor: isolate memory mode behind app adapters"
```

---

### Task 8: Add Redis Fixed-Window Rate Limiting And Storage Facades

**Files:**
- Create: `avrag-rs/crates/app/src/adapters/redis_rate_limiter.rs`
- Create: `avrag-rs/crates/storage-pg/src/chat.rs`
- Create: `avrag-rs/crates/storage-pg/src/notebooks.rs`
- Create: `avrag-rs/crates/storage-pg/src/documents.rs`
- Modify: `avrag-rs/crates/storage-pg/src/lib.rs`
- Modify: `avrag-rs/crates/transport-http/src/middleware.rs`
- Test: `avrag-rs/crates/app/tests/redis_rate_limiter.rs`

- [ ] **Step 1: Write the failing Redis limiter test**

Create `avrag-rs/crates/app/tests/redis_rate_limiter.rs`:

```rust
use app::adapters::redis_rate_limiter::RedisFixedWindowRateLimiter;

#[tokio::test]
async fn redis_fixed_window_limiter_blocks_after_limit() {
    let redis_url = std::env::var("TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let limiter = RedisFixedWindowRateLimiter::new(redis_url, 2).await.unwrap();

    assert!(limiter.check("org-1:user-1").await.unwrap().allowed);
    assert!(limiter.check("org-1:user-1").await.unwrap().allowed);
    assert!(!limiter.check("org-1:user-1").await.unwrap().allowed);
}
```

- [ ] **Step 2: Run the targeted test and verify failure**

Run:

```bash
TEST_REDIS_URL=redis://127.0.0.1:6379 cargo test --manifest-path avrag-rs/Cargo.toml -p app --test redis_rate_limiter
```

Expected:
- Fail because the Redis-backed limiter does not exist yet.

- [ ] **Step 3: Implement the Redis limiter and storage facades**

Create `avrag-rs/crates/app/src/adapters/redis_rate_limiter.rs`:

```rust
use redis::AsyncCommands;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: u32,
    pub limit: u32,
}

#[derive(Clone)]
pub struct RedisFixedWindowRateLimiter {
    client: redis::Client,
    limit: u32,
}

impl RedisFixedWindowRateLimiter {
    pub async fn new(redis_url: String, limit: u32) -> anyhow::Result<Self> {
        Ok(Self {
            client: redis::Client::open(redis_url)?,
            limit,
        })
    }

    pub async fn check(&self, key: &str) -> anyhow::Result<RateLimitDecision> {
        let window = chrono::Utc::now().timestamp() / 60;
        let redis_key = format!("rate-limit:{window}:{key}");
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let count: u32 = conn.incr(&redis_key, 1_u32).await?;
        let _: bool = conn.expire(&redis_key, 120).await?;
        let allowed = count <= self.limit;
        let remaining = self.limit.saturating_sub(count.min(self.limit));

        Ok(RateLimitDecision {
            allowed,
            remaining,
            limit: self.limit,
        })
    }
}
```

Create facade modules in `avrag-rs/crates/storage-pg/src/notebooks.rs`, `chat.rs`, and `documents.rs` as thin wrappers around `PgAppRepository`:

```rust
#[derive(Clone)]
pub struct PgWorkspaceQueries {
    repo: std::sync::Arc<crate::PgAppRepository>,
}

impl PgWorkspaceQueries {
    pub fn new(repo: std::sync::Arc<crate::PgAppRepository>) -> Self {
        Self { repo }
    }

    pub async fn list(&self, auth: &avrag_auth::AuthContext) -> Result<Vec<common::Workspace>, crate::PgStorageError> {
        self.repo.list_workspaces(auth).await
    }
}
```

Update `avrag-rs/crates/transport-http/src/middleware.rs` to consume the Redis-backed limiter instead of a process-local `Mutex<HashMap<...>>`.

- [ ] **Step 4: Run verification**

Run:

```bash
TEST_REDIS_URL=redis://127.0.0.1:6379 cargo test --manifest-path avrag-rs/Cargo.toml -p app --test redis_rate_limiter
cargo check --manifest-path avrag-rs/Cargo.toml -p app -p storage-pg -p transport-http
```

Expected:
- Redis rate limiter test passes against the local Redis service
- backend compiles with facade modules and Redis-backed enforcement

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/app/src/adapters/redis_rate_limiter.rs avrag-rs/crates/storage-pg/src/chat.rs avrag-rs/crates/storage-pg/src/notebooks.rs avrag-rs/crates/storage-pg/src/documents.rs avrag-rs/crates/storage-pg/src/lib.rs avrag-rs/crates/transport-http/src/middleware.rs avrag-rs/crates/app/tests/redis_rate_limiter.rs
git commit -m "refactor: add redis rate limiter and storage facades"
```

---

### Task 9: Remove Archived Frontend Members And Harden CI

**Files:**
- Modify: `avrag-rs/Cargo.toml`
- Create: `scripts/check_contract_governance.sh`
- Create: `.github/workflows/context-osv6-contract-governance.yml`
- Modify: `avrag-rs/e2e/helpers.ts`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`

- [ ] **Step 1: Write the failing governance check script**

Create `scripts/check_contract_governance.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

if rg -n "pub struct ChatRequest|pub struct ChatResponse|pub struct WorkspaceResponse|pub struct DocumentStatusResponse" frontend_rust/crates/web-sdk/src avrag-rs/crates/transport-http/src avrag-rs/crates/app/src; then
  echo "manual transport DTO definition found outside contracts crate"
  exit 1
fi

if rg -n '"crates/web-sdk"|"crates/web-ui"' avrag-rs/Cargo.toml; then
  echo "archived frontend crates still present in avrag-rs workspace"
  exit 1
fi
```

- [ ] **Step 2: Run the governance check and verify failure**

Run:

```bash
bash scripts/check_contract_governance.sh
```

Expected:
- Fail because archived frontend workspace members and manual DTO definitions are still present.

- [ ] **Step 3: Remove archived workspace members and add CI**

Update `avrag-rs/Cargo.toml` by deleting:

```toml
  "crates/web-sdk",
  "crates/web-ui",
```

Create `.github/workflows/context-osv6-contract-governance.yml`:

```yaml
name: context-osv6-contract-governance

on:
  push:
    paths:
      - "context-osv6/**"
      - ".github/workflows/context-osv6-contract-governance.yml"
  pull_request:
    paths:
      - "context-osv6/**"
      - ".github/workflows/context-osv6-contract-governance.yml"

jobs:
  governance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: bash context-osv6/scripts/check_contract_governance.sh
      - run: cargo test --manifest-path context-osv6/contracts/Cargo.toml
      - run: cargo test --manifest-path context-osv6/avrag-rs/Cargo.toml -p transport-http --test chat_stream_contract --test router_surface
      - run: cargo test --manifest-path context-osv6/avrag-rs/Cargo.toml -p app --test chat_service_contract --test runtime_adapters
```

- [ ] **Step 4: Run verification**

Run:

```bash
bash scripts/check_contract_governance.sh
cargo metadata --manifest-path avrag-rs/Cargo.toml --no-deps
```

Expected:
- governance check passes
- archived frontend packages are absent from the avrag-rs workspace member list

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/Cargo.toml scripts/check_contract_governance.sh .github/workflows/context-osv6-contract-governance.yml
git commit -m "chore: remove archived frontend members and add governance ci"
```

---

## Self-Review

### Spec coverage

- Shared contract SSOT: covered by Tasks 1 and 3
- Unified `POST /api/v1/chat`: covered by Task 2
- Simplified SSE event set: covered by Task 2
- UI scope freeze for unsupported features: covered by Task 4
- `transport-http` split by responsibility: covered by Task 5
- `AppState` reduction into services: covered by Task 6
- `memory mode` as dev/test adapter only: covered by Task 7
- Redis-backed rate limiting: covered by Task 8
- Archived frontend removal and CI gates: covered by Task 9

### Placeholder scan

- No `TBD`
- No `TODO`
- No deferred code markers
- All code-touching steps include concrete file paths, commands, and code snippets

### Type consistency

- Shared transport types originate in `contracts`
- Backend `common` is transitional re-export only
- Frontend `web-sdk` consumes shared `contracts` types through re-exported `dtos`
- `request_id` is carried as transport metadata on the HTTP layer, not embedded into `ChatRequest`

Plan complete and saved to `docs/superpowers/plans/2026-04-01-architecture-governance-and-contract-convergence-plan.md`. Two execution options:

1. Subagent-Driven (recommended) - I dispatch a fresh subagent per task, review between tasks, fast iteration

2. Inline Execution - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
