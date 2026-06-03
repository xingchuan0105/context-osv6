# Gotchas

- Empty `queries` array returns no results — always provide at least one query.
- Each query must be a standalone sentence, not a keyword list. If you
  have keywords, use `lexical-retrieval` instead.
- `top_k` defaults to 10. Values above 50 may degrade latency without
  improving recall — consider issuing a second narrower call instead.
- Embedding rate limits: under load the embedding service may 429.
  The runtime retries with backoff, but if you get 0 results, check
  the worker's logs for embedding failures rather than assuming
  semantic mismatch.
- Multilingual queries: the embedding model must support the query
  language. For mixed-language corpora, consider `modality: "both"`
  to surface image-anchored context.
- Stale embeddings: if a document was re-ingested, the old chunk IDs
  may still appear in the index briefly during a re-index window.
