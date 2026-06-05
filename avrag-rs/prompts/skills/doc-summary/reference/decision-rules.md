# Decision Rules

## When `doc-summary` is the right tool

- **Pre-retrieval scoping**: the planner doesn't know which doc
  holds the answer and needs a high-level view of each
  candidate doc first. Use `level: "doc"`.
- **Section-level planning**: the planner intends to follow up
  with `index_lookup` and needs the section structure of the
  target doc. Use `level: "section"` to get a TOC.
- **User asks for a summary** ("what does this document cover",
  "give me an overview of X").
- **Disambiguation**: multiple docs in `doc_scope`, planner needs
  to pick the right one.

## When to prefer a different tool

- **Specific chunk known** (you have chunk IDs) → `index_lookup`.
- **Paraphrased factual question** → `dense-retrieval` directly
  (don't waste a call on summary).
- **Only file metadata** needed → `doc_metadata`.
- **Surgical section read** (you know the section name) →
  `index_lookup` after a quick `doc-index` to find chunk IDs.

## `level: "doc"` vs `level: "section"`

- `level: "doc"` — narrative summary of the whole document. Use
  when the planner needs a high-level "what's in here" before
  deciding what to retrieve.
- `level: "section"` — TOC entries: each section with its title,
  heading level, and page. Use when the planner needs to plan
  `index_lookup` for a specific section.

## Combine with other tools

- `doc-summary` → `dense-retrieval` is the standard pre-flight
  pattern when the corpus is wide and the query is open-ended.
- `doc-summary` → `doc-index` → `index_lookup` is the
  precise-read pattern: understand the doc → see its structure
  → fetch the exact chunks.
- Don't chain `doc-summary` → `doc-summary` — one level of
  summary is enough.
