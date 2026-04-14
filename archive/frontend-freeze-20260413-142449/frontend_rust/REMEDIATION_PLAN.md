# Frontend Remediation Plan

> Generated: 2026-03-30
> Based on: 9 findings from code/PRD review
> Scope: `frontend_rust` crate — product gaps + engineering hardening

## Overview

Nine findings from the static code/PRD review are grouped into **three work streams** executed in order. Each stream has concrete deliverables, acceptance criteria, and file-level references.

| Stream | Focus | Findings covered | Estimated effort |
|--------|-------|-----------------|-----------------|
| A | Product surface gaps | #1, #2, #3, #4 | High |
| B | Contract & semantics closure | #5, #6 | Medium |
| C | Engineering hardening | #7, #8, #9 | Medium-High |

**Execution order**: A → B → C (product surfaces first, then contracts, then hardening).

---

## Stream A: Product Surface Gaps

### A1. Dashboard notebook home upgrade (Finding #1)

**Gap**: Dashboard cards only show title, description, updated_at. PRD requires document_count, status_summary, and shared/favorited discoverability. Backend `Notebook` DTO lacks these fields.

**Changes**:

#### Backend (avrag-rs)

1. **Extend `NotebookResponse` DTO** — `avrag-rs/crates/transport-http/src/lib.rs`
   - Add `document_count: i64` to the notebook list handler response
   - Add `status_summary: HashMap<String, i64>` (e.g., `{"ready": 3, "processing": 1, "failed": 0}`)
   - Add `shared: bool` flag
   - These are computed fields — join/aggregate on `documents` table during notebook list fetch

2. **Extend `notebook_list_handler`** — `avrag-rs/crates/transport-http/src/lib.rs`
   - Add LEFT JOIN or subquery for document counts and status aggregation
   - Add share existence check for `shared` flag

#### Frontend (web-sdk)

3. **Extend `Notebook` DTO** — `frontend_rust/crates/web-sdk/src/lib.rs:192`
   ```rust
   pub struct Notebook {
       // ... existing fields ...
       pub document_count: i64,
       pub status_summary: HashMap<String, i64>,
       pub shared: bool,
   }
   ```

#### Frontend (web-ui)

4. **Update dashboard card** — `frontend_rust/crates/web-ui/src/routes/dashboard.rs:530-643`
   - Add document count badge on card
   - Add status summary chips (e.g., "3 sources ready")
   - Show shared indicator icon
   - Both card and list views

5. **Add "Shared Notebooks" section** — `frontend_rust/crates/web-ui/src/routes/dashboard.rs`
   - Filter notebooks where `shared == true` into a separate section
   - Currently only favorites section exists; add a "Shared with me" section

**Acceptance**:
- Notebook card shows title, description, updated_at, document_count, status_summary, shared badge
- "Shared Notebooks" section is visible and populated when relevant
- Empty states render correctly

---

### A2. Workspace shell responsive layout (Finding #2)

**Gap**: Left rail `w-72` and right rail `w-[27rem]` are fixed-width. No resize handle, no mobile drawer/sheet fallback.

**Changes**:

1. **Replace fixed widths with flex-based resizable layout** — `frontend_rust/crates/web-ui/src/routes/dashboard.rs:1464, 1723`
   - Use CSS `resize` or a JS drag-handle between center and right rail
   - Left rail: collapsible (current behavior) with min/max bounds
   - Right rail: resizable split between sources and notes panes

2. **Add mobile breakpoints** — same file
   - Below `md` breakpoint: hide left and right rails
   - Show hamburger buttons to open rails as overlay sheets
   - Use Leptos `<Show>` with media query signals

3. **Sources/Notes split resize** — `frontend_rust/crates/web-ui/src/routes/dashboard.rs:1886`
   - Add a drag divider between sources pane and notes pane in the right rail
   - Store split ratio in `UiPrefsState`

**Acceptance**:
- Desktop: rails are resizable, not fixed-width
- Mobile (<768px): rails collapse into overlay sheets
- Sources/notes split is adjustable

---

### A3. Search productization (Finding #3)

**Gap**: Search results filter local lists with `contains()` instead of backend search. Session hits navigate to `/dashboard/{notebook_id}` but don't open the target session. Visual style uses raw `text-gray-*` / `bg-white` instead of tokens.

**Changes**:

#### Backend search enhancement

1. **Add global search endpoint** — `avrag-rs/crates/transport-http/src/lib.rs`
   - `GET /api/v1/search?q={query}&scope=notebooks,sessions,documents`
   - Returns structured results with notebook_id + session_id for navigation

#### Frontend search rewrite

2. **Replace client-side filtering with API call** — `frontend_rust/crates/web-ui/src/routes/search.rs:125-151`
   - Remove `filtered_notebooks` and `filtered_sessions` local filters
   - Call backend search endpoint
   - Render backend-returned results

3. **Fix session navigation** — `frontend_rust/crates/web-ui/src/routes/search.rs:271`
   - Session results should link to `/dashboard/{notebook_id}?session={session_id}`
   - Workspace must read `?session=` query param and activate that session on mount

4. **Token-based styling** — `frontend_rust/crates/web-ui/src/routes/search.rs:153-300`
   - Replace all `text-gray-*`, `bg-white`, `border-gray-*` with semantic tokens
   - Replace `text-blue-600` with `text-primary`

**Acceptance**:
- Search results come from backend, not local filter
- Clicking a session result navigates to that specific session
- Search page uses design tokens consistently

---

### A4. Research notes: formalize account-synced behavior (Finding #4)

**Gap**: PRD previously said "local-only", but implementation already syncs to account. PRD updated in §7.7. Now need to add missing features: export-to-markdown, multi-note lifecycle awareness.

**Changes**:

1. **Add markdown export button** — `frontend_rust/crates/web-ui/src/routes/dashboard.rs` (notes pane, ~line 2003)
   - Add a toolbar button above the textarea
   - On click, create a Blob from the notes content and trigger download as `{notebook_title}-notes.md`

2. **Add clear/delete action** — same location
   - Button to clear notes content
   - Syncs empty state to server (already handled by `upsert_workspace_draft` removing empty entries)

3. **Update PRD §7.7** — Done (see PRD changes above)

**Acceptance**:
- Notes pane has export-to-markdown button that downloads a `.md` file
- Notes pane has clear button with confirmation
- Sync state badge continues to work correctly
- PRD accurately reflects account-synced notes as product direction

---

## Stream B: Contract & Semantics Closure

### B1. Share semantics completion (Finding #5)

**Gap**: Share settings lack permission model explanation. Public share page has no partial/full distinction. `SharedShareInfo` DTO has no partial/full field.

**Changes**:

#### Backend

1. **Add share scope field** — `avrag-rs/crates/share/src/lib.rs` + `avrag-rs/crates/transport-http/src/lib.rs`
   - Add `scope: String` field to share info (values: `"full"`, `"partial"`)
   - `full` = all sources visible + chat enabled
   - `partial` = limited source metadata + no chat
   - Migrate existing shares to default `"full"`

#### Frontend SDK

2. **Extend `SharedShareInfo`** — `frontend_rust/crates/web-sdk/src/lib.rs:572`
   ```rust
   pub struct SharedShareInfo {
       pub permission: String,
       pub expires_at: Option<String>,
       pub allow_download: bool,
       pub scope: String,  // "full" | "partial"
   }
   ```

#### Frontend UI

3. **Share center: add permission explanations** — `frontend_rust/crates/web-ui/src/components/share/mod.rs:210`
   - Below the access_level selector, render a description block:
     - private: "Only you can access"
     - link: "Anyone with the link can view"
     - public: "Discoverable and accessible to anyone"

4. **Public share page: render scope** — `frontend_rust/crates/web-ui/src/routes/shared.rs:849`
   - Show "Full access" or "Preview only" badge based on `scope` field
   - Hide chat interface when `scope == "partial"`

**Acceptance**:
- Share settings page explains what each permission level means
- Public share page distinguishes full vs partial access
- Backend stores and returns scope field

---

### B2. Settings/Billing alignment (Finding #6)

**Gap**: Settings missing password reset entry. Billing shows "联系管理员" instead of self-service upgrade.

**Changes**:

1. **Add password reset entry in Settings** — `frontend_rust/crates/web-ui/src/routes/settings.rs:443`
   - In Account section, add "Reset password" link that navigates to `/reset-password`
   - This is separate from "Change password" (which changes immediately)

2. **Billing upgrade path** — `frontend_rust/crates/web-ui/src/components/billing/mod.rs:306, 476`
   - If Stripe portal is configured, show "Manage subscription" button (already exists)
   - If on free plan and portal is available, show "Upgrade" CTA that opens portal
   - If no portal configured, show "Contact admin" as fallback (existing behavior)

**Acceptance**:
- Settings Account section has both "Change password" and "Reset password"
- Billing shows upgrade CTA when Stripe portal is configured

---

## Stream C: Engineering Hardening

### C1. i18n centralization (Finding #7)

**Gap**: Most pages use inline `choose(locale, "中文", "English")` instead of key-based i18n dictionary. Only auth routes use the central `i18n.rs` layer.

**Changes**:

1. **Extend i18n dictionary** — `frontend_rust/crates/web-ui/src/i18n.rs`
   - Add key buckets for: dashboard, workspace, search, share, settings, admin
   - Each key maps to `(zh, en)` tuple
   - Example:
     ```rust
     ("dashboard.empty.title", ("还没有知识库", "No notebooks yet")),
     ("dashboard.card.updated", ("更新于", "Updated")),
     ("search.placeholder", ("搜索网页与知识库内容...", "Search the web and notebooks...")),
     ```

2. **Replace inline `choose()` calls in components** — systematic file-by-file pass:
   - `dashboard.rs` (~30+ `choose()` calls → `t("key")`)
   - `search.rs` (~15+ calls)
   - `shared.rs` (~20+ calls)
   - `settings.rs` (~15+ calls)
   - `admin.rs` (~30+ calls)
   - `share/mod.rs` (~10+ calls)

3. **Map backend action values** — `frontend_rust/crates/web-ui/src/routes/shared.rs:156`
   - Share access log `action` field: create a mapping function
   - `"view"` → "查看" / "View"
   - `"chat"` → "对话" / "Chat"
   - `"download"` → "下载" / "Download"

**Acceptance**:
- All user-facing strings in main routes come from `i18n.rs` dictionary
- No raw `choose(locale, "中文", "English")` outside of i18n module
- Backend action values are mapped to localized labels

---

### C2. Visual token migration (Finding #8)

**Gap**: Search and admin pages use hardcoded `text-gray-*`, `bg-white`, `border-gray-*` instead of semantic tokens.

**Changes**:

1. **Tokenize search page** — `frontend_rust/crates/web-ui/src/routes/search.rs:153-300`
   - `text-gray-900` → `text-foreground`
   - `text-gray-500` → `text-muted-foreground`
   - `text-gray-600` → `text-muted-foreground`
   - `text-gray-700` → `text-foreground`
   - `bg-white` → `bg-card`
   - `border-gray-200` → `border-border`
   - `border-gray-300` → `border-border`
   - `bg-gray-50` → `bg-muted`
   - `bg-gray-100` → `bg-muted`
   - `text-blue-600` → `text-primary`
   - `bg-blue-600` → `bg-primary`
   - `hover:bg-blue-700` → `hover:bg-primary/90`
   - `text-red-700` → `text-danger`
   - `bg-red-50` → `bg-danger/10`
   - `border-red-200` → `border-danger/30`

2. **Tokenize admin page** — `frontend_rust/crates/web-ui/src/routes/admin.rs:655, 1031, 1938`
   - Same class replacements as above, systematic pass through all admin sub-routes

3. **Remove dark-mode compat shim** — `frontend_rust/crates/web-ui/src/index.css:302`
   - Once token migration is complete, the `.dark` override shim can be simplified
   - Verify dark mode works purely through CSS custom properties

**Acceptance**:
- Search page uses only semantic tokens (no raw gray/blue/red classes)
- Admin pages use only semantic tokens
- Dark mode works via token layer without compat shim overrides

---

### C3. Test hardening (Finding #9)

**Gap**: Only a few unit tests and one Playwright spec. Release-grade coverage requires SSE parser, chat state machine, document poller, share mapping, route smoke tests.

**Changes**:

1. **SSE parser unit tests** — new file `frontend_rust/crates/web-ui/src/sse_test.rs` or extend existing
   - Test: `data: {"type":"token","content":"hello"}\n\n` → parsed correctly
   - Test: multi-event stream
   - Test: error event handling
   - Test: malformed data handling

2. **Chat state machine tests** — extend `frontend_rust/crates/web-ui/src/routes/dashboard.rs` tests section
   - Test: `Idle → Submitting → Streaming → Done` transition
   - Test: `Streaming → Error` transition
   - Test: concurrent input during streaming is rejected

3. **Research notes sync tests** — extend existing test module
   - Test: debounce behavior (rapid updates don't each trigger a save)
   - Test: conflict resolution (remote preferences changed during edit)
   - Test: empty notes removal

4. **Document poller tests** — new test module
   - Test: polling stops on terminal state
   - Test: retry on transient error
   - Test: status chip renders correct semantic state

5. **Share mapping tests**
   - Test: permission label mapping
   - Test: scope rendering (full vs partial)
   - Test: expired share display

6. **Route smoke tests** — extend Playwright spec `avrag-rs/e2e/rust-frontend-e2e.spec.ts`
   - Navigate to each main route and verify no console errors
   - Verify auth redirect flows
   - Verify workspace loads with mock data

7. **Dark mode verification** — Playwright
   - Toggle dark mode
   - Verify token classes apply correctly
   - Screenshot comparison for key pages

**Acceptance**:
- Unit test coverage ≥80% for SSE parser, chat state machine, notes sync
- Route smoke tests cover all main user routes
- Dark mode visual verification exists

---

## Execution Order and Dependencies

```
Stream A (Product Surfaces)
  A1. Dashboard home ─────────┐
  A2. Workspace shell ────────┤──→ Stream B (Contracts)
  A3. Search ─────────────────┤      B1. Share semantics
  A4. Research notes ─────────┘      B2. Settings/Billing
                                       │
                                       ▼
                                    Stream C (Hardening)
                                      C1. i18n centralization
                                      C2. Visual token migration
                                      C3. Test hardening
```

**Within each stream, tasks can be parallelized.** Stream dependencies exist because:
- A1 (dashboard) needs the backend `Notebook` DTO extension before frontend cards can render new fields
- A3 (search) needs backend search endpoint before frontend can drop local filtering
- B1 (share) needs backend scope field before frontend can render partial/full
- C1/C2 (i18n/tokens) should happen after product surfaces are stable to avoid rework

---

## File Change Summary

| File | Stream | Change |
|------|--------|--------|
| `avrag-rs/crates/transport-http/src/lib.rs` | A1, A3, B1 | Notebook DTO, search endpoint, share scope |
| `avrag-rs/crates/share/src/lib.rs` | B1 | Share scope field |
| `frontend_rust/crates/web-sdk/src/lib.rs` | A1, B1 | Notebook DTO, SharedShareInfo |
| `frontend_rust/crates/web-ui/src/routes/dashboard.rs` | A1, A2, A4 | Dashboard cards, workspace layout, notes export |
| `frontend_rust/crates/web-ui/src/routes/search.rs` | A3, C1, C2 | Backend search, i18n, tokenization |
| `frontend_rust/crates/web-ui/src/routes/shared.rs` | B1, C1 | Share scope, i18n |
| `frontend_rust/crates/web-ui/src/routes/settings.rs` | B2, C1 | Reset password entry, i18n |
| `frontend_rust/crates/web-ui/src/components/share/mod.rs` | B1, C1 | Permission explanation, i18n |
| `frontend_rust/crates/web-ui/src/components/billing/mod.rs` | B2 | Upgrade CTA |
| `frontend_rust/crates/web-ui/src/routes/admin.rs` | C1, C2 | i18n, tokenization |
| `frontend_rust/crates/web-ui/src/i18n.rs` | C1 | Extended dictionary |
| `frontend_rust/crates/web-ui/src/index.css` | C2 | Dark mode shim cleanup |
| `avrag-rs/e2e/rust-frontend-e2e.spec.ts` | C3 | Route smoke, dark mode tests |
| `frontend_rust/FRONTEND_PRD.md` | A4 | §7.7 updated (done) |

---

## Open Questions (carried from review)

1. **Share scope backend migration**: Adding `scope` to existing shares requires a DB migration. Default to `"full"` for all existing rows.
2. **Search backend endpoint**: Need to confirm the backend has (or can add) a unified search endpoint. If not available, a lightweight FTS query against notebooks + sessions tables is needed.
3. **Mobile drawer implementation**: Leptos doesn't have built-in drawer/sheet primitives. May need a small custom component or CSS-only approach with `@media` queries.
