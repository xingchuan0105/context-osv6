# E2E Quality Gates

This document defines pass/fail semantics across Rust Product E2E and Playwright
suites.

**Agent-oriented full coverage matrix** (what to test, parallel groups, real doc parse / LLM RAG / chat / websearch): [`full-functional-e2e-guide.md`](full-functional-e2e-guide.md).

**Post-run analysis** (coverage / regression / attribution / stability / quality): [`e2e-analysis-framework.md`](e2e-analysis-framework.md) + [`e2e-test-registry.yaml`](e2e-test-registry.yaml).

See also [`product-e2e-plan.md`](product-e2e-plan.md).

## Layer overview

| Layer | Runner | Trigger | Execution | Citation gate |
|-------|--------|---------|-------------|---------------|
| PR smoke | `smoke-e2e.yml` | PR | `./scripts/run-product-smoke-e2e.sh` (root `.github/workflows/smoke-e2e.yml`, `defaults.run.working-directory: avrag-rs`) | N/A (mock LLM) |
| Integration | `integration-e2e.yml` | main / manual | `E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e -- --test-threads=1` | Hard in integration tests |
| llm_real | `nightly-llm-real.yml` | schedule / manual | `E2E_MODE=nightly cargo test -p app --test product_e2e --features product-e2e llm_real -- --ignored --test-threads=1` | **Hard** — `assert_citations_non_empty` |
| Playwright skills | `frontend-skills.yml` | schedule / manual | `cd frontend_next && npx playwright test --project=skills` | **Hard** — `must_have_citation` golden entries |
| Playwright judge | `nightly-playwright-judge.yml` | schedule / manual | root workflow + `RUN_QUALITY_JUDGE=1` | Score &lt; 6 → **warn only** |

## Rust Product E2E

### Smoke (PR)

- **Smoke integration modules** (`smoke::`, serial for RAG): `ingestion_smoke`, `rag_smoke`, `rag_fallback_smoke`, `rag_codegen_multitool_smoke`, `memory_multiturn_smoke`, `paddle_image_smoke`
- **Smoke manual-only** (module guard only; `#[ignore]`): `search_real_smoke`, `paddle_pdf_smoke`
- **Non-RAG smoke modules** (parallel): `chat_smoke`, `search_smoke`, `auth_boundary`, `share_boundary`
- **Unit tests** (parallel with non-RAG smoke; no Docker):
  - `setup::tests` (6) — docker port/timestamp parsing, active-container registry, docker id
  - `e2e_gate::tests` (4) — `E2E_MODE` suite gating
  - `test_context::tests` (2) — Milvus collection prefix, PG migration cross-process dedup
  - `mock_routing` (6) — mock LLM route / synthesis contract routing
- Non-RAG smoke + unit tests run **in parallel** (`run-product-smoke-e2e.sh`); RAG smoke modules run **serial** after `wait`
- Orphan Docker cleanup removes only test-owned `avrag-test-pg-*` / `avrag-test-redis-*` names; skips active/young containers (see `setup::cleanup_orphaned_test_containers`). **Milvus** uses the shared compose stack (`milvus-standalone`); CI does not force-remove it — isolation is per-context `MILVUS_COLLECTION_PREFIX` + teardown collection drops
- Gated by `require_smoke_suite()` — fails under `E2E_MODE=nightly`
- CI/local runner: [`scripts/run-product-smoke-e2e.sh`](../scripts/run-product-smoke-e2e.sh) (module list single source of truth; **module coverage guard** compares `cargo test … smoke:: -- --list` against `NON_RAG_MODULES` + `RAG_SERIAL_MODULES` + `SMOKE_MANUAL_ONLY_MODULES` and exits 1 on mismatch; `search_real_smoke` and `paddle_pdf_smoke` are manual-only — registered for guard, skipped in PR execution; quick check: `./scripts/run-product-smoke-e2e.sh --check-modules`; **EXIT trap** removes `avrag-test-*` containers)
- **Module coverage guard (2026-06-13): green.** Parser matches `product_e2e::smoke::<module>::…` via `sed -n 's/.*::smoke::\([^:]*\)::.*/\1/p'`; `backend_launcher` (no submodule segment) is intentionally excluded.
- Mock LLM / Search / Embedding only; E2E bootstrap forces **local** `object_root` (ignores `.env` MinIO/S3 for API)
- Protocol + HTTP assertions; SSE event-order (`start` first, `done` terminal, no post-`done` events) and `done` payload shape in [`transport-http` contract tests](../crates/transport-http/tests/chat_stream_contract.rs) (`cargo test -p transport-http`)
- Main suite uses `REDIS_URL=redis://127.0.0.1:1` (blackhole) to keep embedding failure mocks effective
- **`auth_boundary`**: run with `--test-threads=1` only (shared PG + fixed notebook ids; parallel within module can 500)
- **Strict cite (ADR-0008)**: RAG smoke asserts `assert_citation_referenced_in_answer`; search smoke expects `[[n]]` markers; mock synthesis returns `internal_answer_v1` JSON with `[[cite:CHUNK_ID]]`

### Integration (main)

- Full mock suite **~45** runnable tests (`--test-threads=1`), plus **`#[ignore]`** (`llm_real`, `backend_launcher`, `paddle_pdf_smoke`, utility)
- Citation assertions where the mock route guarantees citations
- `assert_citation_referenced_in_answer` used in selected integration paths
- `assert_observability_contract` on smoke chat/share paths

#### Shared fixtures (`streaming_chat`, `rag_codegen_multitool_smoke`)

- Module-scoped [`shared_rag_fixture()`](../crates/app/tests/product_e2e/fixtures/ready_rag.rs): one cold ingest of `antifragile.txt` per test binary; retains PG/Milvus/object store, **one** `AppState`, mock endpoints, and API `base_url`
- Per-test [`shared_ready_rag_context()`](../crates/app/tests/product_e2e/fixtures/ready_rag.rs) respawns **worker only** on the current `#[tokio::test]` runtime (API + mocks live on [`persistent_runtime`](../crates/app/tests/product_e2e/persistent_runtime.rs))
- **Why**: each `#[tokio::test]` shuts down its runtime on exit; sharing a live `TestContext` across tests left dead API/mock/worker tasks → `Connection refused` / `PoisonError` on the next test
- **Requires** `--test-threads=1` for the full integration suite (enforced in `integration-e2e.yml`); parallel workers would race on shared Milvus collection state during cold bootstrap
- Protocol invariants stay in `transport-http` contract tests; `streaming_chat` only covers mock RAG observability (reasoning delta, trace telemetry, `prompt_snapshot` behind `debug: true`)

#### Concurrent queries (`concurrent_query`)

- `integration::concurrent_query::concurrent_rag_queries_are_safe_on_codegen_bridge` issues two chat requests via `tokio::join!` (not serial await)
- **Current mock-path assertions** (concurrency safety, not answer differentiation): both HTTP 200, `assert_codegen_bridge_dense_retrieval`, `assert_has_citations`, `assert_citation_doc_id`
- **Removed under mock LLM** (see [Integration regression status](#integration-regression-status-jun-2026)): `assert_independent_citation_chunks`, distinct answers, topic keywords — mock synthesis returns the same canned `RagAnswer` regardless of query; same-doc `dense_search` may also return the same top chunk
- **Real-LLM independence gate**: `integration::concurrent_query::real_llm_concurrent_rag_queries_have_independent_citation_chunks` (`#[ignore]`) restores `assert_independent_citation_chunks` under `E2E_MODE=nightly`

#### HTTP client timeouts (Product E2E bootstrap)

Defined in [`test_context/builder.rs`](../crates/app/tests/product_e2e/test_context/builder.rs):

| Constant | Seconds | When |
|----------|---------|------|
| `HTTP_TIMEOUT_DEFAULT_SECS` | 60 | Non-RAG smoke |
| `HTTP_TIMEOUT_RAG_SECS` | 120 | Mock RAG / integration paths |
| `HTTP_TIMEOUT_REAL_LLM_SECS` | 180 | `use_real_llm` / nightly |

Worker ingestion timeout is separate: `E2eBootstrapConfig.worker_timeout_secs` → `AVRAG_INGESTION_TASK_TIMEOUT_SECS`.

### Embedding cache

- `integration::embedding_cache` — starts Redis **after** orphan cleanup (avoids deleting the test container)
- `TestContext::new_embedding_cache()` profile (real Redis, not blackhole)
- Run: `cargo test -p app --test product_e2e integration::embedding_cache -- --test-threads=1`

### llm_real (nightly)

- `#[ignore]` — run with `E2E_MODE=nightly` and `--ignored --test-threads=1`
- Gated by `require_nightly_suite()` — fails under `E2E_MODE=smoke` / `integration` unless filter bypasses body
- Manual acceptance after ADR-0008 changes: `E2E_MODE=nightly cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture`
- Requires real `AGENT_LLM_*`, `EMBEDDING_*`; search tests require `SEARCH_API_KEY`
- `SEARCH_REQUIRE_REAL=1` — Brave unreachable **fails** (no silent mock fallback)
- Streaming requests use `"debug": true` so `prompt_snapshot` trace events are emitted
- Artifacts under `crates/app/tests/e2e_output/llm_real/<run_id>/<test_name>/`:
  - `response.json` — full `ChatResponse`
  - `reasoning_summary.txt` — concatenated `reasoning_summary_delta` SSE chunks
  - `trace_reasoning.jsonl` — one JSON object per line for trace events with `detail.reasoning` (e.g. `plan_decision`, `evaluation`). **Source**: unified agent loop telemetry (`emit_plan_decision_telemetry` / `emit_evaluation_telemetry` in `reasoning_emit.rs`), not LLM eval output — `reasoning` is synthesized from structured fields (`exit_reason`, `observation_preview`, iteration/skills).
  - `prompt_snapshots.json` — array of `stage=prompt_snapshot` trace `detail` payloads (full `system_content`)
  - `metadata.json` — `usage`, model names, reasoning stats, `stream_error_with_done`, `extra` test fields
  - `turn1_reasoning_summary.txt` / `turn2_reasoning_summary.txt` — multi-turn tests only
- `metadata.reasoning_empty_warning: true` when **both** `reasoning_summary.txt` and `trace_reasoning.jsonl` are empty. Because loop telemetry always emits `plan_decision` / `evaluation`, this usually means the SSE stream dropped trace events or the agent loop did not run — **not** “the LLM is a non-thinking model”.
- `metadata.stream_error_with_done: true` when the final retry attempt had both an SSE `error` event and a terminal `done` payload (also mirrored in `metadata.extra` for backward compatibility).
- Mirror copy under `e2e_output/observability/<run_id>/<test_name>/` with the same reasoning files when saved via `save_llm_artifact` (lighter `response.json` + `metadata.json` only for non-llm_real callers).
- Offline tools:
  - `cargo run -p e2e-analyzer -- llm-real list`
  - `cargo run -p e2e-analyzer -- llm-real summary --run crates/app/tests/e2e_output/llm_real/e2e_<timestamp>_<commit>`

## Playwright

### Skills (RAG / Search)

Aligned with golden set `must_have_citation` semantics:

1. **Hard**: HTTP 200, non-empty answer, mode indicator, keyword match, **`citationCount > 0`**
2. **API confirmation**: `waitForDocumentReady` after upload before chat (RAG)

### Functional (Playwright `functional` project)

PR 级 smoke（`testMatch: specs/smoke/*`，排除 `auth*`；预置 `storageState`）：

| Spec | Path | Gate |
|------|------|------|
| Query library | `smoke/query-library.spec.ts` | 发送入库、单次插入、连点拼接、streaming 期间插入忽略 |
| Legal consent | `smoke/legal-consent.spec.ts` | 法律页 / 注册同意 / 重签 gate |
| Admin navigation | `smoke/admin-navigation.spec.ts` | 管理入口可达 |

Vitest 配套：`tests/workspace/query-library-*.test.ts`、`workspace-history-pane.test.tsx`（挂载 + 布局烟测）。

### Journey (Playwright `journey` project)

| Spec | Path | Citation gate | Rationale |
|------|------|---------------|-----------|
| `workspace-upload-rag.spec.ts` | Upload fixture → RAG Q&A | **Hard** — `citationCount > 0` + citation button visible | Fixed `sample-document.txt`; mock/staging stack guarantees retrieval |
| `workspace-chat.spec.ts` (general) | General chat | N/A | No citation expected |
| `workspace-chat.spec.ts` (web search) | Brave / external search | **Soft** (PR journey) / **Hard** when `E2E_TIER=nightly\|staging` | PR: external API variability; nightly/staging: `citationCount > 0` + citation button visible (skills project also hard-gates search) |

### Quality judge (optional)

Set `RUN_QUALITY_JUDGE=1` to attach LLM judge scores via [`judge.ts`](../../frontend_next/e2e/utils/judge.ts).
Nightly workflow uploads judge attachments; score below 6 does **not** fail the job.

## Environment variables

| Variable | Purpose |
|----------|---------|
| `E2E_MODE` | `smoke` → smoke only; `integration` (default) → smoke + integration; `nightly` / `llm_real` → `llm_real` only |
| `AVRAG_WORKER_HEALTH_PORT` | Worker: `0` = bind ephemeral port; publishes to `AVRAG_WORKER_HEALTH_PORT_FILE` (E2E) |
| `SEARCH_REQUIRE_REAL=1` | Fail when Brave Search unreachable (llm_real / nightly) |
| `SEARCH_FORCE_MOCK=1` | Force mock search even with credentials |
| `SEARCH_USE_REAL=1` | 在 smoke 层启用真实 Brave（需 `SEARCH_API_KEY`；`smoke::search_real_smoke` 为 `#[ignore]` 预发用例） |
| `RUN_QUALITY_JUDGE=1` | Enable Playwright LLM judge attachments |
| `RUN_CROSS_BROWSER=1` | Enable Firefox/WebKit journey projects |
| `E2E_TIER` | `nightly` or `staging` — journey web-search citation **hard** gate in `workspace-chat.spec.ts` |

## Local prerequisites (Product E2E)

Milvus must be healthy on `127.0.0.1:19530` before RAG tests. Use the project
compose stack (etcd + minio + standalone), not a single `milvus run standalone`
container — standalone still requires etcd.

```bash
# One-shot precheck (from repo root)
./scripts/e2e-precheck.sh

# Or manually
cd avrag-rs && docker compose -f docker-compose.milvus.yml up -d
curl -s -X POST http://127.0.0.1:19530/v2/vectordb/collections/list \
  -H 'Content-Type: application/json' -d '{"dbName":"default"}'
```

If Milvus is down, tests fall back to `docker compose -f docker-compose.milvus.yml up -d`
and fail fast when `milvus-standalone` exits (no 180s blind wait).

## Integration regression status (Jun 2026)

Tracked while closing the post-refactor integration gate. Last full-suite run before doc update: **not green** (interrupted mid-run). Latest smoke-runner reprobe (2026-06-13): **module coverage guard green** (`--check-modules` / pre-run guard); full suite depends on local Docker + Milvus.

### Fixed and verified

| Item | Symptom | Root cause | Fix | Verification |
|------|---------|------------|-----|--------------|
| `integration::streaming_chat` (4 tests) | Test 2+ `Connection refused`; `PoisonError` on `shared ready_rag lock` | `#[tokio::test]` runtime teardown killed API/mock/worker spawned in test 1; `std::sync::Mutex` held across `.await` | `RagSharedFixture` + `persistent_runtime` + per-test worker via `shared_ready_rag_context()` | **4/4 pass** with `--features product-e2e --test-threads=1` |
| `smoke::search_smoke` (isolated) | Degraded answer *"I could not retrieve web evidence..."* in long suites | `SEARCH_USE_REAL=1` in `.env` enabled real Brave in mock smoke paths | `build_smoke`: force `has_real_search = false` when `!use_real_llm` | **Pass** in isolation; **not re-checked** in full integration run |
| PG pool timeout on fixture respawn | `bootstrap AppState: pool timed out` on 2nd+ streaming test | Repeated `AppState::bootstrap` per test exhausted PG connections | Single `Arc<AppState>` in `RagSharedFixture` | Covered by streaming_chat multi-test pass |
| Drop panic under tokio runtime | `Cannot block the current thread from within a runtime` during teardown | `release_shared_postgres` / `release_shared_milvus` used `blocking_lock()` inside `#[tokio::test]` drop | Move slot cleanup into `block_on_with_timeout` async block | Streaming multi-test pass; other modules **not fully audited** |

### Partially addressed / unverified

| Item | Status | Notes |
|------|--------|-------|
| `integration::concurrent_query` | Mock path renamed to `concurrent_rag_queries_are_safe_on_codegen_bridge`; **verified PASS** (2026-06-12, 20.5s). Real-LLM variant `real_llm_concurrent_rag_queries_have_independent_citation_chunks` added (`#[ignore]`) | Mock synthesis is query-agnostic; per-request `x-mock-rag-query` removed (dead pipe, option b+c). Independence intent lives in the `#[ignore]` real-LLM test |
| `smoke::rag_codegen_multitool_smoke` | Fixed via new fixture; **verified PASS** (2026-06-12, 18.2s) | Was `PoisonError` from dead shared `TestContext`; uses `shared_ready_rag_context()` now |
| Full `E2E_MODE=integration` suite | **GREEN** — 59 pass / 0 fail / 10 ignored, 447s (2026-06-12) | Prior baseline: 49 pass / 6 fail / 10 ignored (~387s) |

### Open issues / tech debt

1. **`mem::forget(abort_tx)`** on persistent API/mock servers — prevents oneshot abort from killing process-lifetime tasks; no explicit shutdown on binary exit
2. **`concurrent_query` semantics** — mock suite tests concurrent codegen-bridge safety only; citation-chunk independence is gated by `real_llm_concurrent_rag_queries_have_independent_citation_chunks` (`#[ignore]`, nightly)
3. **`--features product-e2e` required** — without it, `product_e2e.rs` runs a single skip placeholder. ✅ Confirmed (2026-06-12): `smoke-e2e.yml` and `integration-e2e.yml` both pass the feature. ⚠️ However these workflows live under `avrag-rs/.github/workflows/` which GitHub never reads (repo root is `context-osv6`) — see [test quality review round 4](./brooks-test-quality-review-2026-06-12.md) Critical finding
4. ~~**Ingestion parser layout**~~ — ✅ Resolved (2026-06-13 P4): `mineru/` removed; `router/` + `liteparse*.rs` + `liteparse_probe_bridge.rs` are canonical; compile clean
5. **`docs` drift** — this section. ✅ Stale CI comments mentioning `shared_ready_rag` + `Mutex<TestContext>` cleared repo-wide (2026-06-12)

### Re-run checklist (when resuming)

```bash
cd avrag-rs

# 1. Targeted fixes
E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  integration::streaming_chat -- --test-threads=1 --nocapture

E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  integration::concurrent_query::concurrent_rag_queries_are_safe_on_codegen_bridge -- --test-threads=1 --nocapture

E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  smoke::rag_codegen_multitool_smoke -- --test-threads=1 --nocapture

# 2. Full integration gate
E2E_MODE=integration cargo test -p app --test product_e2e --features product-e2e \
  -- --test-threads=1 --nocapture
```

## Local commands

```bash
# PR smoke (module list in scripts/run-product-smoke-e2e.sh)
./scripts/run-product-smoke-e2e.sh

# Rust mock full suite (integration tier; wrong-suite tests panic)
E2E_MODE=integration cargo test --test product_e2e -p app --features product-e2e -- --test-threads=1 --nocapture

# Rust embedding cache
cargo test -p app --test product_e2e integration::embedding_cache -- --test-threads=1

# Rust real LLM
E2E_MODE=nightly cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture

# Playwright C + D
cd frontend_next && npx playwright test --project=auth --project=functional --project=journey --project=skills

# Goal D one-shot (see scripts/e2e-d-gate.sh)
./scripts/e2e-d-gate.sh
```

## ADR-0008 acceptance matrix (post-implementation)

| Check | Mock / PR gate | Real LLM (manual nightly) |
|-------|----------------|---------------------------|
| Strict cite: no `[[cite]]`/`[[n]]` → empty citations | `smoke::rag_smoke`, `smoke::search_smoke`, `unified_agent_contract` | `llm_real::rag_real`, `llm_real::search_real` |
| Synthesis JSON contract (no prose fallback) | mock `internal_answer_v1` / `internal_search_answer_v1` routes | inspect `synthesis_contract_violation` absent in artifacts |
| Query normalization / multi-turn resolve | unit `query_normalize` | `llm_real::multi_turn` (`--ignored`) |
| PG `turn_metadata.query_resolution` write + read | `avrag-storage-pg` `turn_metadata` roundtrip (`list_messages` + `resolved_query` in normalize) | SQL audit on `chat_messages.turn_metadata` after chat |
| iter0 content blocked without evidence | `exit_policy` unit tests | `llm_real` trace `content_blocked_no_evidence` when applicable |

```bash
# PR gate bundle (ADR-0008)
cargo test -p app --lib 'agents::r#loop::exit_policy'
cargo test -p app --lib 'agents::unified::helpers'
cargo test -p app --lib 'agents::r#loop::query_normalize'
cargo test -p app --lib 'agents::r#loop::answer_contract'
cargo test -p app --test unified_agent_contract
cargo test -p app --test product_e2e smoke::
cargo test -p avrag-storage-pg --lib turn_metadata
cargo test -p app --lib

# Nightly real-LLM manual sign-off
E2E_MODE=nightly cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture
```

## Known seams (E2E bootstrap)

- `E2E_ENABLED` — transport middleware still reads this from process env during bootstrap
- `PG_MIGRATED_URLS` — process-wide `HashSet` of migrated `database_url`s; PG container recycle within the same cargo process re-runs migrations when URL changes
- Mock / API HTTP servers — spawned on [`persistent_runtime`](../crates/app/tests/product_e2e/persistent_runtime.rs) (survive across `#[tokio::test]` cases); listeners bound on that runtime via `bind_persistent_listener()`
- `RagSharedFixture` — holds `Arc<AppState>` and `api_base_url`; dropping per-test `TestContext` from `spawn_from_rag_fixture` must **not** decrement shared PG/Milvus refs (infra owned by fixture)
- Worker health — E2E sets `AVRAG_WORKER_HEALTH_PORT=0` and polls `worker-health.port` under the test object store dir
- Mock RAG dense_search query injection — **decision (2026-06-13, Brooks M10 option b+c):** removed the unused `x-mock-rag-query` chat header and mock-LLM header reader. The only end-to-end reliable path is parsing user messages on the mock LLM request (`dense_search_query_from_messages`). Global `set_mock_rag_codegen_query` remains a single-flight fallback; concurrent tests must not rely on it
