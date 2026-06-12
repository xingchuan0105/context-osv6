# Health Optimization Handoff — 2026-06-11

Session handoff for codebase health work (T13 app split, test debt, facade cleanup).

## 1. Context

Multi-crate app decomposition in progress. Goal: thin `app` facade, domain logic in `app-*` crates, minimal behavior change per phase.

## 2. Completed tracks

| Track | Status | Notes |
|-------|--------|-------|
| T13 Phase 1–2 | Done | Core crates extracted; delegates in place |
| T13 Phase 3 | Done | P3-BOOT + P3-CITATION + P3-CLEANUP |
| P3-BOOT | Done | `app-bootstrap` crate; `AppState::new/bootstrap` delegate |
| P3-CITATION | Done | `app-chat/src/citations.rs`; `citation_delegates.rs` in facade |
| P3-CLEANUP | Done | Docscope dedup, facade file cleanup, shim removal |

## 3. Key files

- Inventory: [`t13-app-split-inventory.md`](./t13-app-split-inventory.md)
- ADR: [`adr/appstate-decomposition-phase2-5.md`](./adr/appstate-decomposition-phase2-5.md)

## 4. Known warnings (non-blocking)

- `app-chat`: unused RAG prompt helpers, dead `current_user_id` in `chat_private.rs`
- `app-core`: unused `map_anyhow_error`
- `ingestion`: unused imports in model/runtime

## 5. Verification commands

```bash
cd avrag-rs
cargo check --workspace
cargo test -p app-core -p app-billing -p app-documents -p app-chat -p app-admin -p app-bootstrap -p app --lib
cargo test -p transport-http
```

## 6. New window checklist

Use this when resuming in a fresh session:

- [x] **T13 Phase 3** — bootstrap + citations + facade polish (workspace check + tests pass)
- [x] **`graphify update .`** — run after Phase 3 structural changes
- [x] **P1: Test debt** — delegate contracts, frontend tests, warning cleanup (2026-06-11)
  - `crates/app/tests/delegate_contract.rs` — 6 facade contract tests
  - `frontend_next`: vitest `@/*` alias + stream/dashboard/settings/share test fixes (224 passed)
  - `app-chat` / `app-core` / `app`: compiler warnings cleared (ingestion deferred)
- [x] **product_e2e** — smoke + integration mock suite (Docker + Milvus); `redis` dev-dep fixed
  - Smoke default: mock Search (set `SEARCH_USE_REAL=1` for live Brave)
  - Run: `cd avrag-rs && set -a && source .env && set +a && E2E_MODE=smoke cargo test -p app --test product_e2e -- --test-threads=1`
  - llm_real still `#[ignore]` — run with `--ignored --test-threads=1`
- [x] **ingestion warnings** — unused imports + `TABLE_QUAL_THRESHOLD` allow cleared

**Current P1 = test debt.** T13 app split (Phase 1–3) is complete; next session focuses on test coverage and warning cleanup.
