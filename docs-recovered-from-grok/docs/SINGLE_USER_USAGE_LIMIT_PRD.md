# Single User Usage Limit PRD

> Project: `context-osv6`
> Updated: 2026-03-30
> Status: confirmed draft
> Scope: single-user LLM usage limit for interactive and summary-related product flows
> Source inputs:
> - current Rust backend implementation in `avrag-rs/`
> - current Rust frontend settings implementation in `frontend_rust/`
> - product clarification confirmed on 2026-03-30:
>   - 5-hour limit uses rolling window
>   - weekly limit uses rolling 7-day window
>   - quota unit uses normalized `usage_units`, not raw tokens

## 1. Document Purpose

This document defines the product and implementation specification for a single-user usage limit feature.

The feature should behave similarly to Codex-style quota presentation:

- a rolling 5-hour limit
- a rolling 7-day limit
- a clear usage surface in the user profile page
- usage counted only for selected large-model features

This PRD is intentionally separate from the existing org-level billing and monthly quota logic.

## 2. Background

The current codebase already has:

- org-level `usage_events`
- org-level `quota_limits`
- monthly billing usage aggregation
- chat preflight checks for `llm_input_tokens` and `llm_output_tokens`

However, the current implementation is not sufficient for this requirement because it does not support:

- user-level accounting
- rolling 5-hour windows
- rolling 7-day windows
- feature-level breakdown for `summary / planner / answer / search / chat`
- clear personal usage display in the profile page
- real usage attribution across all actual LLM calls

Therefore, this feature must be implemented as a new user-level quota system, not as a minor extension of the current org billing counters.

## 3. Product Goal

### 3.1 Primary goal

An authenticated user can always see:

- how much quota has been used in the past 5 hours
- how much quota has been used in the past 7 days
- how much quota remains
- when quota is expected to become available again
- which product capabilities consumed the quota

### 3.2 Enforcement goal

When the user exceeds either rolling limit, new billable LLM requests must be blocked with a clear product-facing message.

### 3.3 Product principle

Quota should reflect actual large-model consumption, not approximate page counts or document counts.

## 4. Scope

### 4.1 Included billable features

The following feature categories are in scope and must count toward the single-user limit:

- `summary`
  - document summary generation in the worker
  - session summary generation during chat memory maintenance
- `planner`
  - RAG planner
  - search planner
- `answer`
  - general answer generation
  - RAG answer synthesis
  - search answer synthesis
- `search`
  - model-backed search execution where the search provider itself consumes LLM quota
  - example: agentic search provider calls
- `chat`
  - model-backed conversational helper steps that are not final answer generation
  - example: general query rewrite based on memory context

### 4.2 Explicitly excluded from quota

The following must not consume the single-user limit:

- `mineru`
- `embedding`
- `rerank`
- non-LLM retrieval work
- storage, parsing, and indexing work without billable LLM calls

### 4.3 Attribution rule

Usage must be attributed to a concrete user.

- interactive requests use the authenticated request user
- worker-side document summary generation uses `requested_by` when available
- system-triggered maintenance or reindex tasks without a user owner do not consume single-user quota

## 5. Core Definitions

### 5.1 Rolling windows

- `rolling_5h`: all billable usage events with `created_at >= now() - 5 hours`
- `rolling_7d`: all billable usage events with `created_at >= now() - 7 days`

These are rolling windows, not calendar buckets.

### 5.2 Quota unit

The enforcement unit is `usage_units`.

`usage_units` is a product-defined normalized unit used to combine usage from different models into one comparable quota system.

It is not:

- raw tokens
- provider bill in currency
- a direct copy of vendor pricing

### 5.3 Usage unit formula

Each billable LLM event stores:

- `prompt_tokens`
- `completion_tokens`
- `total_tokens`
- `usage_units`

`usage_units` is calculated by a model weight table:

`usage_units = max(1, ceil((prompt_tokens / 1000 * input_unit_rate) + (completion_tokens / 1000 * output_unit_rate)))`

Where:

- `input_unit_rate` is the product-defined normalized weight for 1K input tokens of the model
- `output_unit_rate` is the product-defined normalized weight for 1K output tokens of the model

Notes:

- output is usually more expensive than input, so the two rates should not be assumed equal
- weights are configurable by model
- the formula should use actual provider-reported usage whenever available
- when provider usage is unavailable, tokens are estimated locally and the record is marked as estimated

### 5.4 Model weight table

The system must maintain a configurable model weight table.

Minimum fields:

- `provider`
- `model`
- `input_unit_rate`
- `output_unit_rate`
- `enabled`
- `effective_from`

This table is the source of truth for `usage_units` conversion.

## 6. User Stories

### 6.1 Visibility

As a user, I want to see my 5-hour and 7-day quota usage in my profile page, so I know whether I can continue using the product.

### 6.2 Clarity

As a user, I want to know which features consumed my quota, so the quota system feels fair and understandable.

### 6.3 Enforcement

As a user, when I hit the limit, I want the system to tell me:

- which limit I hit
- how much I used
- what the limit is
- when quota is expected to free up again

### 6.4 Upload behavior

As a user, if I upload a document while over limit, I still want the base ingestion flow to succeed where possible, even if model-generated summary is skipped.

## 7. Functional Requirements

### 7.1 Limit policy

The system must support two user-level limits:

- rolling 5-hour limit
- rolling 7-day limit

The effective user policy should be resolved in this order:

1. explicit user override, if present
2. plan-derived default policy, if present
3. global default policy

Minimum policy fields:

- `rolling_5h_limit_units`
- `rolling_7d_limit_units`
- `enabled`
- `plan_id` or `user_id` scope

### 7.2 Quota check

Before a new billable interactive request starts, the system must check:

- current used units in the past 5 hours
- current used units in the past 7 days

If either window is already at or above limit, the request must be rejected.

### 7.3 Billable request types that must be blocked on exceed

When over limit, the system must block new billable interactive flows, including:

- general chat
- RAG chat
- search chat
- any direct API route that invokes billable `summary / planner / answer / search / chat` LLM steps

### 7.4 Background summary behavior

Worker-side document summary generation is special:

- the document ingestion pipeline must not be fully rejected because of summary quota
- if the user is over quota, the worker should skip model-based summary generation
- the pipeline should keep the existing fallback summary behavior or no-summary path
- the skip should be visible in logs and traceable for debugging

### 7.5 Immediate update

After a billable LLM call completes, the corresponding usage must be visible in:

- subsequent quota checks
- profile page usage display
- backend query APIs

### 7.6 Breakdown

The user-facing usage view must include feature breakdown for:

- `summary`
- `planner`
- `answer`
- `search`
- `chat`

## 8. Interaction and UX Requirements

### 8.1 Placement

The quota display must exist in the user profile page, not only in billing.

In the current Rust frontend, the primary placement should be the top area of `ProfileSettings` in [settings.rs](/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/routes/settings.rs).

### 8.2 Required UI content

The profile page must clearly show:

- 5-hour usage bar
- 7-day usage bar
- used units
- total limit units
- remaining units
- usage percentage
- expected recovery time when blocked
- feature breakdown list
- scope description:
  - includes `summary / planner / answer / search / chat`
  - excludes `MinerU / Embedding / Rerank`

### 8.3 Status states

The UI must support:

- normal
- near limit
- blocked
- loading
- API error

### 8.4 Copy requirements

The blocked state must use direct product language. At minimum:

- which window was exceeded
- current usage
- limit
- estimated resume time

## 9. Backend Design

### 9.1 New ledger table

Add a new user-level usage ledger table.

Recommended name:

- `llm_usage_events`

Minimum fields:

- `id`
- `org_id`
- `user_id`
- `feature`
- `stage`
- `provider`
- `model`
- `prompt_tokens`
- `completion_tokens`
- `total_tokens`
- `usage_units`
- `usage_source`
  - `actual`
  - `estimated`
- `session_id` nullable
- `document_id` nullable
- `request_id` nullable
- `trace_id` nullable
- `created_at`

Recommended indexes:

- `(user_id, created_at desc)`
- `(user_id, feature, created_at desc)`
- `(org_id, user_id, created_at desc)`

This table is separate from existing `usage_events`.

### 9.2 Policy tables

Add user-level policy resolution tables.

Recommended structure:

- `usage_limit_plan_policies`
  - `plan_id`
  - `rolling_5h_limit_units`
  - `rolling_7d_limit_units`
  - `enabled`
  - timestamps
- `usage_limit_user_overrides`
  - `user_id`
  - optional override fields
  - `enabled`
  - timestamps

This allows current subscription plan linkage while preserving real user-level enforcement.

### 9.3 Quota query service

Add a dedicated usage limit service in backend application code.

Responsibilities:

- resolve effective policy
- query rolling 5-hour usage
- query rolling 7-day usage
- compute per-feature breakdown
- compute blocked state
- compute next recovery timestamp

### 9.4 Next recovery timestamp

For each window, the backend must compute:

- `next_relief_at`: earliest timestamp at which at least some units expire out of the active window
- `blocked_until`: if currently blocked, earliest timestamp at which usage falls below the limit again

This must be based on active usage events ordered by `created_at`.

For an event in a given window:

- 5-hour expiry time = `created_at + 5 hours`
- 7-day expiry time = `created_at + 7 days`

### 9.5 Existing org billing compatibility

The current org-level billing system remains unchanged.

Rules:

- existing `usage_events` and monthly billing API continue to serve org/billing logic
- new single-user limit logic uses `llm_usage_events`
- the two systems may share raw usage sources, but they do not share the same ledger or enforcement path

## 10. Metering Pipeline Design

### 10.1 Core rule

All billable LLM calls must be metered at the point where actual LLM usage is known.

The current best insertion point is the shared LLM client layer, centered around [client.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/client.rs).

### 10.2 Required metering context

Each billable call must carry a metering context with at least:

- `user_id`
- `org_id`
- `feature`
- `stage`
- `session_id` if applicable
- `document_id` if applicable
- `request_id` or `trace_id`

### 10.3 Feature mapping

The following mapping is required in v1:

- `summary`
  - worker `SummaryGenerator::synthesize`
  - `AppState::build_session_summary`
- `planner`
  - `RetrievalPlanner::plan`
  - `SearchExecutor::plan_query_with_llm`
- `answer`
  - general final answer generation
  - `AnswerSynthesizer::synthesize`
  - search answer synthesis inside search executor
- `search`
  - model-backed external search provider calls that themselves consume LLM quota
  - example: Perplexity Agent API call
- `chat`
  - `AppState::refine_general_query`
  - future conversational helper calls that are not final answer generation

### 10.4 Excluded mapping

These paths must not write to the single-user ledger:

- `EmbeddingClient`
- `RerankerClient`
- MinerU parsing

### 10.5 Actual vs estimated usage

Rules:

- if provider response includes usage, write `usage_source = actual`
- if provider response does not include usage, estimate prompt and completion tokens locally and write `usage_source = estimated`
- the API must expose whether a usage window contains estimated usage

### 10.6 Write timing

The ledger entry should be written immediately after a billable call succeeds and before request completion exits the application flow.

## 11. Quota Enforcement Design

### 11.1 Enforcement model

V1 enforcement is request-start gating plus actual post-call accounting.

Rules:

- before a new billable interactive request starts, check rolling windows
- if user is already over either limit, reject immediately
- if a running request consumes the remaining budget and crosses the limit, allow that in-flight request to complete
- subsequent requests are blocked

This behavior is acceptable for v1 and is simpler than introducing a reservation system for every multi-stage call chain.

### 11.2 Interactive preflight

Interactive entry points should use a unified preflight check before any billable LLM call begins.

Current high-priority insertion points:

- chat preflight in [service.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/chat/service.rs)
- any future dedicated summary, planner, or search API endpoints

### 11.3 Worker enforcement

Worker-side document summary generation must perform its own user quota check before calling the summary model.

If blocked:

- skip billable summary generation
- keep ingestion successful if other required steps succeed

### 11.4 Failure behavior

V1 operational policy:

- if quota read path fails, interactive billable requests should fail closed with a dedicated error
- if usage write path fails after a successful call, return the user result but emit high-severity telemetry and audit data

This keeps abuse risk low on read path while avoiding unnecessary user-visible failures after a model has already produced a result.

## 12. API Design

### 12.1 Primary user API

Add:

- `GET /api/auth/usage-limit`

Response must include:

- effective policy
- rolling 5-hour window usage
- rolling 7-day window usage
- per-feature breakdown
- blocked state
- next recovery timestamps

### 12.2 Suggested response shape

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
    "rolling_7d": {
      "used_units": 360,
      "limit_units": 1000,
      "remaining_units": 640,
      "percent_used": 36.0,
      "blocked": false,
      "next_relief_at": "2026-03-31T04:12:00Z",
      "blocked_until": null
    }
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

### 12.3 Error contract

When a billable request is blocked by quota, return HTTP `429`.

Recommended error code:

- `usage_limit_exceeded`

Required payload semantics:

- identify which window caused the block
- include current used units
- include limit units
- include `blocked_until` if known

Recommended message format:

- Chinese-first product copy
- concise and directly actionable

## 13. Frontend Design

### 13.1 Page placement

In the current Rust frontend:

- show the feature in `Settings > Profile`
- place it above or immediately after the basic profile identity fields

This satisfies the requirement that the user can clearly query quota from the personal information page.

### 13.2 UI structure

Recommended component sections:

1. `个人用量`
2. `5 小时限额`
3. `7 天限额`
4. `功能分布`
5. `统计范围说明`

### 13.3 Visual states

Recommended thresholds:

- under 70%: normal
- 70% to under 90%: warning
- 90% and above: danger
- blocked: explicit blocked state with resume time

### 13.4 Data source

The frontend should not infer limits from the existing billing page APIs.

It must use the dedicated new user usage-limit API.

## 14. Observability and Audit

The system must emit structured telemetry for:

- quota check start
- quota check result
- blocked request
- usage ledger write
- estimated usage fallback
- worker summary skip due to quota

Recommended log dimensions:

- `org_id`
- `user_id`
- `feature`
- `stage`
- `model`
- `usage_units`
- `usage_source`
- `request_id`
- `trace_id`

## 15. Rollout Strategy

### 15.1 Phase 1: shadow mode

Implement ledger writes and profile API first, without blocking requests.

Goals:

- validate feature mapping
- validate unit weights
- validate UI readability

### 15.2 Phase 2: profile visibility

Expose usage in the profile page while still not enforcing hard blocking.

### 15.3 Phase 3: enforce 5-hour limit

Enable interactive blocking for rolling 5-hour limit first.

### 15.4 Phase 4: enforce 7-day limit

Enable weekly blocking after real usage distribution is understood.

## 16. Acceptance Criteria

The feature is complete only if all of the following are true:

- user can see 5-hour and 7-day quota in profile page
- profile page clearly states included and excluded scope
- usage is broken down by `summary / planner / answer / search / chat`
- new interactive billable requests return `429` when over limit
- document ingestion still works when document summary is skipped because of quota
- `mineru`, `embedding`, and `rerank` do not change displayed quota
- provider usage is recorded as actual when available and estimated when unavailable
- existing org billing behavior remains intact

## 17. Non-Goals

The following are explicitly not required in v1:

- merging this feature into current monthly org billing APIs
- using raw tokens directly as the enforcement unit
- backfilling historical per-user usage from old org-level ledgers
- building an admin quota configuration UI in the same milestone
- strict multi-request reservation accounting across all concurrent requests

## 18. Implementation Notes For This Repository

### 18.1 Current backend insertion points

The following existing code paths must be considered implementation anchors:

- `avrag-rs/crates/llm/src/client.rs`
- `avrag-rs/crates/llm/src/planner.rs`
- `avrag-rs/crates/llm/src/synthesizer.rs`
- `avrag-rs/crates/llm/src/summary.rs`
- `avrag-rs/crates/search/src/lib.rs`
- `avrag-rs/crates/app/src/chat/service.rs`
- `avrag-rs/crates/app/src/lib.rs`
- `avrag-rs/bins/worker/src/main.rs`

### 18.2 Current frontend insertion point

- `frontend_rust/crates/web-ui/src/routes/settings.rs`

### 18.3 Existing logic to avoid coupling with

The new single-user limit feature must not be forced into the old DTO model in:

- `frontend_rust/crates/web-ui/src/components/billing/mod.rs`
- `frontend_rust/crates/web-sdk/src/billing.rs`
- `avrag-rs/crates/billing/src/lib.rs`

Those modules are currently shaped around org-level billing and monthly quota semantics, not personal rolling usage windows.
