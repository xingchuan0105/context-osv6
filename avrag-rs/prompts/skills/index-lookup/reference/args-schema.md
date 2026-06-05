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

Target document UUID. The chunk_ids MUST belong to this document;
if any chunk_id is from a different document, the runtime returns
an error.

### `chunk_ids` (required, array of UUIDs, ≥1)

Each entry is a chunk UUID. **MUST come from a prior `doc-index`
call for the same `doc_id`**. Sources of invalid chunk_ids:
- Made-up UUIDs (return empty results for that ID)
- IDs from a different document (return error)
- Stale IDs from a re-ingested document (return empty results)
- IDs from a previous session after re-ingest (stale)

The runtime does NOT validate `chunk_ids` against `doc_id`
ownership — if you mix IDs from different docs, you get whatever
matches in each.

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
