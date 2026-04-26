# storage-qdrant

> Status: legacy/current-implementation adapter.
> The 2026-04-26 target architecture moves retrieval indexing to Milvus: BM25 sparse, text dense, multimodal dense, and graph entity/relation/passages. See [Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md).

Fail-closed vector search contract for Qdrant-facing operations.

M1 + M2 goals:
- never issue a search request without `org_id`
- keep Qdrant as a candidate-recall cache, not an authority
- expose explicit request and filter shapes that the app crate can adapt to a concrete client later

Migration note:
- Existing Qdrant payload and ACL behavior should be preserved as a compatibility contract while the Milvus adapter is introduced.
- New product architecture work should not add new retrieval capabilities only to Qdrant.
