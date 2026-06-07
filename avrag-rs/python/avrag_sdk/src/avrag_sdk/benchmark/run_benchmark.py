"""E2E benchmark runner — code-gen vs baseline.

Two execution modes:

1. **mock mode** (default): uses an in-process mock backend (same one as
   `tests/test_code_gen_vs_baseline.py`). Runs end-to-end with no external
   dependencies. Useful for verifying the framework itself works.

2. **live mode** (`--live`): connects to a real avrag-rs backend (the
   `bins/api` Rust server) and a real LLM provider. Requires:
   - `AVRAG_API_URL` set to the live backend
   - `AVRAG_AUTH_TOKEN` for the backend
   - An LLM provider configured (DeepSeek / DashScope / etc.)
   - Sample documents ingested into the backend

For each query, the benchmark runs BOTH paths:

  - **baseline**: routes through the existing RAG pipeline
    (using the standard `dense` retrieval via the SDK, simulating
    what the existing state machine would do for "simple" queries)
  - **code-gen**: routes through the new `code_gen_query` tool,
    invoking the SDK's code-gen orchestration pattern

Metrics captured per (query, path):
  - wall_clock_ms
  - chunk_count
  - source_diversity (number of unique sources)
  - top_score (highest relevance score in result set)
  - sdk_calls (count of HTTP calls to backend)
  - estimated_cost_usd (rough estimate based on call counts)

Usage:

    # mock mode (no setup needed)
    python -m avrag_sdk.benchmark.run_benchmark

    # live mode
    AVRAG_API_URL=http://localhost:8080 \\
    AVRAG_AUTH_TOKEN=... \\
    python -m avrag_sdk.benchmark.run_benchmark --live

Output:
    - `benchmark_results.json` — machine-readable per-query metrics
    - `benchmark_report.md` — human-readable summary
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import sys
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Any

from avrag_sdk import AvragClient, Chunk

# Import queries (relative to this file)
sys.path.insert(0, str(Path(__file__).parent.parent))
from benchmark.queries import QUERIES, BenchmarkQuery


# ---------------------------------------------------------------------------
# Cost estimation (rough — adjust per your LLM provider's pricing)
# ---------------------------------------------------------------------------

# Per-call cost estimates (in USD). Update for your providers.
EMBEDDING_COST_PER_CALL = 0.0001  # $0.0001 per embed call
RERANK_COST_PER_CALL = 0.001       # $0.001 per rerank call
LLM_INPUT_TOKEN_COST = 0.0001 / 1000   # $0.0001 per 1k input tokens
LLM_OUTPUT_TOKEN_COST = 0.0002 / 1000  # $0.0002 per 1k output tokens


# ---------------------------------------------------------------------------
# Metric collection
# ---------------------------------------------------------------------------

@dataclass
class PathMetrics:
    """Metrics captured for a single (query, path) execution."""
    path: str  # "baseline" | "code-gen"
    query_id: str
    success: bool
    error: str | None = None
    wall_clock_ms: int = 0
    chunk_count: int = 0
    source_diversity: int = 0
    top_score: float = 0.0
    mean_score: float = 0.0
    sdk_calls: int = 0
    estimated_cost_usd: float = 0.0
    chunks: list[dict] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Path implementations
# ---------------------------------------------------------------------------

# Per-call counters — used by both paths to record SDK activity.
_call_counters: dict[str, int] = {}


class CountingClient(AvragClient):
    """Wraps AvragClient to count SDK calls."""

    def _record_call(self, endpoint: str) -> None:
        _call_counters[endpoint] = _call_counters.get(endpoint, 0) + 1

    async def dense(self, query: str, k: int = 10, doc_ids=None):
        self._record_call("dense")
        return await super().dense(query, k, doc_ids)

    async def dense_batch(self, queries, k: int = 10, doc_ids=None):
        self._record_call("dense_batch")
        return await super().dense_batch(queries, k, doc_ids)

    async def lexical(self, query: str, k: int = 10, doc_ids=None):
        self._record_call("lexical")
        return await super().lexical(query, k, doc_ids)

    async def graph(self, entity_names, relation_hints=None,
                    relation_limit=10, supporting_chunk_limit=10):
        self._record_call("graph")
        return await super().graph(entity_names, relation_hints,
                                   relation_limit, supporting_chunk_limit)

    async def rerank(self, query, candidates, top_k=None):
        self._record_call("rerank")
        return await super().rerank(query, candidates, top_k)

    async def rerank_batch(self, query, candidates_list):
        self._record_call("rerank_batch")
        return await super().rerank_batch(query, candidates_list)


async def run_baseline_path(client: CountingClient, query: str) -> list[Chunk]:
    """Baseline path: single dense call, no orchestration.

    This represents what the existing pipeline does for simple/medium
    queries. The Rust pipeline uses RRF + rerank internally, but for
    the purposes of this benchmark, we measure what a single dense
    call would return — the dominant behavior for simple queries.
    """
    _call_counters.clear()
    return await client.dense(query, k=10)


async def run_code_gen_path(client: CountingClient, query: str) -> list[Chunk]:
    """Code-gen path: multi-strategy + dedup + rerank.

    Mirrors Pattern A (cross-doc aggregation) from the skill.
    """
    _call_counters.clear()
    queries = [query, f"{query} 详情", f"{query} 上下文"]
    dense_results = await client.dense_batch(queries, k=10)
    lexical_results = await client.lexical(query, k=10)
    merged: dict[str, Chunk] = {}
    for group in dense_results:
        for chunk in group:
            merged[chunk.chunk_id] = chunk
    for chunk in lexical_results:
        if chunk.chunk_id not in merged or chunk.score > merged[chunk.chunk_id].score:
            merged[chunk.chunk_id] = chunk
    candidates = list(merged.values())
    reranked = await client.rerank(query, candidates, top_k=10)
    return reranked


# ---------------------------------------------------------------------------
# Mock backend for `--mock` mode
# ---------------------------------------------------------------------------

# Module-level handle to the mock server. Keeps it alive for the duration
# of the benchmark (otherwise the `with` block in `_install_mock_backend`
# would shut it down when the function returns).
_mock_server: "socketserver.TCPServer | None" = None
_mock_thread: "threading.Thread | None" = None


def _install_mock_backend() -> str:
    """Install a mock backend on a free port. Returns base_url.

    The server is kept alive at module scope so the benchmark can make
    multiple requests against it. Call `_shutdown_mock_backend()` to stop
    it.
    """
    import http.server
    import socketserver
    import threading

    global _mock_server, _mock_thread

    general = [
        {"chunk_id": f"d-{i}", "doc_id": f"doc-{i}", "content": f"chunk {i}",
         "score": 0.9 - i * 0.05, "source": "dense", "page": 1}
        for i in range(20)
    ]
    lex = [
        {"chunk_id": f"l-{i}", "doc_id": f"lex-doc-{i}", "content": f"lex match {i}",
         "score": 0.85 - i * 0.05, "source": "bm25", "page": 1}
        for i in range(10)
    ]

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_POST(self):  # noqa: N802
            import json
            length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(length) if length > 0 else b"{}"
            try:
                payload = json.loads(body) if body else {}
            except json.JSONDecodeError:
                payload = {}

            if self.path == "/tools/dense_retrieval":
                top = min(payload.get("k", 10), len(general))
                data = {"chunks": general[:top]}
            elif self.path == "/tools/dense_retrieval/batch":
                queries = payload.get("queries", [])
                results = [list(general) for _ in queries]
                data = {"results": results}
            elif self.path == "/tools/lexical_retrieval":
                top = min(payload.get("k", 10), len(lex))
                data = {"chunks": lex[:top]}
            elif self.path == "/tools/rerank":
                cands = payload.get("candidates", [])
                ordered = []
                for i, c in enumerate(cands):
                    improved = dict(c)
                    improved["score"] = min(c.get("score", 0.5) + 0.05, 1.0)
                    ordered.append(improved)
                top_k = payload.get("top_k")
                if top_k is not None:
                    ordered = ordered[:top_k]
                data = {"chunks": ordered}
            else:
                self.send_response(404)
                self.end_headers()
                return

            body = json.dumps(data).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args, **kwargs):  # noqa: A002
            pass

    socketserver.TCPServer.allow_reuse_address = True
    _mock_server = socketserver.TCPServer(("127.0.0.1", 0), Handler)
    port = _mock_server.server_address[1]
    base_url = f"http://127.0.0.1:{port}"
    _mock_thread = threading.Thread(target=_mock_server.serve_forever, daemon=True)
    _mock_thread.start()
    return base_url


def _shutdown_mock_backend() -> None:
    """Stop the mock server if it was started."""
    global _mock_server
    if _mock_server is not None:
        _mock_server.shutdown()
        _mock_server.server_close()
        _mock_server = None


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------

def _estimate_cost(sdk_call_counts: dict[str, int]) -> float:
    """Rough cost estimate based on SDK call counts."""
    cost = 0.0
    cost += sdk_call_counts.get("dense", 0) * EMBEDDING_COST_PER_CALL
    cost += sdk_call_counts.get("dense_batch", 0) * EMBEDDING_COST_PER_CALL
    cost += sdk_call_counts.get("rerank", 0) * RERANK_COST_PER_CALL
    cost += sdk_call_counts.get("rerank_batch", 0) * RERANK_COST_PER_CALL
    # graph and lexical are usually free (in-process or DB)
    return cost


async def _run_one(
    client: CountingClient,
    query: BenchmarkQuery,
    path: str,
) -> PathMetrics:
    """Run a single (query, path) combination and capture metrics."""
    metrics = PathMetrics(path=path, query_id=query.id, success=False)
    started = time.perf_counter()
    try:
        if path == "baseline":
            chunks = await run_baseline_path(client, query.text)
        else:
            chunks = await run_code_gen_path(client, query.text)

        metrics.wall_clock_ms = int((time.perf_counter() - started) * 1000)
        metrics.chunk_count = len(chunks)
        if chunks:
            metrics.top_score = max(c.score for c in chunks)
            metrics.mean_score = sum(c.score for c in chunks) / len(chunks)
            metrics.source_diversity = len({c.source for c in chunks})
        metrics.sdk_calls = sum(_call_counters.values())
        metrics.estimated_cost_usd = _estimate_cost(_call_counters.copy())
        metrics.chunks = [
            {
                "chunk_id": c.chunk_id,
                "doc_id": c.doc_id,
                "score": c.score,
                "source": c.source,
            }
            for c in chunks
        ]
        metrics.success = True
    except Exception as e:
        metrics.wall_clock_ms = int((time.perf_counter() - started) * 1000)
        metrics.error = str(e)
    return metrics


async def run_benchmark(base_url: str, output_dir: Path) -> dict[str, Any]:
    """Run the full benchmark (5 queries × 2 paths) and save results."""
    output_dir.mkdir(parents=True, exist_ok=True)

    client = CountingClient(base_url=base_url, timeout=60.0)

    results: list[PathMetrics] = []
    for query in QUERIES:
        print(f"\n=== {query.id} ({query.complexity}/{query.category}) ===")
        print(f"  Q: {query.text}")
        for path in ["baseline", "code-gen"]:
            print(f"  [{path}] running...", end=" ", flush=True)
            m = await _run_one(client, query, path)
            results.append(m)
            if m.success:
                print(
                    f"chunks={m.chunk_count} top_score={m.top_score:.2f} "
                    f"wall={m.wall_clock_ms}ms calls={m.sdk_calls}"
                )
            else:
                print(f"FAILED: {m.error}")

    # Save JSON results
    json_path = output_dir / "benchmark_results.json"
    with open(json_path, "w") as f:
        json.dump(
            {
                "base_url": base_url,
                "queries": [asdict(q) for q in QUERIES],
                "results": [asdict(r) for r in results],
            },
            f,
            indent=2,
        )
    print(f"\nResults saved to {json_path}")

    # Generate markdown report
    report_path = output_dir / "benchmark_report.md"
    _write_report(QUERIES, results, report_path, base_url)
    print(f"Report saved to {report_path}")

    return {"json": str(json_path), "md": str(report_path)}


def _write_report(
    queries: list[BenchmarkQuery],
    results: list[PathMetrics],
    path: Path,
    base_url: str,
) -> None:
    """Write a human-readable markdown report."""
    by_query: dict[str, dict[str, PathMetrics]] = {}
    for r in results:
        by_query.setdefault(r.query_id, {})[r.path] = r

    lines = [
        "# Code-Gen E2E Benchmark Report",
        "",
        f"- Backend: `{base_url}`",
        f"- Queries: {len(queries)}",
        f"- Date: {time.strftime('%Y-%m-%d %H:%M:%S')}",
        "",
        "## Summary",
        "",
        "| ID | Complexity | Expected Winner | Actual Winner | Δ Top Score | Δ Latency |",
        "|----|------------|-----------------|---------------|-------------|-----------|",
    ]

    for q in queries:
        baseline = by_query.get(q.id, {}).get("baseline")
        codegen = by_query.get(q.id, {}).get("code-gen")
        if not baseline or not codegen:
            continue
        if not baseline.success or not codegen.success:
            actual = "FAILED"
            delta_score = "—"
            delta_latency = "—"
        else:
            actual = "code-gen" if codegen.top_score > baseline.top_score else (
                "baseline" if baseline.top_score > codegen.top_score else "tie"
            )
            delta_score = f"{codegen.top_score - baseline.top_score:+.2f}"
            delta_latency = f"{codegen.wall_clock_ms - baseline.wall_clock_ms:+d}ms"
        lines.append(
            f"| {q.id} | {q.complexity} | {q.expected_winner} | {actual} | {delta_score} | {delta_latency} |"
        )

    lines.extend([
        "",
        "## Per-query details",
        "",
    ])
    for q in queries:
        baseline = by_query.get(q.id, {}).get("baseline")
        codegen = by_query.get(q.id, {}).get("code-gen")
        if not baseline or not codegen:
            continue
        lines.extend([
            f"### {q.id}: {q.text}",
            "",
            f"- Complexity: {q.complexity}",
            f"- Category: {q.category}",
            f"- Expected winner: **{q.expected_winner}**",
            "",
            "| Metric | Baseline | Code-Gen | Δ |",
            "|--------|----------|----------|---|",
            f"| Wall clock (ms) | {baseline.wall_clock_ms} | {codegen.wall_clock_ms} | "
            f"{codegen.wall_clock_ms - baseline.wall_clock_ms:+d} |",
            f"| Chunk count | {baseline.chunk_count} | {codegen.chunk_count} | "
            f"{codegen.chunk_count - baseline.chunk_count:+d} |",
            f"| Top score | {baseline.top_score:.3f} | {codegen.top_score:.3f} | "
            f"{codegen.top_score - baseline.top_score:+.3f} |",
            f"| Mean score | {baseline.mean_score:.3f} | {codegen.mean_score:.3f} | "
            f"{codegen.mean_score - baseline.mean_score:+.3f} |",
            f"| Source diversity | {baseline.source_diversity} | {codegen.source_diversity} | "
            f"{codegen.source_diversity - baseline.source_diversity:+d} |",
            f"| SDK calls | {baseline.sdk_calls} | {codegen.sdk_calls} | "
            f"{codegen.sdk_calls - baseline.sdk_calls:+d} |",
            f"| Estimated cost (USD) | ${baseline.estimated_cost_usd:.4f} | "
            f"${codegen.estimated_cost_usd:.4f} | "
            f"${codegen.estimated_cost_usd - baseline.estimated_cost_usd:+.4f} |",
            "",
        ])

    lines.extend([
        "## Interpretation",
        "",
        "This benchmark compares the **existing dense-only baseline** (a single",
        "dense retrieval call) against the **code-gen path** (multi-strategy with",
        "fan-out, dedup, and rerank).",
        "",
        "**Top score higher is better** — code-gen's rerank is expected to improve",
        "top result quality. **Latency higher is worse** — code-gen does more work.",
        "**SDK calls higher is more expensive** — code-gen trades external API",
        "calls for better results.",
        "",
        "**Expected outcomes**:",
        "- Simple queries (Q1, Q2): both should perform similarly; code-gen adds cost without much benefit",
        "- Medium queries (Q3): code-gen should start to show quality wins",
        "- Complex queries (Q4, Q5): code-gen should win on quality but lose on latency",
        "",
        "If the results contradict the hypotheses, the next step is to investigate:",
        "- Did the LLM write a sub-optimal Python program? (check `code-gen` path's chunks)",
        "- Did the baseline's BM25 path miss important matches? (check Q3)",
        "- Is the mock corpus too uniform? (run against real data for better signal)",
    ])

    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Code-gen E2E benchmark")
    parser.add_argument(
        "--live",
        action="store_true",
        help="Use live backend (defaults to mock mode for self-test).",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("./benchmark_output"),
        help="Where to write results (default: ./benchmark_output)",
    )
    args = parser.parse_args()

    if args.live:
        base_url = os.environ.get("AVRAG_API_URL")
        if not base_url:
            print("ERROR: --live requires AVRAG_API_URL env var", file=sys.stderr)
            return 1
        print(f"Live mode: connecting to {base_url}")
    else:
        print("Mock mode: starting in-process mock backend")
        base_url = _install_mock_backend()
        print(f"Mock backend running at {base_url}")

    try:
        asyncio.run(run_benchmark(base_url, args.output_dir))
    except KeyboardInterrupt:
        print("\nInterrupted")
        return 130
    finally:
        if not args.live:
            _shutdown_mock_backend()
    return 0


if __name__ == "__main__":
    sys.exit(main())
