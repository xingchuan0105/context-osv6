# Gotchas

- Only reads metadata, NOT document content. For content, use
  `dense-retrieval`, `lexical-retrieval`, `index_lookup`, or
  `doc-summary`.
- Empty `doc_ids` returns empty metadata.
- The `fields` filter restricts which keys are returned; omit
  `fields` for the complete metadata object. Common subsets:
  - `['name', 'mime_type']` for a "what kind of file" question
  - `['chunk_count', 'status']` for an "is this ready" question
  - `['toc']` for a section-level map
- The `status` field reflects the latest ingestion state. Values:
  `pending` | `processing` | `completed` | `failed`. If `failed`,
  the document's chunks are not retrievable — the planner should
  refuse the query or fall back to a different document.
- `file_size` is in bytes. For human-readable size, format it
  client-side.
- `chunk_count` is approximate during re-ingestion. A document
  just re-uploaded may show the old `chunk_count` for a few
  seconds.
