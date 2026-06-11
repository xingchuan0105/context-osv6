# Gotchas

## Chunk ID source of truth

**The chunk IDs returned by `doc_index` are the ONLY valid inputs to `index_lookup`.**

- IDs from memory, cache, user input, or previous sessions are **invalid** unless you can confirm the document was NOT re-ingested since.
- If `index_lookup` returns empty results with old IDs, the IDs are likely stale. Re-call `doc_index` first.

## Cache validity and re-ingestion

Chunk IDs can be cached within a single session **only if**:
1. The document was NOT re-ingested since the last `doc_index` call.
2. The `doc_index` call and the `index_lookup` call are in the same plan or closely sequenced.

**If `doc_index` and `index_lookup` are separated by a long gap** (e.g., across multiple turns, after a re-ingest, or after a session break), **re-call `doc_index` before `index_lookup`**. Stale IDs silently return empty results.

## Output structure details

- **Order**: Sections are returned in document order (top to bottom).
- **Prefaces**: Un-numbered prefaces may appear as `level: 0`.
- **Flat documents**: If a document has no headings, the index contains a single root entry with `level: 1`.
- **Duplicate titles**: Duplicate section titles are NOT merged. Each occurrence is a separate entry with its own `chunk_ids`.
- **No content**: The output contains section titles and chunk IDs only. It does NOT contain the section body text.

## Empty and error inputs

- **Empty `doc_ids` array**: Runtime error (`missing required field`).
- **`doc_id` not found**: Returns `error: { code: "DOC_NOT_FOUND" }` inline.
- **No read permission**: Returns `error: { code: "ACCESS_DENIED" }` inline.
- **Unparseable document** (scanned image without OCR, corrupted file): Returns `error: { code: "INDEX_EMPTY" }` or `error: { code: "PARSE_FAILED" }` inline.

## Large documents

For very large documents (1000+ sections), the response can be long. If only a few sections are needed, follow up with `index_lookup` to fetch specific chunks, not the whole index.
