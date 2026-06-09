# Decision Rules

## When to call `dense-retrieval`

Call this tool when the query is **meaning-driven** rather than literal.

| Scenario | Why dense retrieval |
|----------|---------------------|
| Paraphrased or conceptual question | Semantic embedding captures meaning, not exact words |
| Multilingual / cross-lingual query | Embedding model maps across languages |
| "Similar to" / "related to" / "about X in general" | Semantic similarity ranking |
| Answer scattered across multiple chunks | Semantic ranking surfaces distributed evidence |

## When NOT to call `dense-retrieval`

| Scenario | Why not | Use instead |
|----------|---------|-------------|
| Exact-literal match needed (IDs, error codes, filenames, acronyms) | BM25 anchors on literal string | `lexical-retrieval` |
| Entity relationship / multi-hop reasoning ("who owns the service that depends on X") | Needs graph traversal, not vector similarity | `graph-retrieval` |
| Surgical read of known chunks (you already have chunk IDs) | Direct lookup is faster | `index_lookup` |
| Broad doc-level context, don't know which doc to target | Need doc overview first | `doc-summary` |

## Combine with other tools

- `dense-retrieval` + `lexical-retrieval` is the standard hybrid:
  semantic for meaning, BM25 for exact matches. The merger
  combines via RRF. Use when the query has both paraphrased
  phrasing AND specific literals.
- `dense-retrieval` + `graph-retrieval` for "explain the
  relationship, then show me the chunks" questions.
- `dense-retrieval` alone is the default. Do not add other tools
  unless they materially improve recall.

## `top_k` selection

| Query type | `top_k` | Rationale |
|------------|---------|-----------|
| Narrow, single answer expected | 3-5 | Reduces noise in synthesis |
| Standard conceptual question | 10 (default) | Balanced coverage |
| Broad, answer scattered | 20-30 | More context, capped at 50 |
| Need even more | Issue a second narrower call | Avoid >50 latency degradation |
