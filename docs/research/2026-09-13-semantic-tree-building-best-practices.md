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

## 6. 存储现实：向量的"70GB"来自切块粒度，不是基准规模

- 官方 BrowseComp-Plus 检索是**文档级**。Tevatron 官方示例对整篇文档编码（`--passage_max_len 4096`、`--fp16`），向量数 ≈ 文档数（10 万级）：Qwen3-Embedding-0.6B（1024 维）fp16 ≈ **205 MB**，8B（4096 维）fp16 ≈ **820 MB**，即使 4096 维 fp32 也仅 1.6 GB。官方还**直接托管 BM25 与 Qwen3-Embedding 的预构建索引**（`scripts_build_index/download_indexes.sh`），用户下载即用，无需自建。
- 本地 18,437,321 块（≈184 块/篇）来自我们自己的切块策略：1024 维 fp32 = **75.5 GB**——约 184 倍的放大，与基准无关。
- 若坚持块级向量，压缩手段成熟：fp16 37.8 GB → int8 18.9 GB → PQ（64B/块）1.2 GB → 二值（32B/块）0.6 GB；或用"压缩粗排 + 原文精排"两段式；纯 BM25 倒排索引则完全不需要向量。
- 对 VGI 的含义：**把检索粒度与阅读粒度解耦**——阅读继续用小切块（保留引用/续读能力），向量索引按文档（或章节）建，成本降到几百 MB；RAPTOR 式语义树也可以直接建在文档向量上。

## 7. 我们的语料做"文档级向量"的实测口径与模型能力

本地索引实测（`challenge20-index-v1/index.sqlite`）：

- 块/篇：p50=48、p90=260、p99=2,115、max=**235,923**；块长中位约 100–200 字符。
- 文档规模：中位约 5–20k 字符（≈1.5–5k token）→ 单向量直接可行；p90 约 10k token → 超出 bge-m3 的 8k 窗口，需要 32k 上下文模型或截断。
- 长尾极端：**top-100 文档占全部块数的 18.3%、top-1000 占 41.6%**，最长一篇 23.6 万块（百万 token 级）→ 任何单向量都装不下，必须分段编码 + 池化/摘要（或截断并接受损失）。
- 官方同类基线（同基准、文档级、4096 token 截断、Qwen3-Embedding-0.6B）：证据 recall@5 = 6.2%、@100 = 26.4%、@1000 = 59.7% → 文档级单向量的定位是**粗筛层**（把 10 万缩到几百–一千），不是精排。

当前模型能力（2026）：

| 模型 | 上下文 | 维度 | 备注 |
|---|---|---|---|
| bge-m3（现用） | 8,192 | 1,024 | 中位文档够用，长尾不够 |
| Qwen3-Embedding（0.6B/4B/8B） | **32,768** | 4,096 | MRL 可截断到 32 维；可 int8 |
| Cohere Embed v4 | **128k** | 256–1536 | Matryoshka；直接输出 int8/binary/ubinary |
| jina-embeddings-v4 | 长文（多向量/晚交互） | 2,048 | 单向量 + 多向量两种模式，适合长文细粒度 |

结论：**文档级向量可行，但要按"粗排入口"设计**——10 万条向量几百 MB，配 32k 模型覆盖 p90 文档，超长文档切段后按 max-pool 合成文档分数；精读继续交给 grep/read 或块级/晚交互。语义树（RAPTOR 式）可以直接建在文档向量上。

### 实测估算：文档级向量到底占多少（1024 维）

从官方 Parquet 全量统计（100,195 篇，总 3.24B 字符 ≈ 810M token @4 字符/token）：

| 指标 | 数值 |
|---|---|
| 文档字符数 | p50 = 10,241；p90 = 62,269；p99 = 391,797；max = 9,962,379 |
| 超过 32k 字符（8k token 窗）的文档 | 21,036 篇 |
| 超过 512k 字符（128k token 窗）的文档 | 733 篇 |

按"token 计数器均分块、每块一个向量"：

| 方案 | 向量数 | fp32 | fp16 | int8 |
|---|---:|---:|---:|---:|
| **bge-m3（8k 窗口，超窗切批）** | 168,882（+68,687） | **0.69 GB** | 0.35 GB | 0.17 GB |
| **bge-m3（8,192×90% = 7,372 上限，超限切批）** | 176,438（+76,243） | **0.72 GB** | 0.36 GB | 0.18 GB |
| **qwen3.7-text-embedding（128k 窗口）** | 101,467（+1,272） | **0.42 GB** | 0.21 GB | 0.10 GB |
| 参考：块级（现状，1840 万块） | 18,437,321 | 75.5 GB | 37.5 GB | 18.9 GB |

- 假定 1 token ≈ 4 字符；按 3.5 字符/token 时 bge-m3 8k 为 0.74 GB、90% 上限为 **0.78 GB**、qwen 为 0.42 GB（±10% 量级）。
- 8k 窗口下只有 21% 的文档需要切批，但贡献了 69k 个额外向量；128k 窗口下仅 733 篇需要切批（绝大多数网页一段装下）。
- 若用 MRL 把 1024 维截到 256 维，上述 GB 再除以 4。
- 复算：`scripts/analysis/estimate_doc_vectors.py`（独立工程，读原始 Parquet，无模型调用）。

### 附：BM25 现状口径（全库文档级，2026-09-12 校准）

索引构建本身**没有超时**（100,195 篇全量 tokenize → `index_complete` → `calibration_complete`，日志无错误）；此前记录的超时发生在别处——agent 运行期的 grep 工具（45 秒上限、26–52% 调用超时）与"整块载入内存"的旧 runner。精度口径（从校准回执重算，含官方协议的长尾指标）：

| 指标 | 本文档级 BM25 | 官方论文 BM25 | 官方 Qwen3-0.6B 稠密 |
|---|---:|---:|---:|
| 证据 recall@10（官方报 @5） | 2.4%（逐题均分 2.7%） | 1.2%（@5） | 6.2%（@5） |
| 证据 recall@100 | 11.1%（9/20 题命中） | — | 26.4% |
| 证据 recall@1000 | 27.5%（14/20 题命中） | — | 59.7% |

- 结论：裸问题直接喂 BM25 的弱是基准的固有性质（官方论文同款数字），不是索引坏了；BM25 在这个规模上只能当**召回型候选生成**，且明显弱于稠密检索的长尾。
- 待改进：查询侧（官方 agent 会用短查询改写，而不是整段问题）、文档粒度（巨型文档稀释打分；段落级 BM25 通常显著抬升 top-k）、以及对齐官方协议报 @100/@1000。

## 8. 参考

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
- [BrowseComp-Plus 仓库（预构建 BM25/Embedding 索引、文档级检索工具）](https://github.com/texttron/BrowseComp-Plus)
- [Tevatron BrowseComp-Plus 检索示例（整篇文档编码、fp16）](https://github.com/texttron/tevatron/blob/main/examples/BrowseComp-Plus/README.md)
- [Qwen3-Embedding（32,768 token、4,096 维、MRL）](https://github.com/QwenLM/Qwen3-Embedding)
- [Cohere Embed v4（128k 上下文、Matryoshka、int8/binary 输出）](https://docs.cohere.com/changelog/embed-multimodal-v4)
- [jina-embeddings-v4（单向量/多向量长文检索）](https://huggingface.co/jinaai/jina-embeddings-v4)
