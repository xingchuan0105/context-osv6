# Decision Rules

## When to call `doc-index`

| Condition | Action |
|-----------|--------|
| Need section structure / TOC of a document | Call `doc-index` |
| Need chunk IDs for `index_lookup` | **Must** call `doc-index` first |
| User names a specific section ("chapter 3", "the FAQ") | Call `doc-index` to resolve to chunk IDs |
| Already have chunk IDs from this session AND document was not re-ingested | May skip `doc-index`, go directly to `index_lookup` |

## When NOT to call `doc-index`

| Condition | Use instead |
|-----------|-------------|
| Need broad doc-level context / gist | `doc-summary` |
| Need semantic / paraphrased recall | `dense-retrieval` |
| Only need basic file metadata (name, status) | `doc_metadata` |

## Sequencing rules

1. `doc-index` MUST come **before** `index_lookup` in the same plan for the same document.
2. The chunk IDs returned by `doc-index` are the **only** valid inputs to `index_lookup`.
3. IDs from memory, cache, or user input are **invalid** unless verified against a fresh `doc-index` call.
4. It is OK to issue `doc-index` for multiple documents in one plan.
5. Do not call `doc-index` in a loop — fetch once per document per plan.

## `doc-index` vs `doc-summary`

| Aspect | `doc-index` | `doc-summary` |
|--------|-------------|---------------|
| Output | TOC + chunk IDs | Narrative summary |
| Best for | Planning `index_lookup` | Pre-retrieval scoping |
| Returns content? | Section titles only | Summarized content |
| Size | Larger (one entry per section) | Smaller (one paragraph) |
