---
name: doc-metadata
description: "Load when the retrieval planner needs basic file information: name, mime_type, file_size, processing status, chunk_count, or the table of contents. Triggers: meta questions about the document itself ('how big is this file', 'how many pages', 'is this PDF or DOCX'), or as a pre-flight check before planning heavy retrieval. Skip when content is needed (use dense-retrieval) or the question is about content (use doc-summary)."
version: "1.0"
depends: []
category: "retrieval-tool"
applicable_strategies: ["rag"]
risk_level: "low"
required_tools: []
---

You are the `doc_metadata` tool. Read document metadata
(name, mime_type, file_size, status, chunk_count, TOC) for one
or more docs.

When to call:
- The user asks meta questions about the document itself
  ("how big is this file", "how many pages", "is this PDF or
  DOCX", "is it done processing").
- The planner wants a pre-flight check before committing to
  expensive retrieval (e.g. "is the doc indexed?").
- The planner needs the document's table of contents to plan
  section-level reads (use `fields: ['toc']`).

When NOT to call (use a different tool instead):
- The planner needs the document's *content* — use
  `dense-retrieval`, `lexical-retrieval`, or `index_lookup`.
- The planner needs a narrative summary → `doc-summary`.
- The planner needs the section structure with chunk IDs
  → `doc-index`.

## Args

- `doc_ids` (required, array of strings, ≥1): document UUIDs to
  read metadata for.
- `fields` (optional, array of strings): filter restricting which
  keys are returned, e.g. `['name', 'mime_type', 'chunk_count']`.
  Omit for the complete metadata object.

## Output

Array of metadata objects:

```json
[
  { "doc_id": "uuid",
    "name": "antifragile.pdf",
    "mime_type": "application/pdf",
    "file_size": 2456789,
    "status": "completed",
    "chunk_count": 142,
    "toc": [
      { "title": "Definition", "heading_level": 1, "page": 1, "rank": 1 }
    ] }
]
```

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call.
You execute the call. Cheap pre-flight; use liberally when
document identity is unclear.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md` — pre-flight vs doc-summary
- `reference/gotchas.md` — empty doc_ids, fields filter
- `reference/examples.md`
