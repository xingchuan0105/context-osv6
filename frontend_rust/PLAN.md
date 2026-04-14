# Frontend Rust Implementation Plan

> Project: `context-osv6/frontend_rust`
> Updated: 2026-03-21
> Goal: ship a usable Leptos SSR frontend that matches the Rust frontend PRD and the current `avrag-rs` backend contract.

## 1. Delivery Standard

This plan treats the frontend as complete only when all of the following are true:

- `frontend_rust` compiles cleanly enough for day-to-day development.
- `avrag-api` can link and serve the frontend.
- Core user flows work end to end against real backend routes.
- Route names, DTOs, SSE handling, and auth behavior match the current backend contract.
- Remaining placeholder content is explicitly tracked rather than hidden in “coming soon” UI.

## 2. Current Status

### 2.1 Already completed in this round

- Leptos route modules are wired and exported.
- `avrag-api` now mounts `web_ui::App` instead of a single placeholder page.
- `AuthState` is provided at the app root.
- Basic auth persistence was added so token/user survive client-side navigation.
- Notebook route params are aligned to `:notebook_id`.
- Dashboard notebook links use Leptos client navigation instead of hard refresh links.
- SDK alignment fixes landed for:
  - auth envelope handling
  - admin envelope handling
  - billing envelope handling
  - share route fixes
  - sources route fix
  - SSE endpoint + actual incremental parsing
- `frontend_rust` workspace passes `cargo check`.
- `avrag-api` integration passes `cargo check` from a writable mirror workspace.

### 2.2 Still incomplete

- Home route is not a real redirect yet.
- Document upload is still a stub UI, not a real file ingestion flow.
- Settings page still contains placeholders.
- Share center is only a simplified version of the PRD design.
- Admin pages render data but still use simplified assumptions.
- Chat experience is usable at the integration level, but not yet feature-complete compared to the PRD and v5 baseline.

## 3. Phase Plan

## Phase A: Runtime Foundation
Status: mostly complete

### A1. App shell and routing
Status: complete

- Root app mounts Router/Routes.
- Auth context is provided.
- Main user and admin routes exist.

### A2. API contract alignment
Status: complete for current blockers

- Align auth, share, admin, billing, sources and SSE with backend.
- Keep `web-sdk` as the typed contract layer.

### A3. Navigation and auth continuity
Status: partial

- Persist token/user in browser storage.
- Replace hard refresh links on critical authenticated flows.
- Remaining work:
  - add auth bootstrap on first hydration from storage plus optional `/api/auth/me` refresh
  - convert more internal links from `<a>` to router-aware navigation where appropriate

## Phase B: Core Product Flows
Status: in progress

### B1. Home + auth journey
Priority: high

Goals:
- `/` performs an actual redirect to `/dashboard`
- login/register/reset password flow works cleanly
- invalid auth state redirects to `/login` from protected pages

Acceptance:
- user can register/login/reset password without manual page hacking

### B2. Dashboard list
Priority: high

Goals:
- notebook list, create notebook, card/list toggle
- better empty and loading states
- route to workspace via client-side navigation

Acceptance:
- dashboard behaves like a basic notebook launcher

### B3. Workspace shell
Priority: high

Goals:
- left: sessions / sources / drafts
- center: chat
- right: evidence / trace / session
- real notebook title, not only notebook id

Acceptance:
- workspace is the real operating surface, not just a route skeleton

### B4. Document ingestion flow
Priority: highest remaining product task

Goals:
- implement real file selection
- call `POST /api/v1/notebooks/{id}/documents`
- upload bytes to `upload_url`
- call `POST /api/v1/documents/{id}/complete-upload`
- poll `GET /api/v1/documents/{id}/status`
- refresh source list on success

Acceptance:
- user can upload a real file and watch it reach completed/failed state

### B5. Chat end-to-end
Priority: highest remaining UX task

Goals:
- create/reuse chat session correctly
- stream through `/api/v1/chat`
- handle `trace`, `planner_complete`, `rag_trace`, `rag_sources`, `token`, `citations`, `done`, `error`
- show citations and partial answers
- support `rag`, `search`, `general`

Acceptance:
- user can ask a question and receive real streamed output with citations

## Phase C: Product Completeness
Status: pending

### C1. Evidence panel

Goals:
- citation list
- active citation state
- better detail view
- parsed preview/content handoff from the selected source

### C2. Draft notes

Goals:
- keep local-only positioning explicit
- improve save/load lifecycle
- optional import-to-document flow

### C3. Share center

Goals:
- real share settings
- create/disable link flow
- analytics and access logs against live backend data
- member list and invitation operations

### C4. Shared notebook page

Goals:
- display shared notebook metadata + sources
- allow read-only chat using `source_type=share`
- improve UX for invalid/expired tokens

### C5. Invite flow

Goals:
- accept/decline should reflect backend state
- better success/error states

### C6. Billing page

Goals:
- present current subscription and usage with backend-compatible semantics
- support checkout and portal redirection
- remove fake plan assumptions where possible

### C7. Settings page

Goals:
- replace placeholders for profile/security/notification sections
- wire update profile + change password

## Phase D: Admin Surface
Status: pending

### D1. Organizations list

- show real org rows
- block/unblock mutation with optimistic refresh

### D2. Organization detail

- real org information
- usage summary from backend, not mock values
- clear blocked state transitions

### D3. Users / Usage / Health

- remove current “first org only” shortcut once org selection/filtering is implemented
- align field semantics with backend output

## Phase E: PRD Polish
Status: pending

### E1. Route and information architecture polish

- add missing user-facing routes from the PRD that are already supported
- decide whether `/settings` remains a page or becomes an in-workspace drawer later

### E2. Loading, empty, and degrade UX

- proper banners for degraded chat states
- better trace/evidence presentation
- better upload and session feedback

### E3. Styling consistency

- consolidate repeated utility classes
- move toward a shared visual system

## 4. Execution Order

Current execution order from here:

1. Home redirect and auth bootstrap polish
2. Real document upload flow
3. Session-aware chat flow
4. Workspace title + source/session refresh behavior
5. Share center correctness and UX
6. Settings/profile/security completion
7. Admin data fidelity improvements

## 5. Verification Checklist

After each major step:

- `cargo check --manifest-path frontend_rust/Cargo.toml`
- `cargo check -p avrag-api` from a writable mirror when needed

Before calling the frontend “alpha usable”:

- login works
- notebook list works
- entering a workspace works
- upload works
- chat works
- citation panel works
- share link works
- billing page loads
- admin pages render backend data without contract errors

## 6. Definition of Done for Next Milestone

The next meaningful milestone is:

`Alpha: authenticated user can create a notebook, upload a document, wait for ingestion, chat against it, and inspect citations in the workspace UI.`
