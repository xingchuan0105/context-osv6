# Args Schema

The full JSON Schema for `index-lookup` args.

```json
{
  "type": "object",
  "properties": {
    "doc_id": {
      "type": "string",
      "format": "uuid",
      "description": "Target document UUID."
    },
    "chunk_ids": {
      "type": "array",
      "items": { "type": "string", "format": "uuid" },
      "minItems": 1,
      "description": "Exact chunk UUIDs to fetch (from doc_index)."
    }
  },
  "required": ["doc_id", "chunk_ids"]
}
```

## Field constraints

### `doc_id` (required, UUID)

Target document UUID. The `doc_id` is used as a cache key and
ordering hint. It does NOT act as a strict filter on `chunk_ids`;
see Ownership semantics below.

### `chunk_ids` (required, array of UUIDs, ≥1)

Each entry is a chunk UUID. **MUST come from a prior `doc-index`
call for the same `doc_id`**. Sources of invalid chunk_ids:
- Made-up UUIDs (return empty results for that ID)
- Stale IDs from a re-ingested document (return empty results)
- IDs from a previous session after re-ingest (stale)

### Ownership semantics

- The **HTTP layer** enforces `doc_scope` (read access) — chunks
  for documents the caller cannot read are not returned.
- The **tool layer** does NOT cross-validate `chunk_ids` against
  the supplied `doc_id`. If you pass IDs from a different doc,
  the runtime will look them up in the document they actually
  belong to (if accessible) — i.e. the `doc_id` arg is a
  **hint for ordering/caching, not a strict filter**.
- Practical effect: always pass IDs from a single document,
  matching the `doc_id` you supply, to avoid surprising results.

## Output schema

```json
{
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "chunk_id": { "type": "string" },
      "doc_id":   { "type": "string" },
      "text":     { "type": "string" },
      "score":    { "type": "number", "description": "Always 1.0 for direct lookup." },
      "page":     { "type": "integer" },
      "source":   { "type": "string", "description": "Always 'index_lookup'." }
    }
  }
}
```

Results are returned in the same order as the `chunk_ids` array.
Missing or invalid IDs are silently skipped (no entry in output).

### Error shapes

`index-lookup` returns errors as a single object, NOT an array:

```json
{
  "error": {
    "code": "DOC_NOT_FOUND | ACCESS_DENIED | INVALID_DOC_ID | BAD_CHUNK_ID_FORMAT",
    "message": "Human-readable description."
  }
}
```

| Error code | When it happens | Caller action |
|------------|-----------------|---------------|
| `DOC_NOT_FOUND` | `doc_id` is a valid UUID but unknown to the workspace. | Verify the UUID, or re-run `doc-index` to discover the right one. |
| `ACCESS_DENIED` | Caller lacks read permission for the document. | Do not retry; inform the user. |
| `INVALID_DOC_ID` | `doc_id` is not a valid UUID format. | This is a programmer error — fix the caller, not the data. |
| `BAD_CHUNK_ID_FORMAT` | One or more `chunk_ids` are not valid UUIDs. | Re-call `doc-index` to obtain well-formed IDs. |
