# 非 LLM 生成结构对 agentic 检索的价值调研（BrowseComp-Plus / VGI）

日期：2026-09-18。问题：在 ~100K 文档、证据**不主题聚集**的语料上，非 LLM 生成的树/图结构能否提升 (a) 检索 recall、(b) agent 导航/理解——对照现有 flat vector(bge-m3)+passage-BM25(tantivy)+RRF+rerank。结论先行：**语料侧结构（树/图/主题标签）没有能打的证据；query 侧扩展（PRF）与呈现侧粒度（passage/snippet 返回）是两个有文献与本榜单直接证据的杠杆。**

背景锚点（本仓实测，见 [eval-m1-vector-ab-results](eval-m1-vector-ab-results.md)）：hybrid RRF(60) @100=0.2358 @1000=0.5066，+rr200 @100=0.2683；2 级球面 kmeans 树 route recall ~200 文档暴露仅 0.121（flat @200≈0.28），逐档皆劣；agent 级失败桶 ~4/20 未检索、~4/20 检到未读、~5/20 读了答不出。

## 1. 伪相关反馈 / 查询扩展（RM3、Rocchio、KL 展开）

**机制**：search → 取 top-K 当伪相关集 → 抽词项/向量重新加权 → 同一次工具调用内二次检索合并。RM3 原典：Abdul-Jaleel et al., "UMass at TREC 2004: Novelty and HARD"（CIKM 2004 workshop）；系统对比见 Lv & Zhai, CIKM 2009（RM3 是经典 PRF 里最强档）。

**量化证据（recall 与 MAP 均涨）**：
- Anserini/Lucene Robust04 回归：BM25 MAP 0.2531 → BM25+RM3 0.2903（默认）/ 0.3043（调参），约 +15–20% 相对。来源：https://github.com/castorini/pyserini/blob/master/docs/experiments-robust04.md 与 anserini experiments-forum2018.md。
- PyTerrier Robust04：BM25 MAP 0.2418 → Bo1 0.2795 / KL 0.2794 / RM3 0.2765；P@10 0.426→0.454。来源：in_d_docs_demo Robust04.md（Terrier 官方参数 fb=3 docs/10 terms）。
- TREC 2025 Product（DUTH）：BM25+RM3 + 加权 RRF，「RM3 and fusion yield consistent improvements over the BM25 baseline across … Essential Recall@1000」。https://trec.nist.gov/pubs/trec34/papers/DUTH.product.pdf
- 稠密侧 Vector-PRF（Li, Mourad, Zhuang, Koopman, Zuccon，TOIS 2023；arXiv:2108.11044）：反馈文档**向量**做 Average/Rocchio 融合，ANCE R@1000 0.7554→0.7825、MAP 0.371→0.425；跨数据集/指标一致提升，代价 ~2× BM25。Pyserini 已内置复现：https://github.com/castorini/pyserini/blob/master/docs/experiments-vector-prf.md
- ColBERT-PRF（Wang et al., ICTIR 2021 / TOIS 2023，doi:10.1145/3572405）：多向量表征下 MAP +26%（TREC'19）/+10%（TREC'20），并明确提升 candidate-set recall。
- 「扩展后两路再插值」：Li et al., SIGIR 2022 "To Interpolate or not to Interpolate"（ielab.io/files/li-sigir-2022-inter.pdf）——sparse RM3 与 dense Vector-PRF 各自扩展再融合仍有收益，与本栈 hybrid+RRF 结构天然契合。

**风险与边界（要写进预期）**：top-K 有噪声时 topic drift（arXiv:2601.11238 的动机）；**本语料刻意采了 hard negatives**（主题近似但非证据），盲信 top-K 会漂移——缓解：fb_docs 取小（3–5）、用 RRF 两路**共同命中**的 doc 当反馈集（双路同现 ≈ 免费的相关性信号）、RM3 权重 λ≥0.5 保原查询。RM3 展开的 top 词送进 BERT 系 reranker 会掉点（Padaki et al.，TOIS 2023 转引）——**扩展只用于 recall 路，rerank 仍用原查询**。MS MARCO 类稀疏判定上 PRF 增益小（pyserini 注记），但本语料是「难查询+深 recall」画像，正是 PRF 的传统甜区。

**本栈落地**：tantivy 无内置 PRF，但 `SegmentReader::inverted_index(field)` → `InvertedIndexReader::get_term_info/doc_freq`（docs.rs/tantivy，TermInfo.doc_freq）可拿 df；passage 是 indexed-not-stored，反馈文档正文从 sqlite `documents.body` 取、按同一 tokenizer 复切算 tf——Lucene MoreLikeThis 官方文档就是这套「re-tokenize + docFreq」启发式（lucene.apache.org MLT javadoc）。Rocchio 向量侧更便宜：窗向量已在库，q′=α·q+β·mean（反馈 doc 的窗向量均值）。**一次 `search` 调用 = 首查→反馈→扩查→RRF 合并**，不耗 agent 轮次——这是 20 轮预算下 PRF 的正确形态。

**verdict**：强证据、对口「never-retrieved」桶；是本调研里唯一「非 LLM 且有量化的 recall 杠杆」。预期 @100 量级提升 10–20% 相对（TREC 量级，非保证）。

## 2. k-NN / 相似度图导航（"找已命中文档的邻居"）

**事实**：HNSW 索引本身就是一张可导航相似度图（Malkov & Yashunin, TPAMI 2020）；暴露 `more_like_this(doc_id)` ≈ 用文档均值向量再查一次 ≈ k=1 的 Vector-PRF。Lucene 的 MoreLikeThis 是同族启发式——其 javadoc 里 Doug Cutting 的原话把它定位为「搜索结果页的 more-like-this 按钮」，明说「想赢 TREC 差那一两个点的话这些启发式没用」。

**证据评估**：doc 级相似度扩展提升多跳 recall 的**直接证据很薄**。真正的多跳图检索证据来自**真实超链接**语料：MDR（Xiong et al., ICLR 2021）在 HotpotQA 上用 Wikipedia 超链接链式检索有效——但那是人工编辑的链接图，BrowseComp-Plus 语料是 web 快照，`documents` 表只有 doc_id/body/heading/url，**没有抽取出的互链**。相似度邻居 ≈ 同主题近邻——而本语料 evidence 跨主题分散（19/20 题证据跨主题，见 deferred.md），邻居扩展理论上会重蹈聚类覆辙（同主题加强、跨主题够不着），只是粒度更软。

**verdict**：实现便宜（窗向量均值→search_vector），当 agent 省推理轮的「同款文档」按钮可以上；但别指望它修 never-retrieved——它扩的是「已找到的东西周围」，不是「没找到的方向」。**非优先**。

## 3. 非 LLM 聚类标签（c-TF-IDF、LDA/NMF、keyterm）

BERTopic 的默认主题表征就是 **c-TF-IDF**（class-based TF-IDF：把每簇拼成一篇文档再算 TF-IDF 变体），非 LLM——来源：arXiv:2203.05794 与官方文档 maartengr.github.io/BERTopic（ClassTfidfTransformer，bm25_weighting 选项）。LDA/NMF 同理无 LLM。

但标签解决的是「人/ agent 浏览簇」的可读性，不是 recall。经典判例：聚类假设（Jardine & van Rijsbergen 1971, doi:10.1016/0020-0271(71)90051-9）声称「相互关联的文档对同一请求趋向同相关」；Voorhees 的 SIGIR'85 实证检验（"The cluster hypothesis revisited", doi:10.1145/253495.253524）直接给出本实验的先验结论——**当相关文档两两不相似时该假设不成立，且整簇返回通常差于簇内按文档打分**。Scatter/Gather（Hearst & Pedersen, SIGIR'96）的定位也只是「对检索结果做聚类浏览」，是 UX 辅助。本栈 kmeans 树逐档输给 flat 就是这条 40 年前的结论在新语料上的复现；给败北的簇贴 c-TF-IDF 标签修不了 route recall。

**verdict**：对 recall 无证据；对 agent 导航只在「人看簇」场景有意义。**不做**（deferred.md 的「语义树 v2 门」维持关闭判断）。

## 4. 语料固有结构（URL 层级、域名、时间戳、wikilinks）

本语料 schema：`{doc_id, body, heading, url}`（corpus.md）；实测 `heading` 全空、无时间戳字段；`url` 是文档自身 locator，**文档间链接图不存在**。可用的非语义结构只剩 url 的 domain/path 前缀。

已发表证据：**没有**「靠 URL/domain 结构提升 agentic 检索 recall」的对口工作。MDR/HotpotQA 那套是 wikilink 图（见 §2），不适用。唯一可辩护的用法是 **facet 过滤**（agent 按域名收窄，如题目暗示来源类型）或 url 段进 BM25 字段——tantivy 加个 indexed domain 字段成本极低，但无增益证据，仅当 Challenge-20 里题目含站名线索时才有条件价值。BrowseComp 题型是「按多重约束找实体」，域名约束偶尔出现但非常态。

**verdict**：**无证据**。至多留一个 `search(query, domain=?)` 过滤参数级别的便宜口子，不做结构建设。

## 5. BrowseComp-Plus 榜首系统到底用什么

数据源：官方 leaderboard 后台 CSV（huggingface.co/datasets/Tevatron/BrowseComp-Plus-results → agent_results.csv / retriever_results.csv，随 tevatron-browsecomp-plus.hf.space 渲染）+ 各提交者自述。

| 系统 | 准确率 | 调用数 | 结构 |
|---|---:|---:|---|
| GPT-5（CSV 记名；提交于 2026-04）+ Hybrid(Qwen2-7B + Reason-ModernColBERT)，AI21 | **95.18%** | 209.27 | 「multi-agent setup coordinating up to 8 agents」，工具只有 `search` + `get_doc`；AI21 博客（ai21.com/blog/maestro-deep-research-agents）说明 Maestro 框架跑 **rollout ensemble**（如 <Minimax,LateInteraction,+get_doc>×32 + <GPT-5,Dense>×4，sequential 达标即停）。检索是 sparse+dense+late-interaction 混合，**无树/图**。 |
| GPT-5 + Reason-ModernColBERT + get_document，LightOn | 87.59% | 13.27 | 标准 `search`（top5×512tok）+ `get_document`；LightOn 博客明写 "No reranking model, no chunk oracle"——**扁平两工具**。 |
| GPT-5 + Mixedbread v3 + get_document，Mixedbread | 90.48% | 11.53 | 同上扁平。 |
| GLM-5.1-FP8 + Hybrid BM25+Qwen3-8B，Sail Research | 90.72% | 12.53 | multi-agent swarm；「Searcher returns k truncated documents, extending documents when needed」——扁平 hybrid + 按需取全文。 |
| GPT-5 + Mixedbread，Agentica/Symbolica | 78.41% | 44.67 | REPL 任意代码执行 + `search`(top-200!) + `get_doc_by_id`——大 k 扁平。 |
| GPT-5-mini + openJiuwen | 80% | 6.05 | **检索结果切块、只回最相关 chunks**（替代 512tok 截断）+ 多 agent。 |
| OpenResearcher(30B) + Qwen3-8B | 60.92% | 41 | `search`（lucene highlighter 过滤正文）+ `open` + `find`（文档内 grep）——和本栈 `rg` 同形。 |
| gpt-oss-120b + BM25 | 43.61% | 46.51 | 原生 `search`/`open`/`find`。 |
| oss-20b-high + 段落语料 BM25 + monoT5-3B，爱丁堡（arXiv:2602.21456, SIGIR'26） | 68.92% | 27.57 | **passage 级索引+段落直返**（对照：文档级 BM25 50.60%，段落级 57.23%，+rerank 68.92%）。 |

**结论**：**榜上没有任何树/图/聚类结构提交**。retriever_results.csv 里全部 6 个检索器（BM25、JinaColBERT-v2、Qwen3-0.6B/4B/8B、ReasonIR-8B）也都是单路扁平。榜首的实际机制 = **扁平检索 + get_doc/read + 大量迭代**（agent 能力 × 轮数 × 偶发的多 agent ensemble）。论文自身图 1 也写明：「agents mostly improve accuracy at a cost of more search calls」。迭代+扩展是已证实的机制，导航结构不是。

## 6. passage 级返回对 agent 理解的证据

- **榜单直接证据**：爱丁堡 Meng et al. "Revisiting Text Ranking in Deep Research"（arXiv:2602.21456，SIGIR 2026）——同一 oss-20b-high agent，文档级 BM25 50.60% → **段落级索引+段落直返 57.23% → +monoT5-3B rerank 68.92%**。原因：agent 查询是 web-search 风格（含引号精确匹配），段落粒度避开长文档长度归一问题、省 context。openJiuwen 用 GPT-5-**mini** 拿到 80% 也靠「只回相关 chunks」。
- **论文自身 ablation**（arXiv:2508.06600 §4.3/§4.8.3）：标准工具面就是 top-5 × **512-token 预览**（不是裸 doc_id）；加 `get_document` 全文读取后 gpt-4.1 35.42%→43.61%（+8.2pp），均摊 1.85 次 get_doc/题。512 tok 截断时 86.5% 题仍至少有一篇金标文档的答案落入预览。
- **对本栈的含义**：现在 `search` 只回 doc_id 列表，agent 要再花 `read` 调用才能看内容——「检到未读」桶正是这个摩擦。tantivy passage 索引天然知道**命中的是哪一段**（passage→doc max_pool 时顺手带 char 区间），search 直接回 `(doc_id, passage_offset, ~512tok snippet, url)` 零额外检索成本，等价于榜单标准的 preview+get_doc 形态。「紧凑命中」语义（retrieval.md：doc_id+短片段+offset）本来就往这个方向写了，把它升成 search 的默认返回即可。

**verdict**：**最高性价比**。直接对口「retrieved-not-read」桶 + flash-tier 的理解力短板；证据来自同一 benchmark 的多个独立提交。

## 对比表

| 方向 | recall 证据 | agent 导航/理解证据 | 本语料适配 | 实现成本 | 判定 |
|---|---|---|---|---|---|
| RM3/词法 PRF（一次调用两查） | 强：Robust04 MAP +15–20%、TREC'25 recall@1000 | 间接（少耗轮次） | hard negatives 有漂移风险，需 RRF 共现反馈集 | 中（tantivy get_term_info + 复切 tf） | **做** |
| Rocchio/Vector-PRF（向量空间） | 中强：TOIS'23 跨集一致，R@1000 +2–4pp 绝对 | 同左 | 窗向量现成；与 RM3 扩展结果再 RRF | 低 | **做** |
| passage/snippet 直返 + read | 不改 recall@k 但改「有效 recall」 | 强：get_doc +8.2pp、段落语料 +18pp、chunked 80% | 天然对口「检到未读」 | 低 | **做（最先）** |
| more_like_this / kNN 邻居 | 薄：=k=1 PRF | 薄 | 证据跨主题，邻居≈同主题，重蹈聚类 | 低 | 可选，非优先 |
| 单次大 k 返回（top-200 列表） | 免费加深 | Agentica 78.4% 实证 | 直接 | 极低 | **做**（参数改动） |
| c-TF-IDF/BERTopic/LDA 标签 | 无 | 仅浏览 UX；Voorhees'85 判过簇检索死刑 | 已实测败北 | 中 | 不做 |
| URL/domain facet | 无对口证据 | 无 | url 存在但无互链/时间戳 | 低 | 口子可留，不建设 |
| 语义树/层级图导航 | 负：实测逐档劣于 flat | 负 | 聚类假设不成立的语料 | 高 | **不做**（维持 M3 关闭） |

## 建议（按本栈期望收益排序）

基线：flash-tier agent、20 轮/64 调用预算、hybrid RRF(60)+rr200（@100=0.2683）。

1. **search 返回 passage 级片段**（`doc_id`+`char_offset`+~512tok snippet+`url`），read 保留取全文。成本最低、榜单证据最直接、对口检到未读桶。预期：等同官方 preview+get_doc 工具面（gpt-4.1 +8.2pp 的同款设计），并把「agent 读到的证据密度」提一个量级。
2. **PRF 内嵌进 search**：首查 → 取 RRF 两路共现的 top3–5 doc → RM3 词法展开（λ≈0.5、~10 词）+ 窗向量 Rocchio（α≈0.6/β≈0.4）→ 双路复跑 → 全部 RRF 合并返回。一次调用、不耗轮。对口 never-retrieved 桶；先在 `eval --mode hybrid` 的 Challenge-20 oracle 上离线量 @100/@1000 再接 agent。风险（hard-negative 漂移）用共现反馈集控制；rerank 阶段保持原查询。
3. **search 默认返回数调大**（top-k 5→~50/200 可选）：Agentica 证明大 k 扁平列表对 agent 是净收益；配合 snippet 返回成本可控。
4. （可选）`more_like_this(doc_id)`：窗向量均值→search_vector，半个下午的事，给 agent 一个省推理的「同款」按钮；不指望 recall。
5. **不做**：任何树/簇/主题标签导航（聚类假设在本语料已被证伪两次——Voorhees'85 理论上、kmeans 实测上）；domain facet 最多留个过滤参数。

**诚实结论**：能在「证据不聚集」语料上拿出非 LLM 增益的结构**不存在**——文献里没有，榜上也没人用。本语料的 never-retrieved 桶本质是**词表错配**问题，对症的是 query 侧扩展（PRF），不是语料侧结构；retrieved-not-read 桶是**呈现粒度**问题，对症的是 snippet 返回；read-but-can't-solve 桶是 agent 能力/预算问题，检索结构一概不管用，榜上靠更多轮次和更强 agent 解。另外一个非结构性方向（超出本题范围但值得记录）：榜上前排差距大头是**检索器模型本身**（Reason-ModernColBERT 87.6%、AgentIR-4B 把 reasoning trace 喂进检索器 +17.6pp、arXiv:2603.04384）——bge-m3 窗级向量与 8B 级文档检索器的差距比任何结构都大。

## 主要引用

- BrowseComp-Plus 论文：arXiv:2508.06600 / ACL 2026（aclanthology.org/2026.acl-long.1023）；§4.3 工具面（top5×512tok）、§4.8.1 oracle 93.49%、§4.8.3 get_doc ablation、表 2 检索器 recall。
- Leaderboard 数据：huggingface.co/datasets/Tevatron/BrowseComp-Plus-results（agent_results.csv、retriever_results.csv）；AI21 博客 ai21.com/blog/maestro-deep-research-agents；LightOn 博客 lighton.ai（"The Bloated Retriever Era Is Over"、"Deep Research is now Open"）。
- PRF：Abdul-Jaleel et al. CIKM'04（RM3）；Lv & Zhai CIKM'09；pyserini Robust04 回归（BM25 0.2531→+RM3 0.2903/0.3043）；PyTerrier Robust04（Bo1/KL/RM3）；DUTH TREC'25 Product；Li et al. TOIS'23 Vector-PRF（arXiv:2108.11044，pyserini experiments-vector-prf.md）；Wang et al. ColBERT-PRF（doi:10.1145/3572405）；Li et al. SIGIR'22 插值（ielab.io/files/li-sigir-2022-inter.pdf）；arXiv:2601.11238（PRF 漂移动机）。
- 聚类：Jardine & van Rijsbergen 1971（doi:10.1016/0020-0271(71)90051-9）；Voorhees SIGIR'85（doi:10.1145/253495.253524）；Hearst & Pedersen SIGIR'96 Scatter/Gather（doi:10.1145/243199.243216）。
- 标签：BERTopic arXiv:2203.05794 + maartengr.github.io/BERTopic（c-TF-IDF 默认）。
- 实现：tantivy `InvertedIndexReader::get_term_info`/`doc_freq`（docs.rs/tantivy）；Lucene MoreLikeThis javadoc（含 Doug Cutting 定位引文）。
- 段落粒度：Meng et al. arXiv:2602.21456（SIGIR'26，"Revisiting Text Ranking in Deep Research"）；openJiuwen / Agentica / OpenResearcher 提交者 scaffold 描述（agent_results.csv Scaffold 列）。
- 检索器方向（记录用）：AgentIR arXiv:2603.04384（reasoning-aware retrieval）；Reason-ModernColBERT huggingface.co/lightonai/Reason-ModernColBERT。
