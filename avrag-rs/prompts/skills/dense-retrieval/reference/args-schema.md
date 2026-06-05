# Args Schema

The full JSON Schema for `dense-retrieval` args, as enforced by
the runtime at the call boundary.

```json
{
  "type": "object",
  "properties": {
    "queries": {
      "type": "array",
      "items": { "type": "string" },
      "minItems": 1,
      "description": "One or more standalone semantic queries."
    },
    "modality": {
      "type": "string",
      "enum": ["text", "mm", "both"],
      "default": "text",
      "description": "Retrieval modality."
    },
    "top_k": {
      "type": "integer",
      "default": 10,
      "minimum": 1,
      "maximum": 50,
      "description": "Number of top results to retrieve."
    }
  },
  "required": ["queries"]
}
```

## Field constraints

### `queries` (required, array of strings, ≥1)

Each entry is a standalone sentence the embedding model will vectorize.
The runtime does NOT combine them — each query produces its own ranked
list, and the merger combines via Reciprocal Rank Fusion (RRF).

**Good**:
- "What is the Barbell strategy in finance?"
- "Antifragility vs resilience"
- "Why does Taleb criticize modern finance"

**Bad** (caught at runtime or wastes recall):
- "" (empty string) — runtime error
- "antifragility, taleb, finance" (keyword list) — works but suboptimal
- A 200-word paragraph — wastes tokens, no recall gain

### `modality` (optional, default `"text"`)

- `"text"` (default): text chunks only.
- `"mm"`: image-bearing chunks only (figures, tables with embedded
  images, OCR'd diagrams).
- `"both"`: text + image chunks, fused by embedding model. Use
  when the query might be answered by a figure ("what does the
  table on page 14 show about channel budgets?").

If the workspace has no image-bearing chunks, `"mm"` and `"both"`
degrade gracefully to `"text"`.

### `top_k` (optional, default 10, range [1, 50])

- Default 10 is the safe choice for most queries.
- Lower (5) for narrow questions where you expect a single answer.
- Higher (20-30) for broad questions where the answer is scattered.
- Above 50 is rejected by the runtime. If you need more, issue a
  second narrower call instead.

## Output schema

The runtime returns an array of chunk objects, sorted by relevance
score descending:

```json
{
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "chunk_id":  { "type": "string", "description": "Unique chunk identifier." },
      "doc_id":    { "type": "string", "description": "Parent document identifier." },
      "text":      { "type": "string", "description": "Retrieved text content." },
      "score":     { "type": "number", "description": "Relevance score (higher is better)." },
      "page":      { "type": "integer", "description": "Page number in the source document." },
      "source":    { "type": "string", "description": "Tool that produced this chunk." }
    }
  }
}
```

Empty array if no chunks exceed the relevance threshold.
