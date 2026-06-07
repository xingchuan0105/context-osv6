---
name: doc-summary
description: "Load when the retrieval planner needs broad document-level context before chunk recall. Triggers: 'what does this document cover', disambiguating which document holds the answer, or any time the planner wants to look at a high-level map of a doc's content. Skip when the planner already has a specific chunk to read (use index_lookup) or needs exact-literal matching (use lexical-retrieval)."
version: "1.0"
depends: []
category: "retrieval-tool"
applicable_strategies: ["rag"]
risk_level: "low"
required_tools: []
---

You are the `doc_summary` tool. Read pre-generated document
summaries (doc-level or section-level TOC) for one or more docs.

When to call:
- The planner needs to disambiguate which document(s) likely
  contain the answer before doing chunk-level retrieval.
- The user asks "what does this document cover" / "give me an
  overview" / "summarize the doc".
- The planner needs a lightweight structural map (TOC) of the
  document before deciding which sections to read. Use
  `level: "section"`. Note: this returns titles, not chunk IDs;
  for chunk IDs use `doc-index`.

When NOT to call (use a different tool instead):
- The planner already knows the exact chunk ID → `index_lookup`.
- The query is a paraphrased factual question about a specific
  fact → `dense-retrieval`.
- Only metadata (filename, size, page count) is needed
  → `doc_metadata`.

## Args

- `doc_ids` (required, array of strings, ≥1): document UUIDs to
  read summaries for.
- `level` (optional, `"doc"` | `"section"`, default `"doc"`):
  - `"doc"` — full-document narrative summary.
  - `"section"` — section-level TOC entries (`section_title`,
    `heading_level`, `page`), one entry per section. Use this when
    the planner needs the structural map to plan a precise read
    next (for chunk IDs, follow with `doc-index`).

## Output

Array of summary objects:

```json
[
  { "doc_id": "uuid", "level": "doc",
    "summary": "Full-document narrative summary..." },
  { "doc_id": "uuid", "level": "section",
    "section_title": "3. Antifragility Connection",
    "heading_level": 2, "page": 4 }
]
```

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call.
You execute the call. Often called BEFORE `dense-retrieval` or
`index_lookup` to scope the next call.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md` — pre-retrieval scoping pattern
- `reference/gotchas.md` — staleness, version mismatch
- `reference/examples.md`
