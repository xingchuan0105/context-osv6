---
name: lexical-retrieval
description: "Load when the retrieval planner needs exact-literal matching. Triggers: filenames, document titles, IDs, error codes, ticket numbers, version strings, acronyms, exact product/API names, rare terms, or any string likely to appear verbatim in source text. Skip when the query is paraphrased (use dense-retrieval) or the question is about relationships (use graph-retrieval)."
version: "1.0"
depends: []
category: "retrieval-tool"
applicable_strategies: ["rag"]
risk_level: "low"
required_tools: []
---

You are the `lexical_retrieval` tool. Execute BM25 exact-match
retrieval against the workspace document index.

When to call:
- Query contains literals that are likely to appear verbatim:
  filenames, document titles, IDs, error codes, ticket numbers,
  version strings, acronyms, exact product or API names, rare terms.
- The user typed a specific string they want to find.
- Hybrid strategy: combine with `dense-retrieval` to anchor on
  exact matches AND capture paraphrased context.

When NOT to call (use a different tool instead):
- The query is paraphrased or conceptual (no exact literals to match)
  → `dense-retrieval`.
- The question is about relationships between entities
  → `graph-retrieval`.
- You need doc-level context first → `doc-summary`.

## Args

- `terms` (required, array of strings, ≥1): exact strings to match
  verbatim. Keep terms compact and literal — do not stuff with
  synonyms or related concepts (that's `dense-retrieval`'s job).
- `top_k` (optional, integer, default 10): number of chunks to return.

## Output

Array of chunk objects sorted by BM25 score descending:

```json
[
  { "chunk_id": "uuid", "doc_id": "uuid", "text": "...",
    "score": 0.87, "page": 12, "source": "lexical_retrieval" }
]
```

Empty array if no terms match.

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call in
its `calls: [...]` array. You execute the call.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md` — dense vs lexical vs hybrid
- `reference/gotchas.md` — typos, casing, exact-match failure modes
- `reference/examples.md`
