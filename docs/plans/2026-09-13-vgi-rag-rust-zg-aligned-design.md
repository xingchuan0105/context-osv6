# VGI-RAG Rust 重写设计（zg 对齐版）

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
├─ bm25/                  # 段落级倒排索引（tantivy 或 zvec）
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

### 4.3 精确检索与阅读

- `vgi_rg`：托管 ripgrep（作用域、超时、输出上限、字节预算），是"验证路径"；全库扫默认禁止，必须带 scope（doc/目录/passage 邻域）。
- `vgi_read`：按 passage/chunk 窗口读取（`offset/limit`、上下文行），保留现有 chunk 引用（`chunk_id`、offset、locator），供回答引用。

### 4.4 融合与结果压缩

- 向量、BM25、FTS 三路各自返回有序列表 → RRF（k=60）融合 → 截断 → **紧凑输出**：按文档分组、passage 偏移、命中片段（截断长度可配），附 fingerprint/freshness。与 zg 一致：给 agent 的是"可定位的证据"，不是整篇正文。

### 4.5 引擎决策（M0 spike）

| 组件 | 首选 | 备选 | 说明 |
|---|---|---|---|
| 向量 ANN | **zvec-rust**（对齐 zg 引擎；HNSW/IVF-RaBitQ、fp16、Windows 预编译） | `hnsw_rs` / `usearch` + 自写融合 | spike 验证 API、批量插入、过滤与内存 |
| 全文/FTS | **tantivy**（字段 boost、BM25 参数可控、增量） | zvec 原生 FTS | 若 zvec FTS 暴露 BM25 参数与字段控制，可合并到 zvec 单一引擎 |
| 融合 | 自写 RRF | —— | 保持与 Python 原型一致的 k 与权重口径 |

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

## 7. 评测设计

1. **索引级（先做，无 agent）**：向量 recall@k、BM25 recall@k、hybrid RRF recall@k；口径对齐官方（@5/100/1000、nDCG@10）；目标：hybrid @1000 超过稠密单路（≥60%），@100 ≥30%。
2. **结构级**：树形状与节点级定位指标（构建期验收，见 §5）。
3. **Agent 级 A/B**：Challenge-20 20 题、单重复起步；盲评沿用原生 Eval v2 rubric（同族裁判的局限照记）；对照臂 = run-9 grep 臂（同预算复跑）与 zg 形态（排序检索+rg/read）。
4. **资源指标**：input token、工具调用、墙钟、每题检索延迟分布；与质量分列报告，不混算。
5. **工件**：`runs/<id>/`（profile、trials、requests.jsonl、usage.json、analysis.json）+ 证据回执（沿用现约定）。

## 8. 里程碑与验收门

| 里程碑 | 内容 | 验收门 |
|---|---|---|
| M0 | workspace 骨架；摄取（复用已下载语料）；doc token 计数；zvec-rust/tantivy spike | 同语料可重建、指纹一致；引擎决策落定 |
| M1 | 文档级向量流水线（切批/重叠/fp16/memmap）+ 向量检索 + rg/read | 向量 recall@100/1000 ≥ 官方稠密基线；断点续跑可用 |
| M2 | 段落级 BM25（字段 boost/参数）+ RRF 融合 + 紧凑输出 | hybrid ≥ 单路最优；检索 p95 < 300 ms |
| M3 | 语义树 v2 + `vgi_tree`（含 route） | 形状/标签验收通过；节点级定位指标显著优于 run-9 |
| M4 | MCP 服务 + 工具描述 + eval harness（预算） | Challenge-20 A/B 完成；质量与资源双指标出报告 |
| M5 | 加固：增量更新/freshness、Windows 打包、CLI 安装器（对齐 zg `install` 体验）、文档 | 冷启动/增量验收；回归全绿 |

## 9. 风险与未决

- **zvec-rust 成熟度**（绑定覆盖、FTS 打分可控性、Windows 预编译）→ M0 spike；不达标走 tantivy + hnsw_rs 备选，不影响上层接口。
- **passage 重切分**与现有 chunk/证据对齐：evidence 是文档级 ID，风险低；回答引用仍走 chunk 级 read。
- **超长文档**（最长 996 万字符）：文档级向量按 64 批上限截断，检索侧 passage 化即可覆盖尾部；必要时后续加"尾段摘要"。
- **树摘要的 LLM 成本**：内部节点约 2–3 千次调用，一期可只用 c-TF-IDF 标签，摘要作为可选增强。
- **裁判同族**局限与预算（token/费用）在 M4 前单独授权。
- **未决问题（请 review 时定夺）**：① 引擎组合（zvec 单引擎 vs zvec+tantivy）；② 项目落点：独立 `vgi-rs`（推荐，保持独立工程）还是并入 context-osv6 workspace；③ passage 尺寸 512 vs 1024 token、聚合 max vs exp-sum；④ 是否保留 `install` 式 MCP 自动注入（便于评测，但侵入宿主配置）；⑤ bge-m3 vs qwen 128K 作为默认 embedding profile。
