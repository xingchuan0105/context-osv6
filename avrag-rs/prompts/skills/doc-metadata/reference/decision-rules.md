# Decision Rules

## When `doc-metadata` is the right tool

- **Meta questions** about the document: "how big is this file",
  "how many pages", "is this PDF or DOCX", "how many chunks",
  "is it done processing".
- **Pre-flight before expensive retrieval**: confirm a doc is
  `status: "completed"` and has `chunk_count > 0` before calling
  `dense-retrieval` on it. Saves a wasted retrieval call.
- **Section map without chunk IDs**: ask for `fields: ['toc']` to
  see section structure (titles + pages) without committing to
  `doc-index` (which returns chunk IDs you may not need).
- **User asks "is this document processed / ready"** → metadata
  is the cheapest signal.

## When to prefer a different tool

- **Need content** (even one line) → `dense-retrieval`,
  `lexical-retrieval`, `index_lookup`, or `doc-summary`.
- **Need chunk IDs** → `doc-index`.
- **Need narrative summary** → `doc-summary`.

## Sequencing rules

- `doc-metadata` is the **cheapest** RAG tool (single Postgres
  query). Use it as the very first call in a plan when the
  planner needs to verify document state.
- Pair with `doc-summary` or `doc-index` to get a richer view of
  the same documents.

## Field-selection rules

- Omit `fields` for the full metadata object (includes everything
  the runtime can return).
- For "what kind of file" → `fields: ['name', 'mime_type']`.
- For "is it ready" → `fields: ['status', 'chunk_count']`.
- For "section structure" → `fields: ['toc']`. (This is the only
  way to get the TOC via metadata; `doc-index` also returns it
  but with chunk IDs alongside.)
- The runtime never returns content (text, page text) here. If
  the user wants content, use the content tools.
