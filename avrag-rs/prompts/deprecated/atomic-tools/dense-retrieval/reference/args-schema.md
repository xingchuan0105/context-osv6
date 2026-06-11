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
      "description": "One or more semantic queries."
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

Each entry is a query the embedding model will vectorize.
The runtime does NOT combine them — each query produces its own ranked
list, and the merger combines via Reciprocal Rank Fusion (RRF).

**Query shaping guidance**:
- **Prefer full sentences** for broad or conceptual questions: "What is the Barbell strategy in finance?"
- **Short precise phrases are also valid** for narrow lookups: "Barbell strategy", "session timeout default"
- **Avoid pure keyword lists** unless you understand the recall bias: "antifragility, taleb, finance" works but is usually suboptimal
- **Avoid overly long paragraphs** — they waste tokens without improving recall

**Good**:
- "What is the Barbell strategy in finance?"
- "Antifragility vs resilience"
- "Why does Taleb criticize modern finance"
- "Barbell strategy" (narrow, precise)

**Bad** (caught at runtime or wastes recall):
- "" (empty string) — runtime error
- A 200-word paragraph — wastes tokens, no recall gain

### `modality` (optional, default `"text"`)

- `"text"` (default): text chunks only.
- `"mm"`: image-bearing chunks only (figures, tables with embedded
  images, OCR'd diagrams).
- `"both"`: text and image chunks retrieved together, with results
  merged into a single ranked list via embedding-model fusion.

**Fusion strategy for `"both"`**:
- Text chunks and image chunks are embedded through their respective
  vector spaces, then scores are normalized and merged into a unified
  ranking. The returned list contains both text and image chunks
  interleaved by relevance.
- If the workspace has no image-bearing chunks, `"mm"` and `"both"`
  degrade gracefully to `"text"`.
- **Use `"both"` only when image-bearing chunks actually exist.**
  If the corpus is text-only, `"both"` is equivalent to `"text"`.

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
