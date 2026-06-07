# Decision Rules

## When `lexical-retrieval` is the right tool

- The query contains a literal string the user expects to find
  verbatim in chunk text: error code, ticket number, version
  string, acronym, exact product / API / class / identifier name.
- The user types a specific phrase and means that phrase
  ("find the section that mentions 'AUTH_SESSION_VERSION'").
- You want to anchor on a specific term AND combine with semantic
  coverage → pair with `dense-retrieval` in the same plan.
- The corpus is small or has a controlled vocabulary where exact
  matches carry most of the signal.

## When to prefer a different tool

- **Paraphrased or conceptual question** (no exact literals)
  → `dense-retrieval`. BM25 only matches what you typed.
- **Relationship / multi-hop** → `graph-retrieval`.
- **Surgical read of known chunks** (you have chunk IDs from
  `doc-index`) → `index_lookup`.
- **Don't know which doc to target** → `doc-summary` first to
  discover the right doc, then `dense-retrieval` (or
  `lexical-retrieval` for literals) against the workspace.

## Combine with other tools

- `dense-retrieval` + `lexical-retrieval` is the canonical hybrid.
  Issue both in the same plan when the query mixes natural
  language with literals (most user queries do).
- Do NOT call `lexical-retrieval` alone unless the query is
  purely a literal search — it will miss paraphrased context.

## Term-selection rules

`terms` is a **bag of independent tokens**, not phrase queries.
The runtime applies BM25 over the bag; the order of `terms` in
the array does not matter, and multi-word entries are tokenized
into independent words.

- Keep terms compact (1-3 tokens each). Whole phrases beat
  single common words.
- **"rollback checklist"** as a single string matches any chunk
  that contains BOTH `rollback` AND `checklist` (in any order,
  with any words between). It does NOT require the two words
  to be adjacent. If you need adjacent-phrase matching, no
  current retrieval tool supports it — use `index_lookup` after
  `doc-index` to fetch a known section's text.
- Multiple entries in `terms` are **OR-combined** (any term
  matches), then re-weighted by IDF. Add synonyms to broaden
  recall; add nothing to narrow.
- Include the most specific form: "Atlas" beats "atlas" beats
  "atlas system".
- For code/identifier search, include the exact case-insensitive
  form. Case is normalized, so don't burn slots on case
  variants — see "Case is NOT a problem" in `gotchas.md`.
- Don't pre-stem or pluralize — pass the form the user typed.
  Light stemming is applied by the runtime.

## Interpreting scores

BM25 scores are unbounded positive numbers (typically 0-30+).
Do not treat them like cosine similarities (e.g. 0.87) — the
example value in the schema is illustrative only. If you see
scores < 1, the merger has likely already normalized them via
RRF; that's a post-merge signal, not raw BM25.

Don't compare scores across tools or across calls against
different corpora. Trust the merged top-k instead.

## `top_k` selection

| Query type | `top_k` | Rationale |
|------------|---------|-----------|
| Narrow, single answer expected | 3-5 | Reduces noise in synthesis |
| Standard literal search | 10 (default) | Balanced coverage |
| Broad scan before `doc-index` + `index-lookup` | 20-30 | More candidates to filter down |
| Need even more | Issue a second narrower call | Avoid >50 latency degradation |
