# Thermo-Nuclear Code Quality Review — 2026-07-09

**Scope:** Full monorepo **excluding** deprecated `frontend_rust/`.  
**Includes:** `avrag-rs/`, `frontend_next/`, `contracts/`, `desktop/src-tauri/`, `scripts/`.  
**Baseline:** Post M0–M5 chain + StorageContext W4e (steps 0–3, commits `cf768a6` / `9034305`).

**Method:** Re-measure every prior CRITICAL/HIGH claim against the live tree; probe god-files, dual paths, kitchen-sink traits, boundary leaks, and missed code-judo moves.

---

## Verdict: ❌ NOT APPROVED

M0–M5 and StorageContext decomposition **materially improved** the codebase: `include!` walls mostly gone, god-constructor defused, several services extracted, frontend envelope/zod hygiene fixed. That progress is real.

It is **not** enough for thermo-nuclear approval. The system still carries structural debt that actively multiplies complexity: a string-shaped error taxonomy (`StateSink` ×67), dual memory/PG control flow (×19), a 22-method persistence kitchen sink, multi-concern contract types, production-shipped dev simulators, and **96 TypeScript errors** from WIP admin surfaces. These are design problems, not polish nits.

---

## Part 0 — What actually got better (do not re-litigate)

| Prior ID | Status | Evidence |
|----------|--------|----------|
| **B4** StorageContext 19-arg god-bag | **FIXED** | Facade + `StorageContextParts` + 4 groups; accessors stable; group getters in step 3 |
| **S1** `include!` flat namespace (storage-pg / transport-http core) | **FIXED** | Real `mod` trees; `PgAppRepository` domain structs |
| **M2-S1** stream zod cast | **FIXED** | `z.discriminatedUnion("event", …)` in `frontend_next/lib/workspace/stream.ts` |
| **NEW-7** tiptap wrong CSS module | **FIXED** | `workspace-note-editor.module.css` |
| **S3** password-reset handler god | **IMPROVED** | `PasswordResetService` exists; handler thinner |
| **H5** desktop `lib.rs` inline commands | **IMPROVED** | Commands live under `commands/`; `lib.rs` ~82 lines |
| **A2** param threading | **IMPROVED** | `RunContext` / `DraftOptions` introduced |
| **A3** unified dispatch copy-paste | **IMPROVED** | `run_react_mode` shared path; arms thinner |
| **C4** RetrievalDataPlane | **PARTIAL** | `RetrievalReadPort` split exists; write defaults still fail at runtime |
| **C2** IndexedChunk triple-def | **FIXED** | Single def in `common` |
| **D4** draft boolean explosion | **FIXED** | `DraftOptions` |
| **E7** auth pass-through crate | **IN PROGRESS** | `crates/auth` gone in working tree; not fully landed as clean history everywhere |

---

## Part 1 — CRITICAL findings (blockers)

### C1. [CRITICAL] WIP admin surfaces break the frontend type system (96 errors)

**Where:** `frontend_next/components/admin/admin-{orgs,users,usage,org-detail,health,shared-ui,utils}*.tsx` (several untracked).  
**Measured:** `pnpm exec tsc --noEmit` → **96** `error TS2345` on missing i18n keys (`common.loading`, `users.subtitle`, `usage.aggregateScope`, …).

**Why this is structural, not WIP noise:**  
Admin already has a dedicated path (`adminText` / `INLINE_COPY` in `admin-i18n.ts`). New surfaces call `adminMessage` → global `UI_MESSAGES` with keys that exist in **neither** system. That is a **second parallel i18n architecture** bolted onto an unfinished product surface while the typechecker is red.

**Code judo:** Pick one seam and delete the other.
1. Either add a proper `admin` message namespace to the typed `UI_MESSAGES` pipeline and only use that;  
2. Or force all admin UI through `adminText` / `INLINE_COPY` and delete `adminMessage` for admin.  
Quarantine untracked surfaces behind a feature flag / out of `tsconfig` until green — **do not** leave 96 errors as ambient noise.

---

### C2. [CRITICAL] `IngestionError::StateSink(String)` is a typed enum that erases types

**Where:** `avrag-rs/crates/ingestion/src/error.rs`  
**Measured:** ~**67** `StateSink` sites across worker + ingestion. Variants still:

```rust
TaskSource(String), AuditSink(String), StateSink(String)
```

Plus `From<uuid::Error> → StateSink`. Security scan, object store, parse route, IR validation, locks — everything becomes a string.

**Why this fails the bar:**  
The “typed error” refactor bought a name, not a taxonomy. Call sites still do `.map_err(|e| IngestionError::StateSink(e.to_string()))`. Retry/metrics/alerting cannot distinguish malware vs timeout vs PG vs parse. This is worse than `anyhow` because it **looks** structured.

**Code judo:** Replace `StateSink(String)` with domain variants + `From` impls:

```text
Storage(Pg/ObjectStore), Parse(ParseRoute/IR), Security(Threat|ZipBomb|ScannerDown),
Lock(Skipped|Failed), Timeout, InvalidId, …
```

Delete every `StateSink(e.to_string())`. If a conversion is missing, the compiler tells you — that is the point.

---

### C3. [CRITICAL] Dual memory/PG control flow still scattered (×19)

**Where:** `app-documents` (notebooks, document_context, url_imports, …), `app-admin` (admin_context, preferences).  
**Pattern:**

```rust
if let Some(store) = storage.document_store() {
    return store.…;
}
// inline MemoryState via storage.inner()
```

M4 claimed Memory*Store ports eliminate dual-backend inlining. **Reality:** documents CRUD often uses `document_store().ok_or_else`, but notebooks/admin still implement full memory branches next to the port path. Nineteen explicit dual branches remain.

**Why this is spaghetti growth:**  
Every new feature re-learns “is the port present?” Org filtering, ownership, and error shapes diverge between arms. `MemoryDocumentStore` already exists — the dual path is **optional port + fallback**, not a clean adapter selection at bootstrap.

**Code judo:** At bootstrap, **always** install a concrete store (`Memory*` or `Pg*`). Domain methods take `&dyn DocumentStorePort` (or the context only holds the port). Delete every `if let Some(store)` / `storage.inner()` dual branch in app-documents/app-admin. Memory mode becomes an adapter choice, not a control-flow mode.

---

## Part 2 — HIGH findings

### H1. [HIGH] `ChatPersistencePort` kitchen sink (22 methods) — ISP still broken

**Where:** `avrag-rs/crates/rag-core-ports/src/chat_persistence.rs` (~164 lines, **22** `async fn`).  
Mixes: notebook search, session CRUD, messages, user profile, conversation history, notifications, usage, document assets, multimodal chunks, audit, chunk retrieval, summary metadata.

**Code judo:** Split into focused traits (`SessionPort`, `MessagePort`, `ProfilePort`, `NotificationPort`, `AssetPort`, …). Adapters implement what they own. Callers depend on the one trait they need. This is the same move that fixed `PgAppRepository` — finish the job at the **port** layer.

---

### H2. [HIGH] `contracts/src/rag_execute.rs` still hosts runtime policy (626 lines)

**Where:** `contracts/src/rag_execute.rs`  
Still contains: `validate()` (multi-variant engine), `ensure_original_query_text_dense_item()`, `PlaceholderTriplet::classify()`, `to_chat_request_compat()` (doc even admits it still exists).

Contracts should be **wire shapes**. Runtime validation/reorder/classification belong in `rag-core` / `app-chat`. Leaving them in `contracts` couples every consumer to policy and blocks independent evolution of the wire format.

**Code judo:** Move policy to rag-core; leave pure types + serde in contracts; delete or isolate compat behind a feature-gated adapter module with an expiry date.

---

### H3. [HIGH] Worker orchestration still god-shaped

| File | Lines | Issue |
|------|------:|-------|
| `document_pipeline.rs` | 742 | `run_document_pipeline` still a long sequential script (parse → IR project → chunk → index → summary → …) |
| `processor.rs` | 525 | `process()` wraps nearly all work in one `timeout` closure: locks, fetch, security, route, pipeline, telemetry |

Partial extractions (`compensation`, `degradation`, `parse_route`, `index_dispatch`) help, but there is still **no stage model**. The reader holds the entire pipeline in their head.

**Code judo:**  
`trait Stage { async fn run(&mut self, ctx: &mut PipelineCtx) -> Result<(), IngestionError>; }`  
`process` = acquire lock → build ctx → `for stage in stages { stage.run(&mut ctx).await? }` → finish.  
`processor` only does lock + timeout + dispatch; stages own side effects.

---

### H4. [HIGH] Analytics still multi-homed

Still live:

- `transport-http` `record_api_product_event_if_available` (hardcodes API surface)
- `AppState::record_product_event_if_available`
- `app-documents::analytics_helpers::record_product_event_if_available`
- `ChatContext::record_product_event_if_available`
- share store’s own `record_share_product_event`

Same concept, multiple entry points → inevitable drift (surface, platform, metadata).

**Code judo:** One trait or one free function on `AnalyticsServiceCtx` with explicit `Surface`. Everything else is a one-line wrapper or dies.

---

### H5. [HIGH] `app-bootstrap` remains a kitchen-sink crate (~8.3k LOC)

63 Rust files, top weights: `pg_auth_store` 588, billing SQL shards, `postgres_delegates` 445, pure pass-throughs (`pg_chat_persistence` 336, `pg_content_store` 113).

M3 fixed storage-pg modularity. Bootstrap still **re-wraps** domain repos into ports with little logic. That is a shallow layer: delete complexity would force callers onto clearer seams, not push it back.

**Code judo:**  
- Prefer domain structs (`repo.documents()`, `repo.sessions()`) where the port adds only error mapping.  
- Collapse pure-delegation adapters or generate them once.  
- Split remaining `include!` billing shards into real modules (still present under `billing_sql/`).

---

### H6. [HIGH] Production binary ships a dev-only token budget simulator

**Where:** `app-chat/src/token_budget/mod.rs` (~804 lines), `pub mod token_budget` in `lib.rs`.  
**Callers in production code:** none found. Front ~500 lines are offline simulation.

**Code judo:** `#[cfg(any(test, feature = "dev-tools"))]` or move to `bins/` / dev crate. Do not ship analysis toys in the agent runtime crate.

---

### H7. [HIGH] Share HTTP path still fat despite `ShareService`

**Where:** `transport-http/.../workspaces/share.rs` (~527 lines).  
`avrag_share::ShareService` exists and is used by share crate handlers, but transport-http still inlines create/revoke/settings/analytics/members with postgres-unavailable checks and local `ApiEnvelope` types.

**Code judo:** Transport handlers become thin: auth guard → `ShareService` → envelope. Delete the second business-logic copy.

---

### H8. [HIGH] `AppState` / `ChatContext` double-own `StorageContext`

```rust
// AppState
storage: StorageContext,
chat: ChatContext, // also has storage: StorageContext
```

Mutations require dual updates (`set_uses_memory_adapters` clones into chat). This is accidental complexity from composition-by-clone.

**Code judo:** `ChatContext` holds only what chat needs (`&`/`Arc` to stores, or narrow ports). Single owner of storage configuration lives on `AppState` / bootstrap. Deleting the clone seam deletes a class of desync bugs.

---

## Part 3 — MEDIUM findings

### M1. Residual `include!` (billing / admin-store / chat service)

Not the old flat-namespace disaster, but still present:

- `app-bootstrap` `billing_sql` multi-include  
- `pg_admin_store` / `pg_share_store` OUT_DIR include for port impl  
- `app-chat/chat/service.rs` includes `service_modes` / `service_postprocess`

Prefer real `mod` trees everywhere; build.rs concat is a smell when Rust 2024 already forced the workaround.

### M2. Files past / near 1k (production + heavy test)

| File | Lines | Note |
|------|------:|------|
| `llm/.../openai_chat.rs` | 1080 | Production protocol — split request/stream/types |
| `desktop/.../license.rs` | 949 | Approaching 1k — split status/heartbeat/file IO |
| `transport-http/.../tests.rs` | 1558 | Test-only; still hurts navigation |
| `storage-pg/.../tests.rs` | 1400 | Same |
| product_e2e / rag_quality tests | 1.1k–1.3k | Acceptable if modularized further |

### M3. Desktop dual error contract

`IpcApiError` vs `Result<_, String>` across license/system/chat_stream. Pick one IPC error shape.

### M4. E2E scripts still `docker rm -f` test PG

`run-product-smoke-e2e.sh`, `run-liteparse-staging-e2e.sh`, `run-staging-ingest-e2e.sh` trap-remove `avrag-test-*`. Contradicts AGENTS.md “leave ephemeral PG running.” Align scripts with policy (reuse vs explicit opt-in tear-down flag).

### M5. History pane still multi-effect message loading

Title derivation still fetches messages via effects (improved caching with refs, but not a shared `useSessionMessages` hook). Search/index paths can still re-fetch. Extract one hook with a session→messages cache.

### M6. UnifiedAgent Write routing still awkward

Write is handled in `pipeline_steps` outside `UnifiedAgent`; match arm is `_ => not handled`. Better than a lying `write_mode_not_implemented`, but “Unified” still means “3/4 modes.” Document the split or route Write through the same service boundary.

### M7. Retrieval write defaults still panic-at-runtime

`RetrievalDataPlane` default methods return `"not implemented"`. Prefer trait split so implementors cannot construct a half-plane that compiles then fails in prod.

---

## Part 4 — Architecture scorecard (current)

| Layer | Grade | Comment |
|-------|-------|---------|
| Domain ports (documents/admin) | C+ | Ports exist; dual memory branches undermine them |
| Storage-pg modularity | B+ | Real mods + domain structs; tests still huge |
| Bootstrap / AppState | C | Context decomposition started; bootstrap still dumping ground |
| Worker pipeline | C | Extracted helpers; no stage model; StateSink fog |
| Agent loop | B- | RunContext/run_react_mode help; param/ownership edges remain |
| Contracts | C | Wire types polluted with policy (`rag_execute`) |
| Frontend_next core workspace | B | Envelope/zod/sanitize improved |
| Frontend_next admin | F | 96 type errors; dual i18n |
| Desktop | B- | Command modules good; error contract + fat license module |
| Scripts / ops | C | Policy vs `docker rm` conflict |

---

## Part 5 — Highest-leverage code-judo backlog (ordered)

### Phase A — Restore a green, honest baseline (1–2 days)

1. **C1** Admin i18n: one system only; typecheck → 0 (or exclude WIP from build).  
2. **H6** Gate `token_budget` out of release.  
3. **M4** Fix e2e docker lifecycle to match AGENTS.md.

### Phase B — Delete dual control-flow (2–3 days)

4. **C3** Always-on store adapters; delete ×19 dual branches.  
5. **H8** Stop cloning full `StorageContext` into `ChatContext`.  
6. **H7** Thin share HTTP via existing `ShareService`.

### Phase C — Error + pipeline model (3–4 days)

7. **C2** Typed `IngestionError` variants; kill StateSink.  
8. **H3** Stage-based document pipeline + slim processor.

### Phase D — Port / contract hygiene (4–5 days)

9. **H1** Split `ChatPersistencePort`.  
10. **H2** Evacuate policy from `rag_execute` contracts.  
11. **H4** Single analytics entry.  
12. **H5** Thin or delete pure pass-through bootstrap adapters.

### Phase E — Size / polish

13. Split `openai_chat.rs`, `license.rs`.  
14. Unify desktop IPC errors.  
15. `useSessionMessages` hook.

---

## Part 6 — Approval bar check

| Criterion | Met? |
|-----------|------|
| No clear structural regression | Partial — recent work improved structure |
| No obvious missed code-judo when path is visible | **No** — dual stores, StateSink, ChatPersistence, rag_execute |
| No unjustified >1k production files | **No** — `openai_chat.rs` |
| No spaghetti special-case growth | **No** — dual store ifs; multi analytics |
| No hacky / magical abstractions | Mixed — facade StorageContext is good; StateSink is fake typing |
| No boundary leaks | **No** — contracts host policy; bootstrap pass-throughs |
| Obvious decompositions remaining | **Yes** — listed above |

**Therefore: NOT APPROVED** for “codebase is maintainable enough.”  
Approve only after Phase A+B land and C2 is at least started (typed errors + dual-store deletion).

---

## Part 7 — Explicit non-findings (avoid rework)

- Do **not** re-open StorageContext 19-arg constructor work — fixed.  
- Do **not** re-convert storage-pg/transport-http core `include!` walls — fixed.  
- Do **not** demand mass rewrite of remaining facade accessors on `StorageContext` — that was the high-risk path deliberately avoided.  
- Test files >1k are lower priority than production dual-path and error taxonomy.

---

## Statistics

| Metric | Value |
|--------|------:|
| CRITICAL open | 3 (admin tsc, StateSink, dual-store) |
| HIGH open | 8 |
| MEDIUM open | 7 |
| Prior CRITICAL closed this cycle | 2+ (StorageContext, include! core) |
| Frontend tsc errors | 96 |
| StateSink sites | ~67 |
| Dual `if let Some(store)` sites | 19 |
| ChatPersistencePort methods | 22 |
| app-bootstrap LOC | ~8.3k |

---

*Reviewer stance: progress since 2026-07-08 is real and should be merged/kept. The bar here is structural honesty — optional ports, string errors, and red typechecks are not “later polish”; they are the architecture.*
