# Thermo-Nuclear Code Quality Review — 2026-07-08 (Session 2, Post M0-M2)

**Scope:** Full codebase excluding `frontend_rust/`. Re-survey of branch `fix/tn-m2-frontend` (8 commits: M0 dead-code deletion, M1 contract integrity, M2 frontend hygiene).

**Method:** 4 parallel deep audits — (1) M0-M2 fix quality, (2) backend crates, (3) transport/worker/contracts, (4) frontend/scripts/desktop.

**Verdict: ❌ NOT APPROVED.** M0-M2 made meaningful progress (13k lines deleted, CRITICAL contract drift fixed, frontend duplication collapsed). But 3 CRITICAL + 8 HIGH structural findings remain unaddressed, 2 prior findings worsened, and new WIP code introduced 102 typecheck errors and a new architectural seam.

---

## Part 1 — M0-M2 Fix Quality Assessment

### What went right (QUALITY_OK × 10)

| Area | Finding |
|------|---------|
| M0 dead code | `AgentRunResult` struct properly cleaned (5 dead fields removed from both definition and constructor). No orphaned references. `cargo check --workspace` passes. |
| M0 dead code | All module deletions clean (`replay.rs`, `secure_services.rs`, `xml_slot_engine.rs`, 6 audit builders). All `mod.rs` declarations updated. |
| M1 W1a | `#[typeshare]` on `ToolSpec` correctly fixes the silent drift. `AgentOperationGuide` + `tool_schemas: ToolSpec[]` + `agent_operation_guide` field now generated. |
| M1 W1b | Contract completeness test is robust — tests all 15 ChatResponse keys with exact set equality + `satisfies` compile-time constraint. Cannot be silently bypassed. |
| M1 W1c | Integer type mappings removed from `typeshare.toml`. Verified: zero diff in generated `contracts.ts` (dead config confirmed). |
| M2 W2a | `requestEnvelope<T>()` extraction clean. All local `unwrapApiData` + `ApiEnvelope` definitions eliminated. `share/client.ts` non-throwing `data ?? []` pattern correctly preserved. |
| M2 W2b | `RawWorkspace` type genuinely shared. Two `mapWorkspace` functions map to different target types (correct). |
| M2 W2d-tiptap | Shared `workspace-html-sanitize.ts` properly used by both `citation-renderer.tsx` and tiptap editor. |
| M2 W2d-history | 10 pure string utilities cleanly extracted to `session-title-text.ts`. No side effects, properly typed. |
| General | No file crossed the 1000-line threshold due to M0-M2 changes. |

### What went wrong (issues introduced or missed)

| ID | Severity | Finding |
|----|----------|---------|
| M0-S1 | MEDIUM | "Pure deletion" commit added WIP stubs with 4 new `TODO(write-mode)` markers. `with_feature`/`with_observer`/`with_tenant` are identity passthroughs (`fn with_feature(self, _feature) -> Self { self }`) — shallow pass-through layers adding indirection without behavior. |
| M1-S1 | MEDIUM | `AnswerBlock` still on ts-rs. Python heredoc still patching generated `contracts.ts` to inject `import type { AnswerBlock }`. Fragile string manipulation on generated output. |
| M2-S1 | MEDIUM | **W2c zod refactor missed the code-judo move.** Schemas stored as `Record<string, z.ZodType>` (erases type inference) → forces `as ChatEvent` cast at return. `z.discriminatedUnion("event", [...])` would eliminate the cast, the `CHAT_EVENT_NAMES` Set, and the manual event-name validation in one move. Current approach gets runtime validation but none of zod's type-safety — worst of both worlds. |
| M2-S2 | MEDIUM | **W2d-history transcript fetch hook not extracted.** `listWorkspaceSessionMessages` still called at 2 separate sites (title derivation + search indexing) for the same session. Plan called for `use-session-transcripts.ts` hook with caching. |
| M2-S3 | LOW | `plainTextToHtml` still has inline `escapeHtml` in tiptap editor — a bespoke duplicate of a standard operation. Technically safe (different concern: text escaping vs HTML sanitization). |

---

## Part 2 — Current State of Prior Findings

### CRITICAL (3 remain, 1 fixed)

| ID | Status | Detail |
|----|--------|--------|
| **S1** `include!` flat-namespace | **REMAINS** | `storage-pg/src/lib_impl.rs`: 18 `include!` lines. `transport-http/src/lib_impl.rs`: 7 `include!` lines. Zero module boundaries. One fragment alone (`repository_sessions_jobs.rs`) is 1,044 lines in this flat namespace. |
| **B4** `StorageContext` god-bag | **REMAINS** | `app-core/src/storage_context.rs:25-45`: 19 fields (stores + caches + config + object-store settings). 18-positional-arg constructor with `#[allow(clippy::too_many_arguments)]`. |
| **D1** `run_document_pipeline` god function | **REMAINS** (renamed) | `bins/worker/src/pipeline/document_pipeline.rs:120-718`: ~598 lines (was 620). Renamed from `_inner`. No `Stage` trait exists. Still inlines 11 sequential stages in one body. |
| **D2** `IngestionError::StateSink` | **REMAINS & SPREAD** | `.map_err(\|e\| IngestionError::StateSink(e.to_string()))` now at **~54 call sites** across 8 files (was 32+ in 4 files). The typed enum buys nothing over `anyhow` — it's metastasized into a catch-all that erases all typed errors into Strings. |

### HIGH (8 remain, 2 worsened, 2 fixed)

| ID | Status | Detail |
|----|--------|--------|
| **S3** `reset.rs` handler logic | **REMAINS** | 537 lines. `PasswordResetConfig::from_env()` called 5× per-request. SMTP transport construction inlined in handler. |
| **S3** `billing/api.rs` | **REMAINS & GREW** | 510 lines (was 466, +44). `BillingConfig::from_env()` 3× per-request. Service layer wearing "api" costume. |
| **S4** Memory fallback inlining | **REMAINS** | `documents.rs` 868 lines. `if let Some(store) = storage.document_store() { ... } else { memory }` pattern ×11 occurrences. |
| **A2** ReActLoop param threading | **REMAINS** | `run_synthesis_phase`: 14 params. `finish_run`: 12 params. `finish_degraded_no_evidence_run`: 13 params. No context struct introduced. |
| **B3** `app-bootstrap` kitchen-sink | **WORSENED** | 7,898 lines (was 7,683, +215). New `adapters/` subdir added pass-through adapters rather than reducing the crate. |
| **C3** `ChatPersistencePort` | **WORSENED** | 22 methods (was 20). Kitchen-sink trait mixing sessions, messages, profiles, notifications, usage, assets, audit, chunks, summaries. Textbook ISP violation. |
| **D4** `draft_sections` params | **REMAINS** | 11 params, 3 positional booleans (`mpc`, `primed`, `one_sentence_per_line`). Creates 8 implicit code paths. |
| **D5** `processor.rs` god function | **REMAINS** | `process()` ~377 lines (was 415). Per-task client cloning throughout. |
| **E5** Dual analytics paths | **REMAINS** | `record_api_product_event_if_available` (6 calls, 3 files) vs `record_product_event_if_available` (33 calls, 11 files). Same concept, two APIs. |
| **F5** `rag_execute.rs` multi-concern | **REMAINS** | 626 lines. Validation engine + retrieval-prep policy + graph classification + compat conversion. **NEW**: misleading doc comment says "replaces `to_chat_request_compat()`" but the function still exists and is actively called. |
| **C6** `channels.rs` copy-paste | **FIXED** | Consolidated into `sse_sink.rs` (594 lines, one `map_event` function). |
| **E6** `llm/summary.rs` cache duplication | **FIXED** | Well-structured `SummaryGenerator`. No duplicated cache-check pattern. |

### MEDIUM (10 remain, 2 partially fixed, 1 fixed)

| ID | Status | Detail |
|----|--------|--------|
| **A3** Unified dispatch copy-paste | **REMAINS** | `unified/mod.rs:145-327`: Chat/Rag/Search arms ~50-75 lines each, duplicate mode-config-load + LLM-unavailable + `with_observer` + `ReActLoop::new`. |
| **A4** `token_budget/mod.rs` | **REMAINS** | 804 lines. `#[cfg(test)]` only at line 503. Front 502 lines are dev-only simulator with zero production callers, shipped in release binary. |
| **A5** Retired-skills denylist | **REMAINS** | `registry.rs:133-157`: hardcoded 20-entry `matches!` denylist. No data-driven source. |
| **B6** `pg_auth_store.rs` | **PARTIAL** | Renamed to `repository_auth_user.rs`, 588→335 lines. But read-only SELECTs still pointlessly wrapped in super-admin transactions (22 `begin/commit` across 11 methods). |
| **C2** Pass-through adapters | **REMAINS** | `PgContentStore` (124L) + `PgChatPersistenceAdapter` (315L) = pure delegation. `IndexedChunk` defined 3× (2 live, 1 dead). |
| **C4** `RetrievalDataPlane` | **REMAINS** | 9 methods, 4 return `Err("not implemented")` defaults. Fat trait with runtime panics. |
| **S5** Guard ceremony | **REMAINS** | 12× `auth_store` guard, 26× `forbid_api_key`, 4 error-response entry points. |
| **E7** `auth` crate pass-through | **REMAINS** | 2-line `pub use contracts::auth_runtime::*`. Build-graph node for ~12 dependents adding zero value. |
| **D3** `experiment.rs` copy-paste | **PARTIAL** | 494 lines (was 819, -40%). 3 copy-paste arms remain (A/B/BLines differ only in boolean params). |
| **D6** `fingerprint_workspace` | **FIXED** | 1 definition, all others are imports. |
| **H5** Desktop `lib.rs` inline commands | **REMAINS** | 11 `#[tauri::command]` functions inline. `commands/` subdir contains only helpers. Dual error contract (`IpcApiError` vs `String`). |

---

## Part 3 — New Findings

### NEW-1. [HIGH] Dead `Write` arm in UnifiedAgent — architectural seam

`avrag-rs/crates/app-chat/src/agents/unified/mod.rs:328-331`:
```rust
AgentKind::Write => Err("write_mode_not_implemented"),
```
But Write **is** implemented — `pipeline_steps.rs:57` routes `AgentKind::Write` directly to `crate::writer::run_write_mode`, **bypassing `UnifiedAgent` entirely**. The "UnifiedAgent" is only 3/4 unified. The `Write` arm is a dead, misleading error stub that claims a capability the system has. Either `UnifiedAgent` should delegate Write to `WriterOrchestrator`, or the arm should be removed and the routing documented.

### NEW-2. [MEDIUM] Orphaned dead file `outcomes.rs` — M0 missed

`avrag-rs/crates/storage-pg/src/lib_impl/outcomes.rs` (94 lines) defines `IndexedChunk` and `DocumentScopeState` — duplicates of live definitions in `errors_and_mappers.rs`. `outcomes.rs` is **not included** by `lib_impl.rs` and not referenced by any `mod`/`use`. M0's 13k-line dead-code sweep missed this.

### NEW-3. [MEDIUM] Dev-only simulator shipped in production

`avrag-rs/crates/app-chat/src/agents/token_budget/mod.rs` lines 1-502: "TokenBudgetSimulator — offline development analysis" with zero production callers, yet `pub mod token_budget` compiles it into the release binary. Should be `#[cfg(any(test, feature = "dev-tools"))]` or moved to a dev crate.

### NEW-4. [MEDIUM] `notebooks/share.rs` — 600 lines of inlined handlers

`avrag-rs/crates/transport-http/src/handlers/workspaces/share.rs`: 10 handlers (create/revoke/get/update/validate/access-level/analytics/logs/token/api-keys) with access-control + business logic inlined per handler. Same S3 pattern as `reset.rs` and `billing/api.rs`. Candidate for a `ShareService`.

### NEW-5. [CRITICAL] 102 typecheck errors from WIP admin components

Untracked WIP files in `frontend_next/components/admin/` have **102 TypeScript errors** — all from missing i18n keys. Components call `adminMessage(locale, "common.loading")`, `"accounts.subtitle"`, etc., but these keys don't exist in any message file. The WIP code is uncompilable.

Root cause: Two parallel i18n systems in the admin layer (`INLINE_COPY` table in `admin-i18n.ts` vs `UI_MESSAGES` in `lib/i18n/messages/`). WIP surfaces bypass `adminText` (which reads `INLINE_COPY`) and use `adminMessage` → `formatUiMessage` (which reads `UI_MESSAGES`) with keys that exist in neither.

### NEW-6. [HIGH] `e2e-precheck.sh` `docker rm -f` — M0 should have fixed this

`scripts/e2e-precheck.sh:34-36` still has `docker rm -f avrag-test-pg-*` lines. The M0 plan explicitly called for removing these (user confirmed decision #3: "测试容器复用"). This directly contradicts `AGENTS.md` §"Test PostgreSQL Containers" which says "intentionally left running for performance, DO NOT prune."

### NEW-7. [HIGH] `workspace-note-editor-tiptap.tsx` still imports wrong CSS module

`workspace-note-editor-tiptap.tsx:12` still imports from `./workspace-right-rail.module.css`. A note editor importing right-rail styles is a naming/ownership boundary violation. M2 reduced the file size but did not fix the CSS module ownership.

### NEW-8. [LOW] `shared-workspace-surface.tsx` double cast

`frontend_next/components/share/shared-workspace-surface.tsx:176`: `event.payload as unknown as ChatResponse`. A type boundary that should be made explicit.

### NEW-9. [LOW] `tool-result-card.tsx` — only `: any` in frontend codebase

`frontend_next/components/workspace/tool-result-card.tsx:238`: `(r: any, i: number)` in a `.map()` callback.

### NEW-10. [LOW] `help/write/page.tsx` — inline styles + hardcoded bilingual strings

`frontend_next/app/(app)/help/write/page.tsx`: 92 lines of inline styles on every element, hardcoded `locale === "zh-CN" ? ... : ...` ternaries mixed with `formatUiMessage` calls. Inconsistent i18n approach.

---

## Part 4 — WIP Code Quality (new modules)

| File | Lines | Assessment |
|------|-------|------------|
| `pipeline/compensation.rs` | 74 | **GOOD** — Clean compensating-transaction log. Deep module: `push()` + `rollback()`, LIFO execution. |
| `pipeline/degradation.rs` | 36 | **GOOD** — Minimal `DegradationPolicy` enum. Small, focused. |
| `pipeline/predicate_normalize.rs` | 93 | **GOOD** — Clean normalization with synonym table + tests. |
| `ingestion/format_registry.rs` | 189 | **EXCELLENT** — Consolidates 4 duplicate mapping sites into one `FormatFamily` registry. Gold standard for what M0-M2 refactoring should look like. |
| `app-billing/usage_observer_impl.rs` | 102 | **GOOD** — Clean adapter. Minor: 2× `if let Err(e) = result { tracing::warn!(...) }` could be a helper. |
| `app-chat/writer/material_pack.rs` | 320 | **GOOD** — Clean data type with builder methods. Minor: `render_appendix_zh()` is prompt-rendering in a data module. |
| `app-chat/writer/refine_loop/` | 1,937 (6 files) | **GOOD** — Properly modularized. Correctly imports `fingerprint_workspace` from `heavytail` rather than copy-pasting. Good module boundaries. Watch for A2 param-threading pattern in `mod.rs:run`. |

---

## Part 5 — Prioritized Remediation

### Immediate (blocks compilation / destroys infrastructure)

1. **NEW-5**: Fix 102 typecheck errors — add missing i18n keys or switch admin surfaces to `adminText`/`INLINE_COPY`. The WIP code is uncompilable.
2. **NEW-6**: Remove `docker rm -f` from `e2e-precheck.sh:34-36`. M0 plan item missed.

### Phase 1: Delete remaining dead weight (~400 lines)

3. **NEW-2**: Delete orphaned `storage-pg/src/lib_impl/outcomes.rs` (94 lines, not included by any `mod`).
4. **NEW-3**: Gate `token_budget/mod.rs` lines 1-502 behind `#[cfg(test)]` (502 lines of dev-only code in production binary).
5. **NEW-1**: Remove dead `Write` arm from `UnifiedAgent` or wire it through properly.
6. **E7**: Delete `auth` crate (2-line pass-through) — force callers to `use contracts::auth_runtime`.

### Phase 2: Fix the code-judo missed in M2

7. **M2-S1**: Replace `Record<string, z.ZodType>` + `as ChatEvent` cast in `stream.ts` with `z.discriminatedUnion("event", [...])`. Eliminates cast + `CHAT_EVENT_NAMES` Set + manual event validation in one move.
8. **M2-S2**: Extract `useSessionMessages(sessionId)` hook in history pane to consolidate 2× `listWorkspaceSessionMessages` fetches.
9. **NEW-7**: Fix tiptap CSS module — create `workspace-note-editor.module.css` or rename shared module.
10. **M1-S1**: Investigate whether `AnswerBlock` can use `#[serde(tag = "type", content = "content")]` (adjacent tag) to enable typeshare, or document why ts-rs is permanently required.

### Phase 3: Kill the `include!` pattern (M3 — critical path)

11. **S1**: Convert `transport-http/lib_impl.rs` 7 `include!` → real `mod` tree.
12. **S1**: Convert `storage-pg/lib_impl.rs` 18 `include!` → `mod` tree. Split `PgAppRepository` by aggregate.

### Phase 4: Extract business logic from handlers (M4)

13. **S3**: `PasswordResetService` (config read once at bootstrap, handlers → ~10-line controllers).
14. **S3**: `BillingService` (rename `api.rs` → `service.rs`, inject config at construction).
15. **NEW-4**: `ShareService` (10 handlers in `notebooks/share.rs` → thin controllers).
16. **S4**: `Memory*Store` port impls (delete dual-backend inlining from 11 methods in `documents.rs`).

### Phase 5: Collapse remaining copy-paste (M5)

17. **D2**: Split `IngestionError::StateSink(String)` into typed categories (`Storage`, `Parse`, `Security`, `Index`) with `From` impls. Delete ~54 `.map_err(|e| StateSink(e.to_string()))` sites.
18. **D1**: Define `trait Stage { async fn run(&mut self, ctx) }`. Pipeline becomes `for stage in stages { stage.run(&mut ctx).await? }`.
19. **A2**: Introduce `RunFinalizer` struct bundling 14 params.
20. **B4**: Decompose `StorageContext` (19 fields) into focused store references.
21. **D4**: Introduce `DraftOptions { mpc, primed, one_sentence_per_line, ... }` with `Default`.
22. **NEW-7**: Decompose desktop `lib.rs` — move 11 `#[tauri::command]` functions into `commands/` submodules.

---

## Statistics

| Metric | Value |
|--------|-------|
| Crates/areas audited | 35+ Rust crates, 1 TS frontend, 27 scripts, desktop |
| CRITICAL findings | 4 (3 prior remains + 1 new WIP) |
| HIGH findings | 10 (8 prior remains + 2 new) |
| MEDIUM findings | 14 (10 prior + 4 new) |
| LOW findings | 3 (new) |
| Prior findings fixed | 4 (C6, D6, E6, B6-partial) |
| Prior findings worsened | 3 (B3, C3, D2) |
| New WIP modules (good quality) | 7 |
| Deletable dead code remaining | ~600 lines (outcomes.rs + token_budget dev half + auth crate) |
| Files over 1k lines | 2 (in tests, not M0-M2) |
| WIP typecheck errors | 102 (all from untracked files) |
| Verdict | ❌ NOT APPROVED |

---

## What M0-M2 Achieved

- **13,021 lines deleted** (dead code, dead scripts, dead files)
- **F1 CRITICAL fixed**: `agent_operation_guide` contract drift eliminated. TS golden test guards against recurrence.
- **Frontend duplication collapsed**: `ApiEnvelope`/`unwrapApiData` unified, notebook mapper shared, DOMPurify shared, string utilities extracted. -265 lines net.
- **No regressions**: zero new test failures, zero typecheck errors in modified tracked files.

## What M0-M2 Did NOT Achieve

- **Zero structural CRITICAL findings addressed** (S1 include!, B4 StorageContext, D1 god function, D2 StateSink all remain)
- **2 findings worsened** (B3 app-bootstrap grew +215 lines, C3 ChatPersistencePort grew +2 methods)
- **1 HIGH plan item missed** (e2e-precheck.sh `docker rm -f` — user-confirmed decision not executed)
- **1 code-judo move missed** (W2c zod: used `Record<string, z.ZodType>` instead of `discriminatedUnion`, forcing a cast)
- **1 hook extraction missed** (W2d-history: transcript fetch consolidation not done)
- **M0 introduced 4 new TODO markers** in a "pure deletion" commit (WIP stubs for write-mode)

The debt remains **concentrated, not systemic**. The new WIP modules (`compensation.rs`, `degradation.rs`, `format_registry.rs`, `refine_loop/`) are genuinely well-built. The rot clusters around the same patterns as the prior review — `include!` god-objects, inlined memory fallbacks, handler-embedded infra, and `StateSink` stringly-typed errors — each of which has a clear code-judo move that deletes complexity rather than rearranging it.
