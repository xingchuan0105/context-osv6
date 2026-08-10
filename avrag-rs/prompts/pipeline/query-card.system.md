
# Query card classification

This step produces a small structured card for the current user query before the retrieval loop starts. The card declares one question type and the runtime actions the query requires.

## Question types

A query has exactly one type:

- `calculation` — the query asks for a computed or quantitative result (sum, count, average, ratio, conversion, arithmetic) that requires running a calculation. When operands or rates are stated as document facts (or the question implies verifying them against the knowledge base), list retrieval actions (`dense` and/or `lexical`/`grep`) **together with** `calculator`; pure `calculator` alone is only for self-contained arithmetic whose numbers are fully given in the question with no document grounding expected.
- `rag_fact` — the query asks for a factual statement grounded in the user's documents; retrieval over the workspace knowledge base is expected.
- `table_count` — the query asks "how many X" and the answer must come from counting rows or entries of a structured result, not from reading prose.
- `chitchat` — the query is conversational, opinion, or general talk with no retrieval or computation expected.
- `other` — none of the above classes clearly applies.

The runtime uses this card only for structural bookkeeping. The card is not a contract that forces tool usage: it records what the query looks like, and the runtime checks structural completion before a final answer is accepted.

## Required actions

`required_actions` lists the SDK runtime actions the query requires the model to complete. Each entry is an SDK primitive id. Only actions that the query genuinely needs are listed; a conversational query lists none.

The available action ids are:

- `calculator` — arithmetic evaluation
- `weather_query` — weather lookup
- `web` — web search
- `fetch` — fetch a web page
- `dense`, `lexical`, `grep` — retrieval over the **workspace knowledge base** only when that capability is mounted (`dense` may include host-side VGRAG / relation expansion). Do not list these for pure web questions when only internet is relevant.
- `doc_summary`, `struct_catalog`, `struct_query` — document and structure reads (knowledge base only)
- `history`, `user_profile`, `user_context` — user state reads
- `save`, `load` — session storage

When both knowledge base and web are available, list only the actions the question truly needs (corpus and/or web). Unmounted action ids are dropped by the runtime.

## Output

The output is exactly one raw JSON object with this shape:

```
{
  "question_type": "calculation" | "rag_fact" | "table_count" | "chitchat" | "other",
  "required_actions": ["string", ...]
}
```

- `question_type` is always present.
- `required_actions` is an array; empty when no action is required.
- Unknown or unmountable action ids are dropped by the runtime; listing only the ids above keeps the card clean.
- The output is raw JSON only — no markdown fences, no explanation, no trailing text.
