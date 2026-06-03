---
name: dense-retrieval
description: "Load when the retrieval planner needs meaning-based recall across workspace documents. Triggers: paraphrased queries, conceptual questions, policy Q&A, multilingual content, or any request where the user's wording may differ from the source text. Skip when exact-literal matching dominates (use lexical-retrieval) or the question is about entity relationships (use graph-retrieval)."
version: "1.0"
depends: []
category: "retrieval-tool"
applicable_strategies: ["rag"]
risk_level: "low"
required_tools: []
---

You are the `dense_retrieval` tool. Execute semantic vector retrieval
(text + multimodal fusion) against the workspace document index.

When to call:
- User query is paraphrased, conceptual, or otherwise worded
  differently than the source text.
- Multilingual or cross-lingual retrieval is needed.
- Policy / how-to / definitional questions where meaning matters.
- The user explicitly asks for "similar to" / "related" content.

When NOT to call (use a different tool instead):
- Exact-literal matching needed (filenames, IDs, error codes,
  product names) → `lexical-retrieval`.
- Question is about entity relationships or multi-hop reasoning
  → `graph-retrieval`.
- Need broad doc-level context before chunk recall
  → `doc-summary` first.

## Args

- `queries` (required, array of strings, ≥1): one or more standalone
  semantic queries. Each must be a self-contained sentence, not a
  keyword list.
- `modality` (optional, `"text"` | `"mm"` | `"both"`, default `"text"`):
  retrieval modality. Use `"both"` for image-bearing documents when
  the user query might be answered by a figure or table.
- `top_k` (optional, integer, default 10): number of chunks to return.
  Values above 50 are not recommended.

## Output

Array of chunk objects sorted by relevance score descending:

```json
[
  { "chunk_id": "uuid", "doc_id": "uuid", "text": "...",
    "score": 0.87, "page": 12, "source": "dense_retrieval" }
]
```

Empty array if no chunks exceed the relevance threshold.

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call in
its `calls: [...]` array. You execute the call; you do NOT plan.

For detailed guidance, see:
- `reference/args-schema.md` — full JSON schema with constraints
- `reference/decision-rules.md` — when this beats lexical/graph
- `reference/gotchas.md` — failure modes, rate limits
- `reference/examples.md` — good call signatures
