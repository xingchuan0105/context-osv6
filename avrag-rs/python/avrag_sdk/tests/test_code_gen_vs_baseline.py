"""End-to-end comparison: code-gen orchestration vs naive baseline.

This test exercises the SDK's `code_gen_query`-style orchestration against
a mock backend, and compares it to a naive single-tool baseline.

It does NOT exercise the Rust `code_gen_query` tool directly (that is
covered by the Rust integration tests in `crates/rag-core/src/runtime/
tools/code_gen_query.rs`). It tests the Python side end-to-end:

    1. Start a mock HTTP backend
    2. Run "naive baseline" — single dense retrieval
    3. Run "code-gen orchestration" — graph + dense_batch + lexical + rerank
    4. Compare chunk quality (relevance scores, diversity)

The point of this test is to validate the SDK's design for code-gen
patterns BEFORE the full Rust tool + LLM integration is wired up.
"""

from __future__ import annotations

import asyncio
import http.server
import json
import socketserver
import threading
from contextlib import contextmanager
from typing import Iterator

import pytest

from avrag_sdk import AvragClient, Chunk


# ---------------------------------------------------------------------------
# Mock backend — simulates the Rust retrieval tool's HTTP API
# ---------------------------------------------------------------------------

# Mock corpus: pretend we have 5 documents. Each "tool" returns a
# different subset ordered by relevance to the query "XX 客户 合同纠纷".

QUERY = "XX 客户 合同纠纷"

CORPUS = {
    "dense-1": {"chunk_id": "dense-1", "doc_id": "d-001", "content": "XX 客户与本公司的合同纠纷...", "score": 0.92, "source": "dense", "page": 1},
    "dense-2": {"chunk_id": "dense-2", "doc_id": "d-002", "content": "XX 客户过往合同签订背景...", "score": 0.85, "source": "dense", "page": 2},
    "dense-3": {"chunk_id": "dense-3", "doc_id": "d-003", "content": "XX 客户行业分析报告...", "score": 0.78, "source": "dense", "page": 1},
    "dense-4": {"chunk_id": "dense-4", "doc_id": "d-004", "content": "XX 客户财务状况...", "score": 0.72, "source": "dense", "page": 5},
    "dense-5": {"chunk_id": "dense-5", "doc_id": "d-005", "content": "无关内容", "score": 0.45, "source": "dense", "page": 1},
}

# Lexical results: emphasize exact "纠纷" / "合同" matches
LEXICAL_CORPUS = {
    "lex-1": {"chunk_id": "lex-1", "doc_id": "d-001", "content": "XX 客户与本公司合同纠纷案...起诉...仲裁", "score": 0.95, "source": "bm25", "page": 1},
    "lex-2": {"chunk_id": "lex-2", "doc_id": "d-002", "content": "XX 客户 合同 第3条 争议解决", "score": 0.88, "source": "bm25", "page": 3},
    "lex-3": {"chunk_id": "lex-3", "doc_id": "d-006", "content": "XX 客户 其他纠纷记录", "score": 0.80, "source": "bm25", "page": 2},
    "lex-4": {"chunk_id": "lex-4", "doc_id": "d-007", "content": "XX 客户 合同金额纠纷", "score": 0.75, "source": "bm25", "page": 1},
}

# Rerank simulates reordering by combining signals.
# The reranked "best" chunks are dense-1, lex-1, dense-2, lex-2 (in that order).
RERANKED_ORDER = ["lex-1", "dense-1", "dense-2", "lex-2", "dense-3", "lex-3", "dense-4", "lex-4"]


class MockBackendHandler(http.server.BaseHTTPRequestHandler):
    """Returns canned responses for the SDK's HTTP calls."""

    def do_POST(self) -> None:  # noqa: N802
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if length > 0 else b"{}"
        try:
            payload = json.loads(body) if body else {}
        except json.JSONDecodeError:
            payload = {}

        response_data: dict = {"chunks": []}

        if self.path == "/tools/dense_retrieval":
            # Single dense call
            top_n = min(payload.get("k", 10), 5)
            response_data["chunks"] = list(CORPUS.values())[:top_n]
        elif self.path == "/tools/dense_retrieval/batch":
            # Batched dense — return one list per query
            queries = payload.get("queries", [])
            results = []
            for _q in queries:
                top_n = min(payload.get("k", 10), 5)
                results.append(list(CORPUS.values())[:top_n])
            response_data = {"results": results}
        elif self.path == "/tools/lexical_retrieval":
            top_n = min(payload.get("k", 10), 4)
            response_data["chunks"] = list(LEXICAL_CORPUS.values())[:top_n]
        elif self.path == "/tools/rerank":
            # Reorder candidates per the simulated ranking
            candidates = payload.get("candidates", [])
            candidate_by_id = {c["chunk_id"]: c for c in candidates}
            ordered = []
            for cid in RERANKED_ORDER:
                if cid in candidate_by_id:
                    # Bump the score to simulate rerank improvement
                    improved = dict(candidate_by_id[cid])
                    improved["score"] = min(improved.get("score", 0.5) + 0.1, 1.0)
                    ordered.append(improved)
            top_k = payload.get("top_k")
            if top_k is not None:
                ordered = ordered[:top_k]
            response_data["chunks"] = ordered
        elif self.path == "/tools/web_search":
            response_data["results"] = []
        else:
            self.send_response(404)
            self.end_headers()
            return

        body = json.dumps(response_data).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args) -> None:  # noqa: A002
        # Silence the test logs
        pass


@contextmanager
def start_mock_backend() -> Iterator[str]:
    """Start a mock backend on a free port; return base_url."""
    # Find a free port
    with socketserver.TCPServer(("127.0.0.1", 0), MockBackendHandler) as httpd:
        port = httpd.server_address[1]
        base_url = f"http://127.0.0.1:{port}"

        # Run in a thread (BaseHTTPRequestHandler is sync)
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            yield base_url
        finally:
            httpd.shutdown()
            httpd.server_close()


# ---------------------------------------------------------------------------
# The two strategies being compared
# ---------------------------------------------------------------------------

async def naive_baseline(client: AvragClient, query: str) -> list[Chunk]:
    """Single-tool baseline: just dense retrieval, no orchestration."""
    return await client.dense(query, k=5)


async def code_gen_orchestration(client: AvragClient, query: str) -> list[Chunk]:
    """Code-gen orchestration: multi-strategy + dedup + rerank."""
    # Fan-out queries (multi-strategy)
    queries = [query, f"{query} 起诉", f"{query} 仲裁"]
    dense_results = await client.dense_batch(queries, k=5)
    lexical_results = await client.lexical(query, k=4)

    # Dedupe
    merged: dict[str, Chunk] = {}
    for group in dense_results:
        for chunk in group:
            merged[chunk.chunk_id] = chunk
    for chunk in lexical_results:
        if chunk.chunk_id not in merged or chunk.score > merged[chunk.chunk_id].score:
            merged[chunk.chunk_id] = chunk

    # Rerank the merged set
    candidates = list(merged.values())
    reranked = await client.rerank(query, candidates, top_k=5)
    return reranked


# ---------------------------------------------------------------------------
# The actual E2E test
# ---------------------------------------------------------------------------

@pytest.mark.asyncio
async def test_code_gen_outperforms_naive_baseline() -> None:
    """E2E: code-gen orchestration should retrieve more relevant chunks than naive baseline.

    Naive baseline: just `dense(query, k=5)`.
    Code-gen: fan-out queries + multi-strategy + rerank.

    The mock backend ensures reranked chunks have higher relevance scores
    than the naive baseline's chunks.
    """
    with start_mock_backend() as base_url:
        client = AvragClient(base_url=base_url, timeout=5.0)

        baseline_chunks = await naive_baseline(client, QUERY)
        codegen_chunks = await code_gen_orchestration(client, QUERY)

        # Both should return chunks
        assert len(baseline_chunks) > 0
        assert len(codegen_chunks) > 0

        # The code-gen path's top chunk should have a higher score
        # (mock rerank adds 0.1 to the top candidates)
        baseline_top_score = max(c.score for c in baseline_chunks)
        codegen_top_score = max(c.score for c in codegen_chunks)
        assert codegen_top_score > baseline_top_score, (
            f"code-gen should improve top score: "
            f"baseline={baseline_top_score:.2f}, codegen={codegen_top_score:.2f}"
        )

        # Code-gen should have MORE diverse chunks (covers both dense and lexical sources)
        baseline_sources = {c.source for c in baseline_chunks}
        codegen_sources = {c.source for c in codegen_chunks}
        assert len(codegen_sources) > len(baseline_sources), (
            f"code-gen should cover more sources: "
            f"baseline={baseline_sources}, codegen={codegen_sources}"
        )

        # Code-gen should hit the "exact match" chunks (lex-1, lex-2)
        # that the naive dense baseline missed
        baseline_ids = {c.chunk_id for c in baseline_chunks}
        codegen_ids = {c.chunk_id for c in codegen_chunks}
        new_in_codegen = codegen_ids - baseline_ids
        assert any(cid.startswith("lex-") for cid in new_in_codegen), (
            f"code-gen should bring in lexical chunks missed by dense-only baseline; "
            f"new chunks: {new_in_codegen}"
        )


@pytest.mark.asyncio
async def test_baseline_and_codegen_both_succeed() -> None:
    """Sanity check: both paths produce valid output for the same query."""
    with start_mock_backend() as base_url:
        client = AvragClient(base_url=base_url, timeout=5.0)

        baseline = await naive_baseline(client, QUERY)
        codegen = await code_gen_orchestration(client, QUERY)

        # Both should be valid chunk lists
        for chunk in baseline + codegen:
            assert chunk.chunk_id
            assert chunk.doc_id
            assert chunk.content
            assert 0.0 <= chunk.score <= 1.0


@pytest.mark.asyncio
async def test_codegen_dedupes_chunks_across_retrievers() -> None:
    """Verify that code-gen dedupes chunks that appear in multiple retriever results."""
    with start_mock_backend() as base_url:
        client = AvragClient(base_url=base_url, timeout=5.0)

        # Mock returns dense-1 in BOTH dense and (via dense_batch) for all queries.
        # After dedup, dense-1 should appear only once in the merged set.
        chunks = await code_gen_orchestration(client, QUERY)

        chunk_ids = [c.chunk_id for c in chunks]
        # The candidates BEFORE rerank would have had duplicates
        # The rerank input has 8 unique chunks (5 dense + 3 new from lexical)
        # After dedup, no duplicates
        assert len(chunk_ids) == len(set(chunk_ids)), "code-gen should dedupe chunks"


@pytest.mark.asyncio
async def test_codegen_uses_batch_for_dense() -> None:
    """Verify code-gen uses dense_batch (not loop of dense) — important for cost."""
    call_log: list[str] = []

    class LoggingHandler(MockBackendHandler):
        def do_POST(self) -> None:  # noqa: N802
            call_log.append(self.path)
            super().do_POST()

    with socketserver.TCPServer(("127.0.0.1", 0), LoggingHandler) as httpd:
        port = httpd.server_address[1]
        base_url = f"http://127.0.0.1:{port}"
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            client = AvragClient(base_url=base_url, timeout=5.0)
            await code_gen_orchestration(client, QUERY)
        finally:
            httpd.shutdown()
            httpd.server_close()

        # Verify the code-gen path uses BATCH, not individual dense calls
        assert "/tools/dense_retrieval/batch" in call_log, (
            f"code-gen should use dense_batch endpoint; saw calls: {call_log}"
        )
        # And only ONE batch call (not multiple)
        batch_calls = [c for c in call_log if c == "/tools/dense_retrieval/batch"]
        assert len(batch_calls) == 1, (
            f"code-gen should use a single batch call, not loop; saw {len(batch_calls)}"
        )


@pytest.mark.asyncio
async def test_codegen_uses_single_rerank_at_end() -> None:
    """Verify code-gen reranks once at the end, not multiple times."""
    call_log: list[str] = []

    class LoggingHandler(MockBackendHandler):
        def do_POST(self) -> None:  # noqa: N802
            call_log.append(self.path)
            super().do_POST()

    with socketserver.TCPServer(("127.0.0.1", 0), LoggingHandler) as httpd:
        port = httpd.server_address[1]
        base_url = f"http://127.0.0.1:{port}"
        thread = threading.Thread(target=httpd.serve_forever, daemon=True)
        thread.start()
        try:
            client = AvragClient(base_url=base_url, timeout=5.0)
            await code_gen_orchestration(client, QUERY)
        finally:
            httpd.shutdown()
            httpd.server_close()

        rerank_calls = [c for c in call_log if c == "/tools/rerank"]
        assert len(rerank_calls) == 1, (
            f"code-gen should rerank once at the end, not multiple times; "
            f"saw {len(rerank_calls)} rerank calls"
        )
