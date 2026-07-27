# Single User Usage Limit - Design Spec

> Date: 2026-03-30
> Status: approved
> Scope: Full 4-phase rollout (shadow → profile → 5h enforce → 7d enforce)
> Architecture: New independent `crates/usage-limit` crate

## 1. Architecture

### New crate: `crates/usage-limit`

Responsibilities:
- Resolve effective user policy (user override → plan → global default)
- Query rolling 5h and 7d usage
- Compute per-feature breakdown
- Compute blocked state and recovery timestamps
- Write usage ledger entries after billable LLM calls
- Provide quota check interface for preflight enforcement

Depends on: `storage-pg` (database access), `common` (error types)

Consumed by: `llm` (metering), `app` (preflight check), `transport-http` (API routes), `bins/worker` (summary skip logic)

### Integration points

```
                    ┌─────────────────────────┐
                    │    transport-http        │
                    │  GET /api/auth/usage-limit│
                    └──────────┬──────────────┘
                               │
                    ┌──────────▼──────────────┐
                    │    app / graphflow       │
                    │  preflight check node    │
                    └──────────┬──────────────┘
                               │
               ┌───────────────▼───────────────┐
               │       usage-limit crate        │
               │  UsageLimitService             │
               │  MeteringRecorder              │
               │  PolicyResolver                │
               └───────────────┬───────────────┘
                               │
                    ┌──────────▼──────────────┐
                    │    storage-pg            │
                    │  llm_usage_events table  │
                    └─────────────────────────┘
```

## 2. Database Schema

### Migration 0018: llm_usage_events

```sql
CREATE TABLE llm_usage_events (
    id          BIGSERIAL PRIMARY KEY,
    org_id      UUID NOT NULL,
    user_id     UUID NOT NULL,
    feature     TEXT NOT NULL,       -- summary | planner | answer | search | chat
    stage       TEXT NOT NULL,       -- e.g. "synthesize", "plan", "refine_query"
    provider    TEXT NOT NULL,
    model       TEXT NOT NULL,
    prompt_tokens     INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens      INTEGER NOT NULL DEFAULT 0,
    usage_units       INTEGER NOT NULL DEFAULT 0,
    usage_source      TEXT NOT NULL DEFAULT 'actual',  -- actual | estimated
    session_id   UUID,
    document_id  UUID,
    request_id   TEXT,
    trace_id     TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_llm_usage_events_user_created
    ON llm_usage_events (user_id, created_at DESC);
CREATE INDEX idx_llm_usage_events_user_feature_created
    ON llm_usage_events (user_id, feature, created_at DESC);
CREATE INDEX idx_llm_usage_events_org_user_created
    ON llm_usage_events (org_id, user_id, created_at DESC);
```

### Migration 0019: model weight table + policy tables

```sql
CREATE TABLE llm_model_weights (
    id              BIGSERIAL PRIMARY KEY,
    provider        TEXT NOT NULL,
    model           TEXT NOT NULL,
    input_unit_rate REAL NOT NULL DEFAULT 1.0,
    output_unit_rate REAL NOT NULL DEFAULT 2.0,
    enabled         BOOLEAN NOT NULL DEFAULT true,
    effective_from  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(provider, model)
);

-- Seed default weights
INSERT INTO llm_model_weights (provider, model, input_unit_rate, output_unit_rate) VALUES
    ('default', 'default', 1.0, 2.0);

CREATE TABLE usage_limit_policies (
    id                    BIGSERIAL PRIMARY KEY,
    scope                 TEXT NOT NULL,  -- 'global' | 'plan' | 'user'
    scope_id              TEXT,           -- plan_id or user_id, NULL for global
    rolling_5h_limit_units INTEGER NOT NULL,
    rolling_7d_limit_units INTEGER NOT NULL,
    enabled               BOOLEAN NOT NULL DEFAULT true,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(scope, scope_id)
);

-- Seed global default
INSERT INTO usage_limit_policies (scope, scope_id, rolling_5h_limit_units, rolling_7d_limit_units)
    VALUES ('global', NULL, 100, 1000);
```

## 3. Core Types

```rust
// In crates/usage-limit

/// Features that consume user quota
#[derive(Debug, Clone, strum::Display, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum Feature {
    Summary,
    Planner,
    Answer,
    Search,
    Chat,
}

/// Metering context passed into billable LLM calls
pub struct MeteringContext {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub feature: Feature,
    pub stage: String,
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

/// Usage event to write to ledger
pub struct UsageEvent {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub feature: Feature,
    pub stage: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub usage_units: u32,
    pub usage_source: String,
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

/// Resolved effective policy for a user
pub struct EffectivePolicy {
    pub enabled: bool,
    pub rolling_5h_limit: i64,
    pub rolling_7d_limit: i64,
}

/// Usage status for one rolling window
pub struct WindowUsage {
    pub used_units: i64,
    pub limit_units: i64,
    pub remaining_units: i64,
    pub percent_used: f64,
    pub blocked: bool,
    pub next_relief_at: Option<OffsetDateTime>,
    pub blocked_until: Option<OffsetDateTime>,
}

/// Full usage status response
pub struct UsageLimitStatus {
    pub policy: EffectivePolicy,
    pub rolling_5h: WindowUsage,
    pub rolling_7d: WindowUsage,
    pub breakdown: HashMap<Feature, i64>,
    pub has_estimated_usage: bool,
}
```

## 4. Usage Unit Calculation

```rust
pub fn calculate_usage_units(
    prompt_tokens: u32,
    completion_tokens: u32,
    input_rate: f64,
    output_rate: f64,
) -> u32 {
    let units = (prompt_tokens as f64 / 1000.0 * input_rate)
        + (completion_tokens as f64 / 1000.0 * output_rate);
    std::cmp::max(1, units.ceil() as u32)
}
```

When no model-specific weight exists, use the `default/default` row (1.0/2.0).

## 5. Service Interface

```rust
pub struct UsageLimitService {
    pg: Arc<PgAppRepository>,
}

impl UsageLimitService {
    /// Check if user can make a billable request. Returns Err if blocked.
    pub async fn check_quota(&self, user_id: Uuid) -> Result<EffectivePolicy, UsageLimitError>;

    /// Get full usage status for display
    pub async fn get_usage_status(&self, user_id: Uuid) -> Result<UsageLimitStatus, UsageLimitError>;

    /// Record a billable usage event
    pub async fn record_usage(&self, event: UsageEvent) -> Result<(), UsageLimitError>;

    /// Resolve effective policy for a user
    async fn resolve_policy(&self, user_id: Uuid) -> Result<EffectivePolicy, UsageLimitError>;

    /// Get model weight for usage_units calculation
    async fn get_model_weight(&self, provider: &str, model: &str) -> (f64, f64);
}
```

## 6. Metering Integration

### Approach: Callback trait on LlmClient

Add an optional metering hook to `LlmClient`:

```rust
pub trait UsageMeter: Send + Sync {
    fn record(&self, event: UsageEvent) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send>>;
}

pub struct LlmClient {
    // existing fields...
    usage_meter: Option<Arc<dyn UsageMeter>>,
}
```

After each `complete()` call, if `usage_meter` is set and `metering_ctx` is provided:
- Calculate `usage_units` using model weights
- Fire `usage_meter.record(event)` (fire-and-forget, errors logged but not propagated)

### Feature mapping

| Code path | Feature | Stage |
|-----------|---------|-------|
| `SummaryGenerator::synthesize` | Summary | synthesize |
| `AppState::build_session_summary` | Summary | session_summary |
| `RetrievalPlanner::plan` | Planner | retrieval_plan |
| `SearchExecutor::plan_query_with_llm` | Planner | search_plan |
| `AnswerSynthesizer::synthesize` (RAG) | Answer | rag_answer |
| `AnswerSynthesizer::synthesize` (search) | Answer | search_answer |
| `AppState::execute_general_mode_core` (final answer) | Answer | general_answer |
| Perplexity Agent API | Search | agent_search |
| `AppState::refine_general_query` | Chat | refine_query |

### Excluded paths

These must NOT trigger metering:
- `EmbeddingClient` calls
- `RerankerClient` calls
- MinerU parsing

## 7. Enforcement Points

### Chat graphflow

Add a `QuotaCheck` task node between `Preflight` and `Session`:

```
Preflight -> QuotaCheck -> Session -> ModeSelect -> ...
```

`QuotaCheck` calls `UsageLimitService::check_quota()`. If blocked, returns `AppError::RateLimited` with the usage limit details.

### Worker summary skip

In `PgTaskProcessor` (worker), before calling `SummaryGenerator::synthesize`:
- Check `UsageLimitService::check_quota(user_id)` where `user_id` comes from `requested_by`
- If blocked, skip summary generation, log the skip, continue ingestion

### API routes

Add `GET /api/auth/usage-limit` in `transport-http` that calls `UsageLimitService::get_usage_status()`.

## 8. API Response Shape

```json
{
  "policy": {
    "enabled": true,
    "rolling_5h_limit_units": 100,
    "rolling_7d_limit_units": 1000
  },
  "windows": {
    "rolling_5h": {
      "used_units": 42,
      "limit_units": 100,
      "remaining_units": 58,
      "percent_used": 42.0,
      "blocked": false,
      "next_relief_at": "2026-03-30T15:00:00Z",
      "blocked_until": null
    },
    "rolling_7d": { "..." : "..." }
  },
  "breakdown": {
    "summary": 80,
    "planner": 45,
    "answer": 210,
    "search": 15,
    "chat": 10
  },
  "scope": {
    "included": ["summary", "planner", "answer", "search", "chat"],
    "excluded": ["mineru", "embedding", "rerank"]
  },
  "has_estimated_usage": false
}
```

### Error response (429)

```json
{
  "error": {
    "code": "usage_limit_exceeded",
    "message": "5 小时用量已达上限",
    "window": "rolling_5h",
    "used_units": 105,
    "limit_units": 100,
    "blocked_until": "2026-03-30T15:00:00Z"
  }
}
```

## 9. Frontend Design

### SDK: `web-sdk/src/usage_limit.rs`

New module with:
- `UsageLimitResponse` DTO matching API response
- `ApiClient::get_usage_limit()` -> GET `/api/auth/usage-limit`

### UI: `web-ui/src/components/usage_limit/mod.rs`

New `UsageLimitPanel` component rendered in `ProfileSettings` (settings.rs), placed above profile form fields.

Sections:
1. Title: "个人用量" / "Personal Usage"
2. 5h progress bar with used/limit/remaining
3. 7d progress bar with used/limit/remaining
4. Feature breakdown as a simple table/list
5. Scope description (included/excluded)

Visual thresholds:
- <70%: green (`bg-green-500`)
- 70-90%: yellow (`bg-yellow-500`)
- >=90%: red (`bg-red-500`)
- Blocked: red bar + "已限流" badge + resume time

Data fetching: `run_once_after_hydration` pattern, same as billing.

## 10. Failure Behavior

- **Quota read fails**: Interactive billable requests fail closed → return error
- **Usage write fails after LLM call**: Return user result, emit high-severity log, do not block user
- **Policy resolution fails**: Treat as enabled with global defaults (fail-safe)

## 11. File Map

New/modified files:

### New files
- `avrag-rs/crates/usage-limit/Cargo.toml`
- `avrag-rs/crates/usage-limit/src/lib.rs` — types, service, errors
- `avrag-rs/migrations/0018_llm_usage_events.up.sql`
- `avrag-rs/migrations/0018_llm_usage_events.down.sql`
- `avrag-rs/migrations/0019_usage_limit_policies.up.sql`
- `avrag-rs/migrations/0019_usage_limit_policies.down.sql`
- `frontend_rust/crates/web-sdk/src/usage_limit.rs`
- `frontend_rust/crates/web-ui/src/components/usage_limit/mod.rs`

### Modified files
- `avrag-rs/Cargo.toml` — add workspace member
- `avrag-rs/crates/llm/src/lib.rs` — add `UsageMeter` trait
- `avrag-rs/crates/llm/src/client.rs` — metering hook integration
- `avrag-rs/crates/app/Cargo.toml` — add usage-limit dependency
- `avrag-rs/crates/app/src/lib.rs` — add `UsageLimitService` to `AppState`
- `avrag-rs/crates/app/src/chat/graphflow.rs` — add `QuotaCheck` node
- `avrag-rs/crates/app/src/chat/service.rs` — wire quota check, metering ctx
- `avrag-rs/crates/transport-http/Cargo.toml` — add usage-limit dependency
- `avrag-rs/crates/transport-http/src/lib.rs` — add usage-limit API route
- `avrag-rs/bins/worker/src/main.rs` — add summary skip logic
- `avrag-rs/bins/worker/Cargo.toml` — add usage-limit dependency
- `avrag-rs/crates/storage-pg/src/lib.rs` — add usage event + policy queries
- `frontend_rust/Cargo.toml` — no change needed (sdk/ui are already members)
- `frontend_rust/crates/web-sdk/src/lib.rs` — add usage_limit module
- `frontend_rust/crates/web-ui/src/components/mod.rs` — add usage_limit module
- `frontend_rust/crates/web-ui/src/routes/settings.rs` — add UsageLimitPanel to ProfileSettings

## 12. Testing Strategy

- Unit tests for `calculate_usage_units` formula
- Unit tests for policy resolution logic
- Integration tests for rolling window queries (sqlx test with test DB)
- Integration test for quota check → blocked → recovery flow
- Frontend component rendering tests (Leptos testing)
