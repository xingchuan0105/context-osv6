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

### Success

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
            "level":     { "type": "integer", "description": "Heading level (1 = top). May be 0 for prefaces." },
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

### Error states

When a document cannot be indexed, the runtime returns an error entry
in place of the normal index:

```json
{
  "doc_id": "uuid",
  "error": {
    "code": "DOC_NOT_FOUND | ACCESS_DENIED | INDEX_EMPTY | PARSE_FAILED",
    "message": "Human-readable description."
  }
}
```

| Error code | When it happens | Caller action |
|------------|-----------------|---------------|
| `DOC_NOT_FOUND` | The `doc_id` does not exist in the workspace. | Verify the UUID or ask the user for the correct document. |
| `ACCESS_DENIED` | The caller lacks read permission for this document. | Do not retry; inform the user of the permission issue. |
| `INDEX_EMPTY` | The document exists but has no extractable sections (e.g., scanned image without OCR). | Try `doc-summary` or inform the user the document is not indexable. |
| `PARSE_FAILED` | The document could not be parsed (corrupted file, unsupported format). | Inform the user the document is unreadable. |

**Note**: Error entries are returned inline in the array, alongside
successful entries for other documents. Check for `"error"` before
reading `"index"`.
