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

**Scope boundary**: You召回语义相关的 chunks。You do NOT do exact-literal matching, do NOT reason over entity relationships, and do NOT produce the final answer.

## Input

- `queries` (required, array of strings, ≥1): One or more semantic queries. Prefer full sentences; short precise phrases are also valid. See `reference/args-schema.md` for query shaping guidance.
- `modality` (optional, `"text"` | `"mm"` | `"both"`, default `"text"`): Retrieval modality.
- `top_k` (optional, integer, default 10): Number of chunks to return. Max 50.

## Output

Array of chunk objects sorted by relevance score descending:

```json
[
  { "chunk_id": "uuid", "doc_id": "uuid", "text": "...",
    "score": 0.87, "page": 12, "source": "dense_retrieval" }
]
```

Empty array if no chunks exceed the relevance threshold.

**Note**: Empty results do NOT automatically mean "nothing found". They may also indicate embedding/index issues — see `reference/gotchas.md` for the empty-result triage flow.

## When you are called

The `retrieval-planner` decides whether to include this call in
its `calls: [...]` array. You execute the call; you do NOT plan.

For detailed guidance, see:
- `reference/args-schema.md` — full JSON schema with constraints
- `reference/decision-rules.md` — when this beats lexical/graph
- `reference/gotchas.md` — failure modes, rate limits, empty-result triage
- `reference/examples.md` — good call signatures
