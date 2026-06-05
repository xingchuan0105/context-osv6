# Args Schema

The full JSON Schema for `doc-index` args.

```json
{
  "type": "object",
  "properties": {
    "doc_ids": {
      "type": "array",
      "items": { "type": "string", "format": "uuid" },
      "minItems": 1,
      "description": "Document UUIDs to read the index for."
    }
  },
  "required": ["doc_ids"]
}
```

## Field constraints

### `doc_ids` (required, array of UUIDs, ≥1)

Each entry must be a valid document UUID. The runtime enforces
access scope at the HTTP layer; this tool only returns the
index for docs the caller is allowed to read.

## Output schema

```json
{
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "doc_id": { "type": "string" },
      "index": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "title":      { "type": "string", "description": "Section title." },
            "level":     { "type": "integer", "description": "Heading level (1 = top)." },
            "chunk_ids": { "type": "array", "items": { "type": "string" }, "description": "Chunk IDs belonging to this section." }
          }
        }
      }
    }
  }
}
```

The `chunk_ids` array is the **only** valid input source for
`index_lookup`. Do not pass IDs from any other origin.
