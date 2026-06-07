# Examples

Good call signatures for `graph-retrieval`. Always pair with
`dense-retrieval` in the same plan; the graph call alone is rarely
useful.

## Example 1: Direct relationship

**Context**: User asks "who owns the Atlas system?"

```json
[
  {
    "tool": "graph_retrieval",
    "version": "1.0",
    "args": {
      "graph_hints": [
        { "subject": "Atlas", "predicate": "owned_by", "object": "?" }
      ],
      "hop_limit": 1,
      "query": "Atlas system ownership"
    }
  },
  {
    "tool": "dense_retrieval",
    "version": "1.0",
    "args": {
      "queries": ["Atlas system ownership team responsible"],
      "top_k": 5
    }
  }
]
```

`hop_limit: 1` (default) — direct ownership edge. `?` for the
unknown owner. Always pass `query` to help rerank the matched
relations.

## Example 2: Multi-hop chain

**Context**: User asks "which team is responsible for the rollback
of the system that runs on Atlas?"

```json
[
  {
    "tool": "graph_retrieval",
    "version": "1.0",
    "args": {
      "graph_hints": [
        { "subject": "Atlas", "predicate": "runs_on", "object": "?" },
        { "subject": "?", "predicate": "rollback_owned_by", "object": "?team" }
      ],
      "hop_limit": 2,
      "query": "Atlas rollback ownership chain"
    }
  },
  {
    "tool": "dense_retrieval",
    "version": "1.0",
    "args": {
      "queries": ["Atlas rollback procedure ownership"],
      "top_k": 10
    }
  }
]
```

`hop_limit: 2` for the two-step chain. Uses only `graph_hints`
because both relation types (`runs_on`, `rollback_owned_by`)
are known. The dense call provides the actual chunk text for
the answer synthesizer.

## Example 3: Predicate-unknown lookup

**Context**: User asks "find all X that depend on Y"

```json
[
  {
    "tool": "graph_retrieval",
    "version": "1.0",
    "args": {
      "placeholder_triplets": [
        { "subject": "X", "predicate": "?", "object": "Y" }
      ],
      "hop_limit": 1,
      "relation_limit": 50,
      "query": "components depending on Y"
    }
  }
]
```

Use `placeholder_triplets` (not `graph_hints`) when you don't
know the predicate. The runtime picks the most relevant edge
type from the workspace graph schema.

## Example 4: Comparison via shared dependency

**Context**: User asks "how does service A compare to service B in
terms of dependencies?"

```json
[
  {
    "tool": "graph_retrieval",
    "version": "1.0",
    "args": {
      "graph_hints": [
        { "subject": "service A", "predicate": "depends_on", "object": "?" },
        { "subject": "service B", "predicate": "depends_on", "object": "?" }
      ],
      "hop_limit": 2,
      "relation_limit": 30,
      "query": "service A and B dependency comparison"
    }
  },
  {
    "tool": "dense_retrieval",
    "version": "1.0",
    "args": {
      "queries": ["service A service B comparison dependencies"],
      "top_k": 10
    }
  }
]
```

Two parallel hints, one per service. `hop_limit: 2` to follow
transitive dependencies. The dense call provides the actual
content to compare.

## Example 5: Narrow, no unknown placeholders

**Context**: User asks "which documents reference the Atlas API?"

```json
[
  {
    "tool": "graph_retrieval",
    "version": "1.0",
    "args": {
      "graph_hints": [
        { "subject": "?", "predicate": "references", "object": "Atlas API" }
      ],
      "hop_limit": 1,
      "supporting_chunk_limit": 5,
      "query": "documents referencing Atlas API"
    }
  }
]
```

`?` for the unknown subject (the documents). `supporting_chunk_limit: 5`
because we want fewer-but-relevant chunks per relation (we
just need to identify which docs, then `dense-retrieval` for details).
