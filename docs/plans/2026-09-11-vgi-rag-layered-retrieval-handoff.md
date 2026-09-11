# VGI-RAG 分层检索与模块化消融交接（阶段二）

更新：2026-09-11（Asia/Shanghai）。承接 [VGI-RAG 原型与 Hard-60 评测交接](2026-09-11-vgi-rag-handoff.md)：那一份覆盖六组接口、Hard-60 材料与 SAC 适配；本文件只记录本轮新增的**实测结论、路线 A/B 取舍、目标架构与模块化消融设计**。前序文档关于材料、接口、边界的描述仍然有效；两处数字口径冲突时以本文件为准（本文件数字来自本轮复算）。

术语：**路线 A** = 入库时全量 LLM 抽取（摘要 + 三元组）；**路线 B** = 检索调用时顺带抽取（只抽种子文档）。

## 1. 先读结论

1. **六组开发集已跑通**（A–E 本地 Agent，F 走产线 SAC；两侧模型统一 `qwen3.8-flash`）。**在 248 份语料 × 20 道开发题上，尚未测出图＋树相对强混合检索的比较优势**——原因不是"图做错了"，而是这个规模没有头寸：100 块种子的文档池在 248 份语料里已覆盖 40%，在 10 万份语料只覆盖 0.1%。
2. **不要放弃向量。** @15 证据覆盖：在线 dense 63.6% vs 本地 TF-IDF 46.7% / LSA 40.2%；混合 BM25+在线向量 66.4% vs BM25+本地 56.1%。放弃向量稳定损失约 10 个百分点。
3. **现有相似度图无效，而且从未真正进入检索路径。** embedding 块 kNN 图相对随机对照 +0.3~−0.7；窗口质心图 −1；`HardAgent.expand_seed` 的图分支只调 `annotate()` 找种子自身窗口，从未遍历边。**与检索器同空间的边不携带新信息。**
4. **换构法有正增益，且必须进排名。** TF-IDF 文档余弦图（零 LLM、零抽取）文档级比随机对照 +2.3~+4.8；作为**第三路融合通道**（RRF，w=0.5）top-15 证据 69→73（64.5%→68.2%）。只当候选过滤器时，显示层增益为 0。
5. **最大单步收益不是图：** 显示名额 15→30，20 题合计 +10 证据，超过本轮所有图结构实验。
6. **全局理解交给语义树**（复用现有向量分层聚类 + TF-IDF 簇标签 + 代表块，零模型调用）：同文档/跨文档余弦可分 0.504 vs 0.328，标签可读，读全景约 320 tokens。**不需要逐篇 LLM 摘要，也不需要 LLM 抽三元组。**
7. **多跳走"查询时抽取"（路线 B）：** 入库全量抽 149,709 文档 × 5,723 tokens ≈ 857M；查询时只抽种子 1,000 次 × 20 篇 ≈ 44M，打平点约 13,000 次查询。抽取风格不敏感（统计抽词 +3.4、抽专名 +3.4），真正起作用的是"用现有检索器全库跳跃"这个动作。
8. **四层分工**：向量负责语义匹配、语义树负责全局理解、结构边（URL/目录/符号/日期）负责层级与连接、路线 B 负责多跳。四者互不替代，全部可开关（见第 5 节）。

## 2. 本轮实测结论（数字口径）

| 结论 | 数字 | 状态 |
|---|---|---|
| 收窄必须强制 | 可选时 24 次检索 0 次使用 ranges；改强制后成为固定步骤 | 已测 |
| 回读走 grep 最优 | 6.22M vs 7.52M tokens（−17%）；墙钟 3.5 vs 3.4 min；质量均值 1.938 vs 1.875 | 已测（四配置盲评） |
| 在线向量不可替代 | @15：BM25 47.7% / 在线 dense 63.6% / 本地 TF-IDF 46.7% / LSA 40.2%；混合 66.4% vs 56.1% | 已测 |
| 相似度图无效 | embedding 块 kNN +0.3~−0.7 vs 随机对照；窗口质心 −1 | 已测 |
| 图从未进检索路径 | `expand_seed` 图分支只用 `annotate()`，未遍历边；20 题图池新增文档为 0 | 已测 |
| TF-IDF 文档图有效 | 文档级 +2.3~+4.8 vs 随机对照（4–5 倍） | 已测 |
| 图作融合通道才进 top-15 | 69→73（w=0.5）；w=1.0/2.0 → +2/+1；related-only −2 | 已测；权重在同批 20 题上选定，待保留集 |
| 显示名额 15→30 | 20 题合计 +10 证据（71→81） | 已测 |
| 三元组桥机制成立但当前无头寸 | 文档级 62/65 → 一跳 65/65；种子覆盖 40%（248 份）vs 0.1%（10 万份） | 前轮已测 |
| 树从未被真正测到 | 材料化丢结构：248 份仅 12 份有真实章节 | 待修 |
| 语义树零成本可用 | 可分 0.504 vs 0.328；16 簇标签可读（例：`mcavoy / sloane / sabbith` 指向 The Newsroom 主题簇）；全景 ~320 tokens | 已测（未端到端跑 Agent） |
| 免费信号充足 | BrowseComp 133 份：URL 133、date 131、title 130、author 63；FreshStack 54 份：URL 54、6 仓库、路径深 6、400 行标题、33 份含代码定义 | 已盘 |
| 摘要可免费替代 | 抽取式首块与其余块词重叠中位 57%；显著专名 top-3 可读；成本 0 vs LLM 5,723 tokens/篇 | 已测（代表性，非下游质量） |
| 路线 B 成本 | 857M（入库全抽）vs 44M（1,000 次查询抽种子）；打平 ~13,000 次查询 | 已测（成本模型） |
| 路线 B 机制 | 抽词 +3.4、抽专名 +3.4（均 vs 随机对照）；单题扩张 10–12 份文档 | 已测（文档级） |

## 3. 未验证项（不得当结论）

1. **语义树的 overview/zoom 工具端到端是否有用** —— 聚类与标签已验证，Agent 会不会用、用了是否提升未跑。
2. **路线 B 关联分作第三路融合能否改善 top-15** —— TF-IDF 图这样用有 +4，三元组路径未测；这是路线 B 能否落地的唯一关卡。
3. **融合权重 +4 的稳定性** —— 权重在同批 20 题上选定，未在保留集验证。
4. **带查询抽取 vs 无查询抽取** —— 前者是路线 B 的设计主张，尚无 A/B。
5. **真实 LLM 抽取质量** —— 本轮用统计等效替代，真实抽取的置信度与噪声未测。
6. **248 份语料上的一切图结论都可能随规模改变** —— 目标规模 10 万份时需全部重测。
7. **F 组 token 计量口径** —— 服务端汇总用量可能不覆盖内部请求，需服务端独立计量确认。

## 4. 目标架构

```text
查询
 ↓
[1] 语义发现   在线 embedding，top-100 块                    ← 不可替代（+16 点）
 ↓
[2] 收窄（强制；各通道独立开关）
     ├ 语义树下钻（簇 → 文档）                               ← 全局理解，零模型
     ├ 结构边（URL/域 → 目录 → 代码符号+PageRank → 日期）     ← 作者显式信号，零成本
     └ 路线B：种子条件抽取 → 检索器跳跃                       ← 多跳，零入库全量抽取
 ↓
[3] 融合       BM25 + dense + related(+jump) 多路 RRF         ← 扩张必须进评分
     显示名额 30
 ↓
[4] 作答       Agent 循环；回读走 grep 全文
```

| 需求 | 机制 | 关键边界 |
|---|---|---|
| 语义匹配 | 在线向量 | 不可用词法替代 |
| 全局理解 | 语义树（聚类+TF-IDF 标签+代表块） | 替代逐篇 LLM 摘要 |
| 层级定位 | 结构边（目录/标题/符号） | 依赖材料化修复 |
| 跨文档连接 | TF-IDF 文档图 / 结构边 | **必须与检索器不同空间** |
| 多跳 | 路线 B 抽取+跳跃 | 默认 1 跳，成本随跳数指数增长 |

## 5. 模块化与消融设计（本轮核心）

设计目标：**每一路都能通过配置单独关闭**，消融不改代码、不按臂分叉实现；每条试验记录自描述配置，事后可直接从回执还原臂定义。

### 5.1 四条原则

1. **单一开关面。** 所有可消融项集中在一个 `RetrievalConfig`，随试验记录序列化。
2. **模块不出圈。** scope 校验、越界审计、`integrity_error`、`infra_error` 都在 runner 层完成；模块不得扩大检索范围，也不得把失败记成零分。
3. **无隐式回退。** 模块关闭或无结果 → 贡献为空集，不报错、不降级、不静默兜底。
4. **一路一开关。** 每个消融臂只允许改一个字段；组合臂显式列出全部改动。

### 5.2 配置对象（示意）

```python
@dataclass(frozen=True)
class RetrievalConfig:
    # 发现层
    discovery: str = "online_dense"       # online_dense | bm25 | tfidf | lsa
    discovery_k: int = 100                # 种子块数
    # 收窄层（核心强制；以下各通道独立开关）
    narrow: bool = True                   # 关闭 = 退回全库直接检索
    tree_depth: int = 2                   # 0 = 关
    graph: str = "none"                   # none | tfidf_doc | embedding_knn | window_centroid
    structural: tuple[str, ...] = ()      # 子集 ("url", "dir", "symbol", "date")
    route_b: bool = False
    route_b_hops: int = 1                 # 默认 1；2 跳成本指数增长
    route_b_seed_docs: int = 20
    route_b_keys_per_doc: int = 8
    # 融合层
    channels: tuple[str, ...] = ("lexical", "semantic")
    weights: dict[str, float] = {"lexical": 1.0, "semantic": 1.0}
    rrf_k: int = 60
    display_limit: int = 30
    # 作答层
    readback: str = "grep"                # grep | preview
    extractor_version: str = "v1"         # 抽取缓存键的一部分
```

### 5.3 模块契约

```python
class NarrowingSource(Protocol):
    name: str
    def propose(self, query, seed_rows, scope, index) -> Proposal: ...
        # Proposal.pool: set[chunk_id]            新增候选
        # Proposal.boosts: dict[chunk_id, float]  关联分（可空）

class RankingChannel(Protocol):
    name: str
    def rank(self, query, candidates, boosts) -> list[chunk_id]: ...
        # 只做排序，不改变候选集合
```

- `expand_seed` 从"返回集合"改为"返回 `(pool, boosts)`"：遍历启用中的 `NarrowingSource`，池取并集，boosts 逐块取 max（避免 hub 主导）。
- 融合是通用 RRF：`rrf([c.rank(...) for c in enabled_channels], weights)`。启用/关闭一路 = 在 `channels` 里增删一个名字。
- 对现有接口的最小改动：`expand_seed` 签名 + `search` 接受 `boosts` 并把它并入 RRF。

### 5.4 消融矩阵（新配置面 ↔ 旧六组）

| 臂 | 配置 | 说明 |
|---|---|---|
| E 平铺 | `channels=("lexical","semantic")`, `narrow=False`（仅平铺）, `display_limit=15` | 现有 E |
| C 去树 | `tree_depth=0` | 现有 C |
| B 去图 | `graph="none"` | 现有 B |
| A 全开 | `tree_depth≥1`, `graph="tfidf_doc"` | 现有 A（换新图构法） |
| +显示 | `display_limit=30` | 本轮 +10 |
| +图融合 | `channels += ("related",)` | 本轮 +4 |
| +结构边 | `structural=("url","dir","symbol","date")` | 待测 |
| +路线B | `route_b=True`, `channels += ("jump",)` | 待测 |

旧 A/B/C/E 从"按臂分叉代码"改为"同一实现的两组开关"；F/D 不变（工具集不同，不走本配置面）。

### 5.5 试验记录自描述

每条 trial 记录内嵌生效的 `RetrievalConfig`、启用模块列表、每路候选/贡献计数、`integrity_error`/`infra_error` 标记。回执脱离代码即可复现臂定义与审计。

## 6. 路线 B 细化设计（三元组导航）

**数据流（默认 1 跳）：**

```text
① 种子发现   在线 embedding top-100 块 → 聚合 ~20 份种子文档
② 带查询抽取 对每份种子文档抽三元组，prompt 带上原查询 → 只抽与问题相关的桥
③ 跳转键     从三元组取【种子实体集合之外】的实体/标识符（桥的定义）
④ 跳跃       用现有检索器（BM25/dense）全库搜索跳转键，受 scope 限制 → 不建新索引
⑤ 融合       跳跃结果带"关联分"作为第三路进 RRF → 再检索，top-30
```

**四个关键设计点：**

1. **抽取带查询** —— 路线 B 相对 A 的核心优势：同样的调用，只抽与问题相关的桥，精度高、成本低。
2. **跳转键 = 种子里没有的实体** —— `jump_keys = {t.object} - seed_entities`；种子里已有的跳过去回到原地。
3. **跳转用检索器，不建全库索引** —— 这是路线 B 不需要入库全量抽取的原因。
4. **关联分进 RRF（取 max 而非 sum）：**
   `relation_score(chunk) = max over (triple t, seed s) [ sim(query,s) × conf(t) × 1/hop × match(t.key, chunk) ]`

**边界条件：**

| 情况 | 处理 |
|---|---|
| hub 实体（全库高频） | IDF 加权 + 单实体产出上限 |
| 跳跃无结果 | 退回 lexical+semantic 两路，不报错 |
| 越界 | 受现有 `scope` 限制；越界记 `integrity_error`，不算分 |
| 抽取失败/超时 | 记 unknown / infra_error，不记零 |
| 多跳 | 默认 1 跳；2 跳显式开启 |
| 缓存 | 键 `(doc_hash, query_hash, extractor_version)`；跨查询复用 |

**审计链（保留）：**

```json
{"jump": "Literary Hub → 2015 年发布", "from_seed": "8314", "to_chunk": "30431:7",
 "path_score": 0.42, "source_triple_id": "..."}
```

**成本**：入库全抽 857M tokens vs 查询时抽 44M（1,000 次查询口径）；打平约 13,000 次查询。

## 7. 实施顺序与门槛

| 阶段 | 动作 | 成本 | 门槛 |
|---|---|---|---|
| 0 | 显示名额 15→30 | 一个常量 | 证据 71→81 |
| 0 | 材料化恢复结构（BrowseComp frontmatter；FreshStack markdown 标题 + 代码定义） | 正则 | 章节覆盖远超 12/248 |
| 1 | 语义树（分层聚类 + TF-IDF 标签 + 代表块；接 `overview`/`zoom` 工具） | numpy | 全景可读（~320 tokens）；Agent 会用 |
| 2 | TF-IDF 文档图作第三路融合（替换相似度图） | 哈希 TF-IDF | 开发集 top-15 提升 |
| 3 | 结构边（URL/域 → 目录 → 符号+PageRank → 日期） | 正则+图遍历 | 每类边超过随机对照 |
| 4 | 路线 B（种子抽取 + 检索器跳跃 + 缓存） | Q×k×c，可摊销 | 成本优于 A 且答案不差；top-15 提升 |
| 5 | 40 道保留题正式验证（2–3 次重复、第二判读者） | 计算 | 效应大于单次 ±50% 方差 |

阶段 1–2 共用 TF-IDF 素材，建议一起做。图类实验（阶段 3）在语料上量后再全面重测。

## 8. 明确不做

| 不做 | 依据 |
|---|---|
| 继续投相似度图 | 两种构法均无效（+0.3~−0.7） |
| 放弃向量 | 损失约 10 个百分点 |
| 入库全量 LLM 抽取 | 857M tokens，当前规模无收益 |
| 逐篇 LLM 摘要 | 抽取式首块词重叠 57% + 语义树负责全局 |
| 在 248 份语料上验证图 | 种子覆盖 40%，结构上无头寸 |
| 现在启用产线三元组 | 成本已付但非瓶颈；留到大规模 |

## 9. 文件、脚本与复现

### 9.1 原型仓库（`C:\Users\xingc\Documents\Codex\repository-tree`）

本轮分析脚本位于 `.eval/hard/`（**该目录被 Git 忽略，属本机产物，换机或清理会丢失；建议下一步把脚本迁到受版本控制的 `scripts/analysis/` 或 `eval/analysis/`**）：

| 脚本 / 产物 | 用途 |
|---|---|
| `signal_inventory.py` | 免费图信号盘点（URL/目录/标题/日期） |
| `fusion_test.py` | 图作第三路融合通道（+4 的出处） |
| `route_b_test.py` / `route_b_v2.py` / `route_b_debug.py` | 路线 B 抽取与跳跃（v2 为可用版本） |
| `summary_signals.py` | 摘要免费信号（首块/质心/显著专名） |
| `consolidate_plan.py` → `plan.json` | 本轮结论与计划的机器可读汇总（10 已测 / 6 未测 / 7 步） |
| `graph_control_test.py` / `tfidf_display_test.py` | 前轮图对照与显示层实验 |

数据与索引（均在 `.eval/hard/`，Git 忽略）：`hard60-index-v1/`（248 文档 / 7,228 块）、`hard60-v1/evaluation-only/questions.json`（20 开发 + 40 保留）、`kg-export/relations.jsonl`（3,658 关系，可全量映射）、`native-sac-binding.json`（248 份绑定）。六组回执 `evidence/hard60-six-arm-dev-run.json`。

```powershell
Set-Location -LiteralPath 'C:\Users\xingc\Documents\Codex\repository-tree'
.venv/Scripts/python.exe -X utf8 .eval/hard/fusion_test.py
.venv/Scripts/python.exe -X utf8 .eval/hard/route_b_v2.py
.venv/Scripts/python.exe -X utf8 .eval/hard/summary_signals.py
.venv/Scripts/python.exe -X utf8 .eval/hard/consolidate_plan.py
```

注意：这些脚本每题调用一次在线 embedding（仅查询侧），语料向量来自本地索引；会消耗少量在线额度。

### 9.2 主仓库（`/home/chuan/context-osv6`）

- `avrag-rs/.env`：F 臂 `AGENT_LLM_*` / `RETRIEVE_LLM_*` 已切到 dashscope `qwen3.8-flash`（备份 `.env.bak.pre-qwen-20260911-102436`）。
- 权威工具描述已更新（提交 `ee6a1564`、`c02bd4c5`）：收窄是固定步骤、发现/回读分工。
- 产线库：`rag_kg_entities` 4,818 / `rag_kg_relations` 3,658，覆盖 174/247 文档；成本已付，当前规模无头寸。

### 9.3 历史提交（原型）

`afddda5`（原型基线）、`e524dd7`（ranges + 强制管线）、`577c47a`（回读走 grep）、`7859a6f`（裁判）、`4debff9`（四配置盲评）。

## 10. 风险与遗留

1. **分析脚本未入版本控制**（`.eval/` 被忽略）——建议尽快迁移到受控路径。
2. **融合权重 +4 未经保留集验证**，且是在同批 20 题上选的。
3. **±50% 单次方差** —— 现有单次运行不足以判定小效应，保留集验证需 2–3 次重复。
4. **材料化结构性丢数据** —— 树维度此前 93% 不可测，修好前任何"树无效"的结论都无效。
5. **F 组 token 口径存疑**，产线成本结论需服务端独立计量。
6. **产线遗留**：markitdown 对弯引号 ASCII 解码崩溃（1 份文档未入库）；空钱包曾致 96 份死信（已充值重入队）；一个本地开发 JWT 曾打印到终端（建议轮换）；上传 URL 指向 8080 而实例监听 18091。

## 11. 接手第一步

按第 7 节阶段 0 开始：显示名额 15→30（一行）、材料化恢复结构（正则）；随后建语义树、接 TF-IDF 图融合。每个阶段的验收以门槛为准，不达标不进入下一阶段。
