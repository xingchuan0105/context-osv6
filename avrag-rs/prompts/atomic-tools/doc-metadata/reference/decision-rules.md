# Decision Rules

## When to call `doc-metadata`

| Scenario | Why `doc-metadata` |
|----------|-------------------|
| Meta question about the document (size, type, page count) | Cheapest signal; no content retrieval needed |
| Pre-flight before expensive retrieval | Verify document is ready before committing to dense/lexical retrieval |
| Need TOC without chunk IDs | Lightweight section map for display or user confirmation |
| Need processing status dashboard | Batch check multiple docs at once |

## Status gate — the only hard rule

**Only `status: "completed"` documents may proceed to expensive retrieval.**

| `status` | Meaning | Action |
|----------|---------|--------|
| `completed` | Document is indexed and ready | Proceed to retrieval |
| `pending` | Document queued for processing | Wait or inform user |
| `processing` | Document being indexed | Wait or inform user |
| `failed` | Ingestion failed | Refuse retrieval; inform user or suggest re-upload |

**Never** call `dense-retrieval`, `lexical-retrieval`, `doc-index`, or `index_lookup` on a document whose `status` is not `completed`.

## `doc-metadata` vs `doc-index` — hard switching rule

| Next step | Use this tool |
|-----------|---------------|
| Need TOC for display or user confirmation only (no chunk IDs needed) | `doc-metadata` with `fields: ["toc"]` |
| Need chunk IDs for `index_lookup` | `doc-index` |

**Rule**: If you do NOT plan to call `index_lookup` after getting the TOC, use `doc-metadata`. If you DO need chunk IDs for precise reading, use `doc-index`. Do not upgrade from `doc-metadata` to `doc-index` unless chunk IDs are actually needed.

## When NOT to call `doc-metadata`

| Need | Use instead |
|------|-------------|
| Content (even one line) | `dense-retrieval`, `lexical-retrieval`, `index_lookup`, `doc-summary` |
| Chunk IDs | `doc-index` |
| Narrative summary | `doc-summary` |

## Sequencing rules

- `doc-metadata` is the **cheapest** RAG tool (single Postgres query). Use it as the very first call when verifying document state.
- Pair with `doc-summary` or `doc-index` for richer views of the same documents.

## Field-selection rules

| Question | `fields` |
|----------|----------|
| "What kind of file?" | `["name", "mime_type"]` |
| "Is it ready?" | `["status", "chunk_count"]` |
| "Section structure?" (no chunk IDs needed) | `["toc"]` |
| "Everything" | omit or `[]` |
