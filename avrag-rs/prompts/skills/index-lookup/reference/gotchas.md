# Gotchas

- `chunk_ids` MUST come from a prior `doc-index` call for the same
  document. Invented, memorized-from-previous-session, or
  guessed IDs return empty results — there is no fuzzy lookup.
- Invalid `doc_id` format (not a UUID) returns an error, not an
  empty result. Validate `doc_id` is a UUID before calling.
- **Always call `doc-index` first** to obtain valid chunk_ids for
  the target document. If unsure about chunk IDs, use
  `dense-retrieval` instead — it's slower but more forgiving.
- The `score` for chunks returned by `index_lookup` is 1.0 (exact
  match by ID), not a relevance score. Do not compare with
  `dense-retrieval` scores.
- Re-ingesting a document can change chunk IDs. Cache invalidation
  is the planner's responsibility — if a session has been open
  across a re-ingest, the cached chunk IDs may be stale.
- `index_lookup` does NOT verify access scope (`doc_scope`). The
  HTTP layer enforces access; this tool only returns chunks for
  documents the caller is allowed to read.
