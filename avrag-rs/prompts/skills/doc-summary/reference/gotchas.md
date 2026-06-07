# Gotchas

- Summary is pre-generated and may not reflect the latest document
  version. If the document was re-ingested, the summary may be
  stale until the next regeneration cycle. For questions about
  the very latest content, prefer content retrieval tools
  (`dense-retrieval`, `index_lookup`) over `doc-summary`.
- Empty `doc_ids` returns empty summaries. Always provide at
  least one doc ID.
- `level: "section"` returns TOC entries, NOT full text summaries.
  Use `level: "doc"` for narrative summaries.
- Calling with many `doc_ids` (e.g. 50+) is expensive — each doc
  summary is a separate read. The recommended path is:
  `doc_metadata` (filter) → `doc-summary` (overview) → deeper
  retrieval. Pre-filter via `doc_metadata` first if the corpus is
  large.
- Section-level summaries do not include the section's text content.
  Use `index_lookup` to fetch the actual text after finding the
  right section.
- The summary may mention content that the planner's `doc_scope`
  does not grant access to. Trust the user's `doc_scope`, not the
  summary, for access-control decisions.
