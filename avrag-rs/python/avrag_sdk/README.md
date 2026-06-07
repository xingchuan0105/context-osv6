# avrag-sdk

Python SDK for `avrag-rs` retrieval primitives. Used inside the
`code-interpreter` sandbox so that the model can write Python that
orchestrates retrieval and post-processing.

## Scope

This package is a **thin HTTP client**. It does no embedding, no
vector search, no graph traversal — those happen in the Rust backend
(`crates/retrieval-data-plane` + `bins/api`). The Python side is for:

- **Orchestration** — fan-out, dedup, branching, iteration
- **Post-processing** — custom filters, aggregations, time-line assembly
- **State management** — writing intermediate results to the session
  directory for cross-turn persistence (see `code-gen-skill.md`)

## Design constraints

- **All methods are async** (httpx-based)
- **No business logic in the SDK** — only transport
- **One method per Rust tool** — keep the surface area small
- **`*_batch` variants exist only where they save external API cost**
  (embedding, rerank). Other tools are stateless and don't need batching.

## Available primitives

| Method                | Best for                                              |
|-----------------------|-------------------------------------------------------|
| `dense(q, k)`         | Semantic similarity, paraphrases, fuzzy intent        |
| `dense_batch(queries)`| Multiple query variations in one embedding API call   |
| `lexical(q, k)`       | Exact terms, contract numbers, names                  |
| `graph(entities)`     | Entity-relation queries (股权 / 上下游 / 组织)        |
| `index_lookup(ids)`   | Document records when you already have doc_ids        |
| `doc_summary(ids)`    | Pre-computed summaries for triage                     |
| `doc_metadata(ids)`   | Structured fields (date / org / type / status)        |
| `rerank(q, cands)`    | Re-score a candidate set                              |
| `rerank_batch(q, ...)`| Rerank multiple candidate sets in one API call        |
| `web_search(q)`       | Public web / news (Brave)                             |

## Endpoints

Each method maps to a single HTTP endpoint. The endpoint design is
documented in `src/avrag_sdk/client.py`.

## Configuration

| Env var               | Default                     | Purpose                       |
|-----------------------|-----------------------------|-------------------------------|
| `AVRAG_API_URL`       | `http://localhost:8080`     | Base URL of the Rust API      |
| `AVRAG_AUTH_TOKEN`    | (none)                      | Bearer token for auth context |

In a sandboxed execution, set `AVRAG_API_URL` to the in-cluster
address of `bins/api` (e.g. `http://avrag-api:8080`).

## Usage in the sandbox

```python
from avrag_sdk import client
import asyncio

async def main():
    # Simple: one-shot vector search
    chunks = await client.dense("XX 客户 合同纠纷", k=10)

    # Complex: fan-out + dedup + rerank
    queries = ["XX 客户 合同", "XX 客户 纠纷", "XX 客户 起诉"]
    tasks = [client.dense(q, k=20) for q in queries]
    results = await asyncio.gather(*tasks)
    merged = {c.chunk_id: c for group in results for c in group}
    top = await client.rerank(query="XX 客户 合同纠纷", candidates=list(merged.values()))
    return top[:10]

chunks = asyncio.run(main())
```

## Testing

```bash
pip install -e ".[dev]"
pytest
```

The test suite uses `respx` to mock HTTP responses, so it runs without
a live `avrag-rs` backend.
