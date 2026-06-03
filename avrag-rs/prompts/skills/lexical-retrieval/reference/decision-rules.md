# Decision Rules

## When `lexical-retrieval` is the right tool

- The query contains a literal string the user expects to find
  verbatim: filename, document title, error code, ticket number,
  version string, acronym, exact product / API / class name.
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
- **Don't know which doc to target** → `doc-summary` first.

## Combine with other tools

- `dense-retrieval` + `lexical-retrieval` is the canonical hybrid.
  Issue both in the same plan when the query mixes natural
  language with literals (most user queries do).
- Do NOT call `lexical-retrieval` alone unless the query is
  purely a literal search — it will miss paraphrased context.

## Term-selection rules

- Keep terms compact (1-3 words each). Whole phrases beat single
  common words.
- Include the most specific form: "Atlas" beats "atlas" beats
  "atlas system".
- For code/identifier search, include the exact case-sensitive
  form. BM25 lowercases by default; if case matters, you'll
  need to post-filter.
- Don't pre-stem or pluralize — pass the form the user typed.
- For multi-word terms, BM25 tokenizes but does not require
  contiguity. A phrase like "rollback checklist" is found in
  any chunk containing both words, in either order.
