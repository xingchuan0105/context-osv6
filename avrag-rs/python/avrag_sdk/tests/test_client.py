"""Tests for avrag_sdk.client.

Uses respx to mock HTTP responses — no live backend required.
"""

from __future__ import annotations

import pytest
import respx
import httpx

from avrag_sdk import AvragClient
from avrag_sdk.exceptions import AvragAPIError, AvragTimeoutError, AvragAuthError


@pytest.fixture
def client() -> AvragClient:
    return AvragClient(base_url="http://test-api:8080", timeout=5.0)


@pytest.mark.asyncio
@respx.mock
async def test_dense_returns_chunks(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/dense_retrieval").mock(
        return_value=httpx.Response(
            200,
            json={
                "chunks": [
                    {
                        "chunk_id": "c1",
                        "doc_id": "d1",
                        "content": "hello",
                        "score": 0.9,
                        "source": "test",
                    }
                ]
            },
        )
    )
    chunks = await client.dense("query", k=5)
    assert len(chunks) == 1
    assert chunks[0].chunk_id == "c1"
    assert chunks[0].score == 0.9


@pytest.mark.asyncio
@respx.mock
async def test_dense_batch_returns_grouped_results(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/dense_retrieval/batch").mock(
        return_value=httpx.Response(
            200,
            json={
                "results": [
                    [{"chunk_id": "c1", "doc_id": "d1", "content": "x", "score": 0.9, "source": "t"}],
                    [{"chunk_id": "c2", "doc_id": "d2", "content": "y", "score": 0.8, "source": "t"}],
                ]
            },
        )
    )
    results = await client.dense_batch(["q1", "q2"], k=5)
    assert len(results) == 2
    assert results[0][0].chunk_id == "c1"
    assert results[1][0].chunk_id == "c2"


@pytest.mark.asyncio
@respx.mock
async def test_lexical_passes_doc_ids(client: AvragClient) -> None:
    route = respx.post("http://test-api:8080/tools/lexical_retrieval").mock(
        return_value=httpx.Response(200, json={"chunks": []})
    )
    await client.lexical("contract 123", k=10, doc_ids=["d1", "d2"])
    request_payload = route.calls.last.request.content.decode()
    assert '"doc_ids":["d1","d2"]' in request_payload


@pytest.mark.asyncio
@respx.mock
async def test_graph_returns_relations(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/graph_retrieval").mock(
        return_value=httpx.Response(
            200,
            json={
                "relations": [
                    {
                        "subject": "Company A",
                        "predicate": "owns",
                        "object": "Company B",
                        "score": 0.95,
                    }
                ]
            },
        )
    )
    relations = await client.graph(entity_names=["Company A"])
    assert len(relations) == 1
    assert relations[0].subject == "Company A"
    assert relations[0].object == "Company B"


@pytest.mark.asyncio
@respx.mock
async def test_rerank_returns_ordered_chunks(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/rerank").mock(
        return_value=httpx.Response(
            200,
            json={
                "chunks": [
                    {
                        "chunk_id": "c2",
                        "doc_id": "d1",
                        "content": "y",
                        "score": 0.95,
                        "source": "t",
                    }
                ]
            },
        )
    )
    candidates = [
        {
            "chunk_id": "c1",
            "doc_id": "d1",
            "content": "x",
            "score": 0.5,
            "source": "t",
        }
    ]
    # Wrap as Chunk models
    from avrag_sdk import Chunk
    chunk_models = [Chunk(**c) for c in candidates]
    result = await client.rerank("query", chunk_models, top_k=1)
    assert result[0].chunk_id == "c2"


@pytest.mark.asyncio
@respx.mock
async def test_http_error_raises_api_error(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/dense_retrieval").mock(
        return_value=httpx.Response(500, text="internal error")
    )
    with pytest.raises(AvragAPIError) as exc_info:
        await client.dense("query", k=5)
    assert exc_info.value.status_code == 500


@pytest.mark.asyncio
@respx.mock
async def test_timeout_raises_timeout_error(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/dense_retrieval").mock(
        side_effect=httpx.TimeoutException("timeout")
    )
    with pytest.raises(AvragTimeoutError):
        await client.dense("query", k=5)


@pytest.mark.asyncio
@respx.mock
async def test_auth_failure_raises_auth_error(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/dense_retrieval").mock(
        return_value=httpx.Response(401, text="unauthorized")
    )
    with pytest.raises(AvragAuthError):
        await client.dense("query", k=5)


@pytest.mark.asyncio
@respx.mock
async def test_web_search(client: AvragClient) -> None:
    respx.post("http://test-api:8080/tools/web_search").mock(
        return_value=httpx.Response(
            200,
            json={
                "results": [
                    {
                        "title": "Example",
                        "url": "https://example.com",
                        "snippet": "test snippet",
                        "citation_index": 1,
                    }
                ]
            },
        )
    )
    results = await client.web_search("test query")
    assert len(results) == 1
    assert results[0].url == "https://example.com"
    assert results[0].citation_index == 1
