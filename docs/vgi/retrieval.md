---
module: retrieval
provides: rrf_merge, max_pool, search_vector, search_lexical, read, rg
depends_on: [corpus, token-batch, embed]
---

# Retrieval identity

M1 实现向量路 + read + rg；M2 接词法路（段落级 BM25）+ hybrid。禁止 18M `chunk_id` 和全库目录树 rg。

## Interface

    RankedList = [(doc_id, rank)]   # rank 从 1
    WindowHit = {uid, score}
    max_pool(hits: [WindowHit]) -> [(doc_id, score)]
    # 每文档取 max(score)；排序：score 降序，同分则 doc_id UTF-8 升序

    search_vector(query, window_k=10000) -> RankedList
    # 查询一条向量；ANN 取 window_k 个窗；max_pool；身份是 doc_id

    search_lexical(query, passage_k=20000) -> RankedList
    # 段落级 BM25；passage 分数 max_pool 到 doc_id（M2）

    rrf_merge(lists: [RankedList], k=60) -> [doc_id]   # 好到差；M1 不调用

    search(query, mode="vector"|"lexical"|"hybrid", rerank=0) -> RankedList
    # hybrid = rrf_merge([search_vector(q), search_lexical(q)], k=60)（M2）
    # rerank=N：对 RankedList 前 N 个 doc 做外部 rerank 重排（M2 后追加）

    read(doc_id, char_offset, char_limit) -> {doc_id, offset, text, heading, url}

    rg(pattern, doc_ids: [doc_id], byte_budget) -> [Hit]
    Hit = {doc_id, char_offset, line, text}

评测与融合的身份永远是 `doc_id`。向量窗 `doc_id#batch` 先按文档聚合（默认 **max**），再进入排序或 RRF。

## Semantics

**max_pool.** 同一 `doc_id` 多窗只留最高分。排序：score 降序，同分 `doc_id` UTF-8。

**search_vector (M1).** 只走向量。`window_k` 默认 10000，保证 max_pool 后仍能填满 recall@1000。zvec cosine 的 `get_score` 是距离（越小越近）；入库 `max_pool` 前取负，当作相似度。

**RRF.** 对每个出现过的 `doc_id`：`score = Σ 1/(k + rank_L)`，只加它出现过的列表。排序：`score` 降序；同分则列表命中数降序；再则向量列表中的 rank 升序（不在向量列表则视为 +∞）；再则 `doc_id` UTF-8。

**search_lexical (M2).** 检索单元是 passage，不是整篇：对 `body` 的 raw token 流（`(?u)\b\w\w+\b`，含停用词）做滑窗，W=512、stride=448（重叠 64）；起点为 0, 448, 896, …，持续至 `start < max(n_tokens − 64, 1)`；passage 覆盖 raw token `[start, start+512)` 截到 n，落回原文 char 区间（首 token 起点到末 token 终点）。引擎 **tantivy**（进程内）：`doc_id` STRING stored，passage 文本 TEXT indexed-not-stored（默认分词，lowercase）；BM25 打分 k1=1.2、b=0.75（Lucene/tantivy 默认）。取 BM25 前 `passage_k`（默认 20000）个 passage，按 doc 取 max(score) 后排序：score 降序，同分 `doc_id` UTF-8——与 `max_pool` 同规则。索引目录 `.vgi/tantivy-passage/`。官方预构建 BM25 oracle 以 bm25s lucene 同参数在 Python 层标定（`scripts/bm25s_passage_oracle.py`），冻结对照见 `fixtures/challenge20-prior-frozen.json`。

**search hybrid (M2).** `rrf_merge([search_vector(q), search_lexical(q)], k=60)`。M2 门：Challenge-20 上 hybrid ≥ 本系统单路最优（bm25s 层预测：@100 0.237 > vector 0.211）。

**rerank（追加阶段）.** `rerank=N>0` 时：取 RankedList 前 N 个 `doc_id`，每 doc 送 `body` 前 1500 字符的摘要到 Bailian `gte-rerank-v2`（`POST …/services/rerank/text-rerank/text-rerank`，key 读 `DASHSCOPE_API_KEY`；按 ~90k 字符/请求分块），按 `relevance_score` 降序重排（同分保原序），N 之后的文档原序拼接。语义：只在 top-N 内重排——recall@k 仅当 k≤N 或 N>k 时可变（N=200 时 @100 可变、@1000 不变）。oracle 实测（`rerank-oracle-search-lists-challenge20.json`，N=200）：hybrid @100 0.236→0.268、vector @100 0.211→0.247；@5 基本持平——它修的是 100–200 区间的证据上浮，不救已错的头部。

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

## Worked example — 段落窗与 lexical pool

body 的 raw token 数 n=600：起点 0、448（448 < 600−64=536；896 ≥ 536 停）→ passage `[0,512)`、`[448,600)`，共 2 个。n=512 只有 `[0,512)`；n=513 为 `[0,512)`、`[448,513)`。

BM25 打分锚（Lucene 公式，k1=1.2、b=0.75，avgdl=13/3≈4.3333）：三篇文档 A=`machine learning is fun`（4 词）、B=`deep learning uses neural networks`（5 词）、C=`vector databases store embeddings`（4 词），查询 `learning`（df=2，N=3 → idf=ln(1+1.5/2.5)=0.4700036）：

    A: tf·(k1+1)/(tf + k1·(1−b+b·dl/avgdl)) = 2.2/2.13077 = 1.03249 → score 0.48527451
    B: 2.2/2.33846 = 0.94079                                    → score 0.44217447

**顺序 A, B**（同 tf 下短文档赢）。passage→doc 聚合复用 max_pool 规则：A#p0 12.0、A#p1 9.5、B#p0 11.0 → A=12.0、B=11.0 → `A,B`。

## Anchor

**Input:** max_pool 上例。 **Expected:** `A,B,C`。  
**Input:** RRF 两路排名。 **Expected:** `["A","B","C","D"]`。  
**Input:** read hello 1,2。 **Expected:** `el`。  
**Input:** rg 空 `doc_ids`。 **Expected:** error。  
**Input:** passage 窗 n=600 / n=513。 **Expected:** `[(0,512),(448,600)]` / `[(0,512),(448,513)]`。  
**Input:** BM25 三文档查询 `learning`。 **Expected:** 顺序 `A,B`，score(A)≈0.48527451、score(B)≈0.44217447。  
**Enforced by:** `test_anchors.py::test_max_pool_three_docs`, `test_rrf_four_docs`, `test_read_char_window`, `test_rg_requires_doc_ids`, `test_passage_windows`, `test_bm25_lucene_three_docs`
