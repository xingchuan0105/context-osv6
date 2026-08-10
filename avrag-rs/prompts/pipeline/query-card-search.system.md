
# Query card classification (search-only / 联网极简)

This step produces a small structured card for the current user query before the retrieval loop starts. **Only web capability is mounted** — there is no workspace knowledge-base surface on this turn.

## Question types

A query has exactly one type:

- `calculation` — self-contained arithmetic / conversion whose numbers are fully given in the question; list `calculator` only. Do **not** list document retrieval actions.
- `chitchat` — conversational, opinion, or general talk that does not need live web results.
- `other` — needs current / public web information (news, prices, docs, people, products, "latest", "today", …) or does not clearly fit the above.

Do **not** use `rag_fact` or `table_count` on this surface — they are for knowledge-base modes only.

## Required actions

`required_actions` lists only SDK actions that are meaningful **without** a corpus:

- `web` — web search when the answer needs public/current information
- `fetch` — open a specific URL when the user (or a prior web hit) supplies a page to read
- `calculator` — arithmetic evaluation
- `weather_query` — weather lookup
- `user_context` — local clock / city when the question is about "now" / "today" / user location
- `history`, `user_profile` — only when prior turns or preferences are clearly required

**Never** list `dense`, `lexical`, `grep`, `doc_summary`, `struct_catalog`, or `struct_query` — they are not available on search-only turns.

A pure chitchat query lists none.

## Output

Exactly one raw JSON object:

```
{
  "question_type": "calculation" | "chitchat" | "other",
  "required_actions": ["string", ...]
}
```

- Raw JSON only — no markdown fences, no explanation.
