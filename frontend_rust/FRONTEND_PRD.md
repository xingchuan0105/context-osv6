# Frontend Rust PRD

> Project: `context-osv6/frontend_rust`
> Updated: 2026-04-15
> Status: active draft
> Scope: Rust frontend product definition + completion plan
> Source inputs:
> - `frontend/docs/rust-frontend-design.md`
> - `frontend_rust/PLAN.md`
> - `docs/superpowers/specs/2026-04-02-context-osv6-stable-layered-monolith-design.md`
> - current `frontend_rust/crates/web-ui` implementation
> - 2026-03-29 frontend review findings
> - 2026-03-30 remediation planning and PRD update
> - 2026-04-15 confirmed dashboard/workspace product decisions
> - Figma Make dashboard reference:
>   `https://www.figma.com/make/zBrhQ0r7sRibkeZMZuARQ3/NotebookLM-Style-Dashboard?t=BNzfhxmILnLsWz4L-1`

## 1. Document Purpose

This document is the active frontend product and delivery spec for `frontend_rust`.

It exists to replace the current split between:

- high-level design intent
- outdated implementation notes
- scattered gap-analysis documents

This doc should be used as the frontend product source of truth when the code, older notes, and partial plans disagree.

Architecture authority is not owned by this PRD. If this PRD conflicts with the approved stable layered monolith spec, the architecture spec wins and this document must be updated to match it.

## 2. Product Positioning

`frontend_rust` is the official web frontend for `context-osv6`.

It is not a Rust rewrite of the old Next.js pages for its own sake. It is a product surface that must:

- expose the existing Rust backend capabilities completely
- keep the v5 product strengths that users already understand
- upgrade the workspace into a session-first, evidence-aware research UI
- make sharing, admin, settings, and API access first-class pages
- provide a stronger visual system than the current page-by-page Tailwind styling

### 2.1 Product Terminology Freeze (2026-04-15)

User-facing product language now uses `Workspace` as the single canonical term.

- `Workspace` is the product term used in titles, tabs, buttons, empty states, menus, and PRD language
- `Notebook` remains an internal compatibility term in routes, DTO names, backend handlers, and legacy code until transport-layer renaming is scheduled separately
- whenever the code and UX copy disagree, the UX copy should prefer `Workspace`

## 3. Current Reality Snapshot

### 3.1 Already working

- route shell, auth guard, and core user/admin routes exist
- login/register/reset-password routes exist
- notebook list and notebook creation exist
- workspace route exists and can chat against live backend SSE
- upload flow is real: create upload, PUT bytes, complete upload
- document status polling exists
- citation lookup and source focus are implemented at a basic level
- share center supports settings, analytics, access logs, member invite/remove
- API access page exists
- search page exists
- admin pages render real data in several domains
- `cargo check --manifest-path frontend_rust/Cargo.toml -p frontend-web-ui` passes

### 3.2 Still materially incomplete

- workspace information architecture does not match the intended three-column product model
- session lifecycle is incomplete: no new-session, rename, pin, or session management UX
- dashboard list is a launcher, not a mature notebook home
- document/source viewer is shallow for deep documents
- settings IA is incomplete
- visual system is thin and inconsistently applied
- user-facing copy is still predominantly English; localization is not product-ready
- public share page is serviceable but not product-complete
- admin is usable but not yet trustworthy or polished enough
- test coverage is far below release-grade expectations

## 4. Product Goals

### 4.1 Primary user goal

An authenticated user can:

1. create or open a workspace
2. upload or connect sources
3. wait for ingestion with clear status feedback
4. run RAG, search, or general chat
5. inspect evidence and jump to source context
6. keep synced research notes while working
7. share the workspace safely

### 4.2 Secondary user goals

- invited collaborators can accept or decline workspace access cleanly
- workspace owners can manage API access and share settings without leaving the product
- org admins can inspect organization, usage, health, and policy surfaces without dead links or fake summaries

### 4.3 Non-goals for this milestone

- frontend-owned transport protocol or DTO layer separate from the shared `contracts` crate
- frontend-only decorative redesign that ignores product IA and state clarity
- parallel desktop/mobile app ambitions
- real-time multi-user collaborative editing of notes

## 5. Design Principles

- Session-first: chat is not a single transcript, it is a notebook-scoped set of sessions.
- Assets stay visible: sources and notes must remain persistent context, not hidden behind mode switches.
- Evidence is actionable: citations should take the user to usable source context, not just show raw metadata.
- Notes sync to the user's account: notes are per-notebook scratchpads that auto-sync via the preferences API, with visible sync state.
- Visual system serves product comprehension: styling should reveal structure, state, and confidence.
- Language is part of usability: the product must not ship as an English-only interface for Chinese users.
- Router-aware navigation by default: avoid full page reloads for internal product flows.
- Degraded output must be visible: degraded or guarded responses cannot look the same as normal answers.
- Contract discipline is mandatory: frontend transport types and chat event semantics come from the shared `contracts` crate through `web-sdk`, not from frontend-local DTO mirrors.

### 5.1 UI Drift Prevention Baseline (2026-04-15)

To keep implementation aligned with Figma and prevent style drift, frontend execution follows this layered baseline:

1. **Design token source of truth**
   - canonical file: `crates/web-ui/src/styles/design_tokens.css`
   - runtime token carrier: `crates/web-ui/src/index.css`
   - sync command: `python scripts/sync_design_tokens.py`
2. **Official styling stack**
   - official stack: Plain CSS + Stylance CSS Modules
   - component and route shell visuals belong in `.module.css` files colocated with Rust components
   - shared resets, tokens, typography, and legacy compatibility stay in `index.css`
3. **Legacy Tailwind policy**
   - existing Tailwind utilities are migration debt, not the target architecture
   - new UI work must not introduce fresh Tailwind utility usage
   - Tailwind stays only as a temporary compatibility layer until legacy routes are migrated
4. **Layout vs state boundary**
   - layout/container components should remain signal-free wherever practical
   - reactive state belongs in route logic and leaf components
5. **Guardrails as executable checks**
   - `python scripts/ui_drift_guard.py` (report mode)
   - `python scripts/ui_drift_guard.py --strict` (enforcement mode)
6. **Agent conventions**
   - project rules are documented in `frontend_rust/AGENTS.md`
   - all AI-generated UI code must comply with token, CSS module, and component boundary constraints

## 6. Information Architecture

### 6.1 Main user routes

- `/`
  - real redirect to `/dashboard`
- `/login`
- `/register`
- `/reset-password`
- `/reset-password/verify`
- `/reset-password/confirm`
- `/dashboard`
  - workspace home
- `/dashboard/:workspace_id`
  - main workspace
- `/dashboard/:workspace_id/analyze`
  - workspace share analytics page
- `/dashboard/:workspace_id/share`
  - share center
- `/dashboard/:workspace_id/share/analytics`
- `/dashboard/:workspace_id/share/access-logs`
- `/dashboard/:workspace_id/api-access`
- `/shared/kb/:token`
  - public share page
- `/invite/:workspace_id/:member_id`
- `/settings`

### 6.2 Admin routes

- `/admin`
  - redirect to `/admin/organizations`
- `/admin/organizations`
- `/admin/organizations/:org_id`
- `/admin/users`
- `/admin/usage`
- `/admin/health`

### 6.3 Future-but-not-blocking admin routes

These may exist as visible roadmap entries later, but should not be treated as milestone blockers:

- `/admin/billing`
- `/admin/rag-health`
- `/admin/feature-flags`
- `/admin/system/workers`
- `/admin/system/degradation`
- `/admin/audit-logs`

## 7. Detailed Product Requirements

### 7.1 Home and auth

Requirements:

- `/` must be a true redirect, not a browser-only placeholder screen
- protected routes must redirect unauthenticated users to `/login`
- authenticated users visiting `/login` or `/register` should be redirected to `/dashboard`
- auth bootstrap should revalidate stored auth on hydration

Acceptance:

- reload, logout, and auth-expired behavior remain predictable across navigation

### 7.2 Dashboard home

The dashboard is the workspace home. It is not a launcher-only list and it is not a separate search page.

Dashboard reference baseline (2026-04-13):

- use the Figma Make sample above as the default dashboard interaction reference
- this is a product/interaction baseline, not a strict pixel-perfect copy requirement
- when conflicts happen:
  - product IA and backend contract correctness win first
  - visual hierarchy and interaction density should remain aligned with the sample

Requirements:

- show workspace scope tabs:
  - `全部`
  - `我的 Workspace`
  - `我的收藏`
- support card and list view
- each workspace card/row should surface:
  - title
  - description
  - last active or updated time
  - source count
  - quick status summary
- provide strong empty states
- preserve fast create-workspace flow

Header requirements:

- the left brand is the unified `Context-OS` logo, not `NotebookLM`
- the dashboard logo is brand-only and must not navigate anywhere when clicked
- the right side contains:
  - lightweight settings entry
  - lightweight account/avatar entry

Toolbar requirements:

- left side:
  - scope tabs for `全部 / 我的 Workspace / 我的收藏`
- right side:
  - search trigger
  - grid/list toggle
  - sort control
  - primary create workspace action

Search requirements:

- dashboard search must not navigate to `/dashboard/search`
- dashboard search opens a minimal global search modal/overlay
- first milestone search scope:
  - workspace title
  - workspace description
- first milestone search output:
  - compact result list
  - click result to enter the target workspace
- non-goals for this search surface:
  - no extra filter chips
  - no analytics controls
  - no AI summary
  - no full-page search center

Favorite requirements:

- `我的收藏` is a first-class tab, not a secondary badge
- add/remove favorite remains a row/card contextual-menu action
- favorite state should stay discoverable without requiring a separate page

Interaction requirements aligned to the Figma sample:

- top control bar should include:
  - left side: workspace scope tabs
  - right side: search trigger, grid/list toggle, sort control, primary create notebook action
- grid/list must use the same underlying dataset and sorting state
- grid mode should support:
  - first-slot "new workspace" quick-create card with a clear affordance
  - workspace cards with concise metadata and contextual menu actions
- list mode should support compact table-like rows with at least:
  - title
  - source count
  - date
  - role/ownership indicator
  - row-level contextual menu trigger
- destructive/secondary workspace actions should be hidden by default and surfaced by contextual menu (not persistent inline button clusters)

Button/function requirements:

- `新建`
  - opens create workspace modal
- row/card click
  - enters the selected workspace
- row/card contextual menu
  - favorite / unfavorite
  - rename
  - delete
- empty-state CTA
  - label: `创建第一个 Workspace`
  - behavior: same create-workspace flow as the primary `新建` button

Empty-state requirements:

- if the active scope contains zero workspaces, show a strong empty state with:
  - a short explanation
  - one primary CTA: `创建第一个 Workspace`
- the empty-state CTA is not a separate feature; it is a duplicate entry to the same create action

Copy requirements:

- all dashboard product copy must use `Workspace`
- user-facing `Notebook` wording should be removed from dashboard copy

Acceptance:

- users can understand workspace state before opening one
- favorited workspaces are discoverable without search
- dashboard search opens in-place and closes back into the same dashboard context
- the empty-state CTA and the primary create CTA are behaviorally identical
- dashboard interactions remain visually and behaviorally consistent with the Figma reference baseline

### 7.3 Workspace shell

Target desktop layout:

- left rail: sessions
- center stage: current chat
- right rail: sources and notes stacked vertically

Secondary surfaces:

- evidence and trace are auxiliary views
- they may appear as:
  - docked sub-panels inside the right rail
  - collapsible drawers
  - mobile fallback modals

Explicit layout requirements:

- sources and notes must stay visible without tab-switching away from sessions
- right rail supports independent scrolling areas for sources and notes
- sources and notes support resize handle or at least adjustable split
- workspace title uses the real workspace title
- mobile view collapses side rails into drawers/sheets

Top-bar requirements:

- left side:
  - `Context-OS` logo
  - current workspace title
- right side action cluster:
  - `New Workspace`
  - `Analyze`
  - `Share`
  - `API`
  - gear menu
  - avatar menu

Top-bar behavior requirements:

- `Share`
  - enters `/dashboard/:workspace_id/share`
  - it is not a copy-link-only quick action
- `Analyze`
  - enters `/dashboard/:workspace_id/analyze`
  - this page is a simple share analytics surface only
  - it does not open search and does not show token analytics
- gear menu
  - second-level menu with:
    - `主题配色`
    - `语言设置`
- avatar menu
  - second-level menu with:
    - account information
    - user tier badge (`Free` / `VIP`)
    - sign out

Workspace title/default naming requirements:

- every newly created workspace must get a deterministic default title if the user does not enter one manually
- default naming must be localized:
  - `zh-CN`: `未命名 Workspace YYYY-MM-DD`
  - `en`: `Untitled Workspace YYYY-MM-DD`
- if the same-day default title already exists for that user, append a stable suffix such as `·2`, `·3`
- once the user manually renames the workspace, the default naming mechanism must stop applying

Analyze page requirements:

- analyze is a workspace-local share analytics page, not a global search or cost page
- required sections:
  - share status
  - total views
  - total unique visitors
  - views by day
  - recent access logs
- if sharing has not been enabled yet:
  - show an empty state
  - primary CTA: `前往 Share`
  - CTA enters the share center for the same workspace

Input/composer requirements:

- the main input must support:
  - `Enter` to send
  - `Shift + Enter` to insert a newline
- the send button and keyboard shortcut must follow the same submission rules
- the default conversation mode is `RAG`
- if the user switches mode manually, future turns should keep the last chosen mode until changed again
- mode memory is per-user UI behavior and should persist across reloads without requiring backend telemetry changes

Session rail requirements:

- left rail remains the thread navigator
- primary action label becomes `New Thread`
- search box filters the thread list in place
- row-level contextual menu continues to own:
  - pin / unpin
  - rename
  - delete

Global preference requirements inside workspace:

- the workspace shell must respect the global language setting (`zh-CN` / `en`)
- the workspace shell must respect the global theme setting (`light` / `dark`)
- language and theme must be switchable from the gear menu, not only from the full settings page

Acceptance:

- workspace feels like a single operating surface instead of three unrelated panes
- workspace top-bar actions have clear destinations and do not overload one control with multiple meanings
- analyze remains intentionally narrow: share analytics only
- composer keyboard behavior matches mainstream chat expectations
- the workspace title is never blank after creation

2026-04-10 freeze decision:

- keep the current three-column architecture as the baseline layout
- do not redesign into single-column or tab-switched primary layout on desktop
- future changes must optimize usability inside the existing three-column structure, not replace it

### 7.4 Session management

Requirements:

- list sessions for the notebook
- open an existing session
- create a new session without destroying the current one
- support rename
- support pin or equivalent prioritization
- preserve session isolation
- refresh session summary after completed chat turns
- left session rail visual/interaction style must follow a Perplexity-like pattern:
  - top search box for filtering sessions
  - clear "new question/new chat" primary entry
  - compact one-line session rows with truncation
  - no inline dense operation clusters in each row
  - row-level actions (rename/pin/delete) moved into contextual menu (e.g. three-dot menu)
  - prioritize readability and scan speed over metadata density

Explicit anti-patterns (must avoid):

- do not place date + pin + rename + delete inline in every row
- do not allow operation controls to squeeze or overlap session titles
- do not require users to parse multi-line metadata before opening a session

Acceptance:

- a user can keep multiple research threads in one notebook without confusion
- session list remains readable at narrow desktop widths and mobile drawer widths
- session title collision/overlap does not occur under long-title stress tests

### 7.5 Document ingestion and sources

Requirements:

- upload file flow remains real and stable
- add URL source flow remains available
- source status polling continues until terminal state
- source status chips use shared semantic states
- failed states offer retry or reindex affordances
- source selection clearly communicates current doc scope

Source viewer requirements:

- open source detail inline in the assets rail
- support parsed preview pagination or page-window loading
- support page-aware citation jump
- visibly highlight the referenced chunk or preview block
- preserve scroll target after citation lookup

Acceptance:

- large documents remain navigable
- citation jump does not silently fail because the target is outside the loaded preview window

### 7.6 Chat and evidence

Requirements:

- support `rag`, `search`, and `general`
- keep the existing SSE state machine:
  - `idle -> submitting -> streaming -> done | error`
- handle the official chat event contract only:
  - `start`
  - `trace`
  - `token`
  - `citations`
  - `done`
  - `error`
- treat planner- or retrieval-specific diagnostics as `trace` payload detail or terminal response detail, not separate event names
- show partial streaming answer
- finalize answer blocks and citations correctly
- show degraded-answer warning when `degrade_trace` is non-empty
- treat guard hits as visible product state, not hidden debug state

Evidence requirements:

- citation chips inside chat stay lightweight
- clicking a citation selects evidence, focuses source, and reveals supporting content
- evidence list must show:
  - source title
  - preview or content excerpt
  - image when available
  - score/layer metadata

Acceptance:

- a user can answer "why should I trust this response?" directly from the UI

### 7.7 Research notes (account-synced)

Research notes are per-notebook scratchpads that automatically sync to the user's account via the preferences API. The current implementation stores notes as `WorkspaceDraftPreference { notebook_id, notes }` inside `UserPreferences.dashboard.workspace_drafts`, with a debounced auto-save loop and a four-state sync indicator.

Requirements:

- notes auto-sync to the current account with visible sync state (Idle / Syncing / Synced / Error)
- support per-notebook persistence: each notebook has its own notes context
- support create/edit lifecycle
- support export-to-markdown
- support note list management when multiple notes per notebook are needed (future)
- integrate note creation with chat affordances later (e.g., "save to notes" action)

Current implementation status:

- single textarea per notebook, auto-synced via preferences API with 700ms debounce
- sync state badge renders correctly in zh-CN and en
- notes persist across sessions on the same account
- no multi-note support, no export, no markdown formatting

Acceptance:

- notes are useful research companions with transparent sync state
- users can export notes to markdown
- notes do not silently lose data on navigation or sync failure

### 7.8 Search

Requirements:

- the primary dashboard search surface is an in-place quick-open overlay, not a dedicated product page
- first milestone scope is keyword search over workspaces only
- result rows must be directly navigable into the selected workspace
- search should stay minimal and operational:
  - one input
  - one result list
  - no extra function buttons
  - no AI answer block
- if a deeper search page is kept internally for debugging or future expansion, it must not become the default dashboard search flow

Acceptance:

- search feels like a product surface, not a backend debug tool
- dashboard search never forces the user out of the dashboard context just to locate a workspace

### 7.9 Share center

Requirements:

- show current access level clearly
- show effective active share token
- support create/disable share
- support expiration
- support member invite/remove
- support analytics and access logs
- explain permission model and public/link/private implications

Acceptance:

- owners understand both current share state and its impact

### 7.10 Public share page

Requirements:

- show workspace title, description, permission, expiration, and source summary
- distinguish `partial` vs `full` share behavior in the UI
- show clearer invalid/expired states
- support share-scoped chat if backend permits
- support favorite or save behavior if product keeps that concept

Acceptance:

- public share feels intentional and trustworthy, not like a reduced internal page

### 7.11 Invite page

Requirements:

- clear accept/decline path
- reflect actual backend state
- show workspace context and final outcome clearly

Acceptance:

- invited users understand what they are accepting

### 7.12 API access

Requirements:

- create/revoke keys
- display permissions, rate limit, expiration, last used
- show integration examples
- explain capability boundaries clearly

Acceptance:

- workspace owners can self-serve API usage safely

### 7.13 Settings, billing, notifications

Settings IA:

- Appearance
  - theme
  - language
- Account
  - profile
  - password change
  - password reset entry
- Billing
  - current plan
  - usage
  - upgrade
  - billing portal
- Security
  - current session/device summary
  - logout
- Notifications
  - list
  - mark as read

Acceptance:

- settings is a coherent center, not a bag of unrelated forms

### 7.14 Localization and copy

Requirements:

- the frontend must support at least `zh-CN` and `en`
- `zh-CN` should be the default UI language for the current target user base
- core product routes must not remain English-only:
  - auth
  - dashboard
  - workspace
  - share
  - settings
  - invite
  - API access
  - admin
- all user-facing labels, buttons, empty states, errors, warnings, and status copy must come from a central i18n layer
- avoid hard-coded inline English strings in route/component files
- preserve backend payload values when they are machine identifiers, but map them to user-facing localized labels in the UI
- localization must cover degraded-state warnings, guard messages, and operational states, not just static page chrome

Copy rules:

- Chinese copy should be concise, product-facing, and consistent
- mixed Chinese/English should be used only when the English term is a product/API term that users need
- raw backend wording should not leak directly into UI copy without normalization

Acceptance:

- a Chinese-speaking user can complete the main flows without encountering large English-only surfaces
- the frontend can switch language without editing component code

### 7.15 Admin

Organizations:

- list real organizations
- show plan, users, notebooks, blocked state
- block/unblock with visible success/failure feedback

Organization detail:

- show real organization data
- show real usage summary
- show real subscription or billing state where supported
- avoid placeholder or fake metrics

Users:

- allow org selection
- allow email search or filter
- show role, created time, last active

Usage:

- show platform or org usage
- support time window selection later if backend supports it

Health:

- show health and ready-style status
- show key degradation or failure summaries when available

Acceptance:

- admin surfaces are safe to trust for operational decisions

## 8. Visual System PRD

### 8.1 Current problem statement

The current frontend has styles, but not a fully operational design system.

Main issues:

- token layer is too thin
- many pages bypass tokens with hard-coded gray/blue classes
- dark mode cannot apply consistently
- UI copy is still mostly English, which weakens perceived polish and usability
- typography hierarchy is weak
- cards/tables/forms/tabs/badges are page-local implementations
- motion exists in config but is barely used
- dashboard, settings, share, and admin do not look like one product family

### 8.2 Visual system target

The frontend should adopt a product-grade visual system with:

- semantic tokens
  - background
  - surface
  - elevated surface
  - border
  - focus
  - success
  - warning
  - danger
  - info
  - chart colors
- typography scale
  - page title
  - section title
  - body
  - metadata
  - mono/data
- shared primitives
  - page header
  - section card
  - status badge
  - empty state
  - skeleton state
  - data table
  - tabs
  - form field
  - modal/sheet
- consistent icon style
- explicit motion rules
  - page enter
  - panel expand/collapse
  - list stagger
  - optimistic state feedback
- bilingual UI support
  - Chinese-first default
  - English fallback and full-route coverage for core user pages
- theme switching
  - light
  - dark
  - system support where already implemented
- unified brand asset
  - one global `Context-OS` SVG logo used across dashboard, workspace, settings, and public shell entries

### 8.3 Visual direction

Desired tone:

- research workstation
- calm, high-signal, dense but not noisy
- more editorial and product-grade than generic admin dashboard
- confident on desktop, adaptive on mobile
- bilingual by design rather than translated as an afterthought

Avoid:

- template-like gray-white flatness
- random mixes of tokenized surfaces and hard-coded legacy colors
- decorative motion without information value
- inconsistent brand marks between dashboard and workspace

Brand asset requirements:

- `Context-OS` uses one shared SVG logo
- visual form:
  - black background
  - white line work
  - concept: `second brain + AI`
- the logo should feel like a compact product mark, not an illustration
- dashboard uses the logo as a static brand marker
- workspace uses the same logo family for product continuity

### 8.4 Color and typography guardrail (2026-04-10)

This project now explicitly adopts a Perplexity-style gray visual scheme for the core product surfaces:

- base palette: warm off-white / neutral gray / charcoal
- accent behavior: restrained, low-saturation, grayscale-first controls
- semantic UI states should default to neutral grayscale hierarchy in normal operation
- avoid neon or highly saturated accent treatments

Color constraints:

- primary controls, active tabs, active session rows, and key workspace accents must use the gray scheme token set
- user-facing core routes must not introduce arbitrary accent colors that break the gray scheme
- if a non-gray color is used, it must be justified by explicit product semantics (e.g. danger/error) rather than visual decoration

Typography constraints:

- preserve Perplexity-like clarity:
  - high legibility sans-serif stack
  - compact but breathable spacing
  - clear title/body/metadata hierarchy
- no oversized decorative heading scale inside operational panes
- dashboard and workspace action labels must remain visually compact enough to avoid toolbar sprawl

### 8.5 Sidebar reference pattern (Perplexity-aligned)

The left rail should feel like a lightweight conversation navigator:

- persistent search input at top
- "new question/new chat" action directly below search
- history section label
- compact list rows with:
  - single-line title
  - active-row contrast state
  - hover state
  - optional contextual menu trigger

Non-goals for sidebar:

- rich card-style rows with heavy badges and dense metadata
- inline multi-action button groups in each row
- visual competition with center chat panel

### 8.6 Dashboard and Workspace Visual Freeze (2026-04-15)

Dashboard:

- reference tone remains NotebookLM-style dashboard density, but branded as `Context-OS`
- primary composition:
  - quiet header
  - centered main content width
  - strong whitespace around the list/grid surface
- toolbar controls should feel compact, rounded, and grayscale-first
- search uses a minimal overlay with no extra chrome

Workspace:

- reference tone remains Perplexity-style three-column workspace
- center chat stage should stretch to fill the viewport vertically
- the main composer sits at the visual bottom of the center pane
- left rail remains compact and utility-like
- right rail remains dense but readable, with sources above notes
- top bar should read as a lightweight command row rather than a marketing header

## 9. Engineering and State Model

Domain contexts:

- `AuthState`
- `DashboardState`
- `WorkspaceState`
- `ChatState`
- `ShareState`
- `BillingState`
- `AdminState`
- `NotesState` (synced research notes per notebook)
- `UiPrefsState`

Rules:

- route-level context first
- minimal persistent local state
- no streaming state in local storage
- centralize status formatting and semantic style helpers
- use router-aware navigation for internal product routes

## 10. Integrated 6-Step Plan

This document uses one integrated 6-step execution plan.

Each step is a delivery slice, not a separate planning system.

### Step 1: Foundation layer

Goal:

- establish the target before more page work is layered on top

Tasks:

- adopt this document as the active frontend spec
- define semantic tokens in `design_tokens.css` and consume them through Plain CSS + Stylance CSS Modules
- define i18n foundations:
  - locale state
  - translation dictionary shape
  - message access helpers
  - fallback behavior
- add shared primitives:
  - page header
  - section card
  - status badge
  - tabs
  - form field
  - skeleton
  - empty state
- remove the most obvious hard-coded color divergence
- enforce Perplexity-style gray scheme tokens in core user-facing flows
- add design-token lint checklist to PR review template (manual gate for now)

Exit criteria:

- new page work can use shared primitives instead of ad hoc classes

### Step 2: Workspace architecture correction

Goal:

- align the product shell with the intended research workflow

Tasks:

- refactor workspace to:
  - left sessions rail
  - center chat stage
  - right assets rail
- preserve the approved three-column structure (do not replace layout paradigm)
- move evidence/trace to auxiliary panels instead of main right-rail tabs
- add notes/source stacked layout
- add resize or split behavior
- replace fixed-width desktop-only assumptions with responsive shell rules
- redesign left session rail to Perplexity-aligned compact list pattern

Exit criteria:

- workspace layout matches the intended IA and remains usable on smaller screens

### Step 3: Session, source viewer, and chat completion

Goal:

- finish the core operating loop

Tasks:

- add new-session flow
- add session rename and prioritization
- refresh session summaries after completed responses
- add parsed preview pagination or page-window loading
- strengthen citation jump and highlight reliability
- improve degrade and guard visualization

Exit criteria:

- user can manage multiple sessions and navigate deep source citations reliably

### Step 4: Product surface completion

Goal:

- upgrade the surrounding product pages from baseline to product-grade

Tasks:

- dashboard:
  - add favorites-first scope tab and empty-state CTA
  - enrich notebook cards
  - align dashboard controls and view-switch interactions with the Figma Make reference baseline
  - replace full-page search with minimal in-place workspace search overlay
- localization:
  - move core user-facing copy into dictionaries
  - deliver `zh-CN` coverage for main routes
  - keep `en` as supported secondary locale
- search:
  - treat dashboard search as quick-open, not as a separate product page
- share center:
  - improve permission explanation and token state UX
- workspace analyze:
  - ship simple share-analytics page only
- public share:
  - add permission-aware presentation
- settings:
  - add appearance section
  - complete security/account IA
- invite:
  - polish success/error outcomes

Exit criteria:

- main user routes feel cohesive and complete

### Step 5: Admin trustworthiness

Goal:

- make admin surfaces operationally credible

Tasks:

- remove first-org shortcut behavior
- add explicit filters
- refresh UI after block/unblock and admin mutations
- replace any remaining fake summaries
- tighten health/usage presentation

Exit criteria:

- admin pages are safe for real operator use

### Step 6: Hardening and release gates

Goal:

- turn a working frontend into a releasable frontend

Tasks:

- add frontend tests for:
  - SSE parser
  - chat state machine
  - document poller
  - share mapping
  - local drafts persistence (research notes sync)
- add route smoke coverage
- verify SSR/hydration and internal navigation behavior
- remove remaining raw `<a>` reloads on internal authenticated flows
- verify dark mode, keyboard focus, and mobile shells

Exit criteria:

- frontend has confidence gates beyond manual clicking

## 11. Prioritized Backlog

Priority 0:

- visual token/primitives foundation
- i18n foundation and locale defaults
- workspace shell correction

Priority 1:

- session management
- source viewer deep-link reliability
- Chinese-first UI localization for core routes
- settings completion
- dashboard workspace home upgrade

Priority 2:

- public share productization
- search polish
- admin trust polish

Priority 3:

- deeper motion and visual polish
- future admin roadmap routes

## 12. Verification Matrix

The frontend is not ready until the following are true:

- real redirect from `/`
- login/register/reset-password all work
- dashboard workspace home works
- workspace create works
- workspace opens without contract errors
- upload and URL-source flows work
- source polling and reindex feedback work
- new chat session and existing session flows work
- session rail follows compact Perplexity-aligned interaction pattern
- session rows have no title/action overlap under long-title and narrow-width scenarios
- dashboard has `全部 / 我的 Workspace / 我的收藏` tabs
- dashboard in-place search modal opens, filters, and navigates correctly
- dashboard empty-state CTA creates a workspace through the same flow as the primary create action
- RAG citations can be opened and source-focused
- workspace top-bar `Share` enters share center
- workspace top-bar `Analyze` enters share analytics page or its empty state
- workspace gear menu can switch theme and language
- workspace avatar menu exposes profile, tier badge, and logout
- workspace composer supports `Enter` send and `Shift + Enter` newline
- workspace conversation mode defaults to `RAG` and persists the user's last manual choice
- newly created workspaces receive a localized default title if the user does not name them
- share link create/disable works
- share analytics/access logs load
- API key create/revoke works
- settings and billing load with real semantics
- notifications can be marked read
- admin orgs/users/usage/health load against real backend
- core product routes are not English-only for `zh-CN` users
- no critical route depends on full page reload for normal internal navigation
- core user-facing palette in workspace/dashboard/settings follows the Perplexity-style gray scheme

## 15. Design Drift Prevention (New)

To prevent future implementation drift, every frontend UX PR touching workspace/dashboard/settings must include:

1. A "PRD mapping" note:
   - list exactly which FRONTEND_PRD requirements are being implemented or changed
2. A visual impact summary:
   - what changed in session rail, color tokens, and typography hierarchy
3. Screenshot evidence:
   - desktop workspace full page
   - mobile workspace drawer state
   - for dashboard-related PRs: desktop dashboard (grid + list) and top controls state
4. Drift checklist confirmation:
   - three-column layout preserved
   - session rail compact pattern preserved
   - Perplexity-style gray scheme preserved
   - dashboard interaction model remains aligned with the Figma Make baseline

If any item fails, PR is not release-ready.

## 13. Definition of Done

The frontend milestone is complete when:

- the workspace matches the intended information architecture
- the visual system is shared and consistent across main routes
- localization is built into the UI layer instead of patched page by page
- core research flows are reliable and test-backed
- admin and share surfaces are credible
- release verification is repeatable, not purely manual

## 14. Execution Note

The single overall plan is the 6 steps in Section 10.

Execution intent:

1. Step 1 establishes visual and language foundations.
2. Step 2 fixes the workspace shell before more feature polish is layered on top.
3. Step 3 completes the core research loop.
4. Step 4 finishes the main user-facing product surfaces and ships Chinese-first coverage across them.
5. Step 5 raises admin credibility.
6. Step 6 turns the frontend into a releaseable surface with tests and gates.
