# Context OSv6 Operations And Commercial Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the first production-ready observability slice for `context-osv6`: low-cardinality runtime metrics in `/metrics`, user/product events and cost events in Postgres, daily user/product rollups, and first-pass anomaly detection for abusive request loops and capacity signals.

**Architecture:** Keep observability on two rails. `crates/telemetry` becomes the single place for Prometheus-friendly runtime metrics with low-cardinality labels, while a new `crates/analytics` owns event and rollup logic backed by PostgreSQL. `transport-http`, `app`, and `worker` emit into those shared modules rather than each inventing their own counters or SQL writes.

**Tech Stack:** Rust workspace crates, Axum middleware/handlers, PostgreSQL migrations + SQLx, Prometheus text exposition, existing `usage-limit` metering, existing `transport-http` `/metrics` route, existing `app` and `worker` execution paths.

---

## File Map

**Analytics schema and service**
- Modify: `avrag-rs/Cargo.toml`
- Create: `avrag-rs/crates/analytics/Cargo.toml`
- Create: `avrag-rs/crates/analytics/src/lib.rs`
- Create: `avrag-rs/crates/analytics/src/events.rs`
- Create: `avrag-rs/crates/analytics/src/service.rs`
- Create: `avrag-rs/crates/analytics/src/rollups.rs`
- Create: `avrag-rs/crates/analytics/src/anomaly.rs`
- Create: `avrag-rs/crates/analytics/src/tests.rs`
- Create: `avrag-rs/migrations/0019_observability_events.up.sql`
- Create: `avrag-rs/migrations/0019_observability_events.down.sql`

**Runtime metrics**
- Modify: `avrag-rs/crates/telemetry/Cargo.toml`
- Modify: `avrag-rs/crates/telemetry/src/lib.rs`
- Create: `avrag-rs/crates/telemetry/src/prometheus.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/router_core.rs`
- Modify: `avrag-rs/crates/transport-http/src/middleware.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/infra_handlers.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/tests.rs`

**Business and cost event emitters**
- Modify: `avrag-rs/crates/app/Cargo.toml`
- Modify: `avrag-rs/crates/app/src/lib_impl/state_types.rs`
- Modify: `avrag-rs/crates/app/src/lib_impl/state_methods.rs`
- Modify: `avrag-rs/crates/app/src/chat/service_postprocess.rs`
- Modify: `avrag-rs/crates/app/src/lib_impl/documents.rs`
- Modify: `avrag-rs/crates/app/src/lib_impl/url_imports.rs`
- Modify: `avrag-rs/crates/transport-http/Cargo.toml`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/auth_primary.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/auth_secondary.rs`
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
- Modify: `avrag-rs/bins/worker/Cargo.toml`
- Modify: `avrag-rs/bins/worker/src/main.rs`

**Rollups and anomaly jobs**
- Modify: `avrag-rs/bins/worker/src/main.rs`
- Create: `avrag-rs/bins/worker/src/analytics_jobs.rs`
- Modify: `avrag-rs/bins/worker/Cargo.toml`

**Verification and docs**
- Modify: `avrag-rs/.env.example`
- Modify: `avrag-rs/README.md`
- Modify: `frontend_rust/DELIVERY_HANDOFF.md`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`

---

### Task 1: Create The Analytics Schema And Core Crate

**Files:**
- Modify: `avrag-rs/Cargo.toml`
- Create: `avrag-rs/crates/analytics/Cargo.toml`
- Create: `avrag-rs/crates/analytics/src/lib.rs`
- Create: `avrag-rs/crates/analytics/src/events.rs`
- Create: `avrag-rs/crates/analytics/src/service.rs`
- Create: `avrag-rs/crates/analytics/src/tests.rs`
- Create: `avrag-rs/migrations/0019_observability_events.up.sql`
- Create: `avrag-rs/migrations/0019_observability_events.down.sql`

- [ ] **Step 1: Write the failing schema and serialization tests**

Create `avrag-rs/crates/analytics/src/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::events::{CostEvent, CostEventName, ProductEvent, ProductEventName, ResultTag, Surface};
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn product_event_serializes_with_required_fields() {
        let event = ProductEvent {
            event_id: Uuid::new_v4(),
            event_time: Utc::now(),
            user_id: Uuid::new_v4(),
            session_id: None,
            notebook_id: None,
            surface: Surface::Workspace,
            event_name: ProductEventName::ChatCompleted,
            result: ResultTag::Success,
            request_id: Some("req-1".to_string()),
            trace_id: Some("trace-1".to_string()),
            client_platform: "web".to_string(),
            metadata: serde_json::json!({"agent_type": "rag"}),
        };

        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["surface"], "workspace");
        assert_eq!(value["event_name"], "chat_completed");
        assert_eq!(value["result"], "success");
    }

    #[test]
    fn cost_event_serializes_provider_and_usage_fields() {
        let event = CostEvent {
            event_id: Uuid::new_v4(),
            event_time: Utc::now(),
            user_id: Uuid::new_v4(),
            session_id: None,
            notebook_id: None,
            event_name: CostEventName::LlmUsageMetered,
            feature: "answer".to_string(),
            provider: "dmxapi".to_string(),
            model: "gemini-3.1-flash".to_string(),
            prompt_tokens: 100,
            completion_tokens: 200,
            embedding_tokens: 0,
            usage_units: 12,
            storage_bytes_delta: 0,
            external_call_count: 1,
            source: "graphflow".to_string(),
            metadata: serde_json::json!({"mode": "rag"}),
        };

        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["event_name"], "llm_usage_metered");
        assert_eq!(value["provider"], "dmxapi");
        assert_eq!(value["usage_units"], 12);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
cargo test -p analytics --lib --manifest-path avrag-rs/Cargo.toml
```

Expected:
- Fails because `crates/analytics` does not exist yet.

- [ ] **Step 3: Create the analytics crate and schema**

Create `avrag-rs/crates/analytics/Cargo.toml`:

```toml
[package]
name = "analytics"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
version.workspace = true

[dependencies]
anyhow.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx = { version = "0.8", default-features = false, features = ["postgres", "runtime-tokio", "uuid", "chrono", "json"] }
uuid.workspace = true
```

Update `avrag-rs/Cargo.toml` workspace members:

```toml
members = [
  "bins/api",
  "bins/worker",
  "crates/admin",
  "crates/analytics",
  "crates/app",
  ...
]
```

Create `avrag-rs/crates/analytics/src/events.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Dashboard,
    Workspace,
    Search,
    SharedKb,
    Settings,
    Api,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultTag {
    Success,
    Failure,
    Cancelled,
    Degraded,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductEventName {
    UserRegistered,
    UserLoggedIn,
    PasswordResetRequested,
    PasswordResetVerified,
    PasswordResetCompleted,
    WorkspaceCreated,
    WorkspaceOpened,
    SessionCreated,
    SessionRenamed,
    SessionPinned,
    SessionDeleted,
    DocumentUploadStarted,
    DocumentUploadCompleted,
    DocumentUploadFailed,
    UrlSourceAdded,
    DocumentReindexed,
    ChatStarted,
    ChatCompleted,
    ChatFailed,
    SearchStarted,
    SearchCompleted,
    SearchFailed,
    SharedKbOpened,
    SharedKbChatStarted,
    SharedKbChatCompleted,
    CitationOpened,
    SourceFocused,
    NoteEdited,
    NoteSynced,
    ShareLinkCreated,
    ShareLinkDisabled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CostEventName {
    LlmUsageMetered,
    EmbeddingUsageMetered,
    SummaryUsageMetered,
    UploadBytesMetered,
    StorageSnapshotRecorded,
    ExternalSearchUsageMetered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductEvent {
    pub event_id: Uuid,
    pub event_time: DateTime<Utc>,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub notebook_id: Option<Uuid>,
    pub surface: Surface,
    pub event_name: ProductEventName,
    pub result: ResultTag,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub client_platform: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEvent {
    pub event_id: Uuid,
    pub event_time: DateTime<Utc>,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub notebook_id: Option<Uuid>,
    pub event_name: CostEventName,
    pub feature: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub embedding_tokens: i64,
    pub usage_units: i64,
    pub storage_bytes_delta: i64,
    pub external_call_count: i64,
    pub source: String,
    pub metadata: serde_json::Value,
}
```

Create `avrag-rs/crates/analytics/src/service.rs`:

```rust
use anyhow::Result;
use sqlx::PgPool;

use crate::events::{CostEvent, ProductEvent};

#[derive(Clone)]
pub struct AnalyticsService {
    pool: PgPool,
}

impl AnalyticsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn record_product_event(&self, event: &ProductEvent) -> Result<()> {
        sqlx::query(
            r#"
            insert into product_events (
                event_id, event_time, event_date, user_id, session_id, notebook_id,
                surface, event_name, result, request_id, trace_id, client_platform, metadata
            ) values ($1, $2, date($2), $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(event.event_id)
        .bind(event.event_time)
        .bind(event.user_id)
        .bind(event.session_id)
        .bind(event.notebook_id)
        .bind(serde_json::to_value(event.surface)?)
        .bind(serde_json::to_value(event.event_name)?)
        .bind(serde_json::to_value(event.result)?)
        .bind(&event.request_id)
        .bind(&event.trace_id)
        .bind(&event.client_platform)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_cost_event(&self, event: &CostEvent) -> Result<()> {
        sqlx::query(
            r#"
            insert into cost_events (
                event_id, event_time, event_date, user_id, session_id, notebook_id,
                event_name, feature, provider, model, prompt_tokens, completion_tokens,
                embedding_tokens, usage_units, storage_bytes_delta, external_call_count,
                source, metadata
            ) values ($1, $2, date($2), $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
        )
        .bind(event.event_id)
        .bind(event.event_time)
        .bind(event.user_id)
        .bind(event.session_id)
        .bind(event.notebook_id)
        .bind(serde_json::to_value(event.event_name)?)
        .bind(&event.feature)
        .bind(&event.provider)
        .bind(&event.model)
        .bind(event.prompt_tokens)
        .bind(event.completion_tokens)
        .bind(event.embedding_tokens)
        .bind(event.usage_units)
        .bind(event.storage_bytes_delta)
        .bind(event.external_call_count)
        .bind(&event.source)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
```

Create `avrag-rs/crates/analytics/src/lib.rs`:

```rust
pub mod events;
pub mod service;
#[cfg(test)]
mod tests;

pub use events::{CostEvent, CostEventName, ProductEvent, ProductEventName, ResultTag, Surface};
pub use service::AnalyticsService;
```

Create `avrag-rs/migrations/0019_observability_events.up.sql`:

```sql
CREATE TABLE IF NOT EXISTS product_events (
    event_id UUID PRIMARY KEY,
    event_time TIMESTAMPTZ NOT NULL,
    event_date DATE NOT NULL,
    user_id UUID NOT NULL,
    session_id UUID,
    notebook_id UUID,
    surface TEXT NOT NULL,
    event_name TEXT NOT NULL,
    result TEXT NOT NULL,
    request_id TEXT,
    trace_id TEXT,
    client_platform TEXT NOT NULL DEFAULT 'web',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS cost_events (
    event_id UUID PRIMARY KEY,
    event_time TIMESTAMPTZ NOT NULL,
    event_date DATE NOT NULL,
    user_id UUID NOT NULL,
    session_id UUID,
    notebook_id UUID,
    event_name TEXT NOT NULL,
    feature TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    embedding_tokens BIGINT NOT NULL DEFAULT 0,
    usage_units BIGINT NOT NULL DEFAULT 0,
    storage_bytes_delta BIGINT NOT NULL DEFAULT 0,
    external_call_count BIGINT NOT NULL DEFAULT 0,
    source TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS daily_user_metrics (
    event_date DATE NOT NULL,
    user_id UUID NOT NULL,
    is_dau BOOLEAN NOT NULL DEFAULT false,
    is_new_user BOOLEAN NOT NULL DEFAULT false,
    is_activated BOOLEAN NOT NULL DEFAULT false,
    chat_count BIGINT NOT NULL DEFAULT 0,
    search_count BIGINT NOT NULL DEFAULT 0,
    upload_count BIGINT NOT NULL DEFAULT 0,
    shared_kb_open_count BIGINT NOT NULL DEFAULT 0,
    llm_prompt_tokens BIGINT NOT NULL DEFAULT 0,
    llm_completion_tokens BIGINT NOT NULL DEFAULT 0,
    embedding_tokens BIGINT NOT NULL DEFAULT 0,
    storage_bytes BIGINT NOT NULL DEFAULT 0,
    usage_units BIGINT NOT NULL DEFAULT 0,
    estimated_cost_cents BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (event_date, user_id)
);

CREATE TABLE IF NOT EXISTS daily_product_metrics (
    event_date DATE PRIMARY KEY,
    dau BIGINT NOT NULL DEFAULT 0,
    new_users BIGINT NOT NULL DEFAULT 0,
    activated_users BIGINT NOT NULL DEFAULT 0,
    daily_chat_users BIGINT NOT NULL DEFAULT 0,
    daily_search_users BIGINT NOT NULL DEFAULT 0,
    daily_upload_users BIGINT NOT NULL DEFAULT 0,
    daily_shared_kb_users BIGINT NOT NULL DEFAULT 0,
    total_llm_prompt_tokens BIGINT NOT NULL DEFAULT 0,
    total_llm_completion_tokens BIGINT NOT NULL DEFAULT 0,
    total_embedding_tokens BIGINT NOT NULL DEFAULT 0,
    total_upload_bytes BIGINT NOT NULL DEFAULT 0,
    total_estimated_cost_cents BIGINT NOT NULL DEFAULT 0,
    cost_per_dau_cents BIGINT NOT NULL DEFAULT 0,
    cost_per_activated_user_cents BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS user_anomalies (
    anomaly_id UUID PRIMARY KEY,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    user_id UUID NOT NULL,
    anomaly_kind TEXT NOT NULL,
    severity TEXT NOT NULL,
    signature TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_product_events_event_date ON product_events(event_date);
CREATE INDEX IF NOT EXISTS idx_product_events_user_date ON product_events(user_id, event_date);
CREATE INDEX IF NOT EXISTS idx_product_events_name_date ON product_events(event_name, event_date);
CREATE INDEX IF NOT EXISTS idx_cost_events_event_date ON cost_events(event_date);
CREATE INDEX IF NOT EXISTS idx_cost_events_user_date ON cost_events(user_id, event_date);
CREATE INDEX IF NOT EXISTS idx_cost_events_feature_date ON cost_events(feature, event_date);
CREATE INDEX IF NOT EXISTS idx_user_anomalies_detected_at ON user_anomalies(detected_at DESC);
```

Create `avrag-rs/migrations/0019_observability_events.down.sql`:

```sql
DROP TABLE IF EXISTS user_anomalies;
DROP TABLE IF EXISTS daily_product_metrics;
DROP TABLE IF EXISTS daily_user_metrics;
DROP TABLE IF EXISTS cost_events;
DROP TABLE IF EXISTS product_events;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:

```bash
cargo test -p analytics --lib --manifest-path avrag-rs/Cargo.toml
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/Cargo.toml \
  avrag-rs/crates/analytics/Cargo.toml \
  avrag-rs/crates/analytics/src/lib.rs \
  avrag-rs/crates/analytics/src/events.rs \
  avrag-rs/crates/analytics/src/service.rs \
  avrag-rs/crates/analytics/src/tests.rs \
  avrag-rs/migrations/0019_observability_events.up.sql \
  avrag-rs/migrations/0019_observability_events.down.sql
git commit -m "feat: add analytics event schema and service"
```

### Task 2: Add A Minimal Prometheus Runtime Metrics Registry

**Files:**
- Modify: `avrag-rs/crates/telemetry/Cargo.toml`
- Modify: `avrag-rs/crates/telemetry/src/lib.rs`
- Create: `avrag-rs/crates/telemetry/src/prometheus.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/router_core.rs`
- Modify: `avrag-rs/crates/transport-http/src/middleware.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/infra_handlers.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/tests.rs`

- [ ] **Step 1: Write failing metrics tests**

Add to `avrag-rs/crates/transport-http/src/lib_impl/tests.rs`:

```rust
#[tokio::test]
async fn metrics_endpoint_exposes_prometheus_text() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("http_requests_total"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p transport-http --lib metrics_endpoint_exposes_prometheus_text --manifest-path avrag-rs/Cargo.toml
```

Expected:
- FAIL because `/metrics` currently returns 501.

- [ ] **Step 3: Implement the minimal registry and exporter**

Update `avrag-rs/crates/telemetry/Cargo.toml`:

```toml
[dependencies]
prometheus-client = "0.23"
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
```

Create `avrag-rs/crates/telemetry/src/prometheus.rs`:

```rust
use once_cell::sync::Lazy;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use std::sync::Mutex;

#[derive(Clone, Hash, PartialEq, Eq, Debug, EncodeLabelSet)]
pub struct HttpLabels {
    pub route: &'static str,
    pub method: &'static str,
    pub status_class: &'static str,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug, EncodeLabelSet)]
pub struct SingleLabel {
    pub value: &'static str,
}

pub static REGISTRY: Lazy<Mutex<Registry>> = Lazy::new(|| {
    let mut registry = Registry::default();
    registry.register(
        "http_requests_total",
        "Total HTTP requests by route/method/status class",
        Family::<HttpLabels, Counter>::default(),
    );
    registry.register(
        "http_inflight_requests",
        "Inflight HTTP requests by route",
        Family::<SingleLabel, Gauge>::default(),
    );
    registry.register(
        "sse_streams_open",
        "Open SSE streams by surface",
        Family::<SingleLabel, Gauge>::default(),
    );
    registry.register(
        "dependency_failures_total",
        "Dependency failures by dependency name",
        Family::<SingleLabel, Counter>::default(),
    );
    Mutex::new(registry)
});

pub fn encode_metrics() -> String {
    let registry = REGISTRY.lock().expect("registry lock");
    let mut out = String::new();
    encode(&mut out, &registry).expect("metrics encode should succeed");
    out
}
```

Update `avrag-rs/crates/telemetry/src/lib.rs`:

```rust
pub mod prometheus;
```

Update `avrag-rs/crates/transport-http/src/lib_impl/infra_handlers.rs`:

```rust
async fn metrics_handler() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        telemetry::prometheus::encode_metrics(),
    )
        .into_response()
}
```

Update `avrag-rs/crates/transport-http/src/middleware.rs` to increment `http_requests_total` and inflight gauges around each request, using low-cardinality route tags only.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p transport-http --lib metrics_endpoint_exposes_prometheus_text --manifest-path avrag-rs/Cargo.toml
cargo check -p transport-http --manifest-path avrag-rs/Cargo.toml
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/telemetry/Cargo.toml \
  avrag-rs/crates/telemetry/src/lib.rs \
  avrag-rs/crates/telemetry/src/prometheus.rs \
  avrag-rs/crates/transport-http/src/lib_impl/router_core.rs \
  avrag-rs/crates/transport-http/src/middleware.rs \
  avrag-rs/crates/transport-http/src/lib_impl/infra_handlers.rs \
  avrag-rs/crates/transport-http/src/lib_impl/tests.rs
git commit -m "feat: expose minimal prometheus runtime metrics"
```

### Task 3: Emit Product Events From Core User Flows

**Files:**
- Modify: `avrag-rs/crates/app/Cargo.toml`
- Modify: `avrag-rs/crates/app/src/lib_impl/state_types.rs`
- Modify: `avrag-rs/crates/app/src/lib_impl/state_methods.rs`
- Modify: `avrag-rs/crates/app/src/chat/service_postprocess.rs`
- Modify: `avrag-rs/crates/app/src/lib_impl/documents.rs`
- Modify: `avrag-rs/crates/app/src/lib_impl/url_imports.rs`
- Modify: `avrag-rs/crates/transport-http/Cargo.toml`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/auth_primary.rs`
- Modify: `avrag-rs/crates/transport-http/src/lib_impl/auth_secondary.rs`
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`

- [ ] **Step 1: Write a failing analytics emission test**

Add to `avrag-rs/crates/transport-http/src/lib_impl/tests.rs`:

```rust
#[tokio::test]
async fn auth_register_writes_product_event_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("event-{}@example.test", uuid::Uuid::new_v4());

    let req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{email}","password":"password123","full_name":"Events User"}}"#
        )))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let analytics = state.analytics().expect("analytics should exist");
    let count: i64 = sqlx::query_scalar("select count(1) from product_events where event_name = 'user_registered'")
        .fetch_one(analytics.pool())
        .await
        .unwrap();
    assert!(count >= 1);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p transport-http --lib auth_register_writes_product_event_when_database_available --manifest-path avrag-rs/Cargo.toml
```

Expected:
- Fails because `AppState` does not expose analytics yet and no product events are written.

- [ ] **Step 3: Add analytics service to AppState and emit events**

Update `avrag-rs/crates/app/Cargo.toml`:

```toml
[dependencies]
analytics = { path = "../analytics" }
```

Update `avrag-rs/crates/app/src/lib_impl/state_types.rs` to add:

```rust
analytics: Option<Arc<analytics::AnalyticsService>>,
```

Update `avrag-rs/crates/app/src/lib_impl/state_methods.rs` to initialize analytics when PostgreSQL is available and expose:

```rust
pub fn analytics(&self) -> Option<Arc<analytics::AnalyticsService>> {
    self.analytics.clone()
}
```

Emit `ProductEvent` writes in these places:

- `auth_primary.rs`
  - `user_registered`
  - `user_logged_in`
- `auth_secondary.rs`
  - `password_reset_requested`
  - `password_reset_verified`
  - `password_reset_completed`
- `documents.rs`
  - `document_upload_started`
  - `document_upload_completed`
- `url_imports.rs`
  - `url_source_added`
- `service_postprocess.rs`
  - `chat_completed`
  - `search_completed`

Do **not** force public shared-notebook page opens into `product_events` when the access is anonymous.
Public share views should continue to land in `share_access_logs`, and Task 5 should aggregate those
logs back to the notebook owner's `daily_user_metrics.shared_kb_open_count`.

Use a narrow helper in `app`:

```rust
pub async fn record_product_event_if_available(
    &self,
    event_name: analytics::ProductEventName,
    surface: analytics::Surface,
    result: analytics::ResultTag,
    session_id: Option<Uuid>,
    notebook_id: Option<Uuid>,
    metadata: serde_json::Value,
) {
    let Some(ref analytics) = self.analytics else {
        return;
    };
    let Some(actor_id) = self.auth.actor_id() else {
        return;
    };
    let event = analytics::ProductEvent {
        event_id: Uuid::new_v4(),
        event_time: chrono::Utc::now(),
        user_id: actor_id.into_uuid(),
        session_id,
        notebook_id,
        surface,
        event_name,
        result,
        request_id: self.auth.request_id().map(str::to_string),
        trace_id: None,
        client_platform: "web".to_string(),
        metadata,
    };
    let _ = analytics.record_product_event(&event).await;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p transport-http --lib auth_register_writes_product_event_when_database_available --manifest-path avrag-rs/Cargo.toml
cargo check --workspace --manifest-path avrag-rs/Cargo.toml
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/app/Cargo.toml \
  avrag-rs/crates/app/src/lib_impl/state_types.rs \
  avrag-rs/crates/app/src/lib_impl/state_methods.rs \
  avrag-rs/crates/app/src/chat/service_postprocess.rs \
  avrag-rs/crates/app/src/lib_impl/documents.rs \
  avrag-rs/crates/app/src/lib_impl/url_imports.rs \
  avrag-rs/crates/transport-http/Cargo.toml \
  avrag-rs/crates/transport-http/src/lib_impl/auth_primary.rs \
  avrag-rs/crates/transport-http/src/lib_impl/auth_secondary.rs \
  avrag-rs/crates/transport-http/src/handlers.rs
git commit -m "feat: emit product analytics events from core flows"
```

### Task 4: Emit Cost Events And Worker Metrics

**Files:**
- Modify: `avrag-rs/bins/worker/Cargo.toml`
- Modify: `avrag-rs/bins/worker/src/main.rs`
- Modify: `avrag-rs/crates/llm/src/client.rs`
- Modify: `avrag-rs/crates/llm/src/lib.rs`
- Modify: `avrag-rs/crates/llm/src/summary.rs`

- [ ] **Step 1: Write a failing worker metering test**

Add to `avrag-rs/crates/llm/src/tests.rs` or extend `summary/tests.rs`:

```rust
#[test]
fn llm_usage_accumulate_preserves_provider_and_model() {
    let mut total = crate::LlmUsage::zeroed();
    total.accumulate(&crate::LlmUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
        provider: "dmxapi".to_string(),
        model: "gemini-test".to_string(),
    });
    assert_eq!(total.total_tokens, 30);
    assert_eq!(total.provider, "dmxapi");
    assert_eq!(total.model, "gemini-test");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p avrag-llm --lib llm_usage_accumulate_preserves_provider_and_model --manifest-path avrag-rs/Cargo.toml
```

Expected:
- Fails if the helper is missing.

- [ ] **Step 3: Extend usage structures and emit cost events**

Ensure `LlmUsage` carries `provider` and `model` plus:

```rust
impl LlmUsage {
    pub fn zeroed() -> Self { ... }
    pub fn accumulate(&mut self, other: &LlmUsage) { ... }
}
```

Update `SummaryGenerator::synthesize()` to return:

```rust
pub async fn synthesize(...) -> anyhow::Result<(SummaryOutput, crate::LlmUsage)>
```

Then wire `worker/src/main.rs` so summary generation emits:

```rust
let ctx = avrag_usage_limit::MeteringContext {
    user_id,
    org_id: context.org_id().into_uuid(),
    feature: avrag_usage_limit::BillableFeature::Summary,
    stage: "worker_summary".to_string(),
    session_id: None,
    document_id: Some(document_id),
    request_id: None,
    trace_id: None,
};
svc.record_usage(
    &ctx,
    &llm_usage.provider,
    &llm_usage.model,
    llm_usage.prompt_tokens,
    llm_usage.completion_tokens,
    llm_usage.total_tokens,
    "worker",
).await?;
```

Also emit `CostEventName::SummaryUsageMetered` through `analytics::AnalyticsService`.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p avrag-llm --lib --manifest-path avrag-rs/Cargo.toml
cargo check -p avrag-worker --manifest-path avrag-rs/Cargo.toml
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/bins/worker/Cargo.toml \
  avrag-rs/bins/worker/src/main.rs \
  avrag-rs/crates/llm/src/client.rs \
  avrag-rs/crates/llm/src/lib.rs \
  avrag-rs/crates/llm/src/summary.rs
git commit -m "feat: meter summary generation and emit cost events"
```

### Task 5: Build Daily Rollups And First-Pass Anomaly Detection

**Files:**
- Create: `avrag-rs/crates/analytics/src/rollups.rs`
- Create: `avrag-rs/crates/analytics/src/anomaly.rs`
- Modify: `avrag-rs/crates/analytics/src/lib.rs`
- Modify: `avrag-rs/bins/worker/src/main.rs`
- Create: `avrag-rs/bins/worker/src/analytics_jobs.rs`

- [ ] **Step 1: Write failing rollup tests**

Add to `avrag-rs/crates/analytics/src/tests.rs`:

```rust
#[test]
fn activation_rule_requires_notebook_upload_and_chat() {
    let flags = crate::rollups::ActivationInputs {
        created_workspace: true,
        uploaded_document: true,
        completed_chat: true,
    };
    assert!(crate::rollups::is_activated(&flags));
}

#[test]
fn burst_detector_flags_short_window_replay() {
    let result = crate::anomaly::detect_request_burst(&[10, 11, 12, 13, 14], 5, 60);
    assert!(result.is_some());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p analytics --lib activation_rule_requires_notebook_upload_and_chat --manifest-path avrag-rs/Cargo.toml
cargo test -p analytics --lib burst_detector_flags_short_window_replay --manifest-path avrag-rs/Cargo.toml
```

Expected:
- FAIL because rollup and anomaly helpers do not exist yet.

- [ ] **Step 3: Implement rollups and anomaly rules**

Create `avrag-rs/crates/analytics/src/rollups.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct ActivationInputs {
    pub created_workspace: bool,
    pub uploaded_document: bool,
    pub completed_chat: bool,
}

pub fn is_activated(inputs: &ActivationInputs) -> bool {
    inputs.created_workspace && inputs.uploaded_document && inputs.completed_chat
}
```

Create `avrag-rs/crates/analytics/src/anomaly.rs`:

```rust
pub fn detect_request_burst(timestamps_sec: &[i64], threshold: usize, window_sec: i64) -> Option<usize> {
    if timestamps_sec.len() < threshold {
        return None;
    }
    for start in 0..timestamps_sec.len() {
        let end = start + threshold - 1;
        if end >= timestamps_sec.len() {
            break;
        }
        if timestamps_sec[end] - timestamps_sec[start] <= window_sec {
            return Some(start);
        }
    }
    None
}
```

Create `avrag-rs/bins/worker/src/analytics_jobs.rs` with SQL jobs that:

- roll up `product_events` into `daily_user_metrics`
- roll up owner-side public share exposure from `share_access_logs` into `daily_user_metrics.shared_kb_open_count`
- roll up `cost_events` into `daily_user_metrics`
- derive `daily_product_metrics`
- insert `user_anomalies` for:
  - repeated route bursts
  - repeated failed chat loops

Wire the job runner into the worker heartbeat loop so it can be called periodically behind an env-gated switch.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p analytics --lib --manifest-path avrag-rs/Cargo.toml
cargo check -p avrag-worker --manifest-path avrag-rs/Cargo.toml
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/analytics/src/lib.rs \
  avrag-rs/crates/analytics/src/rollups.rs \
  avrag-rs/crates/analytics/src/anomaly.rs \
  avrag-rs/crates/analytics/src/tests.rs \
  avrag-rs/bins/worker/src/main.rs \
  avrag-rs/bins/worker/src/analytics_jobs.rs
git commit -m "feat: add daily analytics rollups and anomaly detection"
```

### Task 6: Document Operations Setup And Verification

**Files:**
- Modify: `avrag-rs/.env.example`
- Modify: `avrag-rs/README.md`
- Modify: `frontend_rust/DELIVERY_HANDOFF.md`
- Modify: `avrag-rs/e2e/rust-frontend-e2e.spec.ts`

- [ ] **Step 1: Write the failing doc and smoke test updates**

Add a new Playwright smoke in `avrag-rs/e2e/rust-frontend-e2e.spec.ts`:

```ts
test("UI-metrics: metrics endpoint exposes runtime counters", async ({ request }) => {
  const resp = await request.get("/metrics");
  expect(resp.ok()).toBeTruthy();
  const text = await resp.text();
  expect(text).toContain("http_requests_total");
});
```

- [ ] **Step 2: Run the test to verify it fails before docs/ops completion**

Run:

```bash
npx playwright test avrag-rs/e2e/rust-frontend-e2e.spec.ts -g "UI-metrics"
```

Expected:
- Fails until the runtime metrics endpoint and test environment are wired.

- [ ] **Step 3: Update env docs and handoff**

Update `avrag-rs/.env.example` to document:

```env
EMAIL_PROVIDER=smtp
SMTP_HOST=smtp.163.com
SMTP_PORT=465
SMTP_USER=
SMTP_PASS=
SMTP_FROM=
SMTP_FROM_NAME=Context OSv6
SMTP_TLS=true
RESET_CODE_SECRET=
ANALYTICS_ROLLUP_ENABLED=false
```

Update `avrag-rs/README.md` and `frontend_rust/DELIVERY_HANDOFF.md` to include:

- required metrics endpoint expectation
- analytics tables
- first-pass anomaly detection
- SMTP/password reset dependencies
- “what to watch after launch” checklist

- [ ] **Step 4: Run verification**

Run:

```bash
cargo check --workspace --manifest-path avrag-rs/Cargo.toml
cargo check -p frontend-web-ui -p frontend-web-sdk --manifest-path frontend_rust/Cargo.toml
bash scripts/check_file_size_limits.sh
bash scripts/check_contract_governance.sh
bash scripts/check_layer_dependencies.sh
```

Expected:
- PASS

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/.env.example \
  avrag-rs/README.md \
  frontend_rust/DELIVERY_HANDOFF.md \
  avrag-rs/e2e/rust-frontend-e2e.spec.ts
git commit -m "docs: add observability and operations rollout guidance"
```

## Self-Review

### Spec coverage

Spec requirements covered:

- `L1` runtime metrics: Task 2
- `L2` product events: Task 3
- `L3` cost events: Task 4
- `daily_user_metrics` and `daily_product_metrics`: Task 5
- anomaly detection first pass: Task 5
- rollout docs and verification: Task 6

No spec gaps found.

### Placeholder scan

Checked for:

- `TODO`
- `TBD`
- “appropriate error handling”
- “write tests for the above”

None remain.

### Type consistency

Confirmed consistent names across tasks:

- `ProductEvent`, `CostEvent`
- `ProductEventName`, `CostEventName`
- `AnalyticsService`
- `daily_user_metrics`, `daily_product_metrics`
- `user_anomalies`

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-08-context-osv6-operations-and-commercial-observability-plan.md`. Two execution options:

1. Subagent-Driven (recommended) - I dispatch a fresh subagent per task, review between tasks, fast iteration

2. Inline Execution - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
