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

**Scope boundary**: You read metadata only. You do NOT retrieve
content, do NOT summarize, and do NOT return chunk IDs.

## Input

- `doc_ids` (required, array of UUID strings, ≥1): Document UUIDs to
  read metadata for.
- `fields` (optional, array of strings): Filter restricting which
  keys are returned. Omit for the complete metadata object.
  `fields: []` is equivalent to omitting `fields` (returns all fields).

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

**Critical gate**: Only `status: "completed"` documents should
proceed to expensive retrieval. See `reference/decision-rules.md`
for the status gate rules.

## Call this tool when the planner has selected it

The `retrieval-planner` decides whether to include this call.
You execute the call. Cheap pre-flight; use liberally when
document identity is unclear.

For detailed guidance, see:
- `reference/args-schema.md`
- `reference/decision-rules.md`
- `reference/gotchas.md`
- `reference/examples.md`
