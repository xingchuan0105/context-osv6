# Args Schema

The full JSON Schema for `lexical-retrieval` args.

```json
{
  "type": "object",
  "properties": {
    "terms": {
      "type": "array",
      "items": { "type": "string" },
      "minItems": 1,
      "description": "Exact strings to match verbatim."
    },
    "top_k": {
      "type": "integer",
      "default": 10,
      "minimum": 1,
      "maximum": 50,
      "description": "Number of top results to retrieve."
    }
  },
  "required": ["terms"]
}
```

## Field constraints

### `terms` (required, array of strings, ≥1)

Each entry is matched verbatim against chunk text via BM25.
The runtime does NOT pre-stem or pluralize — pass the exact form
the user (or you) expect to find in the text.

**Good**:
- "Atlas"
- "AUTH_SESSION_VERSION"
- "E-2047"
- "rollback checklist"
- "Barbell strategy"

**Bad**:
- "" (empty string) — runtime error
- "atlas system" if the doc has only "atlas" (both words
  must appear; "atlas" alone is sufficient and less fragile)
- "rollback" when the doc spells it "roll-back" — no match

### `top_k` (optional, default 10, range [1, 50])

Same semantics as `dense-retrieval`'s `top_k`. Default 10; lower
for narrow literal searches, higher for broad scans.

## Output schema

Same as `dense-retrieval`: array of chunk objects sorted by BM25
score descending. The `source` field will be `"lexical_retrieval"`.

```json
{
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "chunk_id": { "type": "string" },
      "doc_id":   { "type": "string" },
      "text":     { "type": "string" },
      "score":    { "type": "number", "description": "BM25 score, not directly comparable to dense scores." },
      "page":     { "type": "integer" },
      "source":   { "type": "string", "description": "Always 'lexical_retrieval'." }
    }
  }
}
```

Empty array if no terms match any chunk.

## Error shapes

Unlike `doc-index`, `lexical-retrieval` rarely errors at the
tool level because it operates over the whole workspace index.
Possible failure modes:

| Failure | Symptom | Caller action |
|---------|---------|---------------|
| `terms` is empty array | Returned array is `[]` (treated as no match) | Fix caller: always pass ≥1 term. |
| Index is rebuilding after bulk re-ingest | Returned array is `[]` for terms that should match | Wait a few seconds and retry once. |
| Workspace is empty (no docs indexed) | Returned array is `[]` for any term | Fall back to `doc-summary` on a known doc to confirm corpus state. |
| Caller lacks read access to the only matching docs | Returned array is `[]` (access-filtered silently) | Inform the user; not a tool bug. |

The runtime does NOT return an error object for any of the
above — `[]` is the only signal. Treat persistent `[]` on a
high-confidence term as a corpus/access issue, not a "no
results" answer.
