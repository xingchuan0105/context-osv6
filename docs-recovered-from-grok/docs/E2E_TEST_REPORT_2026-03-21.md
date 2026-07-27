# context-osv6 E2E Test Report

> Date: 2026-03-21
> Tester: Codex via Playwright CLI
> Scope: PRD-driven real-browser lifecycle smoke / E2E
> Mode: continue testing, record issues, do not auto-fix product/runtime bugs found during E2E
> Historical report. Qdrant references describe the tested environment on 2026-03-21, not the current target architecture. See [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

## 1. Environment

- Browser automation:
  - Playwright CLI
  - Google Chrome running inside WSL
- App server:
  - `/home/chuan/context-osv6/avrag-rs/target/debug/avrag-api`
- Worker:
  - `/home/chuan/context-osv6/avrag-rs/target/debug/avrag-worker`
- Supporting services:
  - PostgreSQL
  - Redis
  - Qdrant
  - MinIO
- Browser bundle generated and served:
  - `/pkg/web_ui.js`
  - `/pkg/web_ui_bg.wasm`
- Test upload file:
  - `/home/chuan/context-osv6/output/playwright/upload-source.txt`

## 2. Major progress this round

Compared to the original blocked state:

- SSR no longer crashes with `ERR_EMPTY_RESPONSE`.
- Leptos hydration now runs in the browser.
- Browser interactions now trigger real API requests.
- Auth/register/login/notebook-create flow is working end to end.

## 3. PASS

### P01. Health / readiness
- `/health` returns OK.
- `/ready` returns a readiness payload.

### P02. Root route
- `/` now returns a redirect to `/login`.

### P03. Login page SSR
- `/login` renders the Sign In page HTML.

### P04. Register page SSR
- `/register` renders the Create Account page HTML.

### P05. Dashboard SSR
- `/dashboard` renders the notebooks page shell.

### P06. Admin SSR
- `/admin` renders the admin shell and organizations screen shell.

### P07. Browser hydration/runtime is active
- Browser loads:
  - `/pkg/web_ui.js`
  - `/pkg/web_ui_bg.wasm`
- Form submits are no longer pure browser fallback.
- Button clicks trigger client-side behavior on core pages.

### P08. Register new user
- Registration for `e2e@example.com` succeeded.
- Browser navigated to `/dashboard` after successful registration.

### P09. Login existing user
- Fresh browser session login for `e2e@example.com` succeeded.
- Browser navigated to `/dashboard`.

### P10. Create notebook
- Created notebook `e2e-notebook`.
- Workspace appeared in dashboard list.

### P11. Enter workspace
- Entered `/dashboard/6aef85fb-2262-4546-a189-f34b8a8f0326`.
- Workspace shell rendered with:
  - left tabs
  - center chat panel
  - right evidence panel

### P12. Upload chain reaches backend
- Frontend successfully performed:
  - `POST /api/v1/notebooks/{id}/documents`
  - `PUT /uploads/{doc_id}...`
  - `POST /api/v1/documents/{id}/complete-upload`
- Worker log shows task processing completed for uploaded document.
- PostgreSQL row exists for uploaded document and status is `completed`.

### P13. API Access page shell
- `/dashboard/:notebook_id/api-access` renders server-side.
- Form and integration snippets are present.

### P14. Share Center shell
- `/dashboard/:notebook_id/share` renders.
- Share tabs are present.

### P15. Search page shell
- `/dashboard/search` renders.
- Existing notebook grouping appears in search results shell.

## 4. FAIL

### F01. Uploaded document is not visible through notebook query surfaces
- Frontend optimistic UI shows uploaded source as `queued`.
- Worker processed ingest task successfully.
- PostgreSQL shows uploaded document row with `completed` status.
- But notebook-scoped read surfaces return empty:
  - `GET /api/v1/sources?notebook_id=...` => `{"sources":[]}`
  - `GET /api/v1/documents?notebook_id=...` => `{"documents":[]}`
- Result:
  - workspace never shows source as `ready`
  - chat cannot be meaningfully validated against uploaded content

### F02. API Access page has hydration mismatch
- SSR HTML is present.
- After hydration, browser console shows fatal mismatch:
  - expected `<span>`
  - got unexpected node
- Result:
  - wasm runtime panics on this page
  - API Access interactive testing is unreliable

### F03. Admin / Feature Flags page has hydration mismatch
- SSR HTML is present.
- Browser console shows fatal hydration mismatch in admin/common rendering.
- Result:
  - wasm runtime panics on this page
  - interactive admin feature flag testing is unreliable

### F04. Search submit contract mismatch
- Search page renders and accepts input.
- Submitting search query returns:
  - `Search failed: API error: 400`
- Backend log shows:
  - `notebook_required notebook_id is required`
- Result:
  - current Search page request shape does not match backend expectation

### F05. Test user is not admin
- Feature Flags page reports:
  - `Failed to load feature flag requests: admin access denied`
- This blocks testing admin workflows with the current session.

## 5. PARTIAL / BLOCKED

### B01. Workspace chat
- Chat shell renders and input is present.
- Blocked for meaningful RAG validation because uploaded source is never exposed back through notebook-scoped read APIs.

### B02. Citation jump
- Not reached in this round because no successful answer with citations was produced.

### B03. Public share page
- Share center shell renders.
- Public shared notebook flow was not completed after hydration restore.

### B04. API Access mutation flow
- Page shell renders.
- Hydration mismatch blocks trustworthy mutation testing.

### B05. Admin mutation flows
- Admin shell renders.
- Hydration mismatch plus lack of admin role blocks request/approve/override validation.

## 6. Key evidence

### Browser-side success evidence
- Register:
  - browser transitioned to `/dashboard`
- Login:
  - fresh browser session transitioned to `/dashboard`
- Workspace create:
  - notebook card appeared
- Workspace:
  - three-column shell visible

### Backend evidence
- Worker processed uploaded document task:
  - `worker task processed ... kind=IngestDocument`
- PostgreSQL document row exists for uploaded file and is `completed`

### Inconsistency evidence
- Storage layer contains the uploaded/completed document.
- Workspace-scoped `sources` / `documents` APIs still return empty.
- Therefore the failure is not “upload never happened”; it is a read/query consistency issue.

### Hydration mismatch evidence
- API Access:
  - hydration error around `/routes/api_access.rs`
- Admin / Feature Flags:
  - hydration error around `/components/common` / admin rendering
- Console shows:
  - `Unrecoverable hydration error`
  - wasm panic after mismatch

### Search mismatch evidence
- Browser submit triggers real API request.
- Backend returns `400 notebook_id is required`.

## 7. Conclusion

The app is now far beyond the original blocked SSR state:

- SSR works
- Hydration works on core auth/dashboard/workspace flows
- Real user registration/login/notebook creation now function
- Upload reaches backend and worker

But full PRD-level E2E is still not complete because three major issues remain:

1. Workspace upload/read consistency is broken.
2. API Access and Admin / Feature Flags still have hydration mismatches.
3. Search page request contract does not match backend.

## 8. Recommended next fixes

1. Fix notebook-scoped `sources` / `documents` read consistency after ingestion.
2. Fix hydration mismatches on:
   - API Access
   - Admin / Feature Flags
3. Fix Search page request contract (`notebook_id is required`).
4. Re-run E2E for:
   - upload -> ready
   - chat -> citations
   - share flow
   - API access mutation
   - admin workflows
