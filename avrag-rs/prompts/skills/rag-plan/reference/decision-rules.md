# Decision Rules

## Tool selection matrix

| User query characteristic | Primary tool | Secondary tool | When to add secondary |
|---------------------------|--------------|----------------|----------------------|
| Natural-language question, no exact literals | `dense_retrieval` | — | — |
| Contains error codes, IDs, version strings, exact API names | `dense_retrieval` + `lexical_retrieval` | — | Add `lexical` to anchor on exact literals AND capture paraphrased context via `dense`. |
| Asks about relationships between entities ("how does X relate to Y") | `dense_retrieval` + `graph_retrieval` | — | Add `graph` only when the relationship is the core of the question, not incidental. |
| Broad overview ("what's in this doc about X") | `doc_summary` | `dense_retrieval` | Use `doc_summary` first for orientation, then `dense_retrieval` for detail if needed. |
| Needs structural context ("how is the doc organized", "list the sections") | `doc_metadata` | — | — |
| Explicitly names a section or chunk ("the Atlas rollback checklist") | `index_lookup` | — | Only when the user gives a specific section name or valid chunk IDs from a prior `doc_index`. |
| Mixed: broad + specific literals | `dense_retrieval` + `lexical_retrieval` | `doc_summary` | `doc_summary` only if the corpus is large and the user needs orientation first. |

## Anti-patterns

### Do NOT call every tool "just in case"

Bad:
```json
{
  "strategy": [
    { "tool": "dense_retrieval", "queries": ["..."], "modality": "text", "top_k": 10 },
    { "tool": "lexical_retrieval", "terms": ["..."], "top_k": 10 },
    { "tool": "graph_retrieval", "queries": ["..."], "top_k": 10 },
    { "tool": "doc_summary", "doc_ids": ["..."] }
  ]
}
```

This is over-engineering. Most queries need 1–2 tools. Extra tools add latency and noise.

### Do NOT use `lexical_retrieval` as a weaker duplicate of `dense_retrieval`

If the query has no exact literals, `lexical_retrieval` adds no signal. Use it only when literal matching is genuinely valuable.

### Do NOT treat session history as a retrieval source

Bad: using a `chunk_id` or `doc_id` from a previous turn as if it were a fresh retrieval result.

Session history is for **reference resolution** ("the doc you mentioned earlier"), not for evidence. Always issue fresh retrieval calls against the current query.

### Do NOT rewrite `doc_scope`

The `doc_scope` field is provided by the runtime based on user selection or workspace defaults. Do not add, remove, or rewrite document IDs in your output.

### Do NOT emit `calls` + `action: "clarify"` in the same object

These are mutually exclusive. Pick one shape.

Bad:
```json
{
  "action": "clarify",
  "message": "...",
  "strategy": [{ "tool": "dense_retrieval", ... }]
}
```

### Do NOT include `doc_scope` in the plan output

`doc_scope` lives on the request, not in your JSON. The runtime already knows it.

### Do NOT use near-duplicate queries

Bad:
```json
{
  "strategy": [
    { "tool": "dense_retrieval", "queries": ["Atlas rollback"], "top_k": 10 },
    { "tool": "dense_retrieval", "queries": ["Atlas rollback procedure"], "top_k": 10 }
  ]
}
```

These two queries overlap heavily. Merge them into one call with a better query, or drop the weaker one.
