---
module: eval
provides: recall_at_k, protocol labels
depends_on: [retrieval]
---

# Eval protocol

## Interface

    Gold = {qid: str, evidence_doc_ids: set[str]}
    Hits = [doc_id]                 # 已按 retrieval 排好，文档级
    recall_at_k(gold, hits, k) -> float    # |hits[:k] ∩ evidence| / |evidence|

    Protocol = {
      questions: "challenge20" | "official_full",
      encoder: str,                 # e.g. "bge-m3-7372-overlap256-max"
      k_values: [5, 100, 1000]
    }

## Semantics

官方 BrowseComp-Plus 稠密基线（Qwen3-Embedding-0.6B、`--passage_max_len 4096` **整篇截断**、全库 qrels）是 **26.4% @100 / 59.7% @1000**。那是另一套 encoder × 另一套题。

VGI 默认报告 **Challenge-20、20 题、本稿切批编码器** 的绝对 recall。可以并排印官方数字，但它们不是 M1 及格线。

金标是 `evidence_ids`（约 207 个文档，已全部在 M0 sqlite），**不是** 更窄的 `gold_doc_ids`。题集副本：`docs/vgi/fixtures/challenge20-questions.json`（`id` / `question` / `evidence_ids`）。编码器标签：`bge-m3-7372-overlap256-max`。

M1 命令：`vgi eval --questions <json>` 对每题 `search_vector`，写 `.vgi/jobs/eval-m1.json`：`recall` 在 k∈{5,100,1000} 的均值，加 `per_question`。均值 = `(1/n) Σ recall_at_k`。

要对齐官方数字：先下载官方预构建 embedding/BM25 索引当 oracle（同一题集），再比 VGI。oracle 步骤在 M2 词法之前、M1 向量之后均可，不挡 M0。

Agent 级 A/B 仍用原生 Eval v2；同族裁判的局限照记。对照臂需要排序检索面；run-9 的 5 轮 grep 不是同预算对照。

## Worked example — one question

    evidence = {D1, D2, D3}     # |E|=3
    hits     = [D9, D1, D4, D2, …]
    k=100, 其中 D1、D2 在前 100，D3 不在

    recall@100 = 2/3

不是 1.0（漏了 D3），也不是 2/100。分母是金标文档数。

## Anchor

**Input:** 上例。  
**Expected:** `2/3`。  
**Enforced by:** `docs/vgi/anchors/test_anchors.py::test_recall_at_k`
