# Thermo-Nuclear Code Quality Review — 2026-07-09 (post WIP merge)

**Scope:** Full monorepo **excluding** deprecated Rust frontend (`frontend_rust/`, archived `avrag-rs/crates/web-ui` / `web-sdk`).  
**Includes:** `avrag-rs/` (API/worker/domain), `frontend_next/`, `contracts/`, `desktop/src-tauri/`, `scripts/`.  
**Baseline:** Master `634906c` after TN M0–M5 + Phase1–3 + WIP slices W1–W7 + UsageObserver follow-ups (PR #2–#11).

**Method:** Re-measure every prior CRITICAL/HIGH claim against the live tree; re-probe dual paths, ports, metering, pipeline shape, and new debt introduced by exit metering / write refine / admin merge.

---

## Verdict: ❌ NOT APPROVED

Progress since the morning `THERMO_NUCLEAR_REVIEW_2026-07-09.md` is **real and large**:

| Area | Morning status | Now |
|------|----------------|-----|
| Frontend admin tsc | **96 errors** CRITICAL | **0 errors** (`tsc --noEmit` green) |
| `avrag-auth` pass-through | In progress | **Deleted** |
| `token_budget` in prod lib | HIGH (shipped simulator) | **Gated** `cfg(any(test, feature = "dev-tools"))` |
| Write mode | Half-finished park | **Default WriteRefine** + observer-aware `WriterLlm` |
| Exit metering | Incomplete design | **Landed** observer + worker/write wiring + e2e |
| ChatPersistence ISP | 22-method sink | **Split traits exist** (`SessionPort`…); supertrait remains for wiring |
| Share handler | ~527L fat | **~294L** (thinner, not done) |
| `StateSink(String)` | 67 sites | **Variant rename** → `Storage`/`Parse`/… still **string-erased** |
| Dual memory/PG | ×19 dual branches | **Still structural** — memory mode has **no** `ChatPersistencePort` adapter |

Thermo-nuclear bar is **not** “did we land a lot of PRs.” The bar is: no clear structural regression, no obvious dual-control-flow tax, no kitchen-sink boundaries that force every feature to re-learn the same special cases.

Those remain. **Do not approve.**

---

## Part 0 — Do not re-litigate (fixed or clearly improved)

1. **Admin i18n / tsc red** — FIXED. `adminMessage` is a deprecated alias of typed `adminText` / `INLINE_COPY`; `pnpm exec tsc --noEmit` → 0 errors.
2. **Auth pass-through crate** — FIXED. `crates/auth` gone; callers use `contracts::auth_runtime`.
3. **SSE zod cast / tiptap CSS** — FIXED (prior M2.5).
4. **StorageContext 19-arg god-bag** — FIXED into groups + `StorageContextParts`.
5. **`include!` flat namespace** in storage-pg / transport-http core — FIXED (bootstrap billing/admin still has residual `include!` shards — see H5).
6. **WriteRefine default-on + phase tags** — IMPROVED. `WriterLlm::with_phase` sets `feature=write:<phase>` + `stage`.
7. **UsageObserver exit metering** — IMPROVED (see C3 for remaining design debt).
8. **`RetrievalDataPlane` write methods required** — FIXED (no silent no-op defaults).
9. **UnifiedAgent `run_react_mode`** — IMPROVED (dispatch copy-paste largely deleted).
10. **Document pipeline stage names** — IMPROVED (`stage_parse_*`, `stage_project_*`, …) but materialize stage still a god (H1).

---

## Part 1 — CRITICAL findings (blockers)

### C1. [CRITICAL] Memory/PG is still a **control-flow mode**, not an adapter choice

**Evidence (live tree):**

- `new_memory` bootstrap installs `chat_persistence: None` while always installing `MemoryDocumentStore` for documents.
- `app-chat/src/sessions.rs` alone has **7** `if let Some(pg) = self.storage.chat_persistence()` arms, each with a full `storage.inner()` memory twin (search/list/create/update/delete/…).
- Same pattern: `citations.rs`, `agent_runtime.rs`, `chat/service.rs`.
- Documents path is better (`document_store().ok_or_else`) but still special-cases `uses_memory_adapters()` for upload side effects (`documents.rs` ~174–189).
- Focused ports (`SessionPort`, `MessagePort`, …) exist and `PgChatPersistenceAdapter` implements them — **there is no `MemoryChatPersistenceAdapter`**, so call sites cannot “just use the port.”

**Why this fails the bar:**  
M4 claimed dual-backend elimination. ISP port split claimed kitchen-sink relief. Neither is usable until **memory mode is also a port implementation**. Today every new chat/session feature re-implements visibility, org filter, and error shape twice. That is spaghetti growth by architecture.

**Code judo (delete the dual path, don’t paper over it):**

1. Implement `MemoryChatPersistence` (or split memory stores per focused port) backed by `MemoryState`.
2. Bootstrap **always** sets `chat_persistence: Some(Arc::…)` (memory or PG).
3. Domain code takes `&dyn SessionPort` / `MessagePort` only — delete every `storage.inner()` branch in app-chat.
4. Optionally collapse `Option<Arc<dyn DocumentStorePort>>` → non-optional once always installed (bootstrap already uses `Some(...)`).

Until (1)–(3) land, dual-backend is the permanent tax on the highest-churn crate (`app-chat` ~30k LOC under `src/`).

---

### C2. [CRITICAL] `IngestionError` still erases types — rename is not a taxonomy

**Where:** `ingestion/src/error.rs`

```rust
TaskSource(String), AuditSink(String), Storage(String), Parse(String),
Security(String), Index(String), Embedding(String), Internal(String), …
```

Helpers are `Self::Storage(error.to_string())`. Call sites still funnel heterogeneous failures into strings (~28+ constructor sites in worker/ingestion; many more `.map_err(IngestionError::storage)`).

**Why this fails:**  
Morning review called out `StateSink(String)`. The rename to `Storage`/`Parse`/`Security` **looks** better in logs but retry/metrics/alerting still cannot match on cause. A typed enum that only carries `String` is ceremony without leverage.

**Code judo:** Nested causes with `#[from]` / `thiserror` source chains:

```text
Storage(#[from] PgError | ObjectStoreError)
Parse(ParseRoute | IrValidation)
Security(Threat | ZipBomb | ScannerUnavailable)
Lock(Skipped | Failed)
…
```

Delete string constructors for recoverable paths. If conversion is missing, the compiler forces the taxonomy — that is the point.

---

### C3. [CRITICAL] Metering has **two product systems** + fragile feature mapping

Exit metering fixed double-insert into `llm_usage_events` for chat aggregation (good). What remains is a **design dual core**:

| System | Gate | Record path | What it counts |
|--------|------|-------------|----------------|
| Monthly plan metrics | `ensure_metric_quota("llm_input_tokens"…)` | `record_usage_event` / postprocess estimates | Estimated tokens pre/post chat |
| Rolling 5h/7d windows | `check_user_quota` / `usage_limit_phase` | `UsageObserver` → `insert_llm_usage_event` | Actual provider tokens per call |

**Additionally:**

- `PgUsageObserver::map_feature` is **substring heuristics** (`contains("plan")`, `contains("answer")`, embedding → Answer). Wrong feature → wrong billable bucket forever, fail-open.
- Worker observer uses **bootstrap system tenant**, not task `owner_user_id`/`requested_by` — ingestion LLM/embedding spend is attributed to the wrong tenant for multi-tenant billing.
- Write path correctly prefers agent client + observer, but still **falls back to `WriterLlm::from_env()`** (second config channel for the same model).
- `BillingContext.record_llm_usage` remains as a third entry for aggregated/cost analytics alongside observer.

**Why this is structural:**  
Three write paths + two quota dimensions + heuristic feature labels = future double-count, silent under-count, and un-auditable product metrics. The exit-metering design is the right **shape**; the product has not deleted the old shape.

**Code judo:**

1. **Single write path** for token truth: only `UsageObserver` (or only one store API) writes token rows used for rolling windows.
2. Monthly metric either **derived** from `llm_usage_events` or explicitly documented as a separate product meter with different units — not parallel “estimate then forget.”
3. Replace `map_feature(&str)` with a typed `BillableFeature` (or enum) set at client construction (`with_feature(BillableFeature::Summary)`), not parsed from free text.
4. Worker: bind tenant from `task_context(task)` per job (or rebind observer per task), not bootstrap nil/system identity.
5. Delete `from_env` from production write path; env builder stays in heavytail **bins/tests only**.

---

## Part 2 — HIGH findings

### H1. [HIGH] Worker pipeline: stages named, materialize still a god; processor still a bag

| File | Lines | Issue |
|------|------:|-------|
| `document_pipeline.rs` | **863** | `run_document_pipeline` is a short stage list — good — but `stage_materialize_chunks_assets_profile` alone is a multi-hundred-line sequential script (assets, multimodal, chunks, ensure_side_effects loops). |
| `processor.rs` | **516** | One `timeout` wraps lock + fetch + security + route + pipeline. `PgTaskProcessor` is a **17-field** public bag of Option clients. |

**Code judo:**  
- Split materialize into pure functions or sub-stages with a `MaterializeCtx`.  
- `PgTaskProcessor` → `IngestionDeps { storage, embedding, llm, locks }` groups (same move as StorageContext).  
- Optional later: `trait Stage` if it deletes branching; do not introduce Stage trait theater if groups alone suffice.

---

### H2. [HIGH] `contracts/src/rag_execute.rs` still hosts runtime policy (~632 lines)

Still contains: `validate()`, `ensure_original_query_text_dense_item()`, `PlaceholderTriplet::classify()`, `to_chat_request_compat()`.

Contracts should be wire shapes. Policy belongs in `rag-core` / `app-chat`. Compat methods need an expiry or a dedicated adapter crate — not immortal methods on the shared DTO.

---

### H3. [HIGH] Analytics still multi-homed

Still live in parallel:

- `transport-http` `record_api_product_event_if_available`
- `ChatContext::record_product_event_if_available`
- `app-documents::analytics_helpers::record_product_event_if_available`
- share / auth handlers calling into the above

Same concept, different surfaces → drift on platform/metadata is inevitable.

**Code judo:** One function on `AnalyticsServiceCtx` with explicit `Surface`. Everything else dies.

---

### H4. [HIGH] `app-chat` is a mega-crate (~30k LOC under `src/`)

Largest production modules: capability policy/registry, answer_contract, sse_sink, writer refine, chat_private, sessions, token_budget (gated). The crate owns chat HTTP domain, agents, write mode, prompts, RAG glue, sessions, citations, streaming.

**Why it matters:** Dual paths (C1), metering (C3), and write mode all land here. Boundary pressure will keep growing.

**Code judo (incremental, not big-bang):**  
Extract `app-chat-sessions`, `app-write` / keep heavytail, `app-agents` as separate crates **only when** a seam is already clean (ports exist). First win is C1 (memory adapter) so session code can move without carrying dual paths.

---

### H5. [HIGH] `app-bootstrap` remains a shallow kitchen sink (~8.3k LOC)

- Pure port adapters still dominate (`pg_chat_persistence`, billing SQL).
- Residual `include!` under `billing_sql/` and build.rs-assembled admin/share port impls (Rust 2024 constraint — understand, but still a modularity tax).
- God-function `bootstrap()` still wires the world.

**Code judo:** Prefer domain repos where adapter is pure error mapping; generate or macro the rest once; do not add new include shards.

---

### H6. [HIGH] ChatPersistence ISP is **declared** but not **consumed**

Focused traits exist and are documented (“prefer narrow port”). Call sites still take `Option<Arc<dyn ChatPersistencePort>>` and dual-branch. Supertrait remains the only practical handle.

**Code judo:** After Memory adapter (C1), change `ChatContext` / storage to hold `Arc<dyn SessionPort>` etc. where needed, or a small `ChatPorts { sessions, messages, catalog }` struct. Supertrait only for bootstrap “implement everything” adapters.

---

### H7. [HIGH] Desktop `llm_config.rs` (~638 lines) + license service (~525)

Shell UI landed (W5). Command surface is modular, but LLM config command module is approaching god-file size for a desktop adapter. Prefer splitting load/validate/persist from IPC command handlers.

---

### H8. [HIGH] Product e2e harness size / fragility

| File | Lines |
|------|------:|
| `setup.rs` | 1148 |
| `llm_real/mod.rs` | 1202 |
| `rag_quality_prod.rs` | 1374 |
| `transport-http` lib tests | 1558 |

Not production, but this is the gate for every structural change. Compile rot already appeared once (`PgAppRepository::connect` → BootstrapRepository). Treat e2e as a first-class module: split setup, shared PG helpers, and suite gates so product changes don’t require reading 1k+ line files.

---

## Part 3 — MEDIUM findings (high-conviction only)

### M1. Dual i18n names without dual systems
`adminMessage` deprecated alias is fine short-term; delete call sites → only `adminText` to end the dual vocabulary.

### M2. Root-level doc / backup cruft
Repo root still carries `PRD_RUST.md.bak.garbled`, `.autofix.tmp`, stale design dumps, and untracked `heavytail-out` / nested `graphify-out`. Not runtime risk; signals hygiene debt and confuses newcomers. Archive or gitignore consistently.

### M3. `ChatContext` clones `StorageContext` from `AppState`
Flag is Arc-shared (comment claims desync fixed for `uses_memory_adapters`). Still two owners of the same bag; `test_replace_storage` must dual-assign. Prefer single owner + narrow chat ports.

### M4. Share path still thicker than `ShareService` ideal
294 lines is better than 527; remaining HTTP glue should not re-encode access rules if share crate already owns them.

### M5. Heavytail experimental surface in the product dependency graph
`heavytail` is a real dependency of write mode (~7k LOC). Experiment bins and human-sample workflows belong behind features / separate packages so product builds don’t imply research scaffolding.

---

## Part 4 — Missed code-judo opportunities (ranked)

| Rank | Move | Deletes |
|------|------|---------|
| 1 | **MemoryChatPersistence + always-install ports** | Entire dual branch family in sessions/citations/service |
| 2 | **Typed IngestionError causes** | String funnel + unclassifiable retries |
| 3 | **One metering write path + typed BillableFeature** | Heuristic map_feature, double mental model, wrong-tenant worker |
| 4 | **Split materialize stage + deps groups on worker** | 17-field processor bag, 300-line stage body |
| 5 | **Move rag_execute policy out of contracts** | Cross-crate policy coupling |
| 6 | **Single analytics entry** | 4+ record helpers |

Items 1–3 are the only moves that change the **daily cost of change** for the chat/ingestion product. Do those before further feature surface area.

---

## Part 5 — File-size watchlist (>800 LOC, non-generated)

| File | LOC | Notes |
|------|----:|-------|
| `app-chat` total `src/` | ~30k | Mega-crate (H4) |
| `document_pipeline.rs` | 863 | Stage materialize god (H1) |
| `embedding.rs` | 763 | Client + tests co-located |
| `answer_contract.rs` | 759 | Policy-dense |
| `desktop llm_config.rs` | 638 | H7 |
| `rag_execute.rs` (contracts) | 632 | H2 |
| `capability/policy.rs` | 628 | |
| product e2e `setup` / `llm_real` | 1.1k–1.3k | H8 |
| `frontend_next` admin-i18n | 963 | Acceptable if data-like; still large |
| tiptap editor | 783 | Prior M2 partial |

No new production “crossed 1k by this PR” smoking gun beyond e2e/test god-files; the issue is **crate-level** bulk and dual paths, not a single 1k file this week.

---

## Part 6 — Approval checklist (thermo-nuclear)

| Criterion | Status |
|-----------|--------|
| No clear structural regression | ⚠️ New dual metering / system-tenant worker attribution is a regression risk on top of old dual memory |
| No missed dramatic simplification when path is visible | ❌ C1/C2/C3 paths are visible and unlanded |
| No unjustified file-size explosion | ⚠️ e2e + app-chat bulk |
| No spaghetti from special-case branching | ❌ sessions dual path remains the textbook case |
| No hacky abstraction obscuring design | ❌ `map_feature` string contains |
| No architecture-boundary leak | ❌ contracts policy; multi analytics |
| Obvious decomposition that would improve maintainability | ❌ Memory port adapter |

**Result: NOT APPROVED.**

---

## Suggested next sequence (execution order)

1. **C1** Memory `ChatPersistence` (+ always install) → delete dual branches in `sessions` / `citations` / `agent_runtime`  
   *verify:* memory-mode unit/contract tests; no `storage.inner()` in app-chat domain methods  
2. **C2** Typed `IngestionError` causes  
   *verify:* worker maps malware vs timeout vs PG distinctly in logs/metrics  
3. **C3** Metering consolidation + typed feature + task tenant on worker  
   *verify:* existing usage_exit_metering e2e + multi-tenant ingestion attribution test  
4. **H1** materialize stage split + processor deps grouping  
5. **H2 / H3** contracts policy move + analytics single entry  

Do **not** start another feature slice (admin UI, write polish, desktop) until C1 is done — every chat feature multiplies the dual-path tax.

---

## Appendix — Measurement commands (repro)

```bash
# frontend typecheck
cd frontend_next && pnpm exec tsc --noEmit

# dual chat_persistence / memory
rg -n 'chat_persistence\(\)|storage\.inner\(\)' avrag-rs/crates/app-chat --glob '*.rs'

# string IngestionError
rg -n 'IngestionError::(Storage|Parse|Security|Internal)\(|IngestionError::storage' avrag-rs --glob '*.rs'

# metering entry points
rg -n 'UsageObserver|insert_llm_usage|record_usage_event|ensure_metric_quota' avrag-rs/crates --glob '*.rs'

# large files
find avrag-rs frontend_next contracts desktop/src-tauri/src -name '*.rs' -o -name '*.ts' -o -name '*.tsx' \
  | grep -v target | grep -v node_modules | xargs wc -l | sort -n | tail -40
```

---

## Long-tail round (2026-07-09 evening)

Landed in `chore/tn-longtail-round`:

1. **Contracts policy removed**: `ExecutePlanRequest::validate` / `ensure_original_query_*` / `to_chat_request_compat` / `PlaceholderTriplet::classify` deleted; policy tests live in `rag-core::execute_plan_policy`.
2. **Monthly metering prefers actual tokens**: `record_usage_for_execution` uses `llm_usage` prompt/completion when present (still separate from exit-metering `llm_usage_events`).
3. **Admin i18n**: `adminMessage` call sites → `adminText`; deprecated alias removed.
4. **Share handlers**: already thin auth→`ShareService` facade; no structural change this round.

