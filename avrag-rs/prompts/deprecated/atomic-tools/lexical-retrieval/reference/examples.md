# Examples

Good call signatures for `lexical-retrieval`.

## Example 1: Find a literal error code

**Context**: User asks "which document mentions error code E-2047?"

```json
{
  "tool": "lexical_retrieval",
  "version": "1.0",
  "args": {
    "terms": ["E-2047"],
    "top_k": 5
  }
}
```

Single literal term, narrow `top_k`. The user wants to FIND
"which document", not get a long answer.

## Example 2: Multi-term phrase search

**Context**: User asks "find the section about AUTH_SESSION_VERSION"

```json
{
  "tool": "lexical_retrieval",
  "version": "1.0",
  "args": {
    "terms": ["AUTH_SESSION_VERSION", "session version", "auth"],
    "top_k": 5
  }
}
```

The user typed a multi-word phrase. Add the standalone `auth`
term to **also surface chunks that use the abbreviation alone**
— multiple `terms` are OR-combined. (And note: "session
version" as a single string already matches any chunk with
both words, in any order — it is not a phrase query.)

## Example 3: Hybrid with dense (canonical pattern)

**Context**: User asks "show me the rollback checklist for Atlas"

```json
[
  {
    "tool": "dense_retrieval",
    "version": "1.0",
    "args": {
      "queries": ["rollback procedure for Atlas deployment"],
      "top_k": 10
    }
  },
  {
    "tool": "lexical_retrieval",
    "version": "1.0",
    "args": {
      "terms": ["Atlas", "rollback", "checklist"],
      "top_k": 10
    }
  }
]
```

Dense for paraphrased context, lexical for exact matches on
"Atlas" (proper noun) and "rollback checklist" (multi-word
phrase). The merger combines via RRF.

## Example 4: Discover a section, then fetch precisely

**Context**: User asks "show me the exact text about session
timeouts in the Atlas doc." We don't yet have chunk IDs; we
need to discover the section first.

```json
[
  {
    "tool": "lexical_retrieval",
    "version": "1.0",
    "args": {
      "terms": ["session timeout configuration"],
      "top_k": 5
    }
  },
  {
    "tool": "doc_index",
    "version": "1.0",
    "args": {
      "doc_ids": ["<doc-id-from-lexical-results>"]
    }
  },
  {
    "tool": "index_lookup",
    "version": "1.0",
    "args": {
      "doc_id": "<doc-id>",
      "chunk_ids": ["<chunk-ids-from-doc-index-for-timeout-section>"]
    }
  }
]
```

Three-stage surgical fetch:
1. `lexical-retrieval` discovers candidate chunks (and which
   `doc_id` they belong to).
2. `doc-index` returns the full section structure with valid
   chunk IDs for that `doc_id` — these are the only IDs
   `index-lookup` accepts.
3. `index-lookup` fetches the exact text in deterministic order.

**Do NOT skip step 2**: chunk IDs from `lexical-retrieval` are
not a valid source for `index-lookup` per its skill contract.

## Example 5: Acronym expansion search

**Context**: User asks "do we have docs on NLP pipelines?"

```json
[
  {
    "tool": "lexical_retrieval",
    "version": "1.0",
    "args": {
      "terms": ["NLP", "natural language processing", "language model"],
      "top_k": 5
    }
  }
]
```

Include the acronym, the full expansion, AND a related term.
The chunks likely use one of these forms. Don't just search
"NLP" — that matches too narrowly.
