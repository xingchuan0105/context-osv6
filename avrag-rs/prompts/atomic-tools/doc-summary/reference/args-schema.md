# Args Schema

The full JSON Schema for `doc-summary` args.

```json
{
  "type": "object",
  "properties": {
    "doc_ids": {
      "type": "array",
      "items": { "type": "string", "format": "uuid" },
      "minItems": 1,
      "description": "Document UUIDs to read summaries for."
    },
    "level": {
      "type": "string",
      "enum": ["doc", "section"],
      "default": "doc",
      "description": "'doc' for full-document narrative summary, 'section' for section-level TOC entries."
    }
  },
  "required": ["doc_ids"]
}
```

## Field constraints

### `doc_ids` (required, array of UUIDs, ≥1)

Each entry must be a valid document UUID. Invalid UUIDs return
an error. The runtime enforces access scope (`doc_scope`) at the
HTTP layer; this tool only returns summaries for docs the caller
is allowed to read.

### `level` (optional, default `"doc"`)

- `"doc"`: full-document narrative summary (one paragraph per
  document). The `summary` field is populated.
- `"section"`: section-level TOC entries (one per section with
  `section_title`, `heading_level`, `page`). The `summary`
  field is empty; sections are listed in `section_title` etc.

Strict union semantics:
- When `level` is `"doc"`, `section_title`, `heading_level`, and `page`
  MUST be omitted or empty.
- When `level` is `"section"`, `summary` MUST be omitted or empty.

The output schema differs by level. The runtime always returns
the union; the planner/agent picks the relevant fields. To avoid
downstream validation ambiguity, fields that do not apply to the
chosen level MUST be omitted or set to empty.

## Output schema

```json
{
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "doc_id":         { "type": "string" },
      "level":          { "type": "string", "enum": ["doc", "section"] },
      "summary":        { "type": "string", "description": "Populated when level='doc'." },
      "section_title":  { "type": "string", "description": "Populated when level='section'." },
      "heading_level":  { "type": "integer", "description": "Populated when level='section'." },
      "page":           { "type": "integer", "description": "Populated when level='section'." }
    }
  }
}
```
