# Examples

Good call signatures for `dense-retrieval`.

## Example 1: Paraphrased policy question

**Context**: User asks "what's our policy on customer data retention?"

```json
{
  "tool": "dense_retrieval",
  "version": "1.0",
  "args": {
    "queries": [
      "customer data retention policy and duration",
      "how long is customer data stored after account closure"
    ],
    "top_k": 10
  }
}
```

Two paraphrased queries, each standalone. `top_k: 10` is the
default — explicit for clarity.

## Example 2: Multimodal query

**Context**: User asks "what does the architecture diagram on page 8
show?"

```json
{
  "tool": "dense_retrieval",
  "version": "1.0",
  "args": {
    "queries": ["architecture diagram page 8 system components"],
    "modality": "mm",
    "top_k": 5
  }
}
```

`modality: "mm"` surfaces image-bearing chunks. Low `top_k: 5`
because the user wants a specific figure, not the top-10.

## Example 3: Hybrid with lexical

**Context**: User asks "what does E-2047 say about session timeouts?"

```json
[
  {
    "tool": "dense_retrieval",
    "version": "1.0",
    "args": {
      "queries": ["E-2047 session timeout error code behavior"],
      "top_k": 10
    }
  },
  {
    "tool": "lexical_retrieval",
    "version": "1.0",
    "args": {
      "terms": ["E-2047", "session timeout"],
      "top_k": 10
    }
  }
]
```

Two parallel calls in the same plan. The merger combines via
RRF. The dense call catches the paraphrased "behavior"
context; the lexical call anchors on the exact "E-2047" string.

## Example 4: Pre-summary scoping

**Context**: User asks "what does the compliance manual say about
auditing?"

```json
[
  {
    "tool": "doc_metadata",
    "version": "1.0",
    "args": {
      "doc_ids": ["uuid-of-compliance-manual"],
      "fields": ["status", "chunk_count"]
    }
  },
  {
    "tool": "dense_retrieval",
    "version": "1.0",
    "args": {
      "queries": ["compliance auditing requirements and procedures"],
      "top_k": 15
    }
  }
]
```

`doc-metadata` first to verify the doc is ready; then dense
retrieval. `top_k: 15` (above default) because compliance docs
are large and the user asked an open-ended question.

## Example 5: Narrow lookup, single answer expected

**Context**: User asks "what is the Barbell strategy?"

```json
{
  "tool": "dense_retrieval",
  "version": "1.0",
  "args": {
    "queries": ["Barbell investment strategy extreme safety extreme risk"],
    "top_k": 3
  }
}
```

`top_k: 3` because the user wants a specific definition, not
the top-10. Lower `top_k` reduces noise in the answer synthesis.
