# Args Schema

The full JSON Schema for `doc-metadata` args.

```json
{
  "type": "object",
  "properties": {
    "doc_ids": {
      "type": "array",
      "items": { "type": "string", "format": "uuid" },
      "minItems": 1,
      "description": "Document UUIDs to read metadata for."
    },
    "fields": {
      "type": "array",
      "items": { "type": "string" },
      "default": [],
      "description": "Optional filter. Omit or pass [] for all fields."
    }
  },
  "required": ["doc_ids"]
}
```

## Field constraints

### `doc_ids` (required, array of UUIDs, ≥1)

Each entry must be a valid document UUID. The runtime enforces
access scope at the HTTP layer.

### `fields` (optional, array of strings)

Filter restricting which keys are returned. Valid field names:

- `name` — file name (e.g. `"antifragile.pdf"`)
- `mime_type` — file MIME type (e.g. `"application/pdf"`)
- `file_size` — file size in bytes (format to human-readable for display)
- `status` — processing status (`pending` | `processing` | `completed` | `failed`)
- `chunk_count` — number of chunks (approximate during re-ingest)
- `toc` — table of contents array (sections with title, heading_level, page, rank)

**Semantics**:
- Omit `fields` → returns the complete metadata object.
- `fields: []` → equivalent to omitting `fields` (returns all fields).
- `fields: ["name", "status"]` → returns only `name` and `status`.

## Output schema

```json
{
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "doc_id":       { "type": "string" },
      "name":         { "type": "string" },
      "mime_type":    { "type": "string" },
      "file_size":    { "type": "integer", "description": "Size in bytes. Format to human-readable (KB/MB) for user display." },
      "status":       { "type": "string", "description": "pending | processing | completed | failed" },
      "chunk_count":  { "type": "integer", "description": "Approximate during re-ingest." },
      "toc": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "title":         { "type": "string" },
            "heading_level": { "type": "integer" },
            "page":          { "type": "integer" },
            "rank":          { "type": "integer" }
          }
        }
      }
    }
  }
}
```

Fields not requested via `fields` filter are omitted.
