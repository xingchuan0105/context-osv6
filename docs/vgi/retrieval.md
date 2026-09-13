---
module: retrieval
provides: rrf_merge, read, rg
depends_on: [corpus, token-batch]
---

# Retrieval identity

M0 不实现本模块。接口现在锁死，避免 M1 再引入 18M `chunk_id` 或目录树 rg。

## Interface

    RankedList = [(doc_id, rank)]   # rank 从 1
    rrf_merge(lists: [RankedList], k=60) -> [doc_id]   # 好到差

    read(doc_id, char_offset, char_limit) -> {doc_id, offset, text, heading, url}

    rg(pattern, doc_ids: [doc_id], byte_budget) -> [Hit]
    Hit = {doc_id, char_offset, line, text}

评测与融合的身份永远是 `doc_id`。向量窗 `doc_id#batch` 与将来的 passage 先按文档聚合（默认 **max**），再进入 RRF。

## Semantics

**RRF.** 对每个出现过的 `doc_id`：`score = Σ 1/(k + rank_L)`，只加它出现过的列表。排序：`score` 降序；同分则列表命中数降序；再则向量列表中的 rank 升序（不在向量列表则视为 +∞）；再则 `doc_id` UTF-8。

**read.** 字节/字符窗切在 `documents.body` 的 Unicode 标量偏移上。没有 `chunk_id`。原型 18M 小块不进入 vgi-rs。

**rg.** 语料不是文件树。`doc_ids` 必填、非空。实现把这些行的 `body` 写成临时 UTF-8 文件（或等价内存扫描），再跑 ripgrep；全库扫没有入口。没有「目录」scope。

**紧凑命中.** 给 agent 的是 `doc_id` + 短片段 + `char_offset` + `corpus_fp`，不是整篇 body。

## Worked example — four documents

    vector ranks: A=1, B=2, C=3
    bm25   ranks: B=1, A=2, D=3
    k = 60

    score(A) = 1/61 + 1/62 = 0.032522474881015
    score(B) = 1/62 + 1/61 = 0.032522474881015
    score(C) = 1/63         = 0.015873015873016
    score(D) = 1/63         = 0.015873015873016

A 与 B 同分、都命中 2 列表；A 的向量 rank 更小 → A 在 B 前。C 在向量列表、D 不在 → C 在 D 前。

**结果顺序：A, B, C, D。**

read(`A`, offset=0, limit=5) 在 body=`hello` 上返回 `hello`。rg 不带 `doc_ids` 被拒绝。

## Anchor

**Input:** 上例两路排名。  
**Expected:** 顺序 `["A","B","C","D"]`；`score(A)==score(B)` 且 `score(A) > score(C)==score(D)`。  
**Enforced by:** `docs/vgi/anchors/test_anchors.py::test_rrf_four_docs`
