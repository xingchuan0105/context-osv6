"""Example: cross-document aggregation query.

This is the "模式 A" pattern from the code-gen-skill. The LLM would
write something like this when a user asks "list all contracts with
disputes for XX customer between 2023-2025".

Run with:
    python examples/cross_doc_aggregation.py
"""

from __future__ import annotations

import asyncio
import json
from pathlib import Path

from avrag_sdk import client, Chunk


async def cross_doc_aggregation(user_query: str, customer_entity: str) -> list[Chunk]:
    # Phase 1: anchor to core entity via graph
    relations = await client.graph(entity_names=[customer_entity])

    # Extract document IDs from relation supporting chunks
    supporting_ids = set()
    for rel in relations:
        # Note: in practice, you'd parse supporting_chunk_ids from the
        # relation payload. Simplified here.
        pass

    # Phase 2: parallel retrieval — bm25 for exact IDs, dense for semantic
    queries = [
        f"{customer_entity} 合同",
        f"{customer_entity} 纠纷 起诉",
        f"{customer_entity} 仲裁",
    ]
    tasks = [client.dense(q, k=20) for q in queries]
    tasks.append(client.lexical(f"{customer_entity} 合同", k=20))
    results = await asyncio.gather(*tasks)

    # Phase 3: dedup by chunk_id
    merged: dict[str, Chunk] = {}
    for group in results:
        for chunk in group:
            if chunk.chunk_id not in merged or chunk.score > merged[chunk.chunk_id].score:
                merged[chunk.chunk_id] = chunk

    candidates = list(merged.values())

    # Phase 4: rerank against the original user query
    reranked = await client.rerank(user_query, candidates, top_k=30)

    # Phase 5: optional — write to session for cross-turn persistence
    # (in real usage, the sandbox exposes /session/ write_file)
    out_path = Path("/session/cross_doc_aggregation_result.json")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps([c.model_dump() for c in reranked], indent=2))

    return reranked


async def main() -> None:
    chunks = await cross_doc_aggregation(
        user_query="XX 客户 2023-2025 合同纠纷",
        customer_entity="XX 客户",
    )
    print(f"Got {len(chunks)} reranked chunks")
    for c in chunks[:5]:
        print(f"  [{c.score:.2f}] {c.chunk_id}: {c.content[:60]}")


if __name__ == "__main__":
    asyncio.run(main())
