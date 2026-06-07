# Args Schema

The full JSON Schema for `graph-retrieval` args. This is the most
complex retrieval tool — 7 fields, one required.

```json
{
  "type": "object",
  "properties": {
    "graph_hints": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "subject":   { "type": "string" },
          "predicate": { "type": "string" },
          "object":    { "type": "string" }
        }
      },
      "minItems": 1,
      "description": "Hints for graph traversal."
    },
    "placeholder_triplets": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "subject":   { "type": "string" },
          "predicate": { "type": "string" },
          "object":    { "type": "string" }
        },
        "required": ["subject", "predicate", "object"]
      },
      "default": [],
      "description": "Triplets with placeholders for unknown entities."
    },
    "relation_limit": {
      "type": "integer",
      "default": 20,
      "minimum": 1,
      "maximum": 200,
      "description": "Total relations across all hops."
    },
    "supporting_chunk_limit": {
      "type": "integer",
      "default": 10,
      "minimum": 1,
      "maximum": 50,
      "description": "Chunks to retrieve per relation."
    },
    "hop_limit": {
      "type": "integer",
      "default": 1,
      "minimum": 1,
      "maximum": 3,
      "description": "Max graph hops."
    },
    "fan_out_limit": {
      "type": "integer",
      "default": 10,
      "minimum": 1,
      "maximum": 20,
      "description": "Max fan-out per hop."
    },
    "query": {
      "type": "string",
      "description": "Optional original query for reranking relation paths."
    }
  },
  "required": ["graph_hints"]
}
```

## Field constraints

### `graph_hints` (required, array, ≥1)

Each entry is `{subject, predicate, object}`. Use this field when
**the relation type (predicate) is known**. The runtime traverses
the edge in the direction subject → object.

- The `predicate` MUST be a concrete relation name. Do NOT use
  `?` or placeholders in the `predicate` here — use
  `placeholder_triplets` instead.
- `subject` or `object` may be `?` or named placeholders
  (`?owner`, `?service`) for unknown entities.
- 1 hint is enough to start; add more to disambiguate when
  multiple edges could match.

### `placeholder_triplets` (optional, array)

Use this when **the relation type (predicate) is unknown** or when
**both entity and relation are uncertain**. The runtime resolves
the missing pieces from the workspace graph schema.

- E.g. "find all X owned by Y" → `subject: "X"`, `object: "Y"`,
  `predicate: "?"`.
- Do NOT mix `graph_hints` and `placeholder_triplets` in the same
  call unless the query explicitly requires both modes. Pick one
  field that matches your knowledge state.

### `relation_limit` (optional, default 20, range [1, 200])

Total relations across all hops. Lower for narrow searches,
higher for broad. Cap is 200 to prevent runaway queries.

### `supporting_chunk_limit` (optional, default 10, range [1, 50])

How many chunks to fetch per matched relation. Default 10 is
the safe choice.

### `hop_limit` (optional, default 1, range [1, 3])

- `1`: direct relations only.
- `2`: A → B → C.
- `3`: A → B → C → D (max). Latency scales combinatorially.

### `fan_out_limit` (optional, default 10, range [1, 20])

Max outgoing edges to follow per node. Default 10; raise to 20
only for very dense graphs.

### `query` (string, **strongly recommended**)

The user's original query. **Always provide this when it is
available.** It is used to rerank relation paths and align
results to user intent. Do not omit just because it feels
optional.

### Tuning strategy when results are too wide or slow

If a call returns too many results or times out, tighten in this
order:
1. Reduce `hop_limit` (has the biggest combinatorial impact).
2. Reduce `fan_out_limit`.
3. Reduce `relation_limit`.
If the graph remains too noisy after tightening, fall back to
`dense-retrieval`.

## Output schema

Array of result objects. Each result exposes the matched relation
and its supporting chunk for traceability:

```json
{
  "type": "array",
  "items": {
    "type": "object",
    "properties": {
      "relation": {
        "type": "object",
        "properties": {
          "subject":   { "type": "string" },
          "predicate": { "type": "string" },
          "object":    { "type": "string" },
          "hop_count": { "type": "integer" },
          "path":      { "type": "array", "items": { "type": "string" } }
        }
      },
      "chunk": {
        "type": "object",
        "properties": {
          "chunk_id": { "type": "string" },
          "doc_id":   { "type": "string" },
          "text":     { "type": "string" },
          "score":    { "type": "number" },
          "page":     { "type": "integer" },
          "source":   { "type": "string", "enum": ["graph_retrieval"] }
        }
      }
    }
  }
}
```
