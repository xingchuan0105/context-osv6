# T13 App Split Inventory (baseline 2026-06-11)

## External consumers

| Crate | Imports |
|-------|---------|
| transport-http | `AppState`, `AppConfig`, `agents::*`, `adapters::redis_rate_limiter` |
| bins/api | `AppConfig`, `AppState` |
| bins/worker | `AppConfig`, `AppState`, `load_prompt_template` |

## AppState impl blocks (by file)

| File | Domain |
|------|--------|
| state_methods.rs | core bootstrap, search, cost events, analytics |
| notebooks.rs | documents |
| documents.rs | documents |
| url_imports.rs | documents |
| asset_helpers.rs | documents |
| sessions.rs | chat |
| chat_streaming.rs | chat |
| chat_private.rs | chat + billing |
| chat/service*.rs | chat |
| rag_execute.rs | chat |
| assets_notifications.rs | admin + chat citations |
| preferences.rs | admin/core |
| billing via chat_private + state_methods | billing |

## transport-http AppState method calls

### Core accessors
`auth`, `with_auth`, `pg`, `pg_ready`, `redis_url`, `analytics`, `max_upload_file_size_bytes`, `signed_upload_url`, `verify_upload_signature`, `set_agent_service`, `set_uses_memory_adapters`

### Workspaces
`list_workspaces`, `get_workspace`, `create_workspace`, `update_workspace`, `delete_workspace`

### Documents
`list_documents`, `create_document_upload`, `add_url_source`, `list_sources`, `update_document`, `delete_document`, `get_document_content`, `reindex_document`, `complete_document_upload`, `get_parsed_preview`

### Chat / RAG
`execute_chat`, `execute_chat_stream`, `execute_rag_execute_plan`, `execute_runtime_tools`, `create_session`, `get_session`, `update_session`, `delete_session`, `list_messages`, `list_sessions`, `lookup_citation`, `get_citation_asset`, `search`

### Billing (app layer)
`get_user_usage_limit`

### Admin (workspace)
`list_api_keys`, `create_api_key`, `revoke_api_key`, `list_notifications`, `mark_notification_read`

### Preferences
`current_user_preferences`, `save_current_user_preferences`, `delete_current_agent_preference`

### Analytics
`record_product_event_if_available`

## pub use lib_impl surface (crate root)

From `config`: `AppConfig`, provider configs, `AppConfig::from_env`
From `prompt_loader`: `load_prompt_template`
From `state_types`: `AppState`
From `documents`: `document_is_deleting_or_deleted`
From `chat_streaming`: stream helpers
From `memory_helpers`: status/degrade helpers

## Baseline tests (2026-06-11)

- `cargo test -p app --lib`: 496 passed (Phase 1 baseline)
- T19 complete: `rag-core` no longer depends on `storage-pg`

## Phase 2 migration (2026-06-11)

### Moved to app-documents
- `DocumentContext`: documents, notebooks, url_imports, ingest enqueue, asset helpers, validate_rag_doc_scope
- `PgContentStore` adapter

### Moved to app-chat
- `ChatContext`: chat pipeline, sessions, chat_streaming, rag_execute, memory_helpers, chat_private (chat/RAG), orchestrator/llm context, token_budget, ChatService

### Moved to app-admin
- User preferences on `AdminContext`

### Remaining in app facade
- `state_methods.rs` bootstrap / search / analytics product events
- `assets_notifications.rs` citation lookup + thin admin delegates
- `lib_impl/tests.rs`, `config_helpers` bootstrap helpers

### Phase 2 verification
- `cargo check --workspace`: pass
- `cargo test -p app-chat --lib`: 469 passed
- `cargo test -p transport-http`: 50 passed
