# Gotchas

- **MUST be called before `index_lookup` for the same document** to
  obtain valid chunk IDs. Calling `index_lookup` with IDs invented
  from the summary (or remembered from a previous session) returns
  empty results.
- Empty `doc_ids` returns an empty index.
- The index is LLM-generated. Chunk IDs may be stale if the
  document was re-indexed; in that case the index returned for the
  old `doc_id` may reference chunks that no longer exist.
- If a document has no `level` (flat structure), the index still
  works — `index` will contain a single root entry.
- For very large documents (1000+ sections), the response can be
  long. If only a few sections are needed, follow up with
  `index_lookup` to fetch specific chunks, not the whole index.
- The `level` field in the response is the heading level (1 = top).
  Sections with `level: 0` may appear for un-numbered prefaces.
