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

The project is currently in **Phase 5 (Unified Agent Integration & End-to-End Hardening)**.

- **Backend Status**: All Rust unit tests and contract integration tests pass. The migration from legacy graph flows to the `UnifiedAgentService` is complete. The system relies on Postgres, Redis, Milvus, and MinIO.
- **Frontend Status**: The production frontend (`frontend_next`) is fully updated. Dynamic routing parameters (e.g., `params.token`) have been updated to support Next.js 15's promise-based architecture. Unit tests (Vitest) are fully passing.
- **E2E Test Architecture**: Playwright is configured to run integration suites (`auth` and `functional`). The `avrag-worker` has been added to the Playwright `webServer` lifecycle with a dedicated TCP health check listener (on port `8081`) to automatically process background tasks like document parsing during tests.

---

## 4. Remaining Gaps & Goals

### Gaps
1. **Document Ingestion Worker Latency in E2E**: Background tasks processed via `avrag-worker` are asynchronous. Ingestion specs in E2E tests must implement robust polling with sensible timeouts to wait for worker status updates.
2. **Environment Configuration Safety**: Local testing relies heavily on the presence of Milvus, MinIO, Redis, and Postgres. CI environments need mock configurations or containerized service bindings to prevent failures.

### Goals
1. **Complete E2E Verification**: Ensure all 12+ functional and auth Playwright specifications execute and pass.
2. **Incremental Commit**: Commit the consolidated codebase to `master` to preserve the fixed middleware rate-limits, promise-based Next.js page modifications, and test suite improvements.
3. **Clean Root Hygiene**: Keep the workspace root clean, storing all design blueprints, checklists, and specs inside `docs/`.
