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
term to broaden slightly (BM25 doesn't require contiguity, so
"session version" matches in chunks containing both words
separately).

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

## Example 4: Pre-index_lookup validation

**Context**: Planner intends to call `index_lookup` next and
needs to confirm chunk IDs are still valid.

```json
{
  "tool": "lexical_retrieval",
  "version": "1.0",
  "args": {
    "terms": ["session timeout configuration"],
    "top_k": 20
  }
}
```

Higher `top_k: 20` for a broad scan. After getting the chunks,
filter to the IDs you need, then issue `index_lookup` for
precise fetch.

## Example 5: Acronym expansion search

**Context**: User asks "do we have docs on NLP pipelines?"

```json
[
  {
    "tool": "lexical_retrieval",
    "version": "1.0",
    "args": {
      "terms": ["NLP", "natural language processing", "language model"],
      "top_k": 10
    }
  }
]
```

Include the acronym, the full expansion, AND a related term.
The chunks likely use one of these forms. Don't just search
"NLP" — that matches too narrowly.
