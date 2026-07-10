# storage-pg

Tenant-safe PostgreSQL access helpers.

Current architecture note:
- Postgres is the product control plane: users (personal accounts), workspaces, auth/session, chat history, agent memory metadata, ingestion jobs, audit, usage, billing, and document lifecycle state.
- Milvus is the target retrieval data plane for BM25 sparse, vectors, multimodal chunks, and graph relation retrieval.
- See [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

Key constraints encoded here:
- all business queries must execute inside a transaction with `app.current_user` configured
- PostgreSQL remains the authority for permissions, body retrieval, and audit
- callers should fail closed when `owner_user_id` is missing or mismatched
