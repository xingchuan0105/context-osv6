# Code-Gen E2E Benchmark Report

- Backend: `http://127.0.0.1:19523`
- Queries: 5
- Date: 2026-06-03 12:13:49

## Summary

| ID | Complexity | Expected Winner | Actual Winner | Δ Top Score | Δ Latency |
|----|------------|-----------------|---------------|-------------|-----------|
| Q1 | simple | tie | code-gen | +0.05 | -80ms |
| Q2 | simple | tie | code-gen | +0.05 | +6ms |
| Q3 | medium | code-gen | code-gen | +0.05 | +7ms |
| Q4 | complex | code-gen | code-gen | +0.05 | +6ms |
| Q5 | complex | code-gen | code-gen | +0.05 | +6ms |

## Per-query details

### Q1: XX 客户的注册地址是什么？

- Complexity: simple
- Category: factual
- Expected winner: **tie**

| Metric | Baseline | Code-Gen | Δ |
|--------|----------|----------|---|
| Wall clock (ms) | 88 | 8 | -80 |
| Chunk count | 10 | 10 | +0 |
| Top score | 0.900 | 0.950 | +0.050 |
| Mean score | 0.675 | 0.725 | +0.050 |
| Source diversity | 1 | 1 | +0 |
| SDK calls | 1 | 3 | +2 |
| Estimated cost (USD) | $0.0001 | $0.0011 | $+0.0010 |

### Q2: 2024 年第一季度销售报告的关键数字

- Complexity: simple
- Category: single-doc
- Expected winner: **tie**

| Metric | Baseline | Code-Gen | Δ |
|--------|----------|----------|---|
| Wall clock (ms) | 2 | 8 | +6 |
| Chunk count | 10 | 10 | +0 |
| Top score | 0.900 | 0.950 | +0.050 |
| Mean score | 0.675 | 0.725 | +0.050 |
| Source diversity | 1 | 1 | +0 |
| SDK calls | 1 | 3 | +2 |
| Estimated cost (USD) | $0.0001 | $0.0011 | $+0.0010 |

### Q3: XX 客户过去 12 个月签订的所有合同列表

- Complexity: medium
- Category: cross-doc
- Expected winner: **code-gen**

| Metric | Baseline | Code-Gen | Δ |
|--------|----------|----------|---|
| Wall clock (ms) | 2 | 9 | +7 |
| Chunk count | 10 | 10 | +0 |
| Top score | 0.900 | 0.950 | +0.050 |
| Mean score | 0.675 | 0.725 | +0.050 |
| Source diversity | 1 | 1 | +0 |
| SDK calls | 1 | 3 | +2 |
| Estimated cost (USD) | $0.0001 | $0.0011 | $+0.0010 |

### Q4: 2023-2024 年期间所有涉及金额超过 100 万的合同纠纷

- Complexity: complex
- Category: time-aggregated
- Expected winner: **code-gen**

| Metric | Baseline | Code-Gen | Δ |
|--------|----------|----------|---|
| Wall clock (ms) | 2 | 8 | +6 |
| Chunk count | 10 | 10 | +0 |
| Top score | 0.900 | 0.950 | +0.050 |
| Mean score | 0.675 | 0.725 | +0.050 |
| Source diversity | 1 | 1 | +0 |
| SDK calls | 1 | 3 | +2 |
| Estimated cost (USD) | $0.0001 | $0.0011 | $+0.0010 |

### Q5: A 产品和 B 产品在客户使用场景上的差异

- Complexity: complex
- Category: comparison
- Expected winner: **code-gen**

| Metric | Baseline | Code-Gen | Δ |
|--------|----------|----------|---|
| Wall clock (ms) | 2 | 8 | +6 |
| Chunk count | 10 | 10 | +0 |
| Top score | 0.900 | 0.950 | +0.050 |
| Mean score | 0.675 | 0.725 | +0.050 |
| Source diversity | 1 | 1 | +0 |
| SDK calls | 1 | 3 | +2 |
| Estimated cost (USD) | $0.0001 | $0.0011 | $+0.0010 |

## Interpretation

This benchmark compares the **existing dense-only baseline** (a single
dense retrieval call) against the **code-gen path** (multi-strategy with
fan-out, dedup, and rerank).

**Top score higher is better** — code-gen's rerank is expected to improve
top result quality. **Latency higher is worse** — code-gen does more work.
**SDK calls higher is more expensive** — code-gen trades external API
calls for better results.

**Expected outcomes**:
- Simple queries (Q1, Q2): both should perform similarly; code-gen adds cost without much benefit
- Medium queries (Q3): code-gen should start to show quality wins
- Complex queries (Q4, Q5): code-gen should win on quality but lose on latency

If the results contradict the hypotheses, the next step is to investigate:
- Did the LLM write a sub-optimal Python program? (check `code-gen` path's chunks)
- Did the baseline's BM25 path miss important matches? (check Q3)
- Is the mock corpus too uniform? (run against real data for better signal)