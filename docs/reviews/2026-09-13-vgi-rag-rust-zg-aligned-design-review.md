# Design Document Review: VGI-RAG Rust 重写设计（zg 对齐版）

日期：2026-09-13。对象：[2026-09-13-vgi-rag-rust-zg-aligned-design.md](../plans/2026-09-13-vgi-rag-rust-zg-aligned-design.md)（HEAD/`69bc0758` 锁定稿，工作区无 drift）。本稿只做设计就绪评审，不改设计、不开实现。

对照过：run-9 分析、语义树构建评审、BM25 评审、检索调研、Challenge-20 交接、ADR-0009、`AGENTS.md` 设计原则，以及 `avrag-rs/crates/code-interpreter/src/bridge.rs`、`rag-core/src/merge.rs`、`storage-milvus`。`vgi-rs` 目录不存在。zg BrowseComp-Plus 数字与 zvec 原生 FTS 打分参数未独立重抓，不作伪证。

### Summary

**needs revision。** 本机独立 `vgi-rs` + 文档级向量 + 循环内排序检索，方向对，zvec M0 spike 也站得住。但 M0 验收契约未写出；非目标与后文互相否决；M1/M3 门在 Challenge-20 与既定方法下不可测或结构性达不到。按本文开 M0 会先搭 12–13 个 crate 和云/SAC 骨架，而不是一条能跑的摄取+指纹切片。

### Issue 1: M0 门「指纹一致」无法执行——算法、语料路径、schema 都没有
- **Severity**: critical
- **Section**: §3 数据目录、§9 M0、§10「可开 M0」
- **Description**: M0 验收是「同语料可重建、指纹一致；zvec 本机验证通过」。`manifest.json` 只写「版本、指纹（corpus/questions/index/model）」；全文没有哈希输入、canonical 序列化、粒度（文件 / parquet row / 文档字段）、是否含 tokenizer 版本。语料「复用已下载语料」，但路径、Parquet revision（交接稿写 `b27b02bc3e45511b8b82a13e6f90ce761df726f6`）、与 Windows 原型 `C:/Users/xingc/Documents/Codex/repository-tree/` 的关系都未锁定。`.vgi/corpus.sqlite` 只列「documents / passages / 字段」，无 DDL、无主键、无 `doc_id` 与官方 evidence ID 的对应。没有这些，M0 门不可判定，摄取也无法按文档开工。
- **Suggestion**: 补三样再写「可开 M0」：(1) 指纹伪代码（字段集合 + 哈希）；(2) 语料 URI + revision + 期望文档数；(3) `documents` 最小 DDL（id、正文、heading、url/locators、token 数）。passages 表可标「M2 再切」。
- **Status**: open

### Issue 2: 非目标与后文互相否决，crate 树还多出一个不存在的 `vgi-store`
- **Severity**: major
- **Section**: §2 非目标、§3 crate 树、§4.5、§7、§11.1
- **Description**: §2：「不接 SAC/产线 Rust 栈」「不引入向量数据库服务器」「不复刻 zg 的代码」。同稿随后要求：§3 的 `vgi-sandbox`（「SAC 形态」）；§4.5 / §11.1 云端 **Milvus**（`vgi-store` 第二实现，「沿用 context-osv6 云端栈」）；§11.4 `vgi install` 注入 Codex/Claude。`vgi-store` 出现在 §11.1，不在 §3 的 12 个 crate 里。VGI 是固定 ~10 万篇的评测工具，不是高并发产品线；云端 Milvus、无状态多副本、增量 `jobs/`、daemon「模型池」都是在为不存在的负载做架构。
- **Suggestion**: 非目标改成可执行的裁剪：默认本机-only；Milvus / `vgi-store` / `vgi-sandbox` / install 从 crate 树和 M0 骨架删除，最多列为 M5 以后的可选。若坚持云路径，把它写进目标并补 crate，不要与非目标并存。
- **Status**: open

### Issue 3: 12 crate + 自研倒排 + 云 Milvus + SAC 沙箱，违反「最简能用」和分层生长
- **Severity**: major
- **Section**: §3、§4.2、§4.5、§6–7、§9、§11.1；对照 `AGENTS.md` 设计原则
- **Description**: 原则要求「从能跑的端到端切片长出来」「复用先于自建」。本稿一次交出 12 个 crate（再加 `vgi-store`）、自研段落倒排（delta+varint、mmap、BM25F、lucene/bm25l/bm25+）、云向量库、代码模式沙箱、MCP daemon、宿主 install。M0「workspace 骨架」按 §3 会先铺空壳。评测工具的第一条能跑切片应是：摄取 → 指纹 → 真 token 计数 → zvec spike，而不是 IR 引擎 + 沙箱 RPC + 云 trait。
- **Suggestion**: M0 只留 `vgi-core` / `vgi-ingest` / 一个 bin（token 计数 + zvec spike）。`vgi-lexical` 放到 M2 且先比嵌入式库；`vgi-mcp`/`vgi-eval` 放到 M4；`vgi-sandbox` 默认不进树。
- **Status**: open

### Issue 4: 「对齐 zg」过声称——默认四工具、托管 rg、语义树、非文件语料
- **Severity**: major
- **Section**: 文首一句话、§1、§4.3、§6；对照调研 §8
- **Description**: 一句话写「单一检索工具」。调研所引 zg 默认 MCP 是**一个**工具 `zvec_grep_search`；精确查找走**宿主原生** rg；语料是文件工作区。本稿 §6 默认四个工具（`vgi_search` / `vgi_tree` / `vgi_rg` / `vgi_read`），rg 由本进程托管，另加语义树 v2 与 §7 代码模式。VGI 语料是 Parquet/SQLite 文档，不是文件树。zg BrowseComp-Plus 资源侧数字（input token −37.6% 等）未独立重抓；即使属实，那是「同一宿主、文件工作区、一个 search 工具」的结果，不能直接证明四工具 + 树 + 托管 rg + 代码模式。
- **Suggestion**: 把 zg 降为「循环里要有排序 hybrid + 紧凑出处」的参照，不要当产品形态。默认工具先锁 `vgi_search`（+ 必要的 `vgi_read`）；树与 rg 是否进默认面，用 Challenge-20 再定。
- **Status**: open

### Issue 5: RRF 融合身份、阅读定位键、`vgi_rg` 语料形态都未锁
- **Severity**: major
- **Section**: §4.1–4.4、§10 passage/chunk
- **Description**: 官方 BrowseComp-Plus 检索是**文档级** evidence ID。本稿三套粒度并存：向量 `doc_id#batch`、BM25 ~512 token passage、read「保留现有 chunk 引用（`chunk_id`、offset、locator）」——原型约 1840 万个 100–200 字符小块。§4.4 只写「两路各自返回有序列表 → RRF（k=60）」；未写 key 是 `doc_id`、passage、还是 embedding batch。产品侧 RRF 以 `chunk_id` 为身份（`avrag-rs/crates/rag-core/src/merge.rs`）；此处没有等价定义。身份不一致时 hybrid recall 不可复现，M2 门无意义。
  `vgi_rg`：「必须带 scope（doc/目录/passage 邻域）」。本语料没有 zg 式目录树。未写 rg 扫什么：物化 UTF-8 文件（原型 `FrozenCorpus.grep_ids`）、SQLite 扫描、还是临时抽出。全库 3.24B 字符；不写物化策略，M1 的 rg/read 会停工。
  §10「evidence 是文档级 ID，风险低；回答引用仍走 chunk 级 read」等于为原型小块付兼容税，与「不保留向后兼容」相反。
- **Suggestion**: 评测身份锁 `doc_id`（与官方 qrels 一致）。RRF 在文档列表上做；BM25 先 passage 打分再 max/exp-sum 聚到文档。read 用 `doc_id + char offset/limit`（或 passage_id）；不要把 18M `chunk_id` 带进 vgi-rs。`vgi_rg` 写清：是否物化 `.vgi/docs/{id}.txt`、scope 只接受 `doc_id[]`（外加可选 char 窗），删除「目录」。
- **Status**: open

### Issue 6: M1 / §8 索引门把官方全库数字套到不同模型、不同切分、未锁题集
- **Severity**: major
- **Section**: §1、§8.1、§9 M1；对照调研 §6–7、BM25 评审
- **Description**: 官方稠密基线是 Qwen3-Embedding-0.6B、`--passage_max_len 4096` **整篇截断**、文档级、证据 recall@100=26.4%、@1000=59.7%（全基准口径）。本稿默认 bge-m3、7372 token **切批 + 重叠 + max 聚合**、176,438 向量。M1：「向量 recall@100/1000 ≥ 官方稠密基线」。§8.1 又把 hybrid 目标写成「@1000 ≥60%、@100 ≥30%」——几乎是官方 0.6B 的全库数字。Challenge-20 是 20 道难量子集；文档级 BM25 的 11.1%/27.5% 也是这 20 题，不是官方 830 题。题集、模型、截断 vs 切批任一未对齐，门都不可比。bge-m3 也禁止 `dimensions`（产线 `EMBEDDING_DIMENSIONS` 必须留空）；稿未提。
- **Suggestion**: 索引级评测锁三件事：题集（Challenge-20 vs 官方全量 qrels）、编码（官方 4096 截断 vs 本稿切批）、模型。M1 先报「同协议下的绝对 recall」，不要把 26.4%/59.7% 当 bge-m3 的及格线。要对官方数字，先跑官方预构建 embedding 索引当 oracle（见 Issue 7）。
- **Status**: open

### Issue 7: 跳过官方预构建索引，直接自研倒排，与「复用先于自建」冲突
- **Severity**: major
- **Section**: §4.2、§4.5、§9 M2、§11.1；对照 BM25 评审 §4、调研 §6
- **Description**: 官方托管 BM25 与 Qwen3-Embedding 预构建索引（`scripts_build_index/download_indexes.sh`）。BM25 评审执行顺序第 4 步就是「与官方预构建 BM25 对齐口径」，融合放最后。本稿零引用这些索引，M2 直接「自研倒排（vgi-lexical）」「不引入 tantivy 等额外引擎」。tantivy 与 zvec 一样是进程内库，不是服务器；排除 tantivy、接受 zvec、再自写 160 万 passage 的压缩倒排，理由不成立。zvec 原生 FTS「仅当暴露所需打分参数」——参数面未核实，不能当计划 B。M2「hybrid ≥ 单路最优；p95 < 300 ms」未定义：query embedding 是否走 HTTP、k、冷/热、硬件；300 ms 可能被一次嵌入请求吃掉。
- **Suggestion**: M2 前增加标定步：官方预构建 BM25（及可选 embedding）在同一题集上出 oracle。词法引擎默认用现成嵌入式库（tantivy 或标定后证明必须自研的部分）；自研倒排从默认路径拿掉。p95 拆成「纯索引」与「含嵌入」。
- **Status**: open

### Issue 8: M3 验收门把「根覆盖」当成可改善指标；硬划分改不了跨主题证据
- **Severity**: major
- **Section**: §5、§9 M3；对照 run-9、语义树评审、调研 §2–4
- **Description**: 19/20 题金标证据的最小覆盖节点是树根，因为证据跨主题。这是「证据集合 × 硬划分」的几何事实，不是标签噪声。调研给出两条路：最小整改 = 约束二分 k-means + c-TF-IDF + 质心路由；**标准方案（推荐）** = RAPTOR 式 UMAP+GMM **软聚类** + collapsed-tree。本稿取最小整改。§5 指标门：「证据可定位到 top-k 节点的比例显著优于 run-9 的根覆盖」。若「根覆盖」= min covering node，硬划分不能把跨主题集合收进子树（硬收会毁掉导航）。若本意是 `route(query)` 的 top-k 命中，则未定义 k、单位（题 / 证据文档）、命中是并集还是单节点，且不该拿 covering-node 当基线。run-9 没有路由，根覆盖不是同度量。语义树评审已写：对此类题「明确声明层次结构不可收窄，不做端到端导航对照」。
- **Suggestion**: 删掉「优于根覆盖」。覆盖指标只作诊断，不作 M3 门。若保留树：M3 只验收形状/标签 + **路由 recall@k**（并集含金标文档的比例），并声明 Challenge-20 上不指望单节点收窄。要解决跨主题，需改软聚类/collapsed-tree，不能只修 c-TF-IDF。
- **Status**: open

### Issue 9: 代码模式「不消耗 loop 预算」与「同预算 A/B」直接矛盾
- **Severity**: major
- **Section**: §6、§7、§8.3、§9 M4b
- **Description**: §6：「两模态共用同一检索层与同一预算核算口径」。§7：「代码执行不消耗 agent loop 预算……沙箱执行及其内部检索调用不计入循环预算」。§8.3 / M4b 又要求 tools / code / both「同模型、同题、同预算」。工具臂受 `max_rounds 12–16`、`max_tool_calls 32–48` 约束；代码臂一次执行可内部无限次 search/rg/read（只有保护性资源上限）。这样比的是不同计算预算，质量差无法归因。§7 风险④已承认「否则不可比」，但预算模型没改。
- **Suggestion**: 三臂共用同一计数器：每次桥接 `search`/`rg`/`read`/`tree` 计一次 tool call，代码执行计 round。保护性上限 ≠ 评测预算。改完之前不要写「同预算」。
- **Status**: open

### Issue 10: §7 把 Unix fd 管道当契约；Windows-first 的现成实现是 TCP loopback
- **Severity**: major
- **Section**: §7；对照 ADR-0009 与 `code-interpreter`
- **Description**: 「fd 管道 RPC（fd3/fd4……不开网络端口）」「先例可抄」。独立 `vgi-rs` 不能依赖 `code-interpreter`。`bridge.rs` ~644–650：Windows「instead of fd3/4 (no dup2/inheritable-fd equivalent)」，loopback TCP + 单次 token，另有 Job Object 与 Python 发现（`AVRAG_SANDBOX_PYTHON` / 捆绑 / PATH，排除 WindowsApps 空 stub）。按 §7 字面做，Windows 优先目标上会重踩已修过的坑。此项不挡 M0，但稿把 `vgi-sandbox` 写进 crate 树，骨架期就会抄错协议。
- **Suggestion**: 若代码模式仍留在远期：写明 Windows = TCP loopback + token，Unix 才是 fd3/4；「抄」= 重写，列必须复刻的行为（scope 强制、shim codegen、资源限制），不要写「fd 管道」当跨平台契约。更干净的是 M4b 整章移出默认设计。
- **Status**: open

### Issue 11: 字段 boost「答案常在标题和路径」在本语料未经测量
- **Severity**: minor
- **Section**: §4.2
- **Description**: 「标题与 URL 高权重——这类题的答案常出现在标题和路径里。」这是 BM25 评审的一般建议，不是 Challenge-20 测量。语义树评审：文档 `name` 是数字 ID，代表标题等于 ID。更早的 BrowseComp 物化曾把 heading 写成空（模块化审计）。`heading` / `locators` 是否可 boost，本仓库看不到 Parquet（原型不在本树），不能当已证。
- **Suggestion**: M2 先统计 heading/url 非空率，以及金标答案是否出现在这些字段；空则不要 BM25F 当默认。
- **Status**: open

### Issue 12: 嵌入成本与限速未入设计，M1 流水线会在此停
- **Severity**: minor
- **Section**: §4.1、§9 M1、§11.5
- **Description**: 调研：3.24B 字符 ≈ 810M token（4 字符/token 启发式）。切批+重叠后嵌入量更大。§4.1 只写「令牌桶限速、断点续跑、逐条记账」，无 token 总量、费用、墙钟、失败重试。产线 bge-m3 在 SiliconFlow（TPM/RPM 有上限）。M1 在 spike 之后立刻全量嵌入，不估价就不能授权。
- **Suggestion**: M1 前用 M0 的真 token 计数给出总量、批次、预估费用与墙钟；写进验收，不要沿用 176,438 这条启发式。
- **Status**: open

### Issue 13: 默认工具与代码原语的 JSON 契约未给出
- **Severity**: minor
- **Section**: §6、§7
- **Description**: 有工具名和口头路由，无 JSON schema：`vgi_search` 的 query/queries、route、top_k、scope；`vgi_tree` 的 node_id；错误形状；fingerprint/freshness 字段。代码模式原语同样只有函数签名草稿。不挡 M0，M4 会停。
- **Suggestion**: M4 前用一页 JSON schema 锁工具面；与 Issue 4 的默认工具裁剪一起做。
- **Status**: open

### Issue 14: 文档计数 100,194 / 100,195 并陈；docs 索引未登记本稿
- **Severity**: nit
- **Section**: 全文；`docs/README.md`
- **Description**: run-9 分析一处写「100,194 文档」，设计与树评审用 100,195。差 1 会直接破坏 M0 指纹。`docs/README.md`「待评审设计」未列入本稿（带日期计划可不进「当前权威」，但评审入口应找得到）。
- **Suggestion**: 以 Parquet 实数为准写入指纹契约；评审文可挂到 README 待评审列表。
- **Status**: open

### Strengths
- 独立 `vgi-rs`、实验依赖不进产品 workspace，与「VGI 是评测工具不是产品线」一致；§11.2 把不能直接复用 `llm` / `code-interpreter` 的代价写明白了。
- 检索粒度与阅读粒度解耦（文档级向量、段落级词法、小窗阅读）对准了「70GB 来自切块」的诊断；文档级向量量级（fp16 约 0.36 GB）判断正确。
- run-9 的真因抓得对：缺的是循环里的**排序检索面**和可迭代预算，不是再加一轮盲 grep。
- 先索引级、后 agent 级；树先形状/标签、后端到端。顺序对。
- 树 v2 对已证实的构建缺陷有对症：c-TF-IDF、质心代表、形状验收、随机对照改为同形状打乱成员。
- zvec-rust M0 spike 有根据（Windows MSVC CI、预编译 C API、fp16、HNSW/IVF-RaBitQ）；不达标走 `hnsw_rs` 也不影响上层。
- §11 五件事给了备选，不是口头「已定」。

### 对 M0 的含义

**开 M0 前必须改文档（否则骨架会做错）：**
1. Issue 1：指纹算法、语料 URI/revision/文档数、`documents` DDL。没有这些，「指纹一致」是空话。
2. Issue 2–3：crate 树与非目标对齐。M0 不要生成 `vgi-sandbox` / `vgi-store` / `vgi-mcp` / 云 Milvus 第二实现；写明本机-only。
3. Issue 4：不要在骨架注释里把「四默认工具 + 托管 rg + 树」写成已定 zg 对齐。M0 不实现工具面，但树会决定以后几个 crate 是否存在。
4. 文档计数锁死（Issue 14 的 100,194 vs 100,195），写进指纹。

**可等对应里程碑再补（不要挡 zvec spike / 摄取）：**
- Issue 5 的 RRF/rg/locator：M1 开工前必须锁；M0 若只建 documents 表可暂缓 passages。
- Issue 6 的 M1 数字口径、Issue 12 的嵌入费用：第一次全量 embed 前。
- Issue 7 的官方索引 oracle 与词法引擎：M2 前。M0 不要开始写倒排。
- Issue 8 的 M3 门：实现树之前必须改写；建议现在就删「优于根覆盖」，以免当成已定。
- Issue 9–10、Issue 13：M4/M4b。更干净的是把 §7 整章标为非默认。
- Issue 11 字段 boost：M2 前先做字段统计。

M0 仍建议做、且稿里已经站得住的部分：独立 workspace 的最小切片、Parquet→SQLite 文档表、XLM-R 真 token 计数、zvec-rust 在 Windows 上的插入/查询/内存 spike。把「可开 M0」收窄到这四项，不要连带 12 crate 和三条改造的验收门一起开。
