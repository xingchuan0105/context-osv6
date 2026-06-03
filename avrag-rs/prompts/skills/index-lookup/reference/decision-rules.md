# Decision Rules

## When `index-lookup` is the right tool

- The planner has **valid chunk IDs** (from a prior `doc-index`
  call) and wants to fetch the exact text blocks.
- The user named a **specific section or anchor** and the planner
  resolved it to a chunk ID.
- **Surgical precision** is needed: read exactly chunks 12, 13,
  14 instead of semantic ranking.
- **Cache hit** scenario: the planner remembers chunk IDs from
  a previous query in the same session and wants to re-read them.

## When to prefer a different tool

- **Don't have chunk IDs** → call `doc-index` first, or fall back
  to `dense-retrieval` for fuzzy recall.
- **Paraphrased query** (no specific chunk in mind)
  → `dense-retrieval`.
- **Need the section structure** first → `doc-index`.
- **Exact-literal match across the doc** (find any chunk
  containing a string) → `lexical-retrieval`.

## Sequencing rules

- `index-lookup` **MUST be preceded by `doc-index`** in the same
  plan. The planner is responsible for sequencing.
- `doc-index` and `index-lookup` for the same document can be in
  the same plan; the runtime executes them in order.
- Do not interleave `index-lookup` with `dense-retrieval` for
  the same document unless you have a specific reason
  (e.g. "the section I want is in chunk 14, but let me also
  semantic-search the rest").

## Why `index-lookup` is the most precise tool

- `dense-retrieval`: fuzzy, returns semantically similar chunks,
  ranking depends on embedding distance.
- `lexical-retrieval`: exact match, ranking by BM25.
- `index-lookup`: **deterministic** — if the chunk ID is valid,
  you get exactly that chunk. No ranking, no surprise.

Use `index-lookup` when you know what you want. Use
`dense-retrieval` when you don't.
