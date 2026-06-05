# Decision Rules

## When `graph-retrieval` is the right tool

- The question is about **relations between entities**:
  ownership, dependency, lineage, authorship, responsibility,
  comparison, cause/effect.
- The question is **multi-hop**: "which service owns the
  pipeline that handles the rollback for the system running on
  Atlas?" — needs to follow a chain.
- The answer **connects chunks via graph edges** rather than
  via direct chunk content.
- You want to surface relations the chunk text doesn't state
  explicitly but the knowledge graph knows about.

## When to prefer a different tool

- The question is about a **single fact in a chunk** ("what year
  was X released") → `dense-retrieval` or `lexical-retrieval`.
- The question is about **document structure** (chapters, TOCs)
  → `doc-index`.
- The question is about **broad doc context** → `doc-summary`.

## Combine with other tools

- `graph-retrieval` alone is rarely useful. **Always pair with
  `dense-retrieval`** so the answer synthesizer has both
  relations and the actual chunk text.
- `graph-retrieval` + `index_lookup` for "show me the chunks
  that support this specific relation".

## Triplet design rules

- Each triplet is `{subject, predicate, object}`. Order matters
  — the runtime traverses the edge in the direction
  subject → object.
- For unknown slots, use `?` or named placeholders (`?owner`,
  `?service`, `?document`). The runtime fills these in.
- Prefer **one unknown slot per triplet**. Two unknowns means
  the graph has to guess, which it does poorly.
- Use `placeholder_triplets` (not `graph_hints`) when you don't
  know the predicate — the runtime picks the relation type
  from the graph schema.
- Avoid overly broad subjects/objects (e.g. "company",
  "document") — they match too many edges. Anchor to specific
  entity names.

## When to increase `hop_limit`

- `hop_limit: 1` (default) — direct relations only.
- `hop_limit: 2` — when the question requires ONE intermediate
  connection ("A owns B that depends on C").
- `hop_limit: 3` (max) — only for explicit "follow the chain"
  questions. Latency scales combinatorially.
