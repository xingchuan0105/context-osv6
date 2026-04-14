# storage-qdrant

Fail-closed vector search contract for Qdrant-facing operations.

M1 + M2 goals:
- never issue a search request without `org_id`
- keep Qdrant as a candidate-recall cache, not an authority
- expose explicit request and filter shapes that the app crate can adapt to a concrete client later
