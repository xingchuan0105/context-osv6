# VGI-RAG Rust 重写设计（zg 对齐版）

> **SUPERSEDED 2026-09-14.** 现行规格是设计文档 DAG：[docs/vgi/](../vgi/README.md)。本稿是评审对象，保留对照。评审：[zg 对齐设计评审](../reviews/2026-09-13-vgi-rag-rust-zg-aligned-design-review.md)。

日期：2026-09-13。承接：[Challenge-20 树消融 run-9 分析](2026-09-13-vgi-rag-challenge20-tree-run9-analysis.md)、[语义树构建评审](../reviews/2026-09-13-challenge20-semantic-tree-build-review.md)、[BM25 评审](../reviews/2026-09-13-challenge20-bm25-review.md)、[检索最佳实践调研](../research/2026-09-13-semantic-tree-building-best-practices.md)（含 zvec-grep 案例与文档级向量估算）。

**一句话**：把 VGI-RAG 从 Python 原型重写为 Rust 本地工具——架构对齐 zvec-grep（zg）：**统一检索层（向量＋BM25＋ripgrep）＋宿主 agent 循环＋单一检索工具＋紧凑带出处的结果**；同时保留三条既定改造：**文档级向量（大块均分）**、**段落级 BM25＋字段 boost＋打分优化**、**语义树 v2（可导航、可路由、指标可验）**。语义树负责"看懂整个库"，检索负责"找到证据"，rg/read 负责"核对原文"。

本稿是设计文档，供评审；未开始实现。

## 1. 背景与设计输入

- **run-9 的教训**：有循环、无排序检索面；树不可导航（形状/标签/代表缺陷）；grep 全库盲扫 26–45 秒且 26–52% 超时；5 轮预算过窄。结论：缺的不是循环，是循环里的排序检索面与可迭代预算。
- **zg 的形态**（参照物，非代码复用）：进程内 C++ 引擎（zvec）＋ TypeScript CLI/MCP；默认只暴露一个 `search` 工具（hybrid：向量+BM25+FTS，`fuse`）；精确查找交给宿主原生 rg；工具描述写"两段路由、一次探针即停、freshness 随结果"；索引增量更新；CLI 与 daemon 双模式。其 BrowseComp-Plus 配对基准：质量持平、input token −37.6%、工具调用 −43.5%、耗时 −38.6%。
- **既有数字**（沿用，不重算）：语料 100,195 篇 / 3.24B 字符；文档级向量 bge-m3（8,192×90%＝7,372 token 上限，逐篇 `ceil`）＝176,438 条，fp16 0.36 GB；qwen3.7-text-embedding（128K）＝101,467 条，fp16 0.21 GB。BM25 现状（文档级）recall@100 11.1%、@1000 27.5%；官方稠密 0.6B 为 26.4% / 59.7%。

## 2. 目标与非目标

**目标**
1. 单机（Windows 优先）Rust 工具：索引 10 万文档级语料；一个进程内统一检索层；MCP 与 CLI 双入口。
2. 循环内提供**排序检索工具**（向量＋BM25 融合、紧凑带出处）＋**语义树工具**（总览/下钻/查询路由）＋**rg/read**（精确核对）。
3. 三条改造全部落地并有指标：文档级向量、段落级 BM25、语义树 v2。
4. 可评测：索引级指标先行，agent 级 A/B 对齐官方协议；预算与资源指标（调用/token/耗时）一并报告。

**非目标**
- 不做通用工作区搜索产品（服务对象是固定大文档库 + 评测）；
- 不接 SAC/产线 Rust 栈（保持独立工程）；不引入向量数据库服务器；不做 Web UI（CLI/MCP 优先）；不复刻 zg 的代码。

## 3. 总体架构

```
vgi-rs/                       ← 独立 Rust workspace（替代 Python 原型，原型冻结作对照）
├─ crates/
│  ├─ vgi-core      语料/配置/指纹/schema/错误；结果与证据数据模型
│  ├─ vgi-ingest    摄取：Parquet/SQLite → 文档与 passage 表；doc token 计数
│  ├─ vgi-embed     分词器（HF tokenizers，XLM-R）× 大块均分 × 提供商 HTTP × fp16 向量写入
│  ├─ vgi-lexical   段落级 BM25：字段 boost、打分变体、参数；倒排索引
│  ├─ vgi-search    统一检索：向量＋BM25＋RRF 融合、过滤、结果压缩与出处
│  ├─ vgi-tree      语义树 v2：构建/标签/代表/路由/摘要（可选）
│  ├─ vgi-read      原文窗口读取（chunk 级，保留引用语义）
│  ├─ vgi-tools     工具实现（CLI 与 MCP 共用）
│  ├─ vgi-mcp       MCP 服务（rmcp，Streamable HTTP，loopback+可选 Bearer）
│  ├─ vgi-sandbox   代码模式沙箱：Python 子进程 + HostBridge（fd 管道 RPC，§7）
│  ├─ vgi-cli       `vgi index|query|tree|read|rg|status|install`
│  └─ vgi-eval      评测 harness：agent 循环/预算/裁判/指标/工件
└─ data/            ← 索引与语料（默认 `<root>/.vgi/`）
```

**进程模型**（对齐 zg）：`direct`（一次性命令，进程内建/开索引）与 `server`（常驻 daemon + 队列 + 模型池）两种模式；MCP 默认挂 daemon。

**数据目录**

```
.vgi/
├─ manifest.json          # 版本、指纹（corpus/questions/index/model）、时间戳、freshness
├─ corpus.sqlite          # documents / passages / 字段（heading、locators、token 数）
├─ vectors.f16            # 文档级向量（memmap；维度与批次由 manifest 锁定）
├─ bm25/                  # 段落级 BM25 自研倒排（vgi-lexical；无额外引擎）
├─ tree.bin               # 语义树（节点、标签、代表、路由质心）
└─ jobs/                  # 后台任务（增量更新/摘要）
```

## 4. 检索面设计（对齐 zg 的三路统一）

### 4.1 文档级向量（保留既定改造）

- **粒度**：一篇文档 = 若干"大块"向量；查询时文档得分 = 各块相似度的 **max**（后续可实验 exp-sum）。
- **切批**：真 token 计数器（bge-m3 = XLM-R 分词器），上限 `floor(8192 × 0.90) = 7372`；`N = max(1, ceil(tokens / 7372))`；相邻批重叠 256 token（≈3.5%）；超长文档设批数上限（默认 64，超出截断并记录）。
- **存储**：fp16、1024 维 → 176,438 条 ≈ 0.36 GB；可选 MRL 256 维再 ÷4。qwen 128K profile 为 101,467 条 ≈ 0.21 GB。
- **实现**：`vgi-embed`（reqwest 调 OpenAI 兼容端点，复用现有 bge-m3 在线配置；令牌桶限速、断点续跑、逐条记账）；向量写入 memmap 文件，UID = `doc_id#batch`。
- **检索**：进程内 ANN（见 4.5 决策）取 top-k 块 → 按文档聚合 → 返回文档级候选与命中块位置。

### 4.2 段落级 BM25（对齐最佳实践）

- **检索单元**：~512 token 的 passage（重叠 64–128 token），**不是**现有 ~100–200 字符小块、也不是整篇文档；passage → 文档聚合（max 或 exp-sum）。预计 ~160 万 passage（810M token / 512）。
- **字段 boost**：body / heading / url-path 三字段加权（BM25F 近似），标题与 URL 高权重——这类题的答案常出现在标题和路径里。
- **打分与参数**：`lucene` 为默认，`bm25l` / `bm25+` 进 A/B（长 passage）；k1/b 在 dev 网格搜索；查询侧支持多查询组 + `fuse`（RRF，k=60）。
- **查询处理**：agent 发短查询（工具描述明确引导）；预留 RM3/PRF 钩子；评测同时记录"原始问题"与"agent 查询"两条口径。
- **指标**：recall@10/100/1000、nDCG@10（对齐官方协议口径）。
- **实现**：**自研倒排索引（vgi-lexical）**——段落级 postings + 字段长度统计，压缩存储（delta+varint，可 mmap）；**不引入 tantivy 等额外引擎**；zvec 只承担向量。

### 4.3 精确检索与阅读

- `vgi_rg`：托管 ripgrep（作用域、超时、输出上限、字节预算），是"验证路径"；全库扫默认禁止，必须带 scope（doc/目录/passage 邻域）。
- `vgi_read`：按 passage/chunk 窗口读取（`offset/limit`、上下文行），保留现有 chunk 引用（`chunk_id`、offset、locator），供回答引用。

### 4.4 融合与结果压缩

- 向量与 BM25（自研倒排）两路各自返回有序列表 → RRF（k=60）融合 → 截断 → **紧凑输出**：按文档分组、passage 偏移、命中片段（截断长度可配），附 fingerprint/freshness。与 zg 一致：给 agent 的是"可定位的证据"，不是整篇正文。

### 4.5 引擎与索引实现（已定）

| 组件 | 本机（默认） | 云端 | 说明 |
|---|---|---|---|
| 向量 ANN | **zvec-rust**（对齐 zg 引擎；HNSW/IVF-RaBitQ、fp16、Windows 预编译） | **Milvus**（vgi-store 第二实现，沿用 context-osv6 云端栈） | M0 spike 验证 zvec API、批量插入、过滤、内存与 Windows 构建 |
| BM25/全文 | **自研倒排（vgi-lexical）**：段落级、字段 boost、k1/b 与变体全部自控 | 同一自研实现（随服务）| **不引入 tantivy 等额外引擎**；zvec 原生 FTS 仅当其暴露所需打分参数时可作为可选加速，不构成依赖 |
| 融合 | 自写 RRF | 同 | 保持与 Python 原型一致的 k 与权重口径 |

选型背景（zvec / qdrant / milvus / pgvector 对比与"本机轻负载、云端高并发"两层策略）见 §11.1。

## 5. 语义树 v2（保留既定优化）

**定位**：帮助"看懂整个库"的导航与路由结构，不是检索主力。工具 `vgi_tree`：`overview(node_id?)` / `list(node_id)` / `route(query)`。

- **构建**：在**文档向量**上做受约束的二分聚类（bisecting k-means）：fanout=8、叶容量 24–40、`depth ≤ 4`（导航预算 = 深度 + 检索 + 阅读 + 回答）；构建后自动验收形状（叶容量分布、平均分枝、深度分布），不达标即失败。
- **标签**：c-TF-IDF（停用词/长度过滤 + IDF）取节点主题词；对内部节点可选生成 LLM 摘要（约 2–3 千个节点，分期实施）。
- **代表文档**：最近质心文档；语料无标题时给出"doc_id + top 词 + 首行片段"组合卡。
- **查询路由**：`route(query)` 返回按质心相似度排序的节点（解决 run-9"树不可用"的核心缺陷）；agent 可用它选枝后再在节点内搜索。
- **对照**：随机树改为"同形状打乱成员"；保留节点级指标（证据定位 recall@k、最小覆盖节点大小）作为构建期验收。
- **指标门**：标签 stop-word/digit 占比 < 10%；叶容量 p50 ≥ 20；证据可定位到 top-k 节点的比例显著优于 run-9 的根覆盖。

## 6. Agent 循环、工具描述与预算（对齐 zg）

**循环归属**：VGI 不实现循环；由宿主 agent（评测时为自有 harness，产品上是 Claude Code/Codex 等）驱动。VGI 提供工具与描述。

**默认工具集（MCP）**：4 个——`vgi_search`（hybrid 排序，唯一默认检索入口）、`vgi_tree`（概览/下钻/路由）、`vgi_rg`（精确验证）、`vgi_read`（原读）。默认不暴露 index/status 等管理工具（`full` 工具集可选）。

**工具描述规范**（照 zg 的三条）：
1. 两段路由：先判断"是否需要在本地库取证"，再选"语义（wording 未知）/精确（锚点已知）"；
2. 一次探针、不相关即停；精确引用/键名/文件名/正则走 rg；
3. 状态随结果：返回带 fingerprint、freshness、命中位置的紧凑证据；错误可行动。

**预算（harness 配置，挑战赛默认）**：`max_rounds 12–16`、`max_tool_calls 32–48`、每题墙钟上限（如 15 分钟）、并发 2/组、总并发 8；closeout 保留"预算用完、基于已读原文作答"语义。目标调用经济性：~10–15 次/题（对照：官方 ~12.6 检索调用、zg 基准 14.36 工具调用、run-9 ~6.3 次且 0 次排序检索）。

**两种检索模态并存**：① 工具调用（zg 形态，本节）；② **代码模式（SAC 形态，见 §7）**——agent 写检索代码，一次多路、多步。profile 切换（`interaction: tools | code | both`），两模态共用同一检索层与同一预算核算口径。

## 7. 检索的代码模式（SAC 风格，新增）

**思路**：agent 不做逐次 tool call，而是**写一段检索代码**——可以同时发起多路检索（向量/BM25/rg/树），也可以把多步逻辑（先搜→筛→去重→回读）写进一次执行。代码在沙箱里跑，检索原语经"检索桥"回到宿主执行（宿主持有索引与 scope），结果压缩后只回传最终证据。

**参照现成形态**（context-osv6 已落地，ADR-0009）：沙箱 codegen → 宿主 **fd 管道 RPC**（fd3/fd4，行分隔 JSON，不开网络端口）；宿主**单入口**强制 scope 后统一派发；Python shim 由 SDK 原语**注册表单源 codegen**；沙箱检索经 captured calls 回流宿主，供引用与降级组装。`code-interpreter` crate 已有 Python 子进程、内存/CPU 限制、Windows 捆绑 Python 的先例可抄。

**为什么对 VGI 成立**：run-9 证明"5 轮 × 盲 grep"不可行——工具模式下多路检索与多步过滤会迅速吃光轮次；代码模式把"多路并发 + 多步逻辑"折叠进一次执行，模型上下文只承担最终证据。对 10 万文档级语料，这是比逐次调用更自然的交互面。

**vgi-rs 设计**
- crate：`vgi-sandbox`。宿主（Rust）持有语料/索引/scope；子进程执行代码（Python 优先；Windows 用捆绑或 PATH 发现，沿用 code-interpreter 的解析策略）。
- 传输与协议：fd 管道行分隔 JSON RPC（对齐 ADR-0009）；沙箱无网络、只读语料。
- 原语集合（v1，保持极简）：`search(query, route=hybrid|vector|bm25|tree, top_k)`、`tree(overview|node|route, ...)`、`rg(pattern, scope)`、`read(doc, offset, limit)`；每次调用可带 scope 与配额字段。
- SDK：由注册表 codegen 出 Python shim（`vgi_sdk`），签名与返回形状单源生成，模型代码不手写协议。
- 沙箱约束：内存/CPU/墙钟/输出上限与单次执行调用上限（**防失控保护，不作为循环预算**）；确定性（同代码 + 同索引 = 同结果）。
- 预算模型：**代码执行不消耗 agent loop 预算**——循环预算（`max_rounds` / `max_tool_calls`）只计 LLM 轮次与工具动作；沙箱执行及其内部检索调用**不计入**循环预算，只做独立的资源核算（token/调用/耗时，用于报告）与保护性上限。closeout 语义不变。
- 结果契约：代码返回结构化证据列表（doc/passage + 出处 + 可选中间说明）；宿主压缩后进模型上下文。
- 失败处理：语法/运行时错误作为**可行动错误**回传（含行号与修复提示）；超时/越界记基础设施事件，不污染质量分。

**与工具模式的关系**：两模态并存，profile 选择（`interaction: tools | code | both`）。简单定位题工具模式更省；多跳/全景题代码模式一次多步更省——M4 以同题、同模型、同预算做三臂 A/B（tools / code / both）量化。

**评测新增维度**：步数、步内调用数、代码失败率（语法/超时/越界）、代码长度、每题 token/耗时；质量仍走盲评。

**风险**：① 模型写代码的质量与调试成本；② Windows 下的 Python 依赖（捆绑 vs 系统）；③ 沙箱安全（无网络、只读挂载、资源限制）；④ 公平性——两模态必须固定原语集合与配额，否则不可比；⑤ 可复现性——代码与桥调用全量留痕。

## 8. 评测设计

1. **索引级（先做，无 agent）**：向量 recall@k、BM25 recall@k、hybrid RRF recall@k；口径对齐官方（@5/100/1000、nDCG@10）；目标：hybrid @1000 超过稠密单路（≥60%），@100 ≥30%。
2. **结构级**：树形状与节点级定位指标（构建期验收，见 §5）。
3. **Agent 级 A/B**：Challenge-20 20 题、单重复起步；盲评沿用原生 Eval v2 rubric（同族裁判的局限照记）；对照臂 = run-9 grep 臂（同预算复跑）与 zg 形态（排序检索+rg/read）；**模态三臂** = tools / code / both（同模型、同题、同预算）。
4. **资源指标**：input token、工具调用、墙钟、每题检索延迟分布；与质量分列报告，不混算。
5. **工件**：`runs/<id>/`（profile、trials、requests.jsonl、usage.json、analysis.json）+ 证据回执（沿用现约定）。

## 9. 里程碑与验收门

| 里程碑 | 内容 | 验收门 |
|---|---|---|
| M0 | workspace 骨架；摄取（复用已下载语料）；doc token 计数；zvec-rust spike（API/吞吐/内存/Windows） | 同语料可重建、指纹一致；zvec 本机验证通过 |
| M1 | 文档级向量流水线（切批/重叠/fp16/memmap）+ 向量检索 + rg/read | 向量 recall@100/1000 ≥ 官方稠密基线；断点续跑可用 |
| M2 | 段落级 BM25 自研倒排（字段 boost/参数/变体）+ RRF 融合 + 紧凑输出 | hybrid ≥ 单路最优；检索 p95 < 300 ms |
| M3 | 语义树 v2 + `vgi_tree`（含 route） | 形状/标签验收通过；节点级定位指标显著优于 run-9 |
| M4 | MCP 服务 + 工具描述 + eval harness（预算） | Challenge-20 A/B 完成；质量与资源双指标出报告 |
| M4b | 代码模式：`vgi-sandbox` + HostBridge + SDK + 预算模型 | tools/code/both 三臂 A/B 完成；失败类型、成本与质量入册 |
| M5 | 加固：增量更新/freshness、Windows 打包、CLI 安装器（对齐 zg `install` 体验）、文档 | 冷启动/增量验收；回归全绿 |

## 10. 风险与未决

- **zvec-rust 成熟度**（绑定覆盖、API 面、Windows 预编译）→ M0 spike；不达标走 `hnsw_rs` 备选（纯 Rust、无额外服务），不影响上层接口。
- **自研 BM25 倒排的工程面**（postings 压缩、增量/mmap、约 160 万 passage 的构建时间与内存）→ M2 验收；zvec 只承担向量，不承担 BM25。
- **passage 重切分**与现有 chunk/证据对齐：evidence 是文档级 ID，风险低；回答引用仍走 chunk 级 read。
- **超长文档**（最长 996 万字符）：文档级向量按 64 批上限截断，检索侧 passage 化即可覆盖尾部；必要时后续加"尾段摘要"。
- **树摘要的 LLM 成本**：内部节点约 2–3 千次调用，一期可只用 c-TF-IDF 标签，摘要作为可选增强。
- **裁判同族**局限与预算（token/费用）在 M4 前单独授权。
- **代码模式沙箱**：Python 依赖（Windows 捆绑 vs 系统）、安全与两模态公平性 → 见 §7 风险清单；M4b 验收。
- **关键决策**：5 项已定（见 §11）；引擎组合（本机 zvec＋自研 BM25；云端 Milvus 继续）与项目落点（独立 `vgi-rs`）已确认，可开 M0。

## 11. 关键决策记录（5 项，均已定）

### 11.1 引擎组合（已定：本机 zvec、云端 Milvus；对比如下）

**量级参考**（公开对比测试：单机 16 vCPU / 32 GB，1M×1536 维 HNSW；Milvus 官方最小部署文档；zvec 官方 VectorDBBench Cohere 1M/10M、16c64g、int8。硬件/维度/参数不同数字会变，只作量级判断）：

| 引擎 | 形态 | 部署依赖 | 索引/量化 | 并发模型 | 1M×1536 参考 | 运维 | Windows | 云扩展路径 |
|---|---|---|---|---|---|---|---|---|
| **zvec** | **嵌入式 C++ 库**（Rust/Node/Python/Go/Dart 绑定） | 无（进程内） | HNSW / IVF-RaBitQ / PQ-INT8 / DiskANN、fp16、WAL | 进程内多线程；多进程只读共享 | 数据量级内存（官方 16c64g 跑 Cohere 1M/10M int8） | 无 | ✓ 预编译 | 内嵌进自研服务 → 多副本；无分布式 |
| qdrant | Rust 独立服务，单二进制 | 服务进程（可选 Docker） | HNSW + 标量/PQ 量化、mmap 磁盘模式 | 多线程服务，gRPC/REST | ~5.1 GB；P99 28 ms @100QPS；插入 85K/s；构建 4 min | 中 | ✓ | 水平扩展（Raft）；1M–50M 甜点 |
| milvus | 分布式系统 | 独立版需 etcd + MinIO（+可选 Kafka/Pulsar） | 索引最丰富（含 DiskANN） | 各角色微服务 | ~7.2 GB + 依赖栈；P99 71 ms @100QPS；插入 120K/s | 高 | 仅 Linux 部署 | K8s Operator；100M+；context-osv6 云端即 Milvus |
| pgvector | Postgres 扩展 | 需 Postgres 实例（context-osv6 桌面捆绑 PG+pgvector） | HNSW / IVFFlat | PG 进程模型；100QPS 下 P99 287 ms（连接/锁竞争退化） | ~6.5 GB；插入 18K/s；构建 18 min | 低（复用 PG 工具链） | ✓（需装 PG） | PG 只读副本；<5M 向量、中等 QPS |

**VGI 的真实规模**：文档级向量 176k×1024 fp16 ≈ **0.36 GB**（qwen profile 0.21 GB）；段落 BM25 ~160 万 postings。任何引擎在容量上都过剩，**选型由部署形态决定，不是吞吐**。

**两层策略（对齐"本机轻负载、云端高并发"）**
1. **本机（默认）**：**zvec 嵌入式（向量）＋ 自研 BM25 倒排（vgi-lexical，无额外引擎）**——无服务、无外部依赖、数据量级内存、Windows 预编译、与 zg 同栈；Milvus Lite 排除（仅 Python、仅 FLAT、无 Windows）；pgvector 不采用。
2. **云端（高并发）**：**Milvus 继续**（沿用 context-osv6 云端栈）——`vgi-store` 定义向量/BM25 的最小 trait 与指纹格式，第二实现接 Milvus（向量 ANN + 持久化）；服务保持无状态多副本，BM25 仍跑自研实现，保证本机/云端同口径。Qdrant / pgvector 仅留档对比，不作为候选。

**决策**：本机 **zvec（向量）+ 自研 BM25（vgi-lexical）**，不引入 tantivy 或任何额外引擎；云端 **Milvus 继续**（`vgi-store` 第二实现）。zvec 原生 FTS 若达标仅作可选加速，不构成依赖。

### 11.2 项目落点

- **A 独立 `vgi-rs`**（与 Python 原型同数据分区）：保持独立工程；实验依赖不进产品仓库；与原型/旧成绩对照方便。代价：不能直接复用 `llm`/`retrieval-data-plane` 等 crate。
- **B 并入 context-osv6 workspace**：可复用既有 crate（llm、search、code-interpreter…）。代价：VGI 是评测原型，会把实验依赖与 CI 面拖进产品仓库；产品边界上 VGI 是评测工具而非产品线。
- **推荐 A**；通过 OpenAI 兼容 HTTP 与 JSONL 工件对接主仓库；若日后产品化，再把 `vgi-search`/`vgi-tree` 抽 crate 回灌。
- **已定（2026-09-13）：独立项目 `vgi-rs`。**

### 11.3 passage 尺寸与文档聚合

- 尺寸：**512 token**（定位更准，postings ≈1.6M）vs **1024 token**（索引更小 ≈0.8M，打分辨率更低）。
- 聚合：**max**（命中一处即可，锐利）vs **exp-sum**（奖励多处覆盖）vs 首段优先（弱先验，不推荐默认）。
- **推荐**：起点 512 token / 64 token 重叠 + 文档级 max；M2 在 dev 上跑 2×2 A/B（512/1024 × max/exp-sum），以 @100/@1000 与跨段证据覆盖定夺。
- **已定：按推荐执行**（512/64 + max 起步；A/B 用于验证与微调，不作为前置门）。

### 11.4 `install` 式 MCP 自动注入

- **A 提供 `vgi install --target codex|claude`**（对齐 zg：只写自己 marker 块、可卸载、权限规则最小化）。优点：评测环境一致、贴近真实用户路径；风险：宿主配置格式漂移（先锁两个目标）。
- **B 只提供配置片段与文档**：零侵入，但评测手工配置易漂移、不可复现。
- **推荐 A**（先 Codex + Claude Code），安装器必须幂等、有 `uninstall`、不触碰非自己管理的配置。
- **已定：做**（先 Codex + Claude Code 两个目标）。

### 11.5 默认 embedding profile

- **A bge-m3**（8,192 / 1,024 维）：现网已就绪、便宜；文档级切批 176,438 条 ≈ **0.36 GB**（fp16）。
- **B qwen3.7-text-embedding**（128K / 1,024 维，百炼）：101,467 条 ≈ **0.21 GB**；98% 文档一段装下；需要凭证、网络与费用。
- **推荐**：manifest **锁定 profile**、双 profile 并存、默认 A；M4 用 B 跑一组对照（只换 embedding）量化"窗口大小 → 质量/成本"。
- **已定：双 profile 并存、默认 bge-m3；M4 用 qwen 跑对照。**
