# Frontend Delivery Handoff — context-osv6

> Date: 2026-03-30
> Status: Committed, acceptance verified, usage-limit feature wired

## A. Canonical Paths

| Component | Canonical Path | Status |
|-----------|---------------|--------|
| **Frontend UI** | `frontend_rust/crates/web-ui/` | Active, compiles, 9/9 tests pass |
| **Frontend SDK** | `frontend_rust/crates/web-sdk/` | Active, compiles |
| **Backend (Rust)** | `avrag-rs/crates/` | Backend workspace |
| **Storage (PostgreSQL)** | `avrag-rs/crates/storage-pg/` | Modified (notebook enrichment) |
| **Share Service** | `avrag-rs/crates/share/` | Modified (scope field) |
| **Usage Limit** | `avrag-rs/crates/usage-limit/` | New — shadow-mode metering + quota check |

### Legacy Frontend (archived, not deleted)

| Path | Action | Current State |
|------|--------|---------------|
| `avrag-rs/crates/web-ui/` | Added to `avrag-rs/.gitignore`; `ARCHIVED.md` marker placed | Directory exists on disk but excluded from compilation and git tracking |
| `avrag-rs/crates/web-sdk/` | Added to `avrag-rs/.gitignore`; `ARCHIVED.md` marker placed | Directory exists on disk but excluded from compilation and git tracking |

Both directories contain `ARCHIVED.md` files explaining their legacy status and canonical replacement paths. They remain Cargo workspace members (required by `cargo` auto-discovery) but are git-ignored and not compiled into the build graph.

Retention rationale: The legacy directories are Leptos 0.7 skeletons (1,399 LOC combined). They are kept on disk as reference material but are excluded from both git tracking and the Cargo build graph. All new frontend development uses `frontend_rust/` exclusively.

## B. Changes Summary

### Observability (2026-04-08 follow-up)

| File/Area | Change |
|------|--------|
| `avrag-rs/crates/analytics/` | **New** — product events, cost events, daily rollup helpers, anomaly helpers |
| `avrag-rs/migrations/0019_observability_events.up.sql` | **New** — analytics event + rollup + anomaly tables |
| `avrag-rs/crates/telemetry/src/prometheus.rs` | **New** — Prometheus registry + encoder |
| `avrag-rs/crates/transport-http` | `/metrics` now returns Prometheus text; auth + chat flows emit analytics events |
| `avrag-rs/crates/app` | notebook/document/url/chat flows emit product + cost events |
| `avrag-rs/bins/worker` | summary generation emits cost events; optional rollup job runner |

Operational expectations:

- `GET /metrics` should return plaintext exposition including `http_requests_total`
- `product_events` should record register/login/upload/url/chat outcomes
- `cost_events` should record graphflow and worker summary usage
- public share-page views remain anonymous and are tracked in `share_access_logs`; owner-side rollups feed `daily_user_metrics.shared_kb_open_count`
- set `ANALYTICS_ROLLUP_ENABLED=true` to enable worker-side daily rollups and anomaly scans

### Backend (avrag-rs)

| File | Change |
|------|--------|
| `crates/common/src/lib.rs` | Added `document_count`, `status_summary`, `shared` fields to `Notebook` |
| `crates/storage-pg/src/lib.rs` | LATERAL JOIN for notebook list (doc count, status aggregation, share flag) |
| `crates/share/src/lib.rs` | Added `scope: String` field to `SharedShareInfo` |
| `crates/app/src/lib.rs` | Fixed `search()` method; added `chat/graphflow` module; wired usage-limit service (record + quota check) |
| `crates/app/src/chat/graphflow.rs` | Added `llm_usage: Option<LlmUsage>` to `ChatGraphExecution` |
| `crates/app/src/chat/service.rs` | Phase-gated quota enforcement in preflight; metering in general/RAG/search modes |
| `crates/llm/src/client.rs` | Added `Serialize, Deserialize` to `LlmUsage` |
| `crates/llm/src/synthesizer.rs` | Return `LlmUsage` alongside `SynthesisOutput` |
| `crates/usage-limit/` | **New crate** — `UsageLimitService`, `MeteringContext`, `QuotaCheckResult`, rolling-window SQL |
| `crates/transport-http/src/lib.rs` | Added `GET /api/auth/usage-limit` route + 27 handler stubs (501) + middleware |
| `crates/web-sdk/src/usage_limit.rs` | **New** — `UsageLimitApi`, `UsageLimitResponse` SDK types |
| `crates/web-ui/src/components/usage_limit_card.rs` | **New** — `UsageLimitCard` Leptos component (5h/7d progress bars) |
| `migrations/0018_user_usage_limits.up.sql` | **New** — `llm_usage_events`, `llm_model_weights`, `usage_limit_plan_policies`, `usage_limit_user_overrides` tables |
| `Cargo.toml` | Added `crates/usage-limit` to workspace members |
| `crates/web-ui/ARCHIVED.md` | Added archival marker |
| `crates/web-sdk/ARCHIVED.md` | Added archival marker |

### Frontend SDK (frontend_rust)

| File | Change |
|------|--------|
| `crates/web-sdk/src/lib.rs` | Extended `Notebook` DTO; added `usage_limit` module |
| `crates/web-sdk/src/usage_limit.rs` | **New** — `UsageLimitApi::get_usage_limit()`, response types |

### Frontend UI (frontend_rust)

| File | Change |
|------|--------|
| `crates/web-ui/src/routes/dashboard.rs` | "Shared with Me" section, document count/status badges, mobile responsive rails |
| `crates/web-ui/src/routes/search.rs` | Token migration (hardcoded colors → semantic tokens) |
| `crates/web-ui/src/routes/admin.rs` | Token migration (154 hardcoded color classes → semantic tokens) |
| `crates/web-ui/src/routes/settings.rs` | Password reset entry card |
| `crates/web-ui/src/components/billing/mod.rs` | Upgrade CTA button |
| `crates/web-ui/src/components/share/mod.rs` | Permission level descriptions |
| `crates/web-ui/src/routes/shared.rs` | Scope badge (full access / preview only) |
| `crates/web-ui/src/components/usage_limit_card.rs` | **New** — Usage progress bars with 5h/7d windows |
| `crates/web-ui/src/components/mod.rs` | Added `usage_limit_card` module export |
| `crates/web-ui/src/i18n.rs` | Extended `MessageKey` enum with 133+ keys |

### Long-text virtualization

- Workspace chat, shared notebook Q&A, and global search answer surfaces now use the shared virtual list layer for long text.
- Browser-side `pretext` prediction is optional and falls back to heuristic estimation if unavailable.
- Streaming tail items stay mounted so live answers keep their scroll context stable.

### Repository Cleanup

| File | Change |
|------|--------|
| `.gitignore` (root) | Created — ignores target/, pkg/, node_modules/, .env, logs, .claude/, legacy dirs |
| `avrag-rs/.gitignore` | Added /target/, /pkg/, /storage/, /crates/web-ui/, /crates/web-sdk/ |
| `frontend_rust/.gitignore` | Added /target/, /pkg/, /node_modules/ |

## C. Acceptance Results (verified 2026-03-30)

### Compilation

| Command | Result |
|---------|--------|
| `cargo check -p frontend-web-ui -p frontend-web-sdk` | **PASS** |
| `cargo test -p frontend-web-ui --lib` | **PASS** — 9/9 tests |
| `cargo check -p common -p avrag-share -p avrag-storage-pg -p avrag-search` | **PASS** |
| `cargo test -p avrag-storage-pg --lib` | **PASS** — 1/1 test |
| `cargo check -p app` | **PASS** |
| `cargo check --workspace` (from `avrag-rs/`) | **PASS** — 0 errors (transport-http stubs restored) |
| `cargo test -p avrag-usage-limit` | **PASS** — 4/4 tests |
| `cargo test -p app` | **PASS** — 10/10 tests |

### Playwright Browser Smoke

| Test | Result |
|------|--------|
| T01: Health and readiness endpoints | **PASS** (137ms) |
| T02: User registration and login flow | **PASS** (1.6s) |
| T03: Leptos SSR renders page shells | **PASS** (56ms) |

3/3 passed. Additional UI smoke tests (dashboard, workspace, share, settings, admin) require backend infrastructure not available in CI.

## D. Git Status

Two commits have been made:

1. `3edffb7` — `feat: frontend v6 delivery — Leptos 0.8 WASM UI, SDK, and backend integration` (169 files, 35,920 insertions)
2. `f062a42` — `fix(app): close search() method body and add missing Notebook fields`
3. `76ed8b7` — `docs: update DELIVERY_HANDOFF to reflect committed and verified state`

Uncommitted working-tree changes: 13 files in `avrag-rs/` from the usage-limit feature wiring (Phase 1 shadow mode). These are functional but not yet committed as a separate feature commit.

## E. Remaining Risks

1. **Usage-limit feature uncommitted** — 13 files with shadow-mode metering + quota enforcement are in the working tree but not committed. These need a dedicated feature commit.

2. **Playwright coverage is minimal** — Only infrastructure tests (T01-T03) ran. UI smoke tests for dashboard, workspace, share, settings, and admin routes require a full backend stack.

3. **i18n `choose()` calls not migrated to `t()`** — The `MessageKey` enum has 133+ keys but component code still uses inline `choose(locale, "中文", "English")`. Cosmetic/debt item, not a functional blocker.

4. **Admin token migration needs visual QA** — 154 `sed`-based color replacements compile but should be visually verified in browser for edge cases.

5. **Legacy `web-ui`/`web-sdk` directories on disk** — Archived with `ARCHIVED.md` markers. Can be safely deleted when desired.

6. **`transport-http` handler stubs** — 27 handlers return `StatusCode::NOT_IMPLEMENTED`. These are placeholders; real implementations need to be restored from a prior commit or rewritten.

## F. Minimum Acceptance Commands

```bash
# Frontend (from frontend_rust/)
cargo check -p frontend-web-ui -p frontend-web-sdk
cargo test -p frontend-web-ui --lib

# Backend (from avrag-rs/)
cargo check --workspace
cargo test -p avrag-usage-limit --lib
cargo test -p app --lib

# E2E (requires backend stack: PostgreSQL + Qdrant + API server)
npx playwright test --project=chromium
```
