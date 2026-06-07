"""AvragClient — async HTTP client for avrag-rs retrieval primitives.

Endpoint design (one-to-one with `crates/rag-core/src/runtime/tools/`):

    POST /tools/dense_retrieval          → dense()
    POST /tools/lexical_retrieval        → lexical()
    POST /tools/graph_retrieval          → graph()
    POST /tools/index_lookup             → index_lookup()
    POST /tools/doc_summary              → doc_summary()
    POST /tools/doc_metadata             → doc_metadata()
    POST /tools/rerank                   → rerank()
    POST /tools/web_search               → web_search()

Batch endpoints (cost amortization):

    POST /tools/dense_retrieval/batch    → dense_batch()
    POST /tools/rerank/batch             → rerank_batch()

The Rust handler for each endpoint is essentially the existing tool
implementation, exposed as HTTP. We do NOT add new business logic in
the Python SDK — it is a thin transport.
"""

from __future__ import annotations

import os
from typing import Optional

import httpx

from avrag_sdk.types import Chunk, Relation, Document, Summary, Metadata, WebResult
from avrag_sdk.exceptions import AvragAPIError, AvragTimeoutError, AvragAuthError

DEFAULT_BASE_URL = "http://localhost:8080"
DEFAULT_TIMEOUT = 30.0


class AvragClient:
    """Async client for avrag-rs retrieval primitives.

    Usage:
        from avrag_sdk import AvragClient
        client = AvragClient(base_url="http://avrag-api:8080")
        chunks = await client.dense("query", k=10)

    Or use the module-level singleton:
        from avrag_sdk import client
        chunks = await client.dense("query", k=10)
    """

    def __init__(
        self,
        base_url: Optional[str] = None,
        timeout: float = DEFAULT_TIMEOUT,
        auth_token: Optional[str] = None,
    ):
        self.base_url = (
            base_url
            or os.environ.get("AVRAG_API_URL")
            or DEFAULT_BASE_URL
        ).rstrip("/")
        self.timeout = timeout
        self._auth_token = auth_token or os.environ.get("AVRAG_AUTH_TOKEN")
        self._http: Optional[httpx.AsyncClient] = None

    async def __aenter__(self) -> "AvragClient":
        await self._ensure_client()
        return self

    async def __aexit__(self, *exc) -> None:
        await self.close()

    async def _ensure_client(self) -> httpx.AsyncClient:
        if self._http is None:
            headers = {}
            if self._auth_token:
                headers["Authorization"] = f"Bearer {self._auth_token}"
            self._http = httpx.AsyncClient(
                base_url=self.base_url,
                timeout=self.timeout,
                headers=headers,
            )
        return self._http

    async def close(self) -> None:
        if self._http is not None:
            await self._http.aclose()
            self._http = None

    async def _post(self, path: str, payload: dict) -> dict:
        client = await self._ensure_client()
        try:
            response = await client.post(path, json=payload)
        except httpx.TimeoutException as e:
            raise AvragTimeoutError(f"timeout calling {path}: {e}") from e
        except httpx.HTTPError as e:
            raise AvragAPIError(0, f"transport error: {e}") from e

        if response.status_code == 401 or response.status_code == 403:
            raise AvragAuthError(f"auth failed: {response.text}")
        if response.status_code >= 400:
            raise AvragAPIError(
                response.status_code,
                response.text,
                body=response.text,
            )
        return response.json()

    # ----------------------------------------------------------------
    # Atomic retrieval primitives
    # ----------------------------------------------------------------

    async def dense(self, query: str, k: int = 10, doc_ids: Optional[list[str]] = None) -> list[Chunk]:
        """Vector similarity search.

        Args:
            query: The query string (will be embedded server-side).
            k: Number of results to return.
            doc_ids: Optional scope to specific document IDs.

        Returns:
            List of scored chunks, ordered by similarity (highest first).
        """
        payload = {"query": query, "k": k}
        if doc_ids is not None:
            payload["doc_ids"] = doc_ids
        data = await self._post("/tools/dense_retrieval", payload)
        return [Chunk(**c) for c in data["chunks"]]

    async def dense_batch(
        self,
        queries: list[str],
        k: int = 10,
        doc_ids: Optional[list[str]] = None,
    ) -> list[list[Chunk]]:
        """Batch vector search — amortizes embedding API cost.

        Use this when you have multiple query variations and want to avoid
        N separate embedding API calls. The server embeds all queries in
        one batched embedding API call.

        Returns:
            List of chunk lists, one per query (same order as input).
        """
        payload = {"queries": queries, "k": k}
        if doc_ids is not None:
            payload["doc_ids"] = doc_ids
        data = await self._post("/tools/dense_retrieval/batch", payload)
        return [[Chunk(**c) for c in group] for group in data["results"]]

    async def lexical(
        self,
        query: str,
        k: int = 10,
        doc_ids: Optional[list[str]] = None,
    ) -> list[Chunk]:
        """BM25 / keyword-based retrieval.

        Best for: exact terms, contract numbers, legal citations, names.
        """
        payload = {"query": query, "k": k}
        if doc_ids is not None:
            payload["doc_ids"] = doc_ids
        data = await self._post("/tools/lexical_retrieval", payload)
        return [Chunk(**c) for c in data["chunks"]]

    async def graph(
        self,
        entity_names: list[str],
        relation_hints: Optional[list[dict]] = None,
        relation_limit: int = 10,
        supporting_chunk_limit: int = 10,
    ) -> list[Relation]:
        """Knowledge graph traversal from named entities.

        Best for: entity-relation queries (股权结构 / 上下游 / 组织关系).
        """
        payload = {
            "entity_names": entity_names,
            "relation_limit": relation_limit,
            "supporting_chunk_limit": supporting_chunk_limit,
        }
        if relation_hints is not None:
            payload["relation_hints"] = relation_hints
        data = await self._post("/tools/graph_retrieval", payload)
        return [Relation(**r) for r in data["relations"]]

    async def index_lookup(
        self,
        doc_ids: list[str],
        fields: Optional[list[str]] = None,
    ) -> list[Document]:
        """Look up documents by ID — returns basic metadata + indexed fields.

        Use this when you already have a doc_id (e.g., from graph result)
        and need the document record.
        """
        payload = {"doc_ids": doc_ids}
        if fields is not None:
            payload["fields"] = fields
        data = await self._post("/tools/index_lookup", payload)
        return [Document(**d) for d in data["documents"]]

    async def doc_summary(self, doc_ids: list[str]) -> list[Summary]:
        """Get pre-computed document summaries.

        Use for quick triage before reading full content.
        """
        payload = {"doc_ids": doc_ids}
        data = await self._post("/tools/doc_summary", payload)
        return [Summary(**s) for s in data["summaries"]]

    async def doc_metadata(
        self,
        doc_ids: list[str],
        fields: list[str],
    ) -> list[Metadata]:
        """Get specific metadata fields for documents.

        Best for: structured filters (date / org / type / status).
        """
        payload = {"doc_ids": doc_ids, "fields": fields}
        data = await self._post("/tools/doc_metadata", payload)
        return [Metadata(**m) for m in data["metadata"]]

    # ----------------------------------------------------------------
    # Rerank
    # ----------------------------------------------------------------

    async def rerank(
        self,
        query: str,
        candidates: list[Chunk],
        top_k: Optional[int] = None,
    ) -> list[Chunk]:
        """Rerank candidate chunks against a query.

        Args:
            query: The query to rerank against.
            candidates: Candidate chunks to rerank.
            top_k: If set, return only top-k after reranking. Otherwise
                return all candidates in new order.
        """
        payload = {
            "query": query,
            "candidates": [c.model_dump() for c in candidates],
        }
        if top_k is not None:
            payload["top_k"] = top_k
        data = await self._post("/tools/rerank", payload)
        return [Chunk(**c) for c in data["chunks"]]

    async def rerank_batch(
        self,
        query: str,
        candidates_list: list[list[Chunk]],
    ) -> list[list[Chunk]]:
        """Batch rerank — rerank multiple candidate lists against the same query
        in one rerank API call.

        Returns:
            List of reranked chunk lists, one per input list.
        """
        payload = {
            "query": query,
            "candidates_list": [
                [c.model_dump() for c in group] for group in candidates_list
            ],
        }
        data = await self._post("/tools/rerank/batch", payload)
        return [[Chunk(**c) for c in group] for group in data["results"]]

    # ----------------------------------------------------------------
    # Web search
    # ----------------------------------------------------------------

    async def web_search(
        self,
        query: str,
        vertical: Optional[str] = None,
    ) -> list[WebResult]:
        """Web search via Brave.

        Args:
            query: Search query.
            vertical: Optional vertical ("news" for news-only).

        Returns:
            Search results with title, url, snippet.
        """
        payload = {"query": query}
        if vertical is not None:
            payload["vertical"] = vertical
        data = await self._post("/tools/web_search", payload)
        return [WebResult(**r) for r in data["results"]]
