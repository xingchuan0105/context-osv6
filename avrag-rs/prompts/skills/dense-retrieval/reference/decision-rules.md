# Decision Rules

## When `dense-retrieval` is the right tool

- The user query is phrased differently than the source text
  (paraphrase, conceptual question, "in plain English" request).
- The answer is likely scattered across multiple chunks or
  documents (semantic ranking helps).
- Multilingual or cross-lingual retrieval is needed.
- The user asks "similar to", "related to", "about X in general".

## When to prefer a different tool

- **Exact-literal match needed** (filenames, IDs, error codes,
  acronyms) → `lexical-retrieval`. BM25 anchors on the literal.
- **Relationship / multi-hop question** ("who owns the service
  that runs the job that depends on X") → `graph-retrieval`.
- **Surgical read of known chunks** (you already have chunk IDs
  from `doc-index`) → `index_lookup`.
- **Broad doc-level context first** (you don't know which doc to
  target) → `doc-summary` before `dense-retrieval`.

## Combine with other tools in the same plan

- `dense-retrieval` + `lexical-retrieval` is the standard hybrid:
  semantic for meaning, BM25 for exact matches. The merger
  combines via RRF. Use when the query has both paraphrased
  phrasing AND specific literals.
- `dense-retrieval` + `graph-retrieval` for "explain the
  relationship, then show me the chunks" questions.
- `dense-retrieval` alone is the default. Do not add other tools
  unless they materially improve recall.

## When to lower `top_k`

- The query is narrow and you expect a single answer → `top_k: 5`.
- The query is broad and you want more context → `top_k: 20`+
  (but cap at 50 to avoid latency).
- Default `top_k: 10` is the safe choice for most queries.
