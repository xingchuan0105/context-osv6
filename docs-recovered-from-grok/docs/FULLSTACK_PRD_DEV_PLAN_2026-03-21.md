# context-osv6 Fullstack PRD Gap Plan

> Date: 2026-03-21
> Scope: `avrag-rs` backend + `frontend_rust` frontend
> Source of truth: `PRD_RUST.md`

## 1. Executive Summary

`context-osv6` has a strong backend core and a now-usable Rust frontend baseline, but it is not yet fully aligned with the full PRD.

Current headline:

- Backend core RAG, ingestion, billing, share, guardrails, chatmemory, API keys, and SSE are substantially implemented.
- Rust frontend now compiles, links into `avrag-api`, supports auth, notebooks, workspace shell, upload, SSE chat, settings, search, share center, and basic admin pages.
- The main remaining gaps are product-completeness gaps rather than foundational build gaps.

The biggest unfinished areas are:

- Backend PRD modules beyond current admin/billing/share scope
- True external URL ingestion pipeline
- Frontend evidence/degrade UX and citation jump flow
- Public share experience completeness
- Full admin surface from the PRD
- Testing, E2E, and release hardening

## 2. Current Assessment

### 2.1 Backend

Status:

- Core product backend is approximately `85%~92%` aligned with the practical PRD execution path.
- Platform/admin/governance parts in PRD sections 27-33 are only partially implemented.

### 2.2 Frontend

Status:

- Rust frontend is approximately `60%~70%` aligned with the frontend PRD surface.
- It has passed the “can compile and integrate” milestone.
- It has not passed the “fully polished WorkspaceLM-grade product UI” milestone.

## 3. Confirmed Gaps

## 3.1 Backend Gaps

### B1. URL source ingestion is still placeholder content

Current code writes synthetic text instead of actually fetching and parsing remote content.

Evidence:
- [app/src/lib.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L1244)
- [app/src/lib.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/lib.rs#L1285)

Impact:
- PRD section 8.2 requires real HTML/web content extraction.
- The current `/sources/url` flow is not production-grade.

Required outcome:
- Real fetch
- content extraction
- metadata capture
- ingestion task integration
- failure handling

### B2. Admin surface is much smaller than PRD section 29

Current router only exposes:
- organizations
- organization detail
- users
- usage
- billing block
- health

Evidence:
- [transport-http/src/lib.rs](/home/chuan/context-osv6/avrag-rs/crates/transport-http/src/lib.rs#L447)

Missing relative to PRD:
- `/admin/billing`
- `/admin/rag-health`
- `/admin/feature-flags`
- `/admin/audit-logs`
- `/admin/system/workers`
- `/admin/system/degradation`

Impact:
- Ops and governance workflow remains incomplete.

### B3. API governance features from PRD section 33 are not fully implemented

Current code has versioned routes and OpenAPI skeleton, but lacks full API-management discipline in code.

Likely missing or partial:
- operation-level metadata completeness
- full version lifecycle / deprecation / sunset support
- broad API contract coverage in generated OpenAPI

Evidence:
- [transport-http/src/lib.rs](/home/chuan/context-osv6/avrag-rs/crates/transport-http/src/lib.rs#L869)

Impact:
- Good enough for active development, not enough for mature external platform governance.

### B4. Request idempotency contract for chat is not fully productized

PRD section 18.2 requires request retry reuse via `request_id`.
Backend already has request context IDs in middleware, but the chat contract does not expose a dedicated client-facing `request_id` field in the current `ChatRequest`.

Impact:
- Retry semantics are not yet explicitly controllable by clients.

### B5. Evaluation pipeline is still skeletal

The PRD asks for a true evaluation pipeline with golden set, metrics, and release gates.
The `tests/rag_quality` harness still contains placeholders.

Evidence:
- `tests/rag_quality/src/harness.rs` has TODO-marked placeholders in search/retrieval/answer wiring.

Impact:
- Quality gates are not yet production-credible.

## 3.2 Frontend Gaps

### F1. Public shared notebook page is still incomplete

Current page loads notebook title and provides chat, but does not render the document/source list, permission metadata, expiration info, or richer public-share structure expected from the existing product direction.

Evidence:
- [routes/shared.rs](/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/routes/shared.rs#L441)

Impact:
- Public share experience is functionally present but materially thinner than v5 and the PRD target.

### F2. Citation jump-to-source flow is incomplete

Current evidence panel lists citations and allows selecting an active citation, but it does not yet:
- open the document viewer
- jump to page/chunk
- highlight the referenced content
- round-trip through parsed preview

Evidence:
- [chat_trace_panel.rs](/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/components/chat/chat_trace_panel.rs#L69)

Impact:
- PRD section 19.2 is not satisfied.

### F3. `degrade_trace` is not surfaced as a user-facing warning band

Backend emits `degrade_trace`, but the frontend currently does not present a dedicated visible degrade banner explaining reliability impact.

Impact:
- PRD section 20.3 is not met.
- Degraded answers are harder for users to interpret correctly.

### F4. Document status polling is still missing

The PRD expects:
- optimistic processing state
- polling every 2 seconds until terminal state

Current frontend refreshes the sources list after upload and reindex, but does not maintain a dedicated status poller loop.

Impact:
- Upload/reindex UX is less reliable and less transparent than required.

### F5. Parsed preview / source viewer integration is still shallow

Current document detail uses content fetch but is not yet a full source viewer with parsed preview pagination, page-aware navigation, and citation-linked highlighting.

Impact:
- PRD sections 17 and 19 are only partially satisfied.

### F6. Dashboard shell still lacks several mature product entry points

Missing or incomplete:
- explicit API access / API key management page
- MCP/OpenAI-compatible setup panel
- v5-style favorite/shared notebook entry area
- richer top-level notification center UX

Impact:
- Platform usability is below the target product level even though backend capabilities exist.

### F7. Admin detail still contains fake data

Organization detail currently renders mock subscription info and an estimated usage card instead of real billing/usage-backed summaries.

Evidence:
- [components/admin/mod.rs](/home/chuan/context-osv6/frontend_rust/crates/web-ui/src/components/admin/mod.rs#L204)

Impact:
- Admin UI is not trustworthy yet.

### F8. Share center is now functional but not complete

Implemented:
- settings
- analytics
- access logs
- members
- invite/remove

Still missing or thin:
- access/permission explanation polish
- token management depth
- richer collaborator UX
- share/public state explanation and abuse signals

## 3.3 Cross-Cutting Gaps

### X1. Full E2E coverage is still missing

PRD asks for critical E2E:
- upload
- ingestion
- RAG chat
- citation lookup
- source viewer
- key flows
- billing/share/admin smoke

Current repo does not yet show a Rust-frontend-aligned E2E suite covering the end-to-end product.

### X2. Release-hardening still needs work

Still needed:
- smoke scripts for Rust frontend paths
- CI gates for frontend workspace
- production asset serving verification
- better observability across frontend-triggered flows

### X3. Frontend-backend PRD alignment docs need consolidation

There are already multiple planning documents:
- `avrag-rs/DEV_PLAN.md`
- `avrag-rs/GAP_ANALYSIS.md`
- `frontend/docs/rust-frontend-design.md`
- `frontend_rust/PLAN.md`

These should be kept, but a single current master execution board is now needed.

## 4. Full Development Plan

## Phase 0: Stabilize the Current Shipping Baseline

Goal:
- ensure the current Rust frontend/backend integration remains a stable base

Tasks:
- remove remaining non-actionable warnings where low-cost
- verify SSR + hydrate asset paths in real dev and prod-like runs
- verify auth bootstrap behavior in browser, including reload and logout
- verify upload path against both memory and postgres runtime modes
- verify search page and shared page basic UX manually or with smoke tests

Deliverables:
- clean local runbook
- smoke checklist
- reduced warning noise

## Phase 1: Close Critical User-Facing Product Gaps

Goal:
- make the product feel complete for end users

Tasks:
- implement document status polling loop after upload and reindex
- add visible degrade banner driven by `degrade_trace`
- implement citation click -> source viewer jump flow
- upgrade document viewer to parsed preview + page/chunk navigation
- enrich public share page with:
  - source list
  - permission info
  - expiration info
  - clearer invalid/expired states
- improve shared notebook chat page to show citations/sources, not only answer text

Acceptance:
- user can upload, wait, ask, inspect, and verify answers end to end

## Phase 2: Complete Platform Access Features

Goal:
- expose already-built platform capabilities in the Rust frontend

Tasks:
- build dedicated API access page for notebook API keys
- add OpenAI-compatible usage examples
- add MCP setup panel
- add API key create/revoke UX
- expose docs/openapi/metrics/dev links for admin/operators where appropriate
- restore or replace favorites/shared notebook dashboard area from v5

Acceptance:
- notebook owner can discover, create, and manage external integrations without leaving the Rust frontend

## Phase 3: Finish Share and Collaboration Surface

Goal:
- make share/collab production-ready

Tasks:
- improve member lifecycle UX:
  - invite pending
  - accepted
  - declined
  - removed
- add clearer access-level explanation
- improve analytics visualization
- make access logs richer and filterable
- add token lifecycle management:
  - show active token
  - show revoked token state if needed
  - generate new link intentionally

Acceptance:
- share center supports realistic collaboration operations with clear state transitions

## Phase 4: Finish Settings and Account Surface

Goal:
- complete account self-service

Tasks:
- polish profile update UX
- polish password reset and password change flows
- improve notifications page:
  - timestamps
  - event metadata
  - pagination or limit strategy
- add language/theme/account controls if still desired in Rust frontend scope

Acceptance:
- user settings stop feeling like a migration placeholder and become a real account center

## Phase 5: Finish Admin Surface

Goal:
- align admin product with PRD section 29 as closely as current backend allows

Tasks:
- remove fake subscription/usage values from org detail
- wire real usage data into org detail
- add org selection/filtering for users and usage pages
- improve block/unblock mutation UX
- add richer health page:
  - service status
  - version
  - metrics summary
- define and implement next admin backend modules:
  - billing overview
  - rag-health
  - feature flags
  - audit logs
  - worker/system status
  - degradation status

Acceptance:
- admin panel is operationally useful and no longer a thin shell

## Phase 6: Backend Platform Feature Expansion

Goal:
- close the major backend PRD gaps outside the current core path

Tasks:
- implement real URL ingestion:
  - fetch
  - sanitize
  - content extraction
  - metadata capture
  - queueing
- extend admin routes to support PRD section 29 modules
- strengthen API governance:
  - richer OpenAPI
  - operation metadata
  - versioning lifecycle hooks
- add explicit chat `request_id` contract support
- complete evaluation harness and release gates

Acceptance:
- backend matches not only the core product path but also the operational PRD envelope

## Phase 7: Test, Quality, and Release Readiness

Goal:
- move from “feature complete” to “release ready”

Tasks:
- Rust frontend component tests for critical paths
- full E2E suite:
  - auth
  - notebook CRUD
  - upload
  - polling
  - chat SSE
  - citation lookup
  - shared page
  - share center
  - admin
- quality gates for frontend and backend in CI
- regression checklist against PRD sections 16-21 and 29-31

Acceptance:
- stable release checklist exists and passes

## 5. Priority Order

### P0

- real URL ingestion backend
- document status polling frontend
- degrade banner frontend
- citation jump/source viewer completion frontend
- org detail fake-data removal

### P1

- API access page
- public share page completeness
- users/usage/admin filtering and polish
- request_id contract for chat
- richer OpenAPI / API governance

### P2

- feature flags admin
- worker/degradation admin
- audit-log admin
- advanced analytics polish

## 6. Milestone Definition

### Milestone M1: Product Complete User Flow

User can:
- log in
- create notebook
- upload document
- wait for completion
- ask via SSE
- inspect citations
- jump to source
- share notebook

### Milestone M2: Platform Complete

Owner/admin can:
- manage API keys
- use OpenAI/MCP access
- manage share members
- review analytics/logs
- use a trustworthy admin panel

### Milestone M3: Release Ready

- CI gates complete
- E2E complete
- no fake-data panels remain
- core PRD gaps closed or explicitly accepted

## 7. Recommended Execution Strategy

Recommended next sequence:

1. finish evidence/degrade/source-viewer UX
2. replace backend URL placeholder ingestion with real implementation
3. finish API access/integration panel
4. remove admin fake data and improve filtering
5. implement missing admin platform modules
6. build E2E suite and release gates
