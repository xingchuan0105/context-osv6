# Thermo-Nuclear Code Quality Review — 2026-07-08

**Scope:** Full codebase excluding `frontend_rust/`. 8 parallel deep audits covering `app-chat`, app-layer crates (`app-core`/`app-documents`/`app-admin`/`app-billing`/`app-bootstrap`/`app`), RAG/retrieval/storage stack (`rag-core`/`rag-core-ports`/`retrieval-data-plane`/`storage-*`/`search`/`chatmemory`), ingestion/worker/heavytail, `transport-http` + cross-cutting crates, `contracts` + codegen pipeline, `frontend_next`, and `scripts`/`desktop`/tooling.

**Verdict: ❌ NOT APPROVED.** The codebase works, but carries significant structural debt concentrated in a small number of high-leverage patterns. This review identifies the code-judo moves that would make the architecture dramatically simpler.

---

## Part 1 — Systemic Themes (highest leverage)

These patterns recur across multiple crates. Fixing them once pays dividends everywhere.

### S1. [CRITICAL] `include!` flat-namespace collapses module boundaries

**Where:** `transport-http/src/lib_impl.rs` (7 files pasted into crate root), `storage-pg/src/lib_impl.rs` (24 files pasted into crate root).

**Problem:** `include!` textually concatenates files into one flat compilation unit. There are no modules, no `pub(crate)` boundaries, no privacy enforcement. IDE go-to-definition and "what depends on X" are untraceable. This is the **root enabler** of the duplication and god-objects below — you can't see the rot because there are no walls.

**Evidence:**
- `transport-http`: `reset.rs` references `AuthEnvelope`, `handlers::error_response`, `CreatePasswordResetTicketInput` with no module path. `router_core.rs` defines free functions that `auth_primary.rs`/`reset.rs` call bare.
- `storage-pg`: `PgAppRepository` becomes a **99-method god-object** with `impl` blocks spread across 24 include'd files, whose names prove the decomposition is incoherent (`repository_retrieval_lifecycle.rs` contains zero retrieval — it's document mutations; `repository_bootstrap.rs` is ingestion writes + infra).

**Code-judo move:** Convert each `include!` target into a real `mod`. For `storage-pg`, split `PgAppRepository` by aggregate (`DocumentRepository`, `ChunkRepository`, `SessionRepository`, `IngestionQueueRepository`, `AssetRepository`…), each holding `Arc<TenantPgPool>`. This converts a 99-method god-object into ~8 focused structs and restores module-private visibility.

---

### S2. [CRITICAL] ~1,750 lines of dead/vestigial speculative infrastructure

Three separate deposits of speculative architecture that the production server never uses:

| Location | Lines | What's dead |
|----------|-------|-------------|
| `app-chat/src/agents/replay.rs` | 725 | Full version-compatibility matrix, SemVer parser, snapshot builder, replay engine. `build_run_result` always sets `snapshot: None`. Zero call sites. |
| `app-chat/src/agents/audit.rs` | ~400 | 6 of 7 record-builder functions, both storage traits, `InMemoryAuditStorage`, `AuditLifecycleManager` — never referenced outside own file+tests. |
| `app/src/services/` + `app/src/runtime/` | ~700 | Parallel "secure service" architecture (`SecureSearchService`/`SecureStorageService` with underscore-prefixed unused deps returning hardcoded stubs), `XmlSlotEngine` (318 lines, self-referenced only), `Runtime`/`ServiceRegistry` (test-only). `bins/api` imports only `AppConfig`/`AppState` (pure re-exports of `app_bootstrap`). |

**Code-judo move:** Delete all three. If any piece is genuinely planned, gate behind a feature flag with a one-line rationale. ~1,750 lines deleted, zero behavior change. These mislead readers into thinking systems exist that don't.

---

### S3. [CRITICAL] Business logic + per-request infra construction inside HTTP handlers

**Where:** `transport-http/src/lib_impl/auth/reset.rs` (537 lines), `billing/src/api.rs` (466 lines), `transport-http/src/lib_impl/auth_primary.rs`, `transport-http/src/lib_impl/auth/profile.rs`.

**Problem:** Handlers own SMTP config + email sending (`PasswordResetConfig::from_env()` re-read on **every request**), bcrypt hashing, provider dispatch, pending-order DB inserts, Alipay client construction, 3-provider signature verification, lease-based idempotency. The "api.rs" in billing isn't even HTTP handlers — it's a service layer wearing an "api" costume, with `BillingConfig::from_env()` per call.

**Code-judo move:** Create app-layer services (`PasswordResetService`, `BillingService`) constructed once at bootstrap (config read once, clients injected). Handlers become ~10-line thin controllers. Deletes ~350 lines of handler-embedded infra per area.

---

### S4. [HIGH] Memory-fallback backend inlined into ~30 service methods instead of being a port impl

**Where:** `app-documents/src/documents.rs` (868 lines, ~50% duplication), `app-admin/src/admin_context.rs`, `app-documents/src/notebooks.rs` (5×), `url_imports.rs` (2×). 30 occurrences across 6 files.

**Problem:** Every method is shaped:
```rust
if let Some(store) = storage.document_store() {
    /* postgres path ~30 lines */ return …;
}
/* memory path: storage.inner().write().await … ~30 lines */
```
The in-memory backend is hand-inlined, duplicating auth/org-id guards, status guards, event recording. This is the single reason `documents.rs` is 868 lines.

**Code-judo move:** Implement `MemoryDocumentStore: DocumentStorePort` (precedent exists: `app-core/src/adapters/memory.rs:8` `MemoryWorkspaceStore`). Then every service method collapses to a single `store.method(…)` call and the `else` branch disappears from ~30 methods. Likely halves `documents.rs`. This is unfinished architecture, not a style issue.

---

### S5. [HIGH] Guard/error/response ceremony scattered via copy-paste, not abstraction

Three flavors of the same anti-pattern:

| Pattern | Occurrences | Fix |
|---------|-------------|-----|
| `ensure_ingestion_side_effects_allowed()` + `.map_err(\|e\| StateSink(e.to_string()))` | 13× in worker pipeline | `processor.guarded_write(ctx, task, doc_id, label, \|repo\| async { … })` |
| `let Some(store) = state.auth_store() else { return error_response(503...) }` | 12× in transport-http auth | axum extractor `RequireSession(State) -> (&AppState, &dyn AuthStore, UserId)` |
| `forbid_api_key` → `actor_id` unwrap → `auth_store` check | 7× in profile handlers | Same extractor |
| Three incompatible error JSON shapes (`{error,message}` / `{ok,data,error}` / `{success,data,error}`) + duplicated `AppError→response` mapping | transport-http + admin | One `IntoResponse` for `AppError` in `common`; pick one envelope |

---

## Part 2 — Per-Area Findings

### A. `app-chat` (agent runtime) — NOT APPROVED

**Top blockers:**
1. **[CRITICAL]** `replay.rs` (725) + `audit.rs` (~400) = ~1,050 lines dead speculative code (see S2).
2. **[HIGH]** Parameter-threading: 6 `ReActLoop` terminal-phase methods each forward 10–17 unchanged parameters through 3–4 call hops. Introduce a `RunFinalizer`/`LoopEpilogue` struct bundling `{iteration, max_iterations, total_tool_calls, telemetry_records, total_usage, reasoning_summary_acc, start_time}`.
3. **[HIGH]** `unified/mod.rs`: Chat/Rag/Search dispatch arms are ~60-line copy-paste blocks (differ only in LLM client field + runtime attachment). Extract `run_react_mode(mode_id, llm, configure_closure, request, sink)` → ~150 lines become ~50.
4. **[HIGH]** `token_budget/mod.rs` (804) is an offline dev simulator re-implementing prompt assembly with hardcoded strings that drift from real prompts. Move behind `#[cfg(test)]` or extract to dev crate.
5. **[HIGH]** Hardcoded 20-entry retired-skills denylist (`is_retired_skill()`) duplicates the existing `deprecation` frontmatter mechanism.

**Well-structured (keep):** `answer_contract.rs` parse/validate/resolve pipeline; `capability/policy.rs` `EnforcementCondition` DSL; `chat_private/profile_merge.rs` confidence arithmetic; `ReActLoop` builder pattern.

---

### B. App-layer crates (`app-core`/`app-documents`/`app-admin`/`app-billing`/`app-bootstrap`/`app`) — NOT APPROVED

**Top blockers:**
1. **[CRITICAL]** Dual-backend inlining (see S4) — 30 methods with inlined memory fallback.
2. **[HIGH]** `app` crate vestigial parallel architecture (~700 lines, see S2).
3. **[HIGH]** `app-bootstrap` is a 7,683-line kitchen-sink: wiring + ALL Pg SQL adapters + `AppState` god-facade in one crate. Billing alone is spread across 8 locations. Extract `adapters/` into a dedicated `app-storage-pg` crate.
4. **[HIGH]** `StorageContext` god-bag: 19 fields mixing infra + 6 domain stores + mutable in-memory state + crypto helpers. 18-positional-arg constructor duplicated in `new_memory` and `bootstrap`.
5. **[HIGH]** `postgres_delegates.rs`: E2E test helpers (`reset_e2e_user_data`, `grant_e2e_admin_role`) + auth-bypass flag (`jwt_auth_version_matches` silently returns `true` when `AVRAG_AUTH_VERSION_BYPASS=true`) bolted onto production `AppState`.
6. **[MEDIUM]** `pg_auth_store.rs` (588): read-only SELECTs pointlessly wrapped in super-admin transactions; duplicated legal-acceptance SQL; domain logic leaked into adapter.
7. **[MEDIUM]** Stateless "context" method-bags (`AdminContext;` is a unit struct) + AppState delegate re-exposure = 3 layers of indirection with zero encapsulation.

---

### C. RAG / retrieval / storage stack — NOT APPROVED

**Top blockers:**
1. **[CRITICAL]** `PgAppRepository` 99-method god-object via `include!` (see S1).
2. **[HIGH]** Two pure pass-through port adapters (`PgContentStore`, `PgChatPersistenceAdapter`) + byte-identical duplicate `IndexedChunk` struct. ~290 deletable lines once types unify.
3. **[HIGH]** `ChatPersistencePort` is a 20-method kitchen sink (sessions, messages, profiles, notifications, usage, assets, audit, chunks, summaries) overlapping `ContentStore`. Port layer fragmented across 3 crates with misleading names.
4. **[HIGH]** `RetrievalDataPlane` fat trait mixes read + write + schema with "not implemented" default bodies — the tell that the trait is too wide. Split into `RetrievalReadPort` + `RetrievalIndexPort`.
5. **[MEDIUM]** Dead code: ~100 lines of PG full-text search (`search_chunks_text`/`search_chunks_bm25`) with zero callers — residue from when PG was a retrieval backend.
6. **[MEDIUM]** `channels.rs`: 3 copy-pasted channel runners (~45 lines each) + 4 identical output structs. One generic `run_channel<Fut, T>()` collapses ~150→~40 lines.

**Well-structured (keep):** `rag-core` runtime (`execute_plan` parallelizes 4 channels via `tokio::join!`, clean `ChunkBudgetTracker`); Milvus-specific concepts correctly contained in `storage-milvus`; `bridge.rs` is cohesive (not a god-object); `chatmemory` is clean.

---

### D. Ingestion / worker / heavytail — NOT APPROVED

**Top blockers:**
1. **[CRITICAL]** `run_document_pipeline_inner` is a 620-line god-function (`#[allow(clippy::too_many_lines)]`) inlining 11 sequential stages with no `Stage` abstraction. Define `trait Stage { async fn run(&mut self, ctx) }`; pipeline becomes `for stage in stages { stage.run(&mut ctx).await? }`.
2. **[HIGH]** `IngestionError::StateSink(String)` is a stringly-typed catch-all used 32+× via `.map_err(|e| StateSink(e.to_string()))`. The typed enum buys nothing over `anyhow`. Introduce real categories (`Storage`, `Parse`, `Security`, `Index`) with `From` impls, or collapse to `Processing(anyhow::Error)`.
3. **[HIGH]** `heavytail/src/bin/experiment.rs` (819): library-grade logic in a bin + 4 copy-paste arm-dispatch blocks. Move logic to lib; replace arms with a table loop.
4. **[HIGH]** `draft_sections` takes 12 params with 3 positional booleans (`true, true, None, false, None` — unreadable). Introduce `DraftOptions { mpc, primed, one_sentence_per_line, persona, … }` with `Default`.
5. **[HIGH]** `processor.rs::process` (415 lines): whole body in one `tokio::time::timeout`; duplicated finish-run branches; re-clones all 6 LLM/embedding clients with usage observer on **every task** (per-task self-mutation).
6. **[MEDIUM]** `fingerprint_workspace` copy-pasted 5× across heavytail modules with 3 different names.

**Well-structured (keep):** `format_registry.rs` (right consolidation, just needs probe/chunker wired through it); `chunker.rs` `split_and_coalesce` extraction; `ir_validator.rs` (checks invariants types genuinely can't encode); `heavytail/compiler.rs` `Directive` enum.

---

### E. `transport-http` + cross-cutting — NOT APPROVED

**Top blockers:**
1. **[CRITICAL]** `include!` flat-namespace (see S1).
2. **[HIGH]** Business logic in handlers + per-request `from_env()` (see S3).
3. **[HIGH]** Guard duplication + triplicated error shaping (see S5).
4. **[HIGH]** `billing/api.rs` is service logic mislabeled "api" (see S3).
5. **[HIGH]** Two parallel analytics-recording paths: free function `record_api_product_event_if_available` (manually reconstructs `ProductEvent`, hardcodes `Surface::Api`, `client_platform:"web"`) vs `AppState::record_product_event_if_available` method. Guaranteed to diverge.
6. **[MEDIUM]** `llm/src/summary.rs`: duplicated cache-check-or-call-LLM pattern (batch vs finalize, ~20 lines each).
7. **[MEDIUM]** `auth` crate is a 2-line pure re-export pass-through (`pub use contracts::auth_runtime::*`) adding a build-graph node for ~12 dependents.

**Well-structured (keep):** `llm/client/mod.rs` (`LlmClient` earns its keep); `code-interpreter` (clean fd3/fd4 IPC, `sandbox_bootstrap` single source of truth); `guardrails` (properly decomposed, no file >236 lines); `telemetry/prometheus.rs` (metric-count-driven, macros collapse boilerplate).

---

### F. `contracts` + codegen pipeline — NOT APPROVED (2 audits, CRITICAL still open)

**Top blockers:**
1. **[CRITICAL]** `ChatResponse.agent_operation_guide` silently dropped from TS contract. Root cause: `ToolSpec` (`tool_call.rs:10`) has no `#[typeshare]`, so typeshare silently drops `AgentOperationGuide` and the field. `grep AgentOperationGuide contracts.ts` → 0 matches. Governance check (`check_contract_governance.sh`) only guards Rust-side dedup — **zero CI coverage** for TS-codegen completeness. **Untouched across two audits.**
2. **[HIGH]** Orphaned source-mutating Python patchers (`patch-chat-contract-codegen.py`, `annotate-contract-typeshare-integers.py`) — no longer invoked but still sitting in `scripts/`, their injected output frozen into source. Running them would re-corrupt `chat.rs`.
3. **[HIGH]** 92 redundant `#[typeshare(serialized_as = "number")]` annotations contradicting `typeshare.toml` `[typescript.type_mappings]`. Two mechanisms for the same job, unreconciled.
4. **[MEDIUM]** `generate-contracts.sh` still does fragile inline Python heredoc + sed surgery on generated TS (injecting `AnswerBlock` import, rewriting `chat_event.ts`). Give `AnswerBlock` a `#[typeshare]` and delete the bridge.
5. **[MEDIUM]** `rag_execute.rs` layering violation deepened: contract crate now hosts a validation engine (9 error variants), retrieval-prep policy (reorder/truncate), and graph-triplet classification — all runtime logic, not wire shapes.

**Code-judo move (fixes 1+3+4 simultaneously):** Standardize on ONE codegen tool. Add `#[typeshare]` to `ToolSpec` + `AnswerBlock`. Add a TS-side golden test asserting full key-set completeness (`expect(Object.keys(loadFixture<ChatResponse>(...)).sort()).toEqual([...])`). Delete the Python patchers, the sed surgery, and the 92 redundant annotations.

---

### G. `frontend_next` (Next.js/TS) — NOT APPROVED

**Top blockers:**
1. **[HIGH]** Three copies of `ApiEnvelope<T>` + two copies of `unwrapApiData` across `admin/client.ts`, `share/client.ts`, `settings/client.ts`. `share` even has a second divergent shape (`success`/`data`/`error`). One `requestEnvelope<T>()` in `lib/http/request.ts` collapses all.
2. **[HIGH]** Two divergent notebook→workspace remappers (`dashboard/client.ts` `mapWorkspace` vs `workspace/client.ts` `remapWorkspace` + inline spreads). One shared `mapWorkspace(raw): Workspace`.
3. **[HIGH]** `workspace-note-editor-tiptap.tsx` (852 lines): hand-rolled HTML sanitizer (~105 lines) reimplementing `dompurify` which is **already a dependency** and used canonically in `citation-renderer.tsx`. Plus link-panel geometry, toolbar state, editor wiring. Also imports CSS from `workspace-right-rail.module.css` (wrong module). Decompose → <400 lines.
4. **[HIGH]** `workspace-history-pane.tsx` (743): fetches the same transcript **3× per session** (title-derivation, title-sync, search-indexing) via chained effects + `Set` guards. ~120 lines of inline pure string utils. Extract `use-session-transcripts.ts` (one fetch, derive both).
5. **[HIGH]** SSE parser (`stream.ts:210-223`) erases generated contract types: rebuilds every field by hand (`String(raw.doc_id ?? "")`) then double-casts `citations as unknown as Array<Record<string, unknown>>`. `zod` is already a dependency but unused. Define zod schemas per `ChatEvent` variant; delete the coercion block.

**Well-structured (keep):** Shared `lib/http/request.ts` (cleanly used by all clients); `lib/runtime/transport.ts` (web vs Tauri split); `right-rail-queries.ts` (clean react-query); `ui-store.ts` (coherent persisted store); `model.ts` (tidy domain model); i18n dictionaries (pure, no logic). `any` is essentially absent from app code.

---

### H. Scripts / desktop / tooling — NOT APPROVED

**Top blockers:**
1. **[HIGH]** 5 dead git-history-rewriting scripts (`_commit-prs.sh`, `_continue-reword.sh`, `_reword-task23.sh`, `_recommit-pr5-pr6.sh`, `pricing-revamp-commits.sh`) — hardcode dead SHAs/`/tmp` files, unreferenced, 3 not even executable. DELETE.
2. **[HIGH]** `e2e-precheck.sh:34-36` forcibly destroys perf-cached test containers (`docker rm -f avrag-test-pg-*`) — directly contradicts `AGENTS.md` §"Test PostgreSQL Containers" which says "intentionally left running for performance, DO NOT prune." They cannot both be right.
3. **[HIGH]** ~105 tracked dead files: `prompts/{_backups,_drafts,deprecated,legacy}` (98 files) + `python/avrag_sdk/{egg-info,benchmark_output}` (7 build artifacts). `git rm -r --cached` + `.gitignore`.
4. **[HIGH]** Regex source-mutation patchers committed with no tests (also flagged in contracts section F).
5. **[MEDIUM]** `desktop/src-tauri/src/lib.rs`: 11 `#[tauri::command]` fns inline in lib.rs while `commands/` contains only helpers (misnamed); `api_call` returns `Result<_, IpcApiError>` while every other command returns `Result<_, String>` (two error contracts).

**Well-structured (keep):** `activate-rust-cache.sh`, `rust-disk-hygiene.sh` (allow-list guard before `rm -rf`, dry-run), `wsl-vhd-compact.ps1` (StrictMode, admin check); Tauri command layer (well-tested, no backend-coupling leak); Python SDK (typed async, proper error taxonomy).

---

## Part 3 — Prioritized Remediation Plan

Ordered by leverage (deletable complexity per hour of effort):

### Phase 1: Delete dead weight (~2,000 lines, zero behavior change)
1. Delete `app-chat/agents/replay.rs` (~650 lines) + unused audit builders (~400 lines)
2. Delete `app/src/services/` + `app/src/runtime/` vestigial architecture (~700 lines)
3. Delete 5 dead rebase scripts + `git rm` ~105 tracked dead files
4. Delete dead PG FTS search methods (~100 lines)
5. Delete orphaned Python patchers (after confirming annotations are in source)

### Phase 2: Fix the contract boundary (prevents silent production bugs)
6. Add `#[typeshare]` to `ToolSpec` + `AnswerBlock` → fixes CRITICAL codegen drift
7. Add TS-side golden test for full key-set completeness
8. Reconcile 92 redundant integer annotations vs `typeshare.toml`

### Phase 3: Kill the `include!` pattern (unlocks everything else)
9. Convert `transport-http/lib_impl.rs` → real `mod` tree
10. Convert `storage-pg/lib_impl.rs` → split `PgAppRepository` by aggregate

### Phase 4: Extract business logic from handlers/wiring
11. `PasswordResetService`, `BillingService` (config read once at bootstrap)
12. `Memory*Store` port impls (delete dual-backend inlining from ~30 methods)
13. Extract `app-bootstrap/adapters/` into dedicated storage crate

### Phase 5: Collapse copy-paste
14. Unified error/response shaping + auth extractor in transport-http
15. `Stage` trait for worker pipeline + `IngestionError` categories
16. `requestEnvelope<T>()` + unified notebook mapper in frontend
17. Decompose god-components (`workspace-note-editor-tiptap.tsx`, `workspace-history-pane.tsx`)
18. SSE parser zod schemas (make generated types load-bearing)
19. Generic `run_channel()` in rag-core channels.rs

---

## Statistics

| Metric | Value |
|--------|-------|
| Crates/areas audited | 35+ Rust crates, 1 TS frontend, 27 scripts, desktop |
| CRITICAL findings | 6 |
| HIGH findings | 25 |
| MEDIUM findings | 18 |
| Deletable dead code | ~2,000 lines |
| Files approaching/over 1k lines | 8 (all flagged for decomposition) |
| Verdict | ❌ NOT APPROVED |

The good news: the debt is **concentrated, not systemic**. The RAG runtime, LLM client, code-interpreter, guardrails, frontend data layer, and most scripts are genuinely well-built. The rot clusters around a few patterns — `include!` god-objects, inlined memory fallbacks, handler-embedded infra, and the codegen pipeline — each of which has a clear code-judo move that deletes complexity rather than rearranging it.
