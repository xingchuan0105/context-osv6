# T13 App Split Inventory

Tracks the decomposition of `crates/app` into domain crates (`app-core`, `app-chat`, `app-documents`, `app-admin`, `app-billing`, `app-bootstrap`) and the remaining `app` facade.

## Crate map

| Crate | Responsibility |
|-------|----------------|
| `app-core` | Shared contexts (`StorageContext`, `AnalyticsContext`), config, domain ports |
| `app-chat` | Chat pipeline, agents, sessions, citations, RAG execute |
| `app-documents` | Notebooks, documents, ingest, URL imports, `build_docscope_metadata` |
| `app-admin` | API keys, notifications, admin operations |
| `app-billing` | Usage limits, quota checks |
| `app-bootstrap` | `new_memory` / `bootstrap`, factory wiring (`config_helpers`) |
| `app` | Thin facade: `AppState` delegates + HTTP-facing re-exports |

## `app` facade (`lib_impl/`)

| File | Role |
|------|------|
| `state_types.rs` | `AppState` struct |
| `state_methods.rs` | Bootstrap `From`, analytics hooks, upload signature delegates |
| `chat_delegates.rs` | Chat/RAG/agent delegates → `ChatContext` |
| `citation_delegates.rs` | Citation delegates → `ChatContext` (P3-CITATION) |
| `admin_delegates.rs` | Admin delegates → `AdminContext` |
| `documents.rs` / `notebooks.rs` / `url_imports.rs` | Document domain delegates |
| `asset_helpers.rs` | Citation asset URL resolution delegate |
| `config_helpers.rs` | Upload signing, UUID helpers |
| `preferences.rs` / `memory_helpers.rs` | User preference delegates |

**Removed in Phase 3:** `docscope_helpers.rs` (app-chat), `chat_private.rs`, `assets_notifications.rs`

## Phase 1–2 (prior)

- Extracted `app-core`, `app-documents`, `app-admin`, `app-billing`, `app-chat`
- `app` re-exports `LlmContext`, `OrchestratorContext`, agents, memory helpers from `app-chat`
- Context shims: `analytics_context`, `admin_context`, `billing_context`, `documents_context`

## Phase 3 — Bootstrap + citations + facade polish (2026-06-11)

### P3-BOOT (`app-bootstrap`)
- New crate: `AppBootstrapResult`, `new_memory()`, `bootstrap()`
- Factory helpers in `app-bootstrap/src/config_helpers.rs` (`make_llm_client`, `build_object_store`, etc.)
- `AppState::new` / `bootstrap` delegate to `app_bootstrap`; `From<AppBootstrapResult> for AppState`
- Heavy deps (milvus, cache-redis, guardrails, ingestion, rig-core) moved off `app` → `app-bootstrap`

### P3-CITATION
- `app-chat/src/citations.rs`: `lookup_citation`, `get_citation_asset`
- `app/src/lib_impl/citation_delegates.rs` — thin delegates to `ChatContext`

### P3-CLEANUP
- Deleted duplicate `app-chat/src/docscope_helpers.rs`; use `app_documents::build_docscope_metadata`
- Removed `chat_private.rs`, dead `agent_user_preferences_json`, redundant `llm_context` / `orchestrator_context` shims
- Split `assets_notifications.rs` → `admin_delegates.rs` + `citation_delegates.rs`

### Verify (2026-06-11)
```bash
cd avrag-rs
cargo check --workspace
cargo test -p app-core -p app-billing -p app-documents -p app-chat -p app-admin -p app-bootstrap -p app --lib
cargo test -p transport-http
```
- app-chat: 469 passed; app: 26 passed; transport-http: 50 passed
