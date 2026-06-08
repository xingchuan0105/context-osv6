# E2E Quality Gates

This document defines pass/fail semantics across Rust Product E2E and Playwright
suites. See also [`product-e2e-plan.md`](product-e2e-plan.md).

## Layer overview

| Layer | Runner | Trigger | Citation gate |
|-------|--------|---------|---------------|
| PR smoke | `smoke-e2e.yml` | PR | N/A (mock LLM) |
| Integration | `integration-e2e.yml` | main / manual | Hard in integration tests |
| llm_real | `nightly-llm-real.yml` | schedule / manual | **Hard** — `assert_citations_non_empty` |
| Playwright skills | `frontend-skills.yml` | schedule / manual | **Soft** — structure + keywords first |
| Playwright judge | `nightly-playwright-judge.yml` | schedule / manual | Score &lt; 6 → **warn only** |

## Rust Product E2E

### Smoke (PR)

- Subset: `smoke::` (ingestion, rag, search, **chat**, **share_boundary**, auth_boundary), top-level `product_e2e::` mock routing tests
- Mock LLM / Search / Embedding only
- Protocol + HTTP assertions; no real provider credentials
- Main suite uses `REDIS_URL=redis://127.0.0.1:1` (blackhole) to keep embedding failure mocks effective

### Integration (main)

- Full **34** mock tests (`--test-threads=1`), plus **6** `#[ignore]` (llm_real, backend_launcher)
- Citation assertions where the mock route guarantees citations
- `assert_citation_referenced_in_answer` used in selected integration paths
- `assert_observability_contract` on smoke chat/share paths

### Embedding cache

- `integration::embedding_cache` — starts Redis **after** orphan cleanup (avoids deleting the test container)
- `TestContext::new_embedding_cache()` profile (real Redis, not blackhole)
- Run: `cargo test -p app --test product_e2e integration::embedding_cache -- --test-threads=1`

### llm_real (nightly)

- `#[ignore]` — run with `--ignored --test-threads=1`
- Requires real `AGENT_LLM_*`, `EMBEDDING_*`; search tests require `SEARCH_API_KEY`
- `SEARCH_REQUIRE_REAL=1` — Brave unreachable **fails** (no silent mock fallback)
- Artifacts under `crates/app/tests/e2e_output/llm_real/` include `metadata.json` with `usage` (prompt/completion/cached tokens)
- Observability artifacts also written to `e2e_output/observability/` on failure paths

## Playwright

### Skills (RAG / Search)

Aligned with golden set `must_have_citation` semantics:

1. **Hard**: HTTP 200, non-empty answer, mode indicator, keyword match
2. **Soft**: `expect.soft(citationCount > 0)` — real LLM may skip retrieval or UI may lag
3. **API confirmation**: `waitForDocumentReady` after upload before chat (RAG)

### Quality judge (optional)

Set `RUN_QUALITY_JUDGE=1` to attach LLM judge scores via [`judge.ts`](../../frontend_next/e2e/utils/judge.ts).
Nightly workflow uploads judge attachments; score below 6 does **not** fail the job.

## Environment variables

| Variable | Purpose |
|----------|---------|
| `SEARCH_REQUIRE_REAL=1` | Fail when Brave Search unreachable (llm_real / nightly) |
| `SEARCH_FORCE_MOCK=1` | Force mock search even with credentials |
| `RUN_QUALITY_JUDGE=1` | Enable Playwright LLM judge attachments |
| `RUN_CROSS_BROWSER=1` | Enable Firefox/WebKit journey projects |

## Local commands

```bash
# Rust mock full suite (33 tests)
cargo test --test product_e2e -p app -- --test-threads=1 --nocapture

# Rust embedding cache (ignored)
cargo test -p app --test product_e2e integration::embedding_cache -- --ignored --test-threads=1

# Rust real LLM
cargo test -p app --test product_e2e llm_real -- --ignored --test-threads=1 --nocapture

# Playwright C + D
cd frontend_next && npx playwright test --project=auth --project=functional --project=journey --project=skills

# Goal D one-shot (see scripts/e2e-d-gate.sh)
./scripts/e2e-d-gate.sh
```
