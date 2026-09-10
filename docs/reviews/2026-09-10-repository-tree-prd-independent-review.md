# 仓库树 PRD 独立技术评审

日期：2026-09-10 · 对象：[PRD v0.1](../plans/2026-09-10-subtex-repository-tree-prd.md) · 结论：**产品方向成立，技术方案需要实质修订后再进入实现。**

本次按用户要求，不以大项目的语言、依赖、服务、账户或开发惯例作为选型约束。保留前序讨论中的目标：文档内部结构树、跨文档语义组织、Agent 高效阅读。独立运行的单机文档检索产品是本次评审假设；未把目标擅自扩展为分布式企业平台。

配套交付：[独立版 PRD v0.2](../plans/2026-09-10-repository-tree-prd-v0.2.md)。v0.1 保留原貌供对照。本评审为文档、上游资料与数学分析，未执行代码、跑分或真实模型测试。

## 1. 结论与优先级

应保留的部分：原文主本；结构关系与统计关系分开；全体原文可直接检索；批量读取、出处与预算；主题引用去重；部分就绪状态；不将 Top-K 视作查全。

主要修订：取消按项目复用决定技术栈；把解析保真、embedding 选择和小候选精排前移；主题树首先服务导航，是否进入默认召回另测；SVD 降为有退化检测的研究项；补齐增量发布、过滤、分页和性能验收的可实施契约。

| ID | 严重度 | 原文位置 | 发现 | v0.2 处理 |
|---|---|---|---|---|
| R01 | P1 | §0.1、§12–13 | 选型理由依赖既有工程，未独立证明功能与性能适配 | 删除项目依赖，比较嵌入式/服务式方案 |
| R02 | P1 | §7.3 | SVD 贡献评分可能退化为向量长度，归一化后接近无区分度 | 推导边界、补负例，移出默认产品路径 |
| R03 | P1 | §6.3、§13.1 | embedding 留作“既有 provider”，关键质量/延迟变量未决定 | 模型候选、输入模板、token 上限与硬件档完整定义 |
| R04 | P1 | §13、§14 | 存储缺少过滤和规模下的性能决策，ANN 与语义树职责未充分分离 | exact/ANN 对照、过滤选择性评测、独立引擎比较 |
| R05 | P1 | §6.1 | 解析选型依赖既有入口，无法独立保证章节与原文定位 | Docling + 文本解析路线，分格式结构验收 |
| R06 | P1 | §8.2–8.3 | source hash、解析版本、快照及索引 generation 关系不完整 | 来源版本/解析版本/发布代次分别定义 |
| R07 | P1 | §9.2–9.4 | 有截断状态却缺 search 续读闭环；多问题共用 scope 不利于批量覆盖 | 分请求 scope，确定性查找分页、语义候选句柄与 coverage |
| R08 | P2 | §7.1、§10.1 | 聚类参数过早固化，主题召回存在与 dense 重复计算/投票 | 稳定导航基线，先不增加默认召回通道 |
| R09 | P2 | §4、§15–16 | reranker 在 SVD 同期或之后验证，投入顺序缺少质量基线 | 检索基线阶段即比较精排 |
| R10 | P2 | §14–15 | 拟 SLO 排除了最重环节，样本数不足以稳健证明 5pp 改善 | 分阶段与端到端同时测，探索样本不充当统计证明 |
| R11 | P2 | §6.2、§9.3 | 字节限制不能保证任意 Agent tokenizer 的 token 硬上限 | 区分软 token 预算与硬字节上限 |

P1：实现前需要关闭的架构/功能问题。P2：应在对应阶段关闭，不代表存在已发生的生产事故。

## 2. 关键发现的证据与修订理由

### R01：把复用便利当成独立选型结论

v0.1 第 25 行以“Rust + SQLite 沿用现有基础”为决策，§13 又以“不新增 Python 总控”排除整条方案。复用是集成项目的合理因素，但不能回答独立产品哪种组合更可行。

独立任务中，解析、tokenizer、模型推理和聚类的数据流都能直接使用 Python 生态，矩阵计算和检索可以交给原生库。Python 编排不等于用 Python 循环计算所有向量；Rust 编排也不消除 PDF、GPU 推理和数据扫描成本。故推荐 Python 控制层，性能关键路径使用原生引擎；这是一项工程判断，非已测得的语言性能排名。

### R02：SVD 评分存在可证明的退化边界

v0.1 定义 `score(i)=Σ(j≤k) σ_j² U_ij²`。设 `M=UΣVᵀ`，第 i 行是候选向量。保留全部非零方向、即 k 等于矩阵秩时：

```text
score(i) = (UΣ²Uᵀ)ii = (MMᵀ)ii = ||M_i||²
```

若输入逐行 L2 归一化，则所有分数均为 1。若矩阵接近各向同性，95% 累计能量可能需要保留大多数方向，评分可能接近这一退化情况。最简单的分析例子是正交单位向量组成的单位矩阵：0.95 阈值在三行样本中保留三个方向，三行完全同分。

这不证明 SVD 对所有语料无效；它证明“能量贡献高”不能直接等同于“更有代表性”，更不等于“包含重要事实”。低秩时评分仍可能有信号，但公共模板与重复套话也可能主导方向。

**修订：** 默认代表文本采用质心邻近原文 + 去重 + 文档/章节多样性。SVD 在实验中与该基线比较，必须记录有效秩、分数分散度、保留率、模板占比和计算成本。其数学依据来自 SVD 恒等式，以上退化结论为本次推导，并非原论文的实验结论。[NumPy SVD 定义](https://numpy.org/doc/stable/reference/generated/numpy.linalg.svd.html)

### R03：模型和输入表示比编排语言更直接地决定质量

原稿没有独立选定 embedding 候选，也没有明确短文本模型输入超限时如何切分。继续沿用某个 provider 会把质量、网络尾延迟和成本混入架构对比。

模型输入模板也属于技术契约。例如 multilingual-e5-small 的检索与聚类前缀有区别，输出 384 维、最长输入 512 tokens。复用检索向量去聚类只能作为可测的近似，不能宣称天然最优。[模型卡](https://huggingface.co/intfloat/multilingual-e5-small/raw/main/README.md)

**修订：** CPU 候选 multilingual-e5-small；质量对照 BGE-M3、Qwen3-Embedding-0.6B。按同一真实语料比较，允许最终只保留一个默认模型。输入规范化、query/document 模板、权重 revision、维度与 pooling 纳入缓存 key。更换模型重新构建对应空间，不能混合检索。

### R04：语义导航树不能代替向量引擎的物理索引

“主题树方便浏览”和“向量查询执行得更快”是两个目标。前者的分组未必适合近邻剪枝；后者可由 flat/IVF/HNSW 等索引解决。文档结构的树深不能直接推导检索复杂度或召回保证。

v0.1 保留全局直接召回是对的，但选用 sqlite-vec 的理由主要是既有依赖。对于更大规模、范围过滤、中文 BM25、索引更新与快照，需要独立比较组合成本。

**本次推荐：** 默认单机产品优先验证 LanceDB OSS：同一检索表组织正文与向量，提供 hybrid、过滤、版本与索引能力；SQLite 专管目录/IR/任务/发布状态。LanceDB 仍有索引维护和双存储一致性成本，不可直接宣布优于 sqlite-vec。高并发服务式部署可评估 Qdrant Server，而不是把 qdrant-client 的 local mode 当服务端性能替身。[LanceDB hybrid](https://docs.lancedb.com/search/hybrid-search)、[Qdrant client](https://github.com/qdrant/qdrant-client)

另一个具体选型坑：USearch 上游能力表显示 Python binding 与 C++/Rust binding 的过滤能力不同，不能因为引擎支持 filter 就假定所有语言 SDK 可用。本次不把它直接指定为 Python 端的过滤检索默认引擎。[USearch 能力表](https://github.com/unum-cloud/usearch)

### R05：结构树的质量上限来自解析，不来自后续聚类

正文读序、表格行列和标题级别一旦解析错，树只是把错误持久化。原稿以“复用既有 parser”替代独立选型，缺少格式分层的验收。

**修订：** 明确 Markdown/纯文本快速路径与 Office/PDF 路径。Docling 是后者的推荐候选，其 IR 支持层级、布局和 provenance；仍要逐格式检验输出，没有提供的坐标保持缺失。扫描件 OCR 单列吞吐，不隐藏在“5 秒索引就绪”内。[Docling IR](https://docling-project.github.io/docling/concepts/docling_document/)

### R06：文件没改，解析器改了，节点身份仍可能变

只有 source hash 不能区分解析器修订、结构算法、正文规范化和 chunk 规则。原稿以 source_version 表达多个含义，会导致缓存复用错误或旧节点误指。

**修订：** 分别定义 source_revision、parse_revision、representation_revision 与 publication_epoch。只把 query 与读取绑定到已发布的一致视图。使用 SQLite + LanceDB 时，两个组件各自支持事务/版本，并不构成跨组件原子事务；v0.2 给出先准备后发布的 manifest 协议和故障测试。

LanceDB 表更新和维护都会产生新版本，旧版本还可能被清理，因此查询句柄生命周期必须与版本保留联动。[LanceDB 版本说明](https://docs.lancedb.com/tables/versioning)

### R07：截断标识不能代替继续读取的接口

原稿 search 只有 `truncated` 和预算，没有明确 cursor 或候选结果句柄。Agent 如果要读更多只能重搜；结果可能变化，也可能重复消耗 query embedding。批量 queries 共用 scope，对逐文件检查还会诱导调用方发出许多小请求。

**修订：** 每条 search request 有 qid 与 scope；literal/regex 可按稳定顺序穷举，hybrid 的 cursor 只遍历当前候选集，不能代表全仓查全。浏览清单、搜索过、原文已读、语义问题已回答分别计数。read 返回剩余范围与可续读句柄。

### R08：聚类应先证明导航收益

原稿把固定分支、深度、单例和次级主题政策一次性设计得较重，同时把主题放入默认召回。引入与 dense 高度相关的候选通道不一定增加证据，可能只是重复投票。

**修订：** 树先作为浏览和可选 scope；主要检索仍以词法与 dense 为基线。主题候选单独消融。聚类计算使用 scikit-learn 公共 API；如果需要导出完整父子关系，产品记录每次分裂，不能依赖库私有树对象。BisectingKMeans 是有效候选，但库的预测能力不等于现成的产品导航树。[scikit-learn API](https://scikit-learn.org/stable/modules/generated/sklearn.cluster.BisectingKMeans.html)

### R09：精排应与结构基线一起测

相关性判断的直接基线是检索后对小候选集做 query-document 精排。它仍需测量实际收益，不能先验保证提升，但应在主题树/SVD之前提供对照，避免把本可由排序修复的问题归因给索引结构。[Sentence Transformers retrieve-rerank](https://sbert.net/examples/sentence_transformer/applications/retrieve_rerank/README.html)

**修订：** 首轮便比较 RRF 与 RRF + reranker；CPU 低延迟模式可不启用精排，质量模式在明确预算下启用。额外模型加载、长文本截断与 GPU/CPU成本全部入账。

### R10–R11：性能与统计口径需要收紧

单独测量本地热查询是必要的，但用户等待还包含 query embedding、模型冷启动、解析积压和 Agent 推理。120 题可以发现缺陷，不能自动充分证明 5 个百分点差异。独立选型不沿用任意“后台两个线程”的项目资源规则，改为显式 CPU/GPU/RAM预算与前后台隔离。

token 估算同样不能同时保证所有 Agent 的真实 tokenizer。v0.2 区分 token 目标、字节硬限与框架额外包装开销；已知 tokenizer 才声称精确计数。

## 3. 独立技术栈结论

| 层 | 推荐 | 推荐理由 | 不能推导出的结论 |
|---|---|---|---|
| 编排 | Python + Pydantic + 官方 MCP SDK | 解析、模型、数值生态完整；schema 易校验 | 不代表 Python 任意实现都快 |
| 重解析 | Docling | 结构/provenance 比纯文本转换贴合需求 | 不保证所有 PDF 正确、零 OCR 成本 |
| 文本解析 | markdown-it-py / 直接文本适配 | 轻量路径独立，保留源码映射 | 不从渲染结果反推字节位置 |
| 检索数据面 | LanceDB OSS | 单检索表覆盖 FTS/dense/filter/version，减少多索引组装 | 不是“装上就具备无损 ANN 和自动维护” |
| 状态/结构 | SQLite | 单机事务、外键、范围与任务状态适配 | 不是对旧项目的继承；也不承担大向量计算 |
| 聚类/代表选择 | scikit-learn + NumPy/SciPy | 公共 API 与可重复实验方便 | 不承诺语义树等于分类真相 |
| 模型运行 | Sentence Transformers / Transformers | 能直接对比 encoder 与 reranker | 模型卡成绩不代表本文语料最优 |
| 精确检索 | ripgrep / 受限正则引擎 | 原始词和表达式保持可控语义 | 全文扫描不具有固定亚秒保证 |

更轻的候选是 Python + SQLite FTS5 + sqlite-vec；如果常规规模已经通过质量、过滤与 P95 门，它完全可以成为最终选择。服务式高并发候选是 Python + Qdrant Server + 状态存储；是否值得增加独立服务取决于实际负载。**本次给的是有依据的推荐顺序，不是未经跑分的胜负宣判。**

## 4. 实施前必须关闭的门

1. 在固定发行版本上验证中文 FTS、过滤前置、未索引新行可见性、历史版本 checkout 和维护后的句柄保留。
2. 验证 SQLite/LanceDB 发布中任意崩溃点都不会让结果混用来源版本。
3. 比较 exact 与 ANN：范围选择率 100%、10%、1%、0.1%，并发 1/4，更新尾部 0/1%/10%。
4. 使用含表格、跨页、重复标题和模板噪声的真实样本验证解析与引用。
5. 在同一 token 预算下比较 grep、hybrid、结构展开和 reranker，再决定主题召回与 SVD。

这些是后续原型验证任务，本次没有执行，也不将上游文档承诺视为本机测试通过。
