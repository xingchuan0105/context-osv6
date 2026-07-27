# context-osv6 Launch Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> Historical plan. Infrastructure references to Qdrant reflect the March 2026 launch-gap implementation profile. Current retrieval architecture target is Milvus; see [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

**Goal:** Close the highest-impact PRD and production-readiness gaps so the canonical frontend and backend can run the main user flows with consistent contracts and enforceable release gates.

**Architecture:** Fix the system in dependency order. First freeze and align the canonical API contracts between `frontend_rust` and `avrag-rs/crates/transport-http`. Then implement the missing backend surfaces that the canonical frontend already depends on, followed by streaming/chat UX correctness and production engineering hardening. Keep the Rust frontend as the canonical user surface and treat `transport-http` as the canonical HTTP release surface.

**Tech Stack:** Rust (`axum`, `sqlx`, `tokio`, `serde`), Leptos, Playwright, PostgreSQL, Qdrant, shell-based verification.

---

## File Map

**Primary backend files**
- Modify: `avrag-rs/crates/transport-http/src/lib.rs`
  - Router registration, auth/profile/preferences handlers, middleware, readiness/metrics/openapi stubs.
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
  - `/api/v1` notebook/chat/search handlers and shared JSON response helpers.
- Modify: `avrag-rs/crates/app/src/lib.rs`
  - Shared app-facing API surface used by HTTP handlers, preferences persistence hooks, URL import verification.
- Modify: `avrag-rs/crates/app/src/chat/service.rs`
  - Interactive quota preflight, streaming readiness, response contract alignment.
- Modify: `avrag-rs/crates/usage-limit/src/lib.rs`
  - DB-backed quota tests and release-gate notes if needed.
- Modify: `avrag-rs/tests/rag_quality/src/harness.rs`
  - Convert the current skeleton into an actually executable harness.
- Create/Modify: `avrag-rs/.github/workflows/*.yml`
  - CI gates once the runtime path is stable.
- Create: `avrag-rs/Dockerfile`
- Create: `avrag-rs/docker-compose.yml`

**Primary frontend files**
- Modify: `frontend_rust/crates/web-sdk/src/auth.rs`
  - Typed auth contract clients.
- Modify: `frontend_rust/crates/web-sdk/src/notebooks.rs`
  - Typed notebook contract clients.
- Modify: `frontend_rust/crates/web-sdk/src/chat.rs`
  - Session/citation/chat contract clients.
- Modify: `frontend_rust/crates/web-sdk/src/documents.rs`
  - Document upload/status/content/parsed-preview contract clients.
- Modify: `frontend_rust/crates/web-sdk/src/lib.rs`
  - Shared DTOs and common API decoding helpers.
- Modify: `frontend_rust/crates/web-ui/src/app.rs`
  - Auth bootstrap behavior after `me()` contract alignment.
- Modify: `frontend_rust/crates/web-ui/src/routes/settings.rs`
  - Profile/security/preferences flow after backend persistence is real.
- Modify: `frontend_rust/crates/web-ui/src/routes/dashboard.rs`
  - Document polling/session/source interactions against finalized backend contracts.
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
  - Streaming and degrade-trace rendering against final SSE event model.
- Modify: `frontend_rust/crates/web-ui/src/components/document/mod.rs`
  - Citation/source focus verification against final backend payloads.

**Docs / release artifacts**
- Modify: `frontend_rust/DELIVERY_HANDOFF.md`
- Create or modify: `docs/` release notes / runbook documents as the release surface hardens.

---

## Task 1: Freeze Canonical API Contract Matrix

**Files:**
- Modify: `docs/superpowers/plans/2026-03-31-context-osv6-launch-gap-closure.md`
- Create: `docs/context-osv6-api-contract-matrix.md`
- Reference: `frontend_rust/crates/web-sdk/src/*.rs`
- Reference: `avrag-rs/crates/transport-http/src/lib.rs`
- Reference: `avrag-rs/crates/transport-http/src/handlers.rs`

- [ ] **Step 1: Document the canonical route inventory**

Create `docs/context-osv6-api-contract-matrix.md` with these sections:

```md
# context-osv6 API Contract Matrix

## Canonical surfaces
- Frontend client: `frontend_rust/crates/web-sdk`
- HTTP server: `avrag-rs/crates/transport-http`

## Status legend
- `implemented`
- `implemented_contract_mismatch`
- `missing_route`
- `stub_501`

## Core auth
| Route | Frontend expects | Backend returns | Status |
| --- | --- | --- | --- |
| GET /api/auth/me | AuthEnvelope | bare user object | implemented_contract_mismatch |
| PUT /api/auth/profile | AuthEnvelope | 501 | stub_501 |
| GET /api/auth/preferences | UserPreferences | default payload only | implemented_contract_mismatch |
```

- [ ] **Step 2: Verify the route inventory from code**

Run:

```bash
rg -n "\.route\(|\.nest\(" avrag-rs/crates/transport-http/src/lib.rs
rg -n "pub async fn " frontend_rust/crates/web-sdk/src/*.rs
```

Expected:
- A concrete list of actual registered backend routes
- A concrete list of frontend SDK methods that depend on them

- [ ] **Step 3: Mark mismatch categories**

Populate at minimum these categories in the matrix:
- auth
- notebooks
- chat
- documents
- sources
- admin
- share
- notifications

- [ ] **Step 4: Commit the contract matrix**

```bash
git add docs/context-osv6-api-contract-matrix.md docs/superpowers/plans/2026-03-31-context-osv6-launch-gap-closure.md
git commit -m "docs: add canonical API contract matrix"
```

---

## Task 2: Fix Auth Contract Mismatches First

**Files:**
- Modify: `avrag-rs/crates/transport-http/src/lib.rs`
- Modify: `frontend_rust/crates/web-sdk/src/auth.rs`
- Modify: `frontend_rust/crates/web-ui/src/app.rs`
- Test: `avrag-rs/crates/transport-http/src/lib.rs`

- [ ] **Step 1: Write failing auth contract tests**

Add/extend handler tests to assert:
- `GET /api/auth/me` returns an auth envelope
- `PUT /api/auth/profile` is not `501`
- `GET /api/auth/preferences` returns persisted values, not a hardcoded default echo path

- [ ] **Step 2: Run the targeted backend tests and verify failure**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http auth_me_handler_returns_envelope -- --exact
```

Expected:
- Fail because current `me` response shape is wrong or the test does not yet exist.

- [ ] **Step 3: Implement minimal backend fixes**

Implement:
- `auth_me_handler` returns `AuthEnvelope`
- `auth_update_profile_handler` performs a real update via PostgreSQL and returns `AuthEnvelope`
- `auth_get_preferences_handler` / `auth_update_preferences_handler` load and persist user preferences

- [ ] **Step 4: Align frontend auth client and bootstrap**

Update:
- `frontend_rust/crates/web-sdk/src/auth.rs`
- `frontend_rust/crates/web-ui/src/app.rs`

So that:
- bootstrap login recovery uses the real envelope shape
- settings/profile paths consume the finalized auth payload

- [ ] **Step 5: Run verification**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --lib
cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui --lib
```

Expected:
- Backend auth tests pass
- Frontend bootstrap/profile helper tests remain green

- [ ] **Step 6: Commit**

```bash
git add avrag-rs/crates/transport-http/src/lib.rs frontend_rust/crates/web-sdk/src/auth.rs frontend_rust/crates/web-ui/src/app.rs
git commit -m "fix: align auth contracts with canonical frontend"
```

---

## Task 3: Fix Workspace Contract Mismatches

**Files:**
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
- Modify: `frontend_rust/crates/web-sdk/src/notebooks.rs`
- Test: `avrag-rs/crates/transport-http/src/lib.rs`

- [ ] **Step 1: Write failing route-shape tests**

Add tests asserting:
- `GET /api/v1/notebooks` returns `{ "notebooks": [...] }`
- `POST /api/v1/notebooks` returns `{ "notebook": { ... } }`
- `GET /api/v1/notebooks/{id}` returns `{ "notebook": { ... } }`

- [ ] **Step 2: Run the targeted backend tests and verify failure**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http notebook_routes_return_enveloped_payloads -- --exact
```

Expected:
- Fail against the current bare-array/bare-object behavior.

- [ ] **Step 3: Implement minimal response envelopes**

Update notebook handlers in `handlers.rs` so they return:

```json
{ "notebooks": [...] }
```

or

```json
{ "notebook": { ... } }
```

consistently with the canonical SDK DTOs.

- [ ] **Step 4: Run verification**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --lib
cargo check --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui -p frontend-web-sdk
```

Expected:
- Workspace contract tests pass
- Frontend compiles against the finalized DTO shape

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/transport-http/src/handlers.rs
git commit -m "fix: align notebook route payloads with SDK contracts"
```

---

## Task 4: Implement Missing Core Document and Session Routes

**Files:**
- Modify: `avrag-rs/crates/transport-http/src/lib.rs`
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
- Modify: `avrag-rs/crates/app/src/lib.rs`
- Test: `avrag-rs/crates/transport-http/src/lib.rs`

- [ ] **Step 1: Write failing route-registration tests**

Add backend tests for:
- `GET /api/v1/chat/sessions`
- `POST /api/v1/chat/sessions`
- `GET /api/v1/chat/sessions/{id}`
- `GET /api/v1/chat/sessions/{id}/messages`
- `DELETE /api/v1/chat/sessions/{id}`
- `GET /api/v1/documents`
- `GET /api/v1/documents/{id}/status`
- `GET /api/v1/documents/{id}/content`
- `GET /api/v1/documents/{id}/parsed-preview`
- `PUT /api/v1/documents/{id}`
- `DELETE /api/v1/documents/{id}`
- `POST /api/v1/documents/{id}/reindex`
- `POST /api/v1/chat/citations/lookup`

- [ ] **Step 2: Run tests and watch them fail**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http missing_core_routes_are_registered -- --exact
```

Expected:
- Failure because the routes are currently absent.

- [ ] **Step 3: Register and implement the routes minimally**

Use existing `AppState` methods where possible. Prefer:
- thin HTTP handlers
- shared error helpers
- DTO compatibility with `frontend_rust/crates/web-sdk`

- [ ] **Step 4: Verify targeted flows**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --lib
cargo check --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui -p frontend-web-sdk
```

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/transport-http/src/lib.rs avrag-rs/crates/transport-http/src/handlers.rs avrag-rs/crates/app/src/lib.rs
git commit -m "feat: add core document and chat session routes"
```

---

## Task 5: Upgrade SSE to Incremental Event Streaming

**Files:**
- Modify: `avrag-rs/crates/transport-http/src/handlers.rs`
- Modify: `frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs`
- Test: `avrag-rs/crates/transport-http/src/lib.rs`

- [ ] **Step 1: Write failing SSE contract tests**

Add a backend SSE test that asserts event ordering includes:
- `start`
- at least one intermediate payload event
- `done`

- [ ] **Step 2: Run the SSE test and verify failure**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http chat_sse_emits_incremental_event_sequence -- --exact
```

Expected:
- Failure because the current handler only emits `answer` and `done`.

- [ ] **Step 3: Implement streaming event translation**

Update the backend SSE handler to expose a stable event protocol compatible with the frontend consumer:
- `start`
- `token`
- `citations`
- `trace` / `degrade`
- `done`

If the app layer still produces whole responses, introduce a translation shim explicitly marked as transitional in comments.

- [ ] **Step 4: Verify frontend compatibility**

Run:

```bash
cargo test --manifest-path avrag-rs/Cargo.toml -p transport-http --lib
cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui --lib
```

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/crates/transport-http/src/handlers.rs frontend_rust/crates/web-ui/src/components/chat/chat_panel.rs
git commit -m "feat: expose incremental SSE event stream"
```

---

## Task 6: Add Release-Blocking CI and Runtime Artifacts

**Files:**
- Create: `.github/workflows/rust-ci.yml`
- Create: `avrag-rs/Dockerfile`
- Create: `avrag-rs/docker-compose.yml`
- Modify: `avrag-rs/README.md`

- [ ] **Step 1: Add CI workflow**

Include these jobs at minimum:
- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test -p avrag-usage-limit --lib`
- `cargo test -p transport-http --lib`
- `cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui --lib`

- [ ] **Step 2: Add Docker build path**

Create a baseline image for `bins/api` plus a compose file wiring:
- API
- PostgreSQL
- Qdrant
- Redis (if required by current config)

- [ ] **Step 3: Run local verification**

Run:

```bash
cargo check --manifest-path avrag-rs/Cargo.toml --workspace
cargo test --manifest-path avrag-rs/Cargo.toml -p avrag-usage-limit --lib
cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui --lib
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/rust-ci.yml avrag-rs/Dockerfile avrag-rs/docker-compose.yml avrag-rs/README.md
git commit -m "build: add baseline CI and container artifacts"
```

---

## Task 7: Strengthen Release Verification

**Files:**
- Modify: `avrag-rs/tests/rag_quality/src/harness.rs`
- Modify: `frontend_rust/DELIVERY_HANDOFF.md`
- Create: `docs/context-osv6-release-checklist.md`

- [ ] **Step 1: Replace obvious TODO skeletons in `rag_quality`**

Wire `evaluate_example()` enough that:
- it can invoke the actual RAG path
- it can produce non-placeholder retrieved chunks / answer text
- failures are explicit, not silent TODO placeholders

- [ ] **Step 2: Add a release checklist**

Create `docs/context-osv6-release-checklist.md` with:
- auth bootstrap
- notebook list/create
- document upload + 2s polling
- chat + SSE
- citation focus
- settings/profile/preferences
- usage-limit card

- [ ] **Step 3: Update handoff**

`frontend_rust/DELIVERY_HANDOFF.md` must describe:
- canonical frontend path
- canonical backend HTTP path
- what still remains
- what commands actually prove release readiness

- [ ] **Step 4: Verify**

Run:

```bash
cargo check --manifest-path avrag-rs/Cargo.toml --workspace
cargo test --manifest-path avrag-rs/Cargo.toml -p avrag-usage-limit --lib
cargo test --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui --lib
```

- [ ] **Step 5: Commit**

```bash
git add avrag-rs/tests/rag_quality/src/harness.rs frontend_rust/DELIVERY_HANDOFF.md docs/context-osv6-release-checklist.md
git commit -m "test: strengthen release verification artifacts"
```

---

## Self-Review

- Spec coverage:
  - PRD completeness gaps covered: auth/profile/preferences, `/api/v1` surface, SSE semantics, release engineering, evaluation harness.
  - Production readiness gaps covered: CI/CD, Docker baseline, release checklist, contract matrix.
- Placeholder scan:
  - Remaining intentionally deferred areas are only the deeper admin/share metrics surfaces that depend on the core route surface being stabilized first.
- Type consistency:
  - The plan consistently treats `frontend_rust` as canonical frontend and `avrag-rs/crates/transport-http` as canonical HTTP release surface.

---

Plan complete and saved to `docs/superpowers/plans/2026-03-31-context-osv6-launch-gap-closure.md`.
