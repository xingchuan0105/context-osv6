# Context-OS Global Context & Domain Dictionary (CONTEXT.md)

This document serves as the project's source of truth for architecture, domain terminology, development status, and remaining work.

---

## 1. Domain Dictionary (Ubiquitous Language)

To maintain semantic consistency across the codebase, tests, and documentation, the following terminology is defined:

| Term | Definition | Context / Scope |
| :--- | :--- | :--- |
| **Unified Agent** | The core single-agent service architecture (V5) that replaced the legacy multi-agent graph flows. It handles stream chats, RAG execution, and web searches. | Backend (`avrag-rs/crates/app`) |
| **avrag-api** | The HTTP/REST API server providing backend functionalities to the frontend, including authentication, chat streams, and workspace uploads. | Backend (`avrag-rs/bins/api`) |
| **avrag-worker** | The background worker that claims and processes asynchronous ingestion, analytics, audit logging, and document cleanup tasks. | Backend (`avrag-rs/bins/worker`) |
| **frontend_next** | The main production frontend application built using Next.js 15+, React, TypeScript, Tailwind CSS, and `pnpm`. | Frontend (`frontend_next`) |
| **RAG Ingestion** | The multi-stage process of converting uploaded documents (PDFs, Markdown, Office Docs) into chunked, normalized representations and indexing them into Milvus/PostgreSQL. | Pipeline (`crates/ingestion` & `bins/worker`) |
| **E2E Throttling Bypass** | The mechanism where the HTTP rate-limiter is raised to 10,000 RPM when `E2E_ENABLED=true` is set, avoiding HTTP 429 errors during automated testing. | Middleware (`crates/transport-http`) |
| **Free Tier** | The free billing tier, providing base quota limits for chat, RAG, and document storage. | Billing (`avrag-rs/crates/billing`) |
| **Plus Tier** | The mid-level subscription tier (replacing the legacy Enterprise tier), offering higher usage quotas and advanced search features. | Billing (`avrag-rs/crates/billing`) |
| **Pro Tier** | The highest subscription tier, offering maximum execution limits and priority resources. | Billing (`avrag-rs/crates/billing`) |
| **Creem Provider** | B2C manual subscription billing provider via Creem checkout for global credit cards. | Billing (`avrag-rs/crates/billing`) |
| **Alipay Provider** | B2C manual subscription billing provider via Alipay precreate QR scan-code for CNY payments. | Billing (`avrag-rs/crates/billing`) |
| **Lazy Billing Downgrade** | Automatic check and state transition of expired user subscriptions to 'expired' and downgrade to the free tier upon access or API query. | Billing (`avrag-rs/crates/billing`) |

---

## 2. Workspace & Git Worktrees

The repository's git worktree structure has been consolidated and cleaned up:

- **Primary Workspace**: `/home/chuan/context-osv6` (on `master` branch) contains all current active developments, unified tests, and frontend wiring.
- **Obsolete/Merged Worktrees Cleaned**:
  - `worktree-agent-ae326bb2e7a264b82`: Fully merged to `master` and physically removed.
  - `worktree-p0-prompt-injection-fixes`: Outdated and superseded by V5 migration. Removed.
  - `worktree-e2e-analyzer`: Obsolete. Active logic integrated directly into the `master` workspace as uncommitted changes.
  - Temporary detached worktrees in `/tmp` have been pruned.

---

## 3. Development Stage Assessment

The project is currently in **Phase 5 (Unified Agent Integration & End-to-End Hardening)** with active **pricing-tier revamp** work on branch `feat/pricing-tiers-revamp`.

- **Backend Status**: All Rust unit tests and contract integration tests pass. The migration from legacy graph flows to the `UnifiedAgentService` is complete. Billing exposes rolling-window usage (`/billing/usage/window`) and structured quota denial reasons (`QuotaDenyReason`). Strategy capability schemas are decoupled from the deprecated strategy runtime via `capability/schemas.rs`. The system relies on Postgres, Redis, Milvus, and MinIO.
- **Frontend Status**: The production frontend (`frontend_next`) is fully updated. Settings billing tab wires `UsageMeter` (5h/7d rolling windows) with `data-testid` hooks (`usage-meter`, `plan-display`). Dynamic routing parameters support Next.js 15's promise-based architecture. Vitest covers billing format/API/UsageMeter components.
- **E2E Test Architecture**: Playwright runs `smoke`, `journey`, `skills`, `billing`, and `visual` suites. Journey specs use isolated run contexts; `avrag-worker` is in the Playwright `webServer` lifecycle with a TCP health check on port `8081` for ingestion polling. Billing E2E asserts usage meter and plan display on `/settings?tab=billing`.

---

## 4. Remaining Gaps & Goals

### Gaps
1. **Document Ingestion Worker Latency in E2E**: Background tasks processed via `avrag-worker` are asynchronous. Ingestion specs must keep robust polling with sensible timeouts for worker status updates.
2. **Strategy Layer Deprecation (Phase C)**: `agents/strategy/` runtime remains for execution; static schemas moved to `capability/schemas/`. Full strategy test migration and `strategy/` deletion deferred.
3. **Environment Configuration Safety**: Local testing relies on Milvus, MinIO, Redis, and Postgres. CI needs containerized service bindings or mocks.

### Goals
1. **Complete E2E Verification**: Run and stabilize all Playwright specs including billing (`usage-settings`, `usage-meter`) and journey suites.
2. **Pricing Revamp Rollout**: Merge tier quota changes (migration 0037), remove dead feature flags, and land structured billing UX.
3. **Clean Root Hygiene**: Keep design blueprints, checklists, and specs inside `docs/`.
