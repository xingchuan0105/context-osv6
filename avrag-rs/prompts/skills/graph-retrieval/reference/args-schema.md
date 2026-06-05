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

Each entry is `{subject, predicate, object}`. The runtime traverses
the edge in the direction subject → object. The `predicate` may be
`?` or a named placeholder to let the graph resolve the relation
type from the schema.

- 1 hint is enough to start; add more to disambiguate when
  multiple edges could match.
- Use `?` or placeholders (`?owner`, `?service`, `?document`)
  for unknown slots. The runtime fills these in.

### `placeholder_triplets` (optional, array)

Use this (NOT `graph_hints`) when you don't know the **predicate**.
E.g. "find all X owned by Y" → `subject: "X"`, `object: "Y"`,
`predicate: "?"`.

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

### `query` (optional, string)

Original user query, used to rerank the matched relation paths
by relevance. Always provide this if the relations are diverse
and the user query has specific keywords.

## Output schema

Array of chunk objects (the supporting chunks for the matched
relations), sorted by relevance to the `query` field if provided.
The `source` field will be `"graph_retrieval"`.
