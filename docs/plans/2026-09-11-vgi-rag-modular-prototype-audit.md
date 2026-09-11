# VGI-RAG 新原型：模块化设计与开发完成度检查

日期：2026-09-11。设计基准为 [阶段二交接](2026-09-11-vgi-rag-layered-retrieval-handoff.md)，实现基准为独立工程 `C:\Users\xingc\Documents\Codex\repository-tree` 的 `cae8d9c`。本次检查源码、入口、配置、测试回执和现存索引；不修改运行逻辑，不调用在线模型，不操作服务。

## 1. 结论与状态纠正

**新原型的模块化设计尚未闭合，模块化开发尚未完成。当前是旧原型与旧评测框架上的局部增强。**

上一轮完成的是 Hard-60 的 A/B/C/E 首轮发现从纯向量改为 BM25＋dense 的 RRF，并核验已有 ripgrep 回读。39 项通过证明这项局部改动的回归检查通过，不能作为统一配置、语义树、图融合、路线 B 或新产品入口完成的依据。

两条当前可执行路径需要分清：

```text
浏览器 / 公开 MCP
  → Repository.search
  → rank_search：关键词子串 + 向量 + RRF

Hard-60 本地评测 Agent
  → HardAgent.hybrid_search
  → BM25 + dense 种子发现
  → 旧章节 / 窗口范围扩张
  → 池内 BM25 + dense 排序
  → grep 工具调用 ripgrep，或 read 连续读取
```

阶段二目标中的“统一可配置管线 → 语义树 / 结构边 / TF-IDF 图 / 路线 B → 分路融合 → 服务与评测共同调用”，目前不存在完整的可执行实现。

## 2. 开发清单核对

| 能力 | 当前状态 | 核对结果 |
|---|---|---|
| 首轮 BM25＋在线向量发现 | 已实现于 Hard-60 | `HardAgent.seed_pool` 固定调用 hybrid；总种子上限默认 100 |
| ripgrep 原文取证 | 已实现于评测工具 | `FrozenCorpus.grep_ids` 启动真实 rg，结果映射回全文块与出处；公开 MCP 未暴露同一路径 |
| `RetrievalConfig` | 未实现 | 代码图与源码搜索均未找到；只存在于设计文档示意 |
| `NarrowingSource / Proposal / RankingChannel` | 未实现 | 没有模块协议、注册、装配和通用执行器；`expand_seed` 仍返回 set |
| 可配置发现 / 收窄 / 权重 / 回读 | 未实现 | 没有新的 CLI 配置入口；A/B/C/E 仍由 `ARMS` 布尔字段分支 |
| 通用加权 RRF | 未实现 | `BenchmarkRetriever.search` 只接受 bm25/dense/hybrid，k=60、等权写死，无 related/jump/boosts 参数 |
| 新语义树与 overview/zoom | 未实现 | 只有旧目录/章节导航；没有跨文档聚类树工具和运行期接线 |
| TF-IDF 文档图融合 | 仅分析原型 | `.eval/hard/fusion_test.py` 有独立实验算法，未接到 Agent 检索器 |
| URL / 目录 / 符号 / 日期结构边 | 未实现 | 有信号盘点脚本，没有统一的结构边索引、遍历和贡献通道 |
| 路线 B：抽取、跳跃、缓存、评分 | 仅统计诊断 | `.eval/hard/route_b_v2.py` 用词频/正则专名与 BM25 跳转；无真实 LLM 抽取、版本化缓存和 jump 融合模块 |
| 材料化恢复结构 | 未完成 | BrowseComp heading 写空；FreshStack heading 写整条路径；索引路径另被哈希成 txt |
| 显示名额 30 | 未接通为管线配置 | `--limit` 进入 Settings/profile 和 coverage，但未传入 Agent；工具默认 15 |
| 实际模块配置与分路贡献入 trial | 未完成 | 有代码/提示哈希及 discovery_mode/seed_chunks，没有完整生效配置、分路贡献及模块耗时 |
| 服务/UI/MCP 复用新管线 | 未完成 | 仍调用旧 Repository/rank_search，未调用 HardAgent 或新的共享检索模块 |
| 新架构端到端和保留集验证 | 未完成 | 当前没有新模块装配与开关的对应测试；历史六组结果属于旧配置 |

## 3. 可定位的实现问题

### P1：统一配置与旧臂定义尚未接通

实现仍使用 [ARMS](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/retrieval.py:9) 与 [HardAgent 构造分支](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/agent.py:54)。单独关闭发现向量、首轮 BM25、自动收窄或某种图贡献，不能通过设计中的配置完成。

尤其是设计将 E 写成 `narrow=False`，但 [hybrid_search](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/agent.py:143) 对所有 hybrid 臂在缺少 ranges 时都会自动发现种子并收窄，E 同样如此。当前 E 是“无树图导航的现有管线”，不能视为已经实现设计中的“全库直接混合检索”。新消融实验必须冻结完整预设，而非仅沿用字母名称。

### P1：`--limit` 回执值不能代表 Agent 实际显示名额

[CLI](C:/Users/xingc/Documents/Codex/repository-tree/scripts/run-hard60.py:63) 接收 `--limit`，[profile](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/runner.py:173) 保存它，但 [create_agent 调用](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/runner.py:264) 没有传入该值。实际 [HybridArgs](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/controlled/tools.py:27) 仍默认 15，由模型每次调用时自行填写 limit；`coverage` 则确实使用 Settings.limit。

因此 `run --limit 30` 可能记录 30，却仍按工具默认 15 运行，不能用于可靠的“仅改显示名额”实验。需要明确 display_limit 是默认值还是强制上限，并让 CLI、schema、执行器及回执读取同一个生效配置。

### P1：自动图路径没有跨文档扩张，也没有图排名贡献

[expand_seed](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/agent.py:93) 的图分支仍调用 annotate 标出种子所在窗口，再取这些窗口的块，没有沿相似边走到其他文档，也没有返回 boosts。最终 [search](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/retrieval.py:56) 不接收图分数。

现有独立 `semantic_graph` 工具仍可提供图视图；本结论针对自动检索管线，不能说整个原型完全没有图功能。TF-IDF 图的独立脚本结果也不能代表自动 Agent 路径已经使用它。

### P1：新检索底座没有接入可交互产品入口

[公开 MCP](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/server.py:85) 与 [HTTP 搜索](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/server.py:128) 调用 [Repository.search](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/repository.py:255)。其 [rank_search](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/ranking.py:7) 词法部分是子串匹配，不是 BM25，也没有新的分层管线。

应先把公共检索逻辑从评测目录中分离成产品模块，再由服务和评测适配器共同调用；不要将含题集加载/金标处理的评测框架直接引入产品。

### P1：结构输入仍不足以支撑新树与结构边实验

源码位置为 [BrowseComp](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/materialize.py:82)、[FreshStack](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/materialize.py:114) 和 [入索引](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/corpus.py:125)：没有恢复多级标题，公共来源的原目录与元数据也没有作为完整结构字段进入索引。

本次只读 SQL 确认当前索引为 248 文档、7,228 块，仅 12 份文档具有多个不同 heading 值。此统计是结构缺失的代理指标，不等于人工证明其余文档本来都有多级章节。修复必须保留原始 source_id、偏移、哈希以及正文，不能只替换展示标题。

### P2：分析脚本仍不能作为可分发模块交付

`.eval/` 被忽略，图融合/路线 B 等脚本未纳入版本控制，不能由一次 clone 恢复；部分旧脚本还调用修改前的 seed_pool 签名。迁移时需要提取通用算法和合成测试，数据加载与金标评分继续留在评测层，不原样把含固定题目/索引路径的分析脚本搬进产品。

### P2：收窄尚未减少主要打分计算量

[BenchmarkRetriever.search](C:/Users/xingc/Documents/Codex/repository-tree/src/repository_tree/evaluation/hard/retrieval.py:56) 虽先形成 eligible 集合，但 BM25 的 get_scores 和向量矩阵乘法仍对整个索引计算，之后才从 eligible 中排序。首轮发现和池内最终排序各做一次全量打分；当前收窄限制了返回候选，不等于计算量已经随候选池变小。新管线需明确分数复用、子集打分或规模化索引方案，并另做性能验证。

## 4. 设计本身尚需补齐的合同

设计不是已经完整、只差编码。以下问题会影响模块实现与消融有效性，应先闭合。

1. **配置合法性与默认预设。** 示例 `weights` 使用可变 dict 默认值，Python 3.12 dataclass 会拒绝；`frozen=True` 也不保证内部 dict 不可变。需规定未知模块、负权重、空通道、关图却开 related、关路线 B 却开 jump 等组合如何校验。区分结构树与语义树的开关；当前单一 tree_depth 含义不够清楚。
2. **候选与关联分不能过早丢失来源。** 单一 `Proposal.boosts[chunk_id]` 然后对所有模块取 max，会合并 related/jump 的独立贡献，无法分别调权、消融或归因。每路应保留候选、原始关联分、路径和来源；同一路内部是否 max 可单独定义，最后按路 RRF。
3. **同步/异步、失败与空结果。** 路线 B 含在线抽取和检索，但 propose 示意是同步接口；缺少取消、超时、请求计量和返回状态。模块关闭/合法空结果可以不贡献；模块已开启但缺索引、解析失败、超时则不能等同空集。需要统一区分 disabled、empty、unavailable、infra_error、integrity_error。
4. **scope 在执行前阻止越界。** 文档把 scope 审计放 runner；当前 runner 主要在 answer 返回后检查。新模块仍需在查询、扩张、缓存命中及向模型返回证据前验证范围，runner 再作独立审计，不能依赖事后发现已经泄露的内容。
5. **路线 B 的跳转键定义需修正。** 若 seed_entities 指种子文档出现的所有实体，那么从种子抽出的对象通常也在该集合中，`{t.object} - seed_entities` 会删掉合法桥接键。需要区分查询已知实体、当前关注主体、已访问文档与可跳转实体；“实体已出现”不等于“相关文档已访问”。
6. **查询相关缓存不支持任意跨查询复用。** 键含 query_hash 时只能复用相同规范查询；不能同时声称跨不同查询复用。需要区分文档级无查询抽取缓存与查询条件抽取缓存，并定义模型/提示、源版本、scope 等缓存有效条件。
7. **树和边索引规格缺失。** 语义树尚缺确定的分层算法、分支数/深度、稳定节点 ID、代表块选择、增量/全量重建与 scope 视图规则；结构边尚缺节点/边类型、方向、证据、冲突与遍历上限。只有“聚类＋标签”“目录＋PageRank”不能作为完整实施合同。
8. **评测门槛要可计算。** “大于 ±50% 方差”未定义统计量；单个开发集上选择的权重不能当作保留集门槛。应固定 paired repeats、实际预算、主要指标、效应与置信区间，分别报告错误、缺失和成本。已有 857M 与每千次查询 44M 的数字直接相除得到约 19,477 次查询，文档 13,000 次的其他假设尚未说明。

这些是待解决的设计问题，不是本次已经完成的重构。

## 5. 建议的补齐顺序与验收

| Gate | 可交付工作 | 验收条件 |
|---|---|---|
| G0：闭合设计 | 定义配置、预设、typed Proposal、异步模块结果、分路证据与索引版本合同 | 配置组合可验证；每个开关对应单独能力与测试；历史臂、新臂映射无歧义 |
| G1：共享最小管线 | Discovery → candidate expansion → ranking → ripgrep/read；修复 limit 接线 | 产品服务与评测用同一核心；hybrid/bm25/dense、narrow on/off、显示名额均由生效配置控制 |
| G2：恢复结构并接树 | 保存标题/目录/元数据，接语义树 overview/zoom | 原文定位不变；完整 corpus 可核验；工具实际调用、范围安全与缓存版本测试通过 |
| G3：图与结构边 | TF-IDF 文档图/结构边独立模块，related 独立排名通道 | 能新增受控候选并实际改变排名；关模块无调用/贡献；分路来源可审计 |
| G4：路线 B | 查询条件抽取、检索跳转、缓存、jump 融合 | 合成两文档桥可到达；噪声/空结果/超时/越界测试通过；调用与成本有记录 |
| G5：六组与新模块验收 | 冻结完整预设、CLI、每条 trial 生效配置、端到端和保留集评测 | 相同预算下可复现消融；外层配置等于实际工具行为；质量收益独立于完成度判断 |

先完成 G0/G1 再开发其余模块，避免继续在 HardAgent 内添加按臂分支。实现验收与质量收益验收分开：模块能正确调用不保证有效；单次脚本有效也不等于模块已实现。

## 6. 本次检查证据与边界

- 代码图先查 RetrievalConfig、HardAgent、RankingChannel；再用 tgrep 和源码精确检索核对。未找到的模块名称以实际源码扫描确认，不仅依赖图索引。
- 只读参数/AST 核验：HardHybridArgs 的 limit 默认 15，runner 的 create_agent 参数没有传 limit；BenchmarkRetriever.search 没有 weights/boosts。
- 只读当前 SQLite 统计：248 文档、7,228 块、12 份文档有多个 heading 值；未改数据。
- 使用等价最小 dataclass 定义复现 weights 可变默认值错误，没有执行设计中的在线操作。
- 回读上一轮 JUnit：2026-09-11 16:24:55 +08:00，39 项；相关源码哈希与回执一致。本次没有重跑这批测试，更没有跑新架构 E2E。
- 本次仅补充检查文档与交接状态链接，没有修复实现问题、迁移脚本、重建索引、重启或部署服务。

本文件记录检查时的现状。后续关闭缺口时，应追加对应代码、验证记录及运行入口证据，再更新完成状态。
