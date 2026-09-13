# Challenge-20 文档级 BM25 评审

日期：2026-09-13。对象：独立工程 `repository-tree` 的文档级 BM25（`src/repository_tree/evaluation/hard/lexical.py`、`retrieval/index.py` 的分词器、`.eval/hard/challenge20-index-v1/bm25-documents` 产物），承接 [run-9 正式分析](2026-09-13-vgi-rag-challenge20-tree-run9-analysis.md) 与 [检索最佳实践调研](../research/2026-09-13-semantic-tree-building-best-practices.md)。

**结论：实现本身是对的（bm25s 0.3.11 Lucene 公式、k1=1.2/b=0.75、Snowball 词干、CJK 一元/二元），与最佳实践的差距不在"算错公式"，而在五个结构性选择：单次检索范式、文档级粒度、无字段加权、参数/变体未调、评估口径不全。**

## 1. 实现事实

| 环节 | 现状 | 位置 |
|---|---|---|
| 分词 | bm25s.tokenize，stopwords="english"，Snowball 词干，`\b\w+\b`，CJK 一元+二元 | `retrieval/index.py:14-23` |
| 建索引 | 每篇**全文**分词（无截断、无字段），vocab→ids，`BM25(k1=1.2, b=0.75, method="lucene", backend="numpy")` | `lexical.py:72-91` |
| 查询 | **整段问题**当词袋；`get_scores(tokens)` 全量打分后排序 | `lexical.py:112-121` |
| 评估 | nDCG@10、recall@10/@50、gold hit@10/@50（ranks 记录完整，可回算 @100/@1000） | `lexical.py:14-34,124-149` |
| 产物 | 758 MB（CSC 矩阵 343MB×2 + vocab 78MB + indptr 28MB），全内存 numpy | `.eval/hard/challenge20-index-v1/bm25-documents/` |

已有口径（重算）：evidence recall@10 = 2.4%（逐题均分 2.7%）、@50 = 6.3%、**@100 = 11.1%（9/20 题）**、**@1000 = 27.5%（14/20 题）**；官方论文 BM25 为 recall@5 = 1.2%，官方稠密 Qwen3-0.6B 为 @100 26.4%、@1000 59.7%。索引构建无超时（此前"超时"来自 grep 工具与旧 runner）。

## 2. 与最佳实践的差距与改进（按优先级）

### P1 结构性

1. **单次检索范式 ≠ agent 检索**。官方 agent 的 `search` 工具是模型反复改写短查询、看结果再收敛；单次原始问题或单次关键词查询都到不了。附实验（3 题）：手工关键词改写与原始问题打平甚至更差——说明瓶颈是"单次"，不是"查询太长"。改进：用 agent 循环测（多轮查询 + 结果反馈），或至少做多查询 + RRF（代码里已有 `rrf_merge`，未用于标定）。
2. **文档级粒度**。语料中位 10k 字符、最长 996 万字符；整篇全文建索引使超长文档的 tf 膨胀、打分被长度归一化稀释，且索引 758MB。最佳实践是**段落级索引 + 文档聚合**（max 或 exp-sum），或 doc2query 文档扩展；对超大文档加长度上限。段落级倒排索引存储远低于向量（无向量、无 70GB 问题）。
3. **无字段加权**。语料自带 `heading`（标题/日期）与 `locators`（URL）元数据，但 BM25 只用正文。BM25F/字段 boost（title、url 加权）对这类"答案在标题/URL"的题通常有效。

### P2 打分与参数

4. **只用了 `method="lucene"`**。bm25s 支持 `robertson` / `atire` / `bm25l` / `bm25+` 五种；长文档场景 BM25L/BM25+ 有明确优势，应在 dev 上 A/B。
5. **k1/b 取的是 Lucene 默认值，未调参**。标准做法是在开发集上小网格搜索（b∈{0.3…0.9}）。
6. **`get_scores(tokens, weight_mask=...)` 支持查询词权重，未使用**；对多约束问题可按判别性给日期/数字/稀有词加权。
7. **无 PRF/RM3 查询扩展**（自动反馈，不需要外部知识，长问题集合上通常有稳定增益）。

### P3 分析与工程

8. 停用词表不完整（实测 `his`/`your` 通过）；`\b\w+\b` 会把 `Ti-6Al-4V` 拆成 `ti/6al/4v`（可接受但要知晓）；未抽取标题/URL 字段文本。
9. 索引全内存 numpy；离线对照可换官方**预构建 BM25 索引**（Pyserini/Anserini）以消歧分词与打分差异，并与榜单数字对齐。
10. 评估口径：补 @100/@1000（已回算），并逐题记录 top-k 变化，避免只用 @10/@50 下结论。

### P4 系统

11. 与稠密两路融合：段落级 BM25（召回长尾）+ 文档级向量（0.7GB）+ RRF + 重排；`rrf_merge` 已有现成实现。

## 3. 附：查询改写小实验（2026-09-13）

同一索引、同一批题（bcp:180/20/22），原始问题 vs 仅据题面手写的关键词查询：

| 题目 | 原始 recall@100/@1000 | 短查询 recall@100/@1000 |
|---|---|---|
| bcp:180 | 0.10 / 0.20 | 0.10 / 0.20 |
| bcp:20 | 0.67 / 0.78 | 0.56 / 0.56 |
| bcp:22 | 0.00 / 0.17 | 0.08 / 0.17 |

读法：**naive 关键词提取不是银弹**；这些题需要的是"探索—发现实体—再检索"的迭代（例如先找到冷却液名称，再搜论文），所以评估必须放进 agent 循环，而不是继续在单次检索上调参。脚本：`.eval/hard/bm25_query_rewrite_demo.py`（一次性演示，未版本化）。

## 4. 建议的执行顺序

1. 段落级 BM25（chunk 索引 + 文档聚合）+ 字段 boost → 重算 @100/@1000；
2. agent 化查询（多轮改写 + 多查询 RRF）在 dev 集上对照单次；
3. 打分发/参数 A/B（lucene vs bm25l/bm25+，b/k1 网格，weight_mask）；
4. 与官方预构建 BM25 对齐口径；
5. 最后才是与稠密融合与重排。
