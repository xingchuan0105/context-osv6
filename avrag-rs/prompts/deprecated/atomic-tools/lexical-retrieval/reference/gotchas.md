# Gotchas

## Term-form gotchas

- **Case is NOT a problem**: BM25 normalizes case (lowercases both
  query and indexed text), so `"User"`, `"user"`, `"USER"` all
  match the same chunks. Do not burn `terms` slots on case
  variants — it adds no recall and may add duplicates after dedup.
  If you must preserve case (rare), post-filter the returned
  chunks by the original casing.
- **Light stemming IS applied**: "user" matches "users" (regular
  `-s` plural), but "child" does NOT match "children" (irregular
  plural).
- **Typos do not fuzzy-match**: "athentication" finds nothing in a
  doc that says "authentication". Use `dense-retrieval` for
  typo-tolerance.
- Terms are matched verbatim apart from case normalization and
  light stemming. Punctuation variations (e.g. "rollback" vs
  "roll-back") return empty results.
- Very common terms (e.g. "the", "request") match too many chunks
  and add no signal. Keep terms specific.
- Acronyms need to be spelled out: searching "NLP" misses
  "natural language processing" chunks. If you don't know the
  expansion, use `dense-retrieval` instead.

## Empty-result interpretation

- Empty `terms` array returns no results — always provide at least
  one term.
- An empty array from `lexical-retrieval` means **none of your
  terms appear in any chunk in the corpus** (unlike
  `dense-retrieval`, where empty can also indicate embedding
  issues). Common causes: typo, wrong form ("auth" vs "AUTH"),
  irregular plural, or wrong corpus. Before concluding the topic
  is absent, re-check spelling and try the `dense-retrieval`
  fallback.

## Score gotchas

- BM25 scores are unbounded positive numbers (typically 0-30+);
  the example value `0.87` in the schema is illustrative only.
  If you see scores < 1, the merger has likely already normalized
  them via RRF — that's a post-merge signal, not raw BM25.
- Don't compare scores across tools or across calls against
  different corpora. Trust the merged top-k.

## Performance limits

- Do not use `lexical-retrieval` as a weaker duplicate of
  `dense-retrieval` for the same intent. If you want both
  semantic and lexical coverage, issue them as separate calls
  in the same plan and let the merger combine via RRF.
