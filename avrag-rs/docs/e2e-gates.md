# E2E Quality Gates

This document defines pass/fail semantics across Rust Product E2E and Playwright
suites. See also [`product-e2e-plan.md`](product-e2e-plan.md).

## Layer overview

| Layer | Runner | Trigger | Citation gate |
|-------|--------|---------|---------------|
| PR smoke | `smoke-e2e.yml` | PR | N/A (mock LLM) |
| Integration | `integration-e2e.yml` | main / manual | Hard in integration tests |
| llm_real | `nightly-llm-real.yml` | schedule / manual | **Hard** — `assert_citations_non_empty` |
| Playwright skills | `frontend-skills.yml` | schedule / manual | **Hard** — `must_have_citation` golden entries |
| Playwright judge | `nightly-playwright-judge.yml` | schedule / manual | Score &lt; 6 → **warn only** |

## Rust Product E2E

### Smoke (PR)

- Subset: `smoke::` (ingestion, rag, search, **chat**, **share_boundary**, auth_boundary), top-level `product_e2e::` mock routing tests
- Gated by `require_smoke_suite()` — fails under `E2E_MODE=nightly`
- CI/local runner: [`scripts/run-product-smoke-e2e.sh`](../scripts/run-product-smoke-e2e.sh) (module list single source of truth)
- Mock LLM / Search / Embedding only; E2E bootstrap forces **local** `object_root` (ignores `.env` MinIO/S3 for API)
- Protocol + HTTP assertions; SSE event-order and `done` payload shape in `transport-http` contract tests
- Main suite uses `REDIS_URL=redis://127.0.0.1:1` (blackhole) to keep embedding failure mocks effective
- **`auth_boundary`**: run with `--test-threads=1` only (shared PG + fixed notebook ids; parallel within module can 500)
- **Strict cite (ADR-0008)**: RAG smoke asserts `assert_citation_referenced_in_answer`; search smoke expects `[[n]]` markers; mock synthesis returns `internal_answer_v1` JSON with `[[cite:CHUNK_ID]]`

### Integration (main)

- Full mock suite **~45** runnable tests (`--test-threads=1`), plus **`#[ignore]`** (`llm_real`, `backend_launcher`, `paddle_pdf_smoke`, utility)
- Citation assertions where the mock route guarantees citations
- `assert_citation_referenced_in_answer` used in selected integration paths
- `assert_observability_contract` on smoke chat/share paths

#### Shared fixtures (`streaming_chat`)

- `integration::streaming_chat` uses module-scoped [`shared_ready_rag()`](../crates/app/tests/product_e2e/fixtures/ready_rag.rs): one cold `TestContext` + ingested `antifragile.txt` per test binary
- **Requires** `--test-threads=1` (enforced in `integration-e2e.yml` and local full-suite commands); parallel workers would race on the shared `Mutex<TestContext>`
- Protocol invariants (`start` first, `done` terminal, payload shape) stay in `transport-http` contract tests; this module only covers mock RAG observability (reasoning delta, trace telemetry, `prompt_snapshot` behind `debug: true`)

#### Concurrent queries (`concurrent_query`)

- `integration::concurrent_query::concurrent_rag_queries_return_independent_citations` issues two chat requests via `tokio::join!` (not serial await)
- Asserts: independent answers (`assert_ne!`), topic-specific keywords, `assert_codegen_bridge_dense_retrieval`, per-query `assert_has_citations` / `assert_citation_doc_id`, **`assert_independent_citation_chunks`** (disjoint `chunk_id` sets)

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

### Journey (Playwright `journey` project)

| Spec | Path | Citation gate | Rationale |
|------|------|---------------|-----------|
| `workspace-upload-rag.spec.ts` | Upload fixture → RAG Q&A | **Hard** — `citationCount > 0` + citation button visible | Fixed `sample-document.txt`; mock/staging stack guarantees retrieval |
| `workspace-chat.spec.ts` (general) | General chat | N/A | No citation expected |
| `workspace-chat.spec.ts` (web search) | Brave / external search | **Soft** — assert button only when `citationCount > 0` | External API variability; skills project owns hard search citation gate |

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
| `SEARCH_USE_REAL=1` | Use real Brave Search in smoke tests (default: mock) |
| `RUN_QUALITY_JUDGE=1` | Enable Playwright LLM judge attachments |
| `RUN_CROSS_BROWSER=1` | Enable Firefox/WebKit journey projects |

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

## Local commands

```bash
# PR smoke (module list in scripts/run-product-smoke-e2e.sh)
./scripts/run-product-smoke-e2e.sh

# Rust mock full suite (integration tier; wrong-suite tests panic)
E2E_MODE=integration cargo test --test product_e2e -p app -- --test-threads=1 --nocapture

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
- `PG_MIGRATIONS_APPLIED` — process-wide `AtomicBool`; first `TestContext` in a cargo process runs migrations
- Worker health — E2E sets `AVRAG_WORKER_HEALTH_PORT=0` and polls `worker-health.port` under the test object store dir
