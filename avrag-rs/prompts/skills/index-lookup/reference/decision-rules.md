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

- **Default**: `index-lookup` is preceded by `doc-index` in the
  same plan. The chunk IDs returned by `doc-index` are the only
  safe source.
- **Cache hit**: if chunk IDs are from session state AND the
  document has not been re-ingested since, you may skip
  `doc-index` and call `index-lookup` directly.
- **Stale cache**: if a re-ingest may have happened, you MUST
  re-call `doc-index` first; old IDs silently return empty.
- `doc-index` and `index-lookup` for the same document can be in
  the same plan; the runtime executes them in order.

## Pairing with other tools

`index-lookup` is a **surgical** tool — you call it when you
know what you want. Pairing with `dense-retrieval` or
`graph-retrieval` in the same plan is valid and common, e.g.:

- `graph-retrieval` returns supporting chunk IDs →
  `index-lookup` fetches the actual text (see `examples.md`).
- `dense-retrieval` surfaces a region of interest →
  `doc-index` for the surrounding section map →
  `index-lookup` for the precise chunks in that section.

Avoid calling `index-lookup` in a tight loop against the same
document; batch the IDs into one call to amortize the overhead.

## Why `index-lookup` is the most precise tool

- `dense-retrieval`: fuzzy, returns semantically similar chunks,
  ranking depends on embedding distance.
- `lexical-retrieval`: exact match, ranking by BM25.
- `index-lookup`: **deterministic** — if the chunk ID is valid,
  you get exactly that chunk. No ranking, no surprise.

Use `index-lookup` when you know what you want. Use
`dense-retrieval` when you don't.
