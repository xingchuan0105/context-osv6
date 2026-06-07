"""5 representative queries for the code-gen E2E benchmark.

These are designed to cover the spectrum of query complexity:

| ID | Complexity | Type | What it tests |
|----|-----------|------|---------------|
| Q1 | Simple    | Factual lookup | Baseline should match code-gen — both are easy |
| Q2 | Simple    | Single doc     | Baseline should match code-gen — both are easy |
| Q3 | Medium    | Cross-doc list | Code-gen's fan-out may help |
| Q4 | Complex   | Time-filtered aggregation | Code-gen's time-window logic should help |
| Q5 | Complex   | Multi-entity comparison | Code-gen's cross-source correlation should help |

The expected_winner column is a hypothesis based on the article's claims
— the actual benchmark may confirm or refute.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass
class BenchmarkQuery:
    id: str
    text: str
    complexity: str  # simple | medium | complex
    category: str    # factual | single-doc | cross-doc | time-aggregated | comparison
    description: str  # what the query is asking, in plain language
    expected_winner: str  # "baseline" | "code-gen" | "tie" — hypothesis


QUERIES: list[BenchmarkQuery] = [
    BenchmarkQuery(
        id="Q1",
        text="XX 客户的注册地址是什么？",
        complexity="simple",
        category="factual",
        description="Single-fact lookup about a specific entity.",
        expected_winner="tie",
    ),
    BenchmarkQuery(
        id="Q2",
        text="2024 年第一季度销售报告的关键数字",
        complexity="simple",
        category="single-doc",
        description="Single document summary with specific time scope.",
        expected_winner="tie",
    ),
    BenchmarkQuery(
        id="Q3",
        text="XX 客户过去 12 个月签订的所有合同列表",
        complexity="medium",
        category="cross-doc",
        description="List of contracts for one entity across many docs.",
        expected_winner="code-gen",
    ),
    BenchmarkQuery(
        id="Q4",
        text="2023-2024 年期间所有涉及金额超过 100 万的合同纠纷",
        complexity="complex",
        category="time-aggregated",
        description=(
            "Time-windowed + value-filtered aggregation. Requires "
            "metadata filtering on date and amount, plus cross-doc merge."
        ),
        expected_winner="code-gen",
    ),
    BenchmarkQuery(
        id="Q5",
        text="A 产品和 B 产品在客户使用场景上的差异",
        complexity="complex",
        category="comparison",
        description=(
            "Multi-entity comparison. Requires retrieving docs about "
            "two products and synthesizing a comparison. Cross-source "
            "correlation is helpful."
        ),
        expected_winner="code-gen",
    ),
]


def all_queries() -> list[BenchmarkQuery]:
    """Return all benchmark queries."""
    return list(QUERIES)


def get_query(query_id: str) -> BenchmarkQuery:
    """Return a specific query by ID."""
    for q in QUERIES:
        if q.id == query_id:
            return q
    raise KeyError(f"unknown query id: {query_id}")
