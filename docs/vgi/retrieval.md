---
module: retrieval
provides: rrf_merge, max_pool, search_vector, read, rg
depends_on: [corpus, token-batch, embed]
---

# Retrieval identity

M1 实现向量路 + read + rg。RRF 函数锁死，M2 才接词法路。禁止 18M `chunk_id` 和全库目录树 rg。

## Interface

    RankedList = [(doc_id, rank)]   # rank 从 1
    WindowHit = {uid, score}
    max_pool(hits: [WindowHit]) -> [(doc_id, score)]
    # 每文档取 max(score)；排序：score 降序，同分则 doc_id UTF-8 升序

    search_vector(query, window_k=10000) -> RankedList
    # 查询一条向量；ANN 取 window_k 个窗；max_pool；身份是 doc_id

    rrf_merge(lists: [RankedList], k=60) -> [doc_id]   # 好到差；M1 不调用

    read(doc_id, char_offset, char_limit) -> {doc_id, offset, text, heading, url}

    rg(pattern, doc_ids: [doc_id], byte_budget) -> [Hit]
    Hit = {doc_id, char_offset, line, text}

评测与融合的身份永远是 `doc_id`。向量窗 `doc_id#batch` 先按文档聚合（默认 **max**），再进入排序或 RRF。

## Semantics

**max_pool.** 同一 `doc_id` 多窗只留最高分。排序：score 降序，同分 `doc_id` UTF-8。

**search_vector (M1).** 只走向量。`window_k` 默认 10000，保证 max_pool 后仍能填满 recall@1000。zvec cosine 的 `get_score` 是距离（越小越近）；入库 `max_pool` 前取负，当作相似度。

**RRF.** 对每个出现过的 `doc_id`：`score = Σ 1/(k + rank_L)`，只加它出现过的列表。排序：`score` 降序；同分则列表命中数降序；再则向量列表中的 rank 升序（不在向量列表则视为 +∞）；再则 `doc_id` UTF-8。

**read.** 切在 `documents.body` 的 Unicode 标量偏移上：`chars().skip(offset).take(limit)`。越界得到前缀或空串，不是错误。未知 `doc_id` 失败。没有 `chunk_id`。

**rg.** `doc_ids` 必填、非空。对列出的 `body` 做内存正则（等价于 scoped ripgrep）。`char_offset` 是匹配起点的 Unicode 标量下标；`line` 是该行（不含 `\n`）。累计 `line` 字节达到 `byte_budget` 则停。全库扫没有入口。

**紧凑命中.** `doc_id` + 短片段 + `char_offset` + `corpus_fp`，不是整篇 body。

## Worked example — max_pool

    A#0 0.90, A#1 0.40, B#0 0.80, C#0 0.80

    A=0.90, B=0.80, C=0.80 → 顺序 A, B, C（B < C 于 UTF-8）

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

read(`A`, offset=0, limit=5) 在 body=`hello` 上返回 `hello`；offset=1, limit=2 返回 `el`。rg 不带 `doc_ids` 被拒绝。pattern `ell` 在 `hello` 上：`char_offset=1`，`line=hello`。

## Anchor

**Input:** max_pool 上例。 **Expected:** `A,B,C`。  
**Input:** RRF 两路排名。 **Expected:** `["A","B","C","D"]`。  
**Input:** read hello 1,2。 **Expected:** `el`。  
**Input:** rg 空 `doc_ids`。 **Expected:** error。  
**Enforced by:** `test_anchors.py::test_max_pool_three_docs`, `test_rrf_four_docs`, `test_read_char_window`, `test_rg_requires_doc_ids`
