# Gotchas

- Both `graph_hints` AND `placeholder_triplets` empty returns random
  relations. Always provide at least one hint or placeholder.
- Overly broad hints (e.g. `subject: "company"`, `object: "document"`)
  may return too many relations and time out. Anchor hints to
  specific entity names when you can.
- `hop_limit` defaults to 1. Values above 2 significantly increase
  latency and often return noisy multi-hop paths. Reserve `hop_limit: 3`
  for explicit "follow the chain" questions.
- `fan_out_limit` caps at 20 (hard ceiling in the runtime). Use
  the default 10 unless you have a specific reason to widen.
- Graph retrieval complements chunk retrieval, not replaces it.
  Pair with `dense-retrieval` in the same plan; the answer
  synthesizer will use both.
- Triplet orientation matters: `subject-predicate-object` follows
  the direction of the edge in the workspace knowledge graph. A
  reversed triplet returns no relations. **An empty result does
  not mean the relationship does not exist** — it may mean the
  direction was reversed.
- The `?` placeholder works for ONE unknown slot per triplet.
  For "find all X owned by Y", use `subject: "X"`, `object: "Y"`,
  `predicate: "?"` and let the graph resolve the relation type.
- If a graph call is too slow or returns too much noise, tighten
  limits in this order: (1) reduce `hop_limit`, (2) reduce
  `fan_out_limit`, (3) reduce `relation_limit`. If it is still
  too noisy, fall back to `dense-retrieval`.
