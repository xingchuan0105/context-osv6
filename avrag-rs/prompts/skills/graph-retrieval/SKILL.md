---
name: graph-retrieval
description: "Load when the retrieval planner needs to traverse entity relationships: ownership, dependency, lineage, authorship, responsibility, comparison, connection paths, or cause/effect across entities. Triggers for multi-hop questions where chunk retrieval alone may miss the connecting link. Skip when the question is about a single fact in a chunk (use dense-retrieval)."
version: "1.0"
depends: []
category: "retrieval-tool"
applicable_strategies: ["rag"]
risk_level: "medium"
required_tools: []
---

You are the `graph_retrieval` tool. Traverse the workspace
knowledge graph and retrieve relations plus supporting chunks.

When to call:
- The question is about relationships between entities:
  ownership, dependency, lineage, authorship, responsibility,
  comparison, connection paths, cause/effect.
- Multi-hop reasoning is required: "which service owns the
  billing pipeline that handles the rollback for the system
  running on Atlas?"
- A dense-retrieval pass already returned relevant chunks but
  the answer requires connecting them via graph edges.

When NOT to call (use a different tool instead):
- The question is about a single fact contained in a chunk
  → `dense-retrieval` or `lexical-retrieval`.
- You need broad doc-level context first → `doc-summary`.
- The user only wants file metadata → `doc_metadata`.

## Args

- `graph_hints` (required, array, ≥1): triplet hints for graph
  traversal, each `{subject, predicate, object}`. Use '?' or named
  placeholders such as '?owner', '?service' for unknown positions.
- `placeholder_triplets` (optional, array): triplets with
  placeholders for unknown entities.
- `relation_limit` (optional, integer, default 20): total relations
  across all hops.
- `supporting_chunk_limit` (optional, integer, default 10): chunks
  to retrieve per relation.
- `hop_limit` (optional, integer, default 1, max 3): max graph hops.
- `fan_out_limit` (optional, integer, default 10, max 20): max
  fan-out per hop.
- `query` (optional, string): original query, used for reranking
  relation paths.

## Output

Array of chunk objects, each from a relation's supporting context:

```json
[
  { "chunk_id": "uuid", "doc_id": "uuid", "text": "...",
    "score": 0.87, "page": 12, "source": "graph_retrieval" }
]
```

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call.
You execute the call. The planner typically pairs `graph-retrieval`
with `dense-retrieval` (graph for structure, dense for chunks).

For detailed guidance, see:
- `reference/args-schema.md` — full schema with all 7 fields
- `reference/decision-rules.md` — when to combine with dense_retrieval
- `reference/gotchas.md` — hop_limit latency, broad hints, etc.
- `reference/examples.md` — good triplet signatures
