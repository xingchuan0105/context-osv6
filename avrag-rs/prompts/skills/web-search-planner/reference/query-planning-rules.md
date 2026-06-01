# Query Planning Rules

## Number of queries
- Generate 1–3 sub-queries only.

## Query style
- Standalone, search-engine-ready.
- Same language as the user's query.
- Concise and keyword-rich, not conversational questions.

## Decomposition strategy
- Decompose by entity or evaluation dimension.
- Do not create near-duplicate paraphrases.

## Time-sensitive queries
- For latest / news / pricing / release / status, include a time anchor:
  - year (e.g., "2026")
  - or "latest"

## Pronoun and ambiguity resolution
- Use history to resolve pronouns and ambiguous references.
- If uncertain, set `needs_clarification` true and generate a safe, broad but relevant sub-query set.

## Budget awareness (Step 3)

This planner operates under a **hard 2-round search stop-loss**. The
system will run at most two search rounds (initial + one follow-up)
before forcing an answer with whatever evidence has been collected.

Implications for your plan:
- Plan sub-queries that are likely to be sufficient in the **first
  round** — prefer one precise query over multiple near-duplicates.
- If the topic is narrow (a specific company, product, or event),
  avoid broad exploratory queries that waste the second round.
- Do **not** plan a query strategy that depends on iterative
  refinement — there is no third round.
