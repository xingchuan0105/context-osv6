# `quality_runner` Gotchas

This file documents non-obvious things that will bite you if you
read `quality_runner.rs` and start making production recommendations
based on the numbers. The runner is a **smoke test**, not a
production benchmark.

## 🚨 GOTCHA 1: The runner is NOT production RAG

The runner implements its own minimal retrieval:
- Single-pass cosine similarity over a flat in-memory chunk index
- Single query, no re-ranking, no query rewriting, no hybrid search
- A hardcoded `--top-k` CLI argument (default 4) that limits chunks
  sent to the LLM for synthesis

It does **NOT** use:
- `avrag_rag_core::RagRuntime` (production)
- The planner LLM (production) — which autonomously decides what
  tools to call, what queries to emit, and what `top_k` to use per
  call based on the user query
- Multi-channel retrieval (dense + BM25 + graph) with RRF merging
  (`crates/rag-core/src/runtime/retrieval.rs`)
- Cross-encoder re-ranking
- The full `RagPlan` budget allocation that splits a fixed
  `TOTAL_CANDIDATE_BUDGET` across plan items by priority
  (`crates/rag-core/src/runtime/planner.rs:242`)

**Implication**: Any recommendation that involves
"the production RAG pipeline" must NOT be derived from runner
numbers. Specifically:
- ❌ "Increase top_k from 4 to 6" — production already uses
  planner-driven top_k (default 10), RRF, and re-ranking. Tuning
  the runner's `--top-k` flag has zero effect on production.
- ❌ "Add re-ranking to fix retrieval miss" — production has
  re-ranking. The runner doesn't.
- ❌ "The retrieval has X% miss rate" — that's the runner's
  flat-cosine miss rate, not production's miss rate.

To get **real** production numbers, the harness needs a
`ProductionRagEvaluator` that calls `RagRuntime::execute()` and
parses the resulting `ChatResponse.citations`. This is a real
project, not a flag flip; expect 2-3 days of wiring.

## 🚨 GOTCHA 2: The hallucination heuristic is not NLI

`metrics::hallucination_check` uses a **word-overlap heuristic**:
split the answer into sentences, and for each sentence check
whether ≥50% of its "significant words" (length > 5, not in a
stopword list) appear in any retrieved chunk.

Known failure modes observed against the fixture corpus:
- **Paraphrase** flagged as hallucination: "iIatrogenics is
  harm caused by the healer" vs chunk "Iatrogenics: harm caused
  by the healer" — the lowercase 'i' makes the word-overlap
  check miss the token. The answer is correct; the metric is
  wrong.
- **Synonym substitution** flagged: "more" vs "greater",
  "removing obstacles" vs "subtraction". The LLM correctly
  paraphrased; the metric thinks it fabricated.
- **Formatting additions** flagged: adding `**bold**` or
  em-dashes around chunk-derived text. The LLM added style, the
  metric thinks it added content.

**Implication**: The runner's "hallucination rate" is a noise
floor, not a product quality signal. A 30% heuristic-flag rate
translates to ~0% true hallucinations when manually audited
(verified: in the latest 24-example run, all 4 RAG flags are
paraphrase false positives, and all 5 adversarial flags are
correct refusals that the heuristic incorrectly calls
hallucinations before the no-context fix).

To get a real hallucination rate, replace the heuristic with
a proper NLI model. See the design doc for options
(LlmNliJudge / OnnxNliJudge); both plug into the
`HallucinationJudge` trait.

## 🚨 GOTCHA 3: The `mode` field is NOT a runtime flag

The runner's `synthesize` looks up the example's `mode` field
from the golden set and picks one of three system prompts
(RAG / Chat / Search). This is correct — the golden set's
`mode` corresponds to `agent_type` in the production
`/api/v1/chat` request body.

**Implication**: When designing new golden examples, the `mode`
field is the test's only way to pick the right system prompt.
Setting it wrong (e.g. labelling a chat-style question as
`"rag"`) will silently run it through the RAG prompt and
produce meaningless numbers.

## 🚨 GOTCHA 4: The mock LLM in `product_e2e/` is a DIFFERENT beast

The 30 `product_e2e/` tests use a **separate** mock LLM defined
in `crates/app/tests/product_e2e/mod.rs::mock_llm_handler`.
That mock returns canned responses based on system-prompt
substring matching. It is **not** the same mock as
`quality_runner`'s retrieval. If you are trying to debug a
quality issue by running product_e2e tests, you are testing
pipeline plumbing, not answer quality.

## How to actually measure product quality

For a real production-quality number today, the path is:

1. **Write a `ProductionRagEvaluator`** that calls
   `RagRuntime::execute(query, doc_scope, mode)`, extracts
   `ChatResponse.citations` (for `retrieve`) and
   `ChatResponse.answer` (for `synthesize`).
   Requires real Postgres + Milvus + a pre-ingested corpus.
   Expect 2-3 days of wiring.

2. **Replace the heuristic with a real NLI judge** behind the
   `HallucinationJudge` trait. The LLM-based zero-shot NLI
   (one LLM call per sentence) is the lowest-effort option;
   the ONNX-based local NLI is the production option.
   Expect 1-3 days depending on option.

3. **Build a 100+ example golden set** with hand-labeled
   hallucination verdicts, so the NLI threshold is calibrated
   against human judgement rather than the heuristic's noise.

Until those three things exist, the numbers in the runner are
useful for **smoke testing the harness** (does the pipeline
crash? does the LLM refuse adversarial questions?) and nothing
else. They are NOT a product-quality signal.
