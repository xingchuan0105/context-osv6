# Decision Rules

## When `doc-index` is the right tool

- The planner needs to know **which chunks belong to which
  sections** of a document before planning a precise read.
- The user names a specific section or anchor, and the planner
  needs to resolve it to a chunk ID.
- The planner intends to follow up with `index_lookup` and
  needs valid chunk IDs.

## When to prefer a different tool

- **Don't know which doc to target** → `doc-summary` first
  (broader view) then `doc-index` for the chosen doc.
- **Don't need chunk IDs** (semantic recall is fine)
  → `dense-retrieval` directly.
- **Only need a narrative summary** (not the structural map)
  → `doc-summary`.
- **Surgical read on already-known chunks** (cache hit) → skip
  this and go straight to `index_lookup` (still validate IDs).

## Sequencing rules

- `doc-index` MUST come **before** `index_lookup` in the same
  plan. The chunk IDs returned here are the only valid inputs
  to `index_lookup`.
- It is OK to issue `doc-index` for multiple documents in one
  plan if the planner needs the section structure of each.
- Do not call `doc-index` in a loop — fetch once per document
  per plan.

## `doc-index` vs `doc-summary`

| Aspect | `doc-index` | `doc-summary` (`level: "doc"`) |
|---|---|---|
| Output | TOC + chunk IDs | Narrative text |
| Best for | Planning `index_lookup` | Pre-retrieval scoping |
| Returns content? | Section titles only | Summarized content |
| Size | Larger (one entry per section) | Smaller (one paragraph) |

Use `doc-index` when you need to know the structural map; use
`doc-summary` when you need to know the gist.
