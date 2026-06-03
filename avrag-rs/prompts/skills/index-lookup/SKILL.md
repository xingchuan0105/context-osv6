---
name: index-lookup
description: "Load when the retrieval planner needs to fetch specific chunks by their UUID. Triggers: the planner already called doc-index and has valid chunk IDs, or the user names a specific section/anchor. Skip when chunk IDs are unknown (use doc-index first, or fall back to dense-retrieval). This is the most precise retrieval tool — use it for surgical reads."
version: "1.0"
depends: []
category: "retrieval-tool"
applicable_strategies: ["rag"]
risk_level: "low"
required_tools: []
---

You are the `index_lookup` tool. Direct chunk ID lookup for
section-level precision reading. Fetch specific chunks by their
UUID from a target document.

When to call:
- The planner already called `doc-index` for the same document
  and has a list of valid chunk IDs.
- The user named a specific section or anchor, and the planner
  resolved it to a chunk ID.
- The planner needs surgical precision (e.g. read exactly chunks
  12, 13, 14) instead of semantic ranking.

When NOT to call (use a different tool instead):
- Chunk IDs are unknown — call `doc-index` first, or fall back
  to `dense-retrieval`.
- The query is paraphrased and chunk IDs aren't pre-resolved
  → `dense-retrieval`.
- Exact-literal match across the whole document is needed
  → `lexical-retrieval`.

## Args

- `doc_id` (required, string): target document UUID.
- `chunk_ids` (required, array of strings, ≥1): exact chunk UUIDs
  to fetch. **MUST come from a prior `doc-index` call for the same
  document.** Invented or stale IDs return empty results.

## Output

Array of chunk objects:

```json
[
  { "chunk_id": "uuid", "doc_id": "uuid", "text": "...",
    "score": 1.0, "page": 12, "source": "index_lookup" }
]
```

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call.
You execute the call. **Always preceded by `doc-index`** in
the same plan — the planner is responsible for sequencing.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md` — doc-index then index_lookup
- `reference/gotchas.md` — invented IDs, stale IDs
- `reference/examples.md`
