# Gotchas

## Empty result triage

An empty array (`[]`) can mean two very different things. Do not assume "nothing found" without checking:

| Step | Check | Action |
|------|-------|--------|
| 1 | Was a document recently re-ingested? | Retry after a short delay; index may still be updating |
| 2 | Check worker logs for embedding/index failures | If embedding service errored (e.g., 429), the issue is infrastructure, not semantic mismatch |
| 3 | Is the query language supported by the embedding model? | Unsupported languages return near-zero scores |
| 4 | Is the query too broad or too narrow? | Reformulate and retry |
| 5 | All above pass | Result is genuinely absent from the corpus; consider `lexical-retrieval` or `doc-summary` as fallback |

**Rule of thumb**: 0 results from a reasonable query on a known corpus is suspicious. Check infrastructure before concluding semantic mismatch.

## `modality: "both"` prerequisites

`"both"` fuses text and image embeddings into a single ranked list. It only makes sense when:
- The workspace actually contains image-bearing chunks (figures, tables, diagrams).
- If the corpus is text-only, `"both"` silently degrades to `"text"` — you gain nothing but add latency.
- Use `"mm"` when you only want image-bearing chunks; use `"both"` when the answer could be in text OR images.

## Query shaping

- **Empty `queries` array** returns no results — always provide at least one query.
- **Each query is vectorized independently** — the runtime does NOT combine them. RRF merges the per-query rankings.
- **Prefer full sentences for broad questions; short phrases are fine for narrow lookups.** See `reference/args-schema.md` for the full guidance.

## Performance limits

- `top_k` defaults to 10. Values above 50 are rejected by the runtime.
- If you need more results, issue a second narrower call instead of raising `top_k`.
- Embedding rate limits: under load the embedding service may 429. The runtime retries with backoff, but sustained pressure may delay results.

## Multilingual queries

The embedding model must support the query language. For mixed-language corpora, consider `modality: "both"` to surface image-anchored context that may be language-independent.

## Stale embeddings during re-index

If a document was re-ingested, old chunk IDs may briefly appear in the index during the re-index window. If results look inconsistent after a re-ingest:
1. Wait a few seconds and retry.
2. If still inconsistent, fall back to `lexical-retrieval` or `doc-summary`.
