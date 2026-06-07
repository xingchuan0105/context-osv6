# Code-Gen E2E Benchmark

This benchmark compares the **code-gen query path** (multi-strategy
fan-out, dedup, rerank) against the **baseline path** (single dense
retrieval) on 5 representative queries.

## What it measures

For each (query, path) combination:

| Metric | Description | Better when... |
|--------|-------------|-----------------|
| `wall_clock_ms` | Total wall clock for the path | Lower is better |
| `chunk_count` | Number of chunks returned | Higher often better (more candidates) |
| `top_score` | Highest relevance score in result set | Higher is better |
| `mean_score` | Mean relevance score | Higher is better |
| `source_diversity` | Number of unique retrieval sources | Higher = more diverse |
| `sdk_calls` | Total SDK HTTP calls | Lower is better (cost) |
| `estimated_cost_usd` | Rough cost estimate from call counts | Lower is better |

## Queries

5 queries are defined in `queries.py`:

| ID | Type | What it tests |
|----|------|---------------|
| Q1 | Simple factual | Both paths should tie |
| Q2 | Single doc | Both paths should tie |
| Q3 | Cross-doc list | Code-gen may help |
| Q4 | Time-aggregated complex | Code-gen should win |
| Q5 | Multi-entity comparison | Code-gen should win |

## Running

### Mock mode (self-test, no setup)

```bash
cd /home/chuan/context-osv6/avrag-rs/python/avrag_sdk
python -m benchmark.run_benchmark
```

This starts an in-process mock backend and runs all 10
(5 queries × 2 paths) executions. Useful for verifying the framework.

### Live mode (against real backend)

Prerequisites:
- A running `bins/api` (Rust backend) with documents ingested
- An LLM provider configured (the live path doesn't actually need an LLM
  directly — code-gen path uses the SDK which calls the Rust tool, but
  the LLM is the LLM that writes the Python; for this benchmark we
  hardcode the Python programs, so we don't need LLM access)
- `AVRAG_API_URL` set to the backend
- `AVRAG_AUTH_TOKEN` if the backend requires auth

```bash
cd /home/chuan/context-osv6/avrag-rs/python/avrag_sdk
export AVRAG_API_URL="http://localhost:8080"
export AVRAG_AUTH_TOKEN="your-token"  # if required
python -m benchmark.run_benchmark --live
```

## Output

Two files in `./benchmark_output/`:

1. **`benchmark_results.json`** — machine-readable metrics per (query, path)
2. **`benchmark_report.md`** — human-readable summary with comparison table

## Limitations

This is a **simplified benchmark**:
- The "baseline" is a single dense call, not the full RAG pipeline with
  RRF + rerank. A more thorough benchmark would compare against the
  actual existing pipeline output.
- The "code-gen" path uses **hardcoded Python programs** in the SDK,
  not actual LLM-generated code. The LLM's code quality is the main
  unknown in production.
- Cost estimates are rough (based on call counts, not actual provider pricing).
- Mock corpus is uniform — real data has more diversity and will produce
  more interesting comparisons.

For a more accurate benchmark, the next step would be:
1. Wire the actual `code_gen_query` Rust tool to the LLM (not SDK-direct)
2. Compare against the full RAG pipeline output (with RRF + rerank)
3. Run with real ingested data and a representative LLM

## Why this is useful even in mock mode

The mock-mode run verifies:
- The framework is correctly instrumented (call counts, score tracking)
- The summary table and report generation work
- The code-gen orchestration pattern produces measurable improvements
  over the naive single-dense baseline (top score, source diversity)

This gives the team confidence that the **design** of code-gen vs baseline
is sound, before investing in the more expensive live benchmark.
