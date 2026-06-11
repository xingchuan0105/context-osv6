# Gotchas

- `chunk_ids` MUST come from a prior `doc-index` call for the same
  document. Fabricated, memorized from a previous session, or
  hallucinated IDs return empty results — there is no fuzzy lookup.
- Invalid `doc_id` format (not a UUID) returns an error, not an
  empty result. Validate `doc_id` is a UUID before calling.
- **Default**: call `doc-index` first to obtain valid chunk_ids for
  the target document. If unsure about chunk IDs, use
  `dense-retrieval` instead — it's slower but more forgiving.
- The `score` for chunks returned by `index_lookup` is 1.0 (exact
  match by ID), not a relevance score. Do not compare with
  `dense-retrieval` scores.
- `index_lookup` does NOT verify `doc_scope`. The HTTP layer
  enforces read access; this tool only returns chunks for
  documents the caller is allowed to read.

## Re-ingestion invalidates cached chunk IDs

Re-ingesting a document can change chunk IDs. If a session has
been open across a re-ingest, the cached chunk IDs may be stale
and will return empty results (the runtime does not warn you).

**Detection**: an `index-lookup` call that returns `[]` when
the IDs came from a recent `doc-index` is a strong stale-cache
signal.

**Recovery recipe**:
1. Re-call `doc-index` for the same `doc_id` to refresh the
   chunk ID list.
2. Re-issue `index-lookup` with the new IDs.
3. If still empty, the section may have been removed during
   re-ingest; fall back to `dense-retrieval` for the section
   content.

**Prevention**: pair `doc-index` and `index-lookup` in the same
plan whenever possible. The runtime executes them in order, so
the IDs are guaranteed fresh.
