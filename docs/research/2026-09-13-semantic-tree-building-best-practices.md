# 调研：文档语义树的检索最佳实践与现成方案

**日期：** 2026-09-13 · **性质：** 时间点调研快照 · **服务对象：** [Challenge-20 语义树构建评审](../reviews/2026-09-13-challenge20-semantic-tree-build-review.md) 的整改方向

**背景：** Challenge-20 的 `semantic-tree`（hashed TF、256 维、2-means 二分、8 路、词标签）实测在形状、标签、区分度、对照一致性四方面失效。本调研回答两个问题：这种建树方式**有没有文献依据**；**有没有更好的现成方案**。

## 1. 有依据的部分（骨架）

- **二分聚类是成熟技术**。Steinbach 等《A Comparison of Document Clustering Techniques》比较了层次聚类与 K-means，结论是 **bisecting k-means 优于标准 K-means、与层次聚类相当或更好**——用"反复对最大簇做 2-means"建树有直接文献支持。
- **哈希特征也是成熟技术**（feature hashing / hashing trick，Weinberger 2009），但标准做法是 **2^18–2^24 维 + IDF 权重**；256 维、无 IDF 偏离常规（词表上万、每篇数百唯一词，256 桶必然饱和碰撞）。
- **聚类浏览有历史先例**：Hearst & Pedersen 的 Scatter/Gather（SIGIR'96）验证了"按簇浏览 + 迭代收敛"优于纯排序列表；其设计要点是**簇摘要（topical term digest）做标签 + 查询时反复 scatter/gather**，而不是一次性固定树 + 盲浏览。

## 2. 没有依据的部分（与最佳实践的偏离）

- **标签**：原始词频众数（结果 `['s','i','his','he','you']`）——没有任何主流方案这样做。通行做法是 **c-TF-IDF**（class-based TF-IDF，BERTopic 用它生成主题词）或 **LLM 摘要**（RAPTOR/GraphRAG）。
- **代表文档**：`min(members)` 无语义；通行做法是 **medoid/最近质心**。
- **静态导航**：生产级树/分区结构全部是**查询驱动下行**——FAISS IVF 先粗量化再扫倒排表、Annoy 随机投影森林、ScaNN 分区、RAPTOR 的 collapsed tree、LlamaIndex 的 tree-select（让 LLM 按查询逐层选子节点）。固定卡片 + 盲浏览在 5 轮预算下不可行。
- **硬划分**：跨主题文档天然属于多个簇；RAPTOR 用 **GMM 软聚类**正是为此。硬划分丢掉了"一篇文档可入多枝"的能力。
- **规模化管理**：树用于检索时的常见做法是**控制叶容量/深度 + 平衡切分**；本项目实测叶中位 10 篇、深度 2–8、87% 叶在深度 ≥5，没有任何可用于导航的容量承诺。
- **簇质量评测**：经典研究对"聚类能否提升检索"结论是**混合的**（Liu & Croft 2004 用语言模型重审后认为可有稳定提升，但早期研究不确定）。因此建树后应先做**索引级评测**（节点级 recall、purity/NMI、标签可用性），再谈端到端问答——直接开 agent 对照是本末倒置。

## 3. 现成方案（按"改造量从低到高"排序）

| 方案 | 定位 | 要点 | 成本/依赖 |
|---|---|---|---|
| **scikit-learn `BisectingKMeans`** | 直接替换手写二分 | 官方实现、支持 `bisecting_strategy`；配 `TfidfVectorizer`（真 IDF）替代 hashed TF | 无新依赖 |
| **BERTopic** | 主题树 + 可读标签 | transformer embedding + HDBSCAN + **c-TF-IDF 标签**；`hierarchical_topics` 用 scipy Ward 在 c-TF-IDF 上合并出层级 | 成熟库；无需 LLM |
| **scipy/fastcluster linkage + UMAP** | 通用层次聚类 | ward/average linkage；UMAP 降维是主题聚类常用前置 | 无新服务 |
| **RAPTOR**（Sarthi et al., ICLR 2024） | RAG 语义树事实标准 | 递归 UMAP+GMM 软聚类 → LLM 摘要做节点文本 → **collapsed tree** 检索（所有层节点统一向量比较） | 每层摘要需 LLM；有参考实现与 LlamaIndex/LangChain 衍生实现 |
| **LlamaIndex TreeIndex / tree-select** | 查询驱动导航 | 节点摘要 + 逐层让 LLM 选子节点，天然把"导航"变成每层一次比较 | 框架依赖 |
| **FAISS IVF / ScaNN / Annoy / HNSW** | 生产检索结构 | 粗量化/分区 + 查询下行，面向 10^5–10^9 规模；是"树用于搜索"的正统形态 | 需要向量 |
| **GraphRAG（Microsoft）** | 全局/多跳问答 | 实体-社区层级 + LLM 社区报告 + map-reduce 全局搜索 | 索引 LLM 成本高；适合全局性问题而非单点事实 |
| **Ψ-RAG（ICML 2026）/ T-Retriever（2026）/ TreeRAG（ACL Findings 2025）** | 最新研究 | 跨文档多跳的层级摘要树 + 多粒度 agentic retriever；强调抽象策略与分布自适应 | 研究代码；与 VGI 目标最接近的近期工作 |

## 4. 结论（对本项目的映射）

1. **骨架有依据、实现无依据**：二分建树可以保留；需要换掉的是特征（真 IDF 或 embedding）、标签（c-TF-IDF/LLM 摘要）、代表（medoid/摘要）、导航方式（查询路由）与形状约束（叶容量/深度）。
2. **两条可行路线**：
   - **最小整改**（保留现有架构）：TF-IDF/SVD 特征 + `BisectingKMeans` + c-TF-IDF 标签 + 质心最近代表 + 叶容量/深度验收 + 暴露"查询→节点"的路由工具（导航从盲浏览改为按查询进入）。
   - **标准方案**（推荐）：沿用已有语料 embedding，按 RAPTOR 路线做 UMAP+GMM 软聚类 + LLM 摘要节点卡，检索用 collapsed tree；标签与导航问题一次性解决。
3. **若目标仍是 BrowseComp-Plus 式问答**：最佳实践是两段式检索（BM25+dense 混合 + rerank）+ agent 查询迭代；树的价值在**多粒度/全局问题**（RAPTOR/GraphRAG）或**成本控制**（粗到细），不在"预算内盲浏览"。
4. **先索引评测、后端到端**：建好树先用节点级 recall / 簇纯度 / 标签可用性验证结构本身，再决定是否值得跑 agent 对照。

## 5. 参考

- [Steinbach et al., A Comparison of Document Clustering Techniques (2000)](https://cs.fit.edu/~pkc/classes/ml-internet/papers/steinbach00tr.pdf)
- [Weinberger et al., Feature Hashing for Large Scale Multitask Learning (2009)](https://arxiv.org/abs/0902.2206)
- [Hearst & Pedersen, Scatter/Gather (SIGIR 1996)](https://people.ischool.berkeley.edu/~hearst/papers/sg-sigir96/sigir96.html)
- [Liu & Croft, Cluster-based retrieval using language models (SIGIR 2004)](https://dl.acm.org/doi/abs/10.1145/1008992.1009026)
- [Sarthi et al., RAPTOR: Recursive Abstractive Processing for Tree-Organized Retrieval (ICLR 2024)](https://arxiv.org/abs/2401.18059)；[参考实现](https://github.com/parthsarthi03/raptor)
- [BERTopic — Hierarchical Topic Modeling（c-TF-IDF + scipy Ward）](https://maartengr.github.io/BERTopic/getting_started/hierarchicaltopics/hierarchicaltopics.html)
- [LlamaIndex — Tree Retrievers / tree-select leaf retriever](https://llamaindexxx.readthedocs.io/en/latest/api_reference/query/retrievers/tree.html)
- [FAISS — Inverted File (IVF) indexes: coarse quantization then fine scan](https://deepwiki.com/facebookresearch/faiss/5.2-inverted-file-(ivf)-indexes)
- [Microsoft GraphRAG — community summaries & global search](https://microsoft.github.io/graphrag/query/global_search/)
- [Ψ-RAG: Hierarchical Abstract Tree for Cross-Document RAG (ICML 2026)](https://arxiv.org/abs/2605.00529)；[T-Retriever (2026)](https://arxiv.org/abs/2601.04945)；[TreeRAG (ACL Findings 2025)](https://aclanthology.org/2025.findings-acl.20/)
