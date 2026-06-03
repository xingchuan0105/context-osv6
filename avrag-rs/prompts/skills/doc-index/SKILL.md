---
name: doc-index
description: "Load when the retrieval planner needs the document's section structure (chapter list with chunk IDs) to plan a precise index_lookup. Triggers: section-level precision reading where the user names a section, or any time the planner wants to know 'which chunks belong to which sections' before targeting. Skip when the planner only needs broad doc-level context (use doc-summary) or paraphrased recall (use dense-retrieval)."
version: "1.0"
depends: []
category: "retrieval-tool"
applicable_strategies: ["rag"]
risk_level: "low"
required_tools: []
---

You are the `doc_index` tool. Read the LLM-generated document
index — a list of sections with their chunk IDs.

When to call:
- The planner needs to know which chunks belong to which
  sections before targeted retrieval.
- The user names a specific section ("read chapter 3", "the
  'Antifragility Connection' section").
- The planner intends to follow up with `index_lookup` and
  needs valid chunk IDs.

When NOT to call (use a different tool instead):
- The planner only needs broad doc-level context → `doc-summary`.
- The query is a paraphrased factual question → `dense-retrieval`.
- Only basic file info is needed → `doc_metadata`.

## Args

- `doc_ids` (required, array of strings, ≥1): document UUIDs to
  read the index for.

## Output

Array of document indices:

```json
[
  { "doc_id": "uuid",
    "index": [
      { "title": "Definition", "level": 1, "chunk_ids": ["uuid1", "uuid2"] },
      { "title": "Key Principles", "level": 1, "chunk_ids": ["uuid3", ...] },
      { "title": "3. Antifragility Connection", "level": 2, "chunk_ids": [...] }
    ] }
]
```

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call.
You execute the call. **Always call `doc-index` BEFORE
`index_lookup` for the same document** — the chunk IDs returned
here are the only valid inputs to `index_lookup`.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md` — why doc-index then index_lookup
- `reference/gotchas.md` — LLM-generated index staleness
- `reference/examples.md`
