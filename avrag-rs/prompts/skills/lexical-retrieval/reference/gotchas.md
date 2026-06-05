# Gotchas

- Empty `terms` array returns no results — always provide at least one term.
- Terms are matched verbatim. Typos, plural/singular mismatch,
  casing differences, or punctuation variations all return empty
  results. BM25 does tokenize, but it does not stem aggressively
  — "user" matches "users" but "child" does NOT match "children".
- Do not use as a weaker duplicate of `dense-retrieval` for the same
  intent. If you want both semantic and lexical coverage, issue
  them as separate calls in the same plan and let the merger
  combine via RRF.
- Very common terms (e.g. "the", "request") match too many chunks
  and add no signal. Keep terms specific.
- Acronyms need to be spelled out: searching "NLP" misses
  "natural language processing" chunks. If you don't know the
  expansion, use `dense-retrieval` instead.
- The score scale differs from `dense-retrieval` (BM25 vs cosine).
  Don't compare scores across tools — trust the merged top-k.
