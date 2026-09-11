# VGI-RAG 原型与 Hard-60 评测交接

更新：2026-09-11（Asia/Shanghai）。本文件是工程交接入口，承接原型开发、历史评测、困难题集准备和最近新增的产线 SAC 接口。本次只整理文档、核验现有证据，不构建、不重跑在线评测、不改变服务。

阶段二（分层检索实测、路线 A/B 取舍、模块化消融设计）见 [VGI-RAG 分层检索与模块化消融交接](2026-09-11-vgi-rag-layered-retrieval-handoff.md)；本轮实测数字与设计以该文为准。

## 1. 先读结论

**VGI-RAG 原型、A–F 六组接口和 Hard-60 候选材料已存在；六组困难题集的真实评测尚未开始，也没有证明图＋树优于强混合检索。** 最近完成的是 F 组原生 SAC 的 HTTP 适配及本地合成契约验证，不是产线联调或测评结果。

下一阶段先完成开发题审阅、完整语料和证据定位，再补足容量、查询编码、强检索基线以及六组运行/评分。F 还缺真实配置、同语料 workspace 绑定和回读验证。不要直接把新组名传给旧四组 scheduler/scorer，也不要把只含部分证据的本地语料当作完整公开基准。

| 当前状态 | 交接结论 |
|---|---|
| Windows 原型 | 已有本地 UI、HTTP/MCP、解析、章节/块结构、在线向量与跨文档相似图 |
| Hard-60 | 20 道开发题＋40 道保留题，全部 `review=pending`，状态 `candidate_not_ready_for_evaluation` |
| 检索/Agent 适配 | A–E 共享本地 Agent；F 走原生 SAC 系统，统一 `answer(...)` 入口 |
| 全量语料 | QASPER 限定论文及自建语料就绪；BrowseComp-Plus / FreshStack 正文未全量落地 |
| 六组批跑和评分 | 未接通；接口存在不代表运行器可直接执行 |
| 最新相关测试 | 2026-09-10 的本地合成测试 51 项通过；本次仅回读记录 |
| SAC 实机 | 未联调；本机 SAC 配置未填、绑定文件不存在，未有实时原文映射证明 |

## 2. 工作区、版本与提交边界

| 位置 | 用途与接手时的基准 |
|---|---|
| `C:\Users\xingc\Documents\Codex\repository-tree` | 独立原型，Windows 原生运行；WSL 对应 `/mnt/c/Users/xingc/Documents/Codex/repository-tree` |
| `/home/chuan/context-osv6` | 主仓库：设计、评测设计、原生产线源码和新增提示权威文件；Windows 路径 `\\wsl.localhost\Ubuntu\home\chuan\context-osv6` |
| 独立原型 `5217587` | 最新功能提交：`Add native production SAC system baseline interface` |
| 独立原型 `f81fea8` | Hard-60 候选准备及共享五组检索适配；后续 F 扩展没有改题 |
| 主仓库 `22ae3e8f` | SAC 接口文档提交；`6e21599d` 为 Hard-60 准备和共享混合检索契约 |

交接开始时，独立原型工作区干净；主仓库 HEAD 为 `5289d13d`，另有其他任务的未提交修改和未跟踪文件。主仓库当前 HEAD 包含与本任务无关的后续工作，不能当作产线部署版本。接手时重新查看 `git status`，保留其他修改；本次只提交交接文件。

两个仓库均沿用本地 `master`，不 push、不建 PR。主仓库 Git 从 WSL 执行，避免 Windows Git 在 UNC 路径上的 ownership 问题，不为此改全局 `safe.directory`。独立仓库使用 Windows Git。

## 3. 已确定的产品和实验约束

- 名称为 **VGI-RAG（Vector Graph Index RAG）**。独立产品原型以 Windows 原生为先，不继承大项目技术栈作为选型前提。
- 保持轻量解析方案；已取消 Docling 与本地 embedding/LLM 部署。embedding 使用在线服务，沿用已配置的 SiliconFlow profile；具体模型、维度和 API 配置须在运行快照中据实记录。
- 用户指定 Agent 使用 `qwen3.8-flash`，不设总 token 预算。模型别名和实际返回模型都要记录；F 的模型由服务端决定，不能用客户端预期值冒充实际配置。
- 每组并发 2，全局最多 8；六组共享这个全局上限。F 的服务内部 Worker 并发另行观察。
- B 只移除跨文档语义图，保留普通向量检索；F 是完整产线 SAC 系统对照。
- 困难题集用于发现瓶颈并优化 VGI-RAG，不能按“只有 VGI 答对”筛题，也不能用测试集成绩反复删题、改金标或调参。
- 评估按语义判断，接受等价措辞、合理替代证据和必要澄清；正确性、完整性、忠实性、引用、行为和系统错误分别记录。旧“所有信息点与引用全通过”的严格成功率不再作为主排名指标。
- 任务仍是文档检索评测，不扩展到完整 E2E 149。既有 128 题结果作为历史证据保留。

本轮用户只要求交接文档，没有启动新一轮下载、在线评测或生产写入。后续短小本地实现/检查按仓库规则推进；全量语料下载、真实模型批跑、生产导入或服务操作需说明具体范围和耗时，并核对已有授权是否覆盖。没有总 token 上限不等于任意长任务自动获准。

## 4. 原型实际技术路线

权威入口：[原型说明](2026-09-10-repository-tree-prototype.md)、[独立 README](C:/Users/xingc/Documents/Codex/repository-tree/README.md)。以下描述当前原型，不能等同于 PRD 中全部规划能力。

| 层 | 已有实现 | 关键边界 |
|---|---|---|
| 解析 | Markdown 使用 markdown-it-py；DOCX/XLSX 使用 AnyDoc；PDF 使用 LiteParse，OCR 关闭 | Windows 轻量栈，不需要 Docling 或本地模型 |
| 文档内结构 | 标题/段落边界、章节、块序号及前后块关系；块上限 320 字符 | 不是已经实现的 embedding 语义边界切块；序号和续读保留跨块观点 |
| 仓库树 | 文件与章节/块的结构导航 | 确定性结构与派生语义关系分开；不是 SVD 自动生成的语义树 |
| 语义图 | 章节/窗口节点，窗口最多 16 块；长度加权向量质心，跨文档余弦邻居，SQLite 派生快照 | 表示相似性与导航关系，不是事实三元组，也不保证推理依赖；尚未实现 SVD/跨文档聚类树 |
| 原始演示检索 | 关键词子串＋NumPy 精确余弦＋RRF | 不能把这一路称为 BM25；新 Hard 基准有独立的真实 BM25 实现 |
| 原型界面 | 本地 HTTP UI `http://127.0.0.1:8765`，MCP `/mcp` | 启停脚本在 `scripts/`；本次没有探测服务运行状态 |

演示上传文件位于 `.demo/documents/`，索引和规则位于 `.demo/index.sqlite`、`.demo/rules.json`。这属于现有原型演示，不等于已满足正式 Subtex 的“用户项目目录为唯一原文主库、不 ingest 复制”产品边界。

现有保护上限包括 8 MB/文件、100 文档、每文档 2,000 块、4,096 图节点。精确向量与成对相似计算不适合直接放大到约十万文档；不能只调大常量绕过容量门槛。索引是派生产物，原文版本/哈希和图快照一致性必须验证。

## 5. 六组对照与接口

| 组 | 路线 | 混合检索 | 树导航 | 跨文档图 |
|---|---|---|---|---|
| A | 完整 VGI-RAG | BM25＋dense＋RRF | 有 | 有 |
| B | 去语义图 | 同 A | 有 | 无 |
| C | 去树导航 | 同 A | 无 | 有 |
| D | 基础桌面 Agent＋grep | 无；真实 ripgrep | 无 | 无 |
| E | 混合检索＋平铺 | 同 A | 无 | 无 |
| F | 原生产线 SAC | 服务端完整检索与回答流程 | 原生产线能力 | 原生产线能力 |

A/B/C/E 构成图×树的 2×2 消融。A–E 共用原文、文件清单、读取和真实 ripgrep；D 不是单次 grep，而是 Agent 可多轮列文件、grep 和读取。平铺仍保留文件名、原始标题文本、块序号和连续读取，只移除派生层级导航。

`BenchmarkRetriever` 支持 `bm25/dense/hybrid`：BM25S 0.3.11、Lucene BM25（k1=1.2、b=0.75）、英文词干/停用词；归一化在线向量做精确余弦；每路最多 100 候选、RRF k=60，稳定 ID 破同分。范围过滤先于候选截取。当前英文配置只适用于本轮材料，reranker 尚未接入，不能称为已校准的最强混合检索基线。

`HardAgent` 复用 `ControlledAgent / RagAgent` 循环。A/C 要求 ready 图并检查 embedding profile、源哈希、块覆盖和边端点；B/D/E 拒绝图快照。`create_agent(...)` 分流 F 到 `SacAgent`，直接构造 `HardAgent(..., arm="F")` 会拒绝。

F 统一返回 `answer(question, document_ids, prior_turns)` 的观察结果，实际走 `POST /api/v1/chat → state.conversation().execute(...)`，保留产线 Lead/Worker、SaC 取证和回答，不在外层再套本地 Agent。F 与 A–E 的解析、切块、向量、reranker、提示和内部调度差异必须单列，不能将系统差距全归因于图。

## 6. Hard-60 数据、隔离与现有诊断

| 来源 | 开发题 | 保留题 | 筛选依据，不等于已证实难度 |
|---|---:|---:|---|
| BrowseComp-Plus | 8 | 12 | 至少 3 份标注证据文档，不自动等于 3 跳推理 |
| FreshStack / LangChain | 4 | 8 | 至少 3 个可映射的证据信息点 |
| QASPER | 4 | 8 | 保留标注跨至少 2 节且可映射到文本 |
| 合成桥接 | 2 | 6 | 跨文档标识与规则、干扰文档 |
| 合成版本/例外 | 1 | 3 | 正式、过期、草案及例外 |
| 合成边界/行为 | 1 | 3 | 连续读取、限定范围无答案、必要澄清 |
| 合计 | **20** | **40** | **44 道公开题＋16 道合成诊断题** |

按固定哈希次序选取、按共享标注证据家族连通分量分隔开发/测试；QASPER 保留原始 dev/test 边界。未知同源关系和预训练记忆没有被完全排除。评分将“任一充分证据集合”作为 OR，集合内部共同必需证据作为 AND；映射失败记未知，不能直接当作检索零分。

本机数据根为 `C:\Users\xingc\Documents\Codex\repository-tree\.eval\hard\hard60-v1`，被 Git 忽略，换机器要单独保留或按固定版本重建。不要提交题目、金标、原始模型响应或凭证。

| 相对数据根路径 | 内容 |
|---|---|
| `manifest.json` | 创建时固定题集、来源版本、哈希及五组 A–E 快照；不可改写成六组历史 |
| `evaluation-only/questions.json` | 60 题和评估专用标注，不进入 corpus/产品提示 |
| `evaluation-only/DEV-REVIEW.md` | 只展开 20 道开发题，40 道保留题保持隔离 |
| `corpus/documents.json`、`corpus/text/` | 已有 61 份原文、579 个来源段落：12 份 QASPER＋49 份自建文档 |
| `raw/`、来源回执 | 固定远端版本、Range 字节缓存和哈希 |
| `source-id-validation.json` | 完整 ID 列及选定证据存在性检查 |
| `source-duplicate-validation.json` | FreshStack 重复 ID/正文差异审计 |
| `local-validation.json` | 开发题的局部 BM25 诊断，无生成答案 |

2026-09-11 回读确认题目 SHA-256 与 manifest/SAC 回执一致：

```text
0d800f12c29a5bfea4993c365584e08d1f5a8d07894be95f945fafd52fe47df1
```

BrowseComp 完整 ID 列已验证 100,195 个唯一 ID；FreshStack 为 49,514 行、49,505 个唯一 ID，2 个 ID 重复、其中 1 个正文不一致，选定 12 题的证据不涉及重复项。后续完整加载采用作者 DataLoader 的“源行顺序最后一行覆盖”语义，并保留重复审计；本次尚未完成全正文规范化。数据来源与版本见 [准备文档](2026-09-10-vgi-rag-hard-benchmark-preparation.md) 及 `hard/sources.py`。

已有 BM25 诊断仅覆盖 8 道开发题，检索单位是来源段落：4 道 QASPER 的充分证据齐备数在 @5/@15/@30 为 1/4、2/4、3/4；4 道合成题的平均信息点覆盖率为 50%、50%、100%。两类范围不同，应分开报告。这不是向量/Agent 的答案得分，也不证明 Hard-60 已足够难或图有效。

## 7. SAC 接口交接重点

详细合同见 [主仓库 SAC 接口说明](2026-09-10-vgi-rag-sac-baseline-interface.md) 和 [独立工程 SAC-INTERFACE.md](C:/Users/xingc/Documents/Codex/repository-tree/eval/SAC-INTERFACE.md)。

- 请求固定使用 `agent_type="rag"`、`capabilities=["rag"]`、绑定的 `workspace_id`、明确 `doc_scope`、`session_id=null`、题目给定前文 `messages`、`stream=false`、`debug=true`。金标、其他题目和评分不入请求。
- `SAC_API_BASE_URL` 应含 `/api/v1`；生产 HTTPS，回环本地可 HTTP。鉴权使用应用 Bearer token，不是在线 LLM key，也不能把 JWT 签名密钥当作用户 token。
- 原生 [ChatRequest](../../contracts/src/chat.rs) 没有模型切换字段。`SAC_EXPECTED_MODEL` 只做响应核对；必须另查服务端 Lead/Worker/embedding/reranker 配置与部署版本。
- `.eval/hard/native-sac-binding.json` 需要原文 ID 到生产 UUID 的一一映射、原文 SHA-256、workspace UUID 和部署版本。适配器只核对本地清单；当前 `source_mapping_verified_live=false`，不代表服务端原文已回读验证。
- `document_ids=None` 展开绑定内全部 ID，仍发送显式 scope；`[]` 在本地拒绝，防止服务端扩大检索范围。请求体上限 2 MiB，在全语料显式 scope 下也需做容量验证。
- 原生答案不改写；成功检索结果和最终引用分开，不用引用倒填 recall。返回行先做越界审计再去重，缺失来源 ID 记未核验，不猜测。
- native chunk ID、原型 chunk ID、公开基准 evidence ID 不同；按原文位置对齐信息点仍未完成。旧 scorer 不能直接比较这些 ID。
- POST 无自动重试/重定向，响应上限 32 MiB；超时、取消、错误保留未知用量，不能记零。服务端汇总用量不能冒充全部内部请求的独立计量。
- 保留 `native_response`、`mode_debug.general`、工具活动和来源审计。正确路由不等于每题已执行预期 SaC 策略，需用真实轨迹确认。

**2026-09-11 本机状态：**独立 `.env` 的 `SAC_API_BASE_URL / SAC_API_TOKEN / SAC_EXPECTED_MODEL / SAC_TIMEOUT_SECONDS` 均未填；绑定文件不存在。模板已提供，但没有可直接开跑的 SAC 配置。后续需要配置时先按仓库约定查现有环境，静默复用有效值，不输出秘密。

## 8. 关键文件导航

独立仓库相对根路径：

| 路径 | 用途 |
|---|---|
| `eval/HARD-BENCHMARK.md`、`eval/SAC-INTERFACE.md` | 候选集、六组工具、请求/观察契约与复现说明 |
| `src/repository_tree/evaluation/hard/retrieval.py` | ARMS、BM25/dense/RRF、范围过滤 |
| `src/repository_tree/evaluation/hard/agent.py` | 本地 Agent 组装和 `create_agent` 工厂 |
| `src/repository_tree/evaluation/hard/sac.py` | SacConfig、SacBinding、原生 SAC HTTP 适配与审计 |
| `src/repository_tree/evaluation/hard/sources.py`、`remote.py`、`selection.py` | 固定来源、受限下载、候选选择/隔离 |
| `scripts/prepare-hard60.py`、`verify-hard60.py`、`hard_case_fixtures.py` | 材料准备、完整性验证、合成诊断材料 |
| `tests/test_hard_benchmark.py`、`test_sac_interface.py`、`test_semantic_experiment.py` | 本轮相关的确定性契约/隔离测试 |
| `evidence/hard60-preparation.json`、`sac-interface-validation.json` | 可提交回执，不含真实题目/金标正文 |
| `src/repository_tree/prompts/controlled/SKILL.md`、`prompts/semantic/semantic_graph.md` | 既有共享工具/Agent 资产 |

主仓库新增混合检索描述权威源为 [hybrid-search.md](../../avrag-rs/prompts/eval/vgi-hard/hybrid-search.md)，由 `hybrid_description_path` 提供。新 LLM-facing prose 继续遵循主仓库提示目录规则，不在产品代码或评测脚本中硬编码带金标倾向的指令。

原生产线核对入口：[路由](../../avrag-rs/crates/transport-http/src/routes/chat.rs)、[handler](../../avrag-rs/crates/transport-http/src/handlers/chat.rs)、[路由挂载](../../avrag-rs/crates/transport-http/src/lib_impl/router_core.rs)、[原生证据提取](../../avrag-rs/tests/rag_quality/src/harness_extract.rs)。本次核对这些文件及 ChatRequest 的哈希与 SAC 回执一致，不证明远端部署也一致。

## 9. 验证证据与历史结果的解释

最新相关测试回执是 **2026-09-10 19:51:47 +08:00，51 项通过、0 失败/错误/跳过，5.63 秒**，JUnit 为独立工程 `evidence/raw/sac-interface-junit.xml`。使用合成 HTTP transport，覆盖路径/字段、范围隔离、前文、检索与引用分离、用量、错误/取消、重定向和容量、六组构造。对应回执记录真实产线问答、模型调用、生产写入均为 0。

本次交接只做文件、哈希、配置存在性和测试回执回读；没有重新执行上述测试，也没有验证真实模型答案、远端鉴权、产线索引状态或服务健康。切勿将历史测试时间写成本次重新测试通过。

历史资料应保留，但不作为新实验结论：

- [128 题说明](2026-09-10-repository-tree-rag128.md)及独立 `evidence/rag128-native-comparison.md`：原型与旧 SAC 的题目版本/模型并非完全一致，不能据总通过率判断架构胜负。
- [图/树实验设计](2026-09-10-repository-tree-semantic-graph-experiment.md)、[结果](2026-09-10-repository-tree-semantic-graph-results.md)及独立 `evidence/semantic20-analysis.md`：已有 20 题×3 次×4 组的回答记录，尚未形成稳定的图/树质量收益证据。
- 独立 `evidence/semantic20-metric-review.md`：旧严格成功不是字面字符串匹配，但把所有信息点、引用、行为和无错误合并为 AND 门槛，叠加裁判不一致/映射缺陷会产生不合理零分。旧结果保留供诊断，不静默重写为新的任务成功率。
- 历史独立上下文模型复核不是人工或跨模型权威裁决。后续要显式标记 judge inconsistency 和映射未知，分开质量缺陷与基础设施失败。

## 10. 接手后的执行顺序与验收门槛

| 顺序 | 工作 | 进入下一阶段的门槛 |
|---|---|---|
| 1 | 审阅 20 道开发题；校准语义评分、替代充分证据、无答案/澄清边界 | 审阅状态明确；相同含义不同措辞不被误杀；金标与运行 corpus 隔离 |
| 2 | 全量 BrowseComp/FreshStack 正文加载、重复 ID 规范化、FrozenCorpus 与出处映射 | 固定源版本、全量完整性/哈希及证据定位可复核；保留题不参与调参 |
| 3 | 容量与在线向量方案、长问题编码、reranker 强基线 | 在实际规模下验证时间/内存/召回，不靠放宽保护常量；错误不伪装为检索零分 |
| 4 | 六组 scheduler/scorer、统一轨迹和用量账本、新运行 manifest | 组能力隔离、scope 一致、评分单位一致；全局并发≤8、每组≤2；旧五组 manifest 不改 |
| 5 | F 的同语料隔离 workspace、绑定和服务回读；短批原生联调 | 实际部署/模型/原文身份可核验，native evidence 可定位；用量未知和内部并发如实记录 |
| 6 | 开发集比较 BM25/dense/hybrid/reranker、金标证据 oracle 与 Agent | 分清检索缺证据、读取不足、回答/裁判失败；配置只在开发集上调整 |
| 7 | 冻结配置后，对 40 道保留题做六组配对重复实验 | 预先固定重复次数、失败处理与统计口径；按来源/题型分层报告质量、成本、延迟及不确定性 |

优先处理的具体问题：

1. **长查询**：一条 FreshStack 候选问题长 7,280 UTF-8 字节，超过现有 online embedding 的 4,096 字节保护。工具参数允许更长不代表 provider 适配已修复；先核对真实输入限制并明确查询编码，不静默截断。
2. **规模**：现有精确向量/图算法和原型上限无法直接代表完整公开语料性能。根据测得的规模选 ANN/稀疏图构建等必要方案，先验证切片，再进入全量。
3. **六组完整链路**：旧四组实现可以复用局部能力，不能把输出假设直接套在 F；原生 ID、工具、用量与错误需要各自适配后统一评分。
4. **难度校准**：先运行强检索和 gold-evidence oracle 才能判断题目是否测到检索瓶颈；局部 BM25 分数低不是困难题集验收。

## 11. 本地复现与操作注意

以下是接手用命令，本次未执行构建、模型测试或下载。针对性本地测试历史耗时约 6 秒；缓存齐全的准备/核验各约数秒。缓存缺失时会访问外网，依赖安装和全语料任务的耗时需另估。

```powershell
Set-Location -LiteralPath 'C:\Users\xingc\Documents\Codex\repository-tree'
git status --short
git log -5 --oneline
.venv/Scripts/python.exe -X utf8 -m pytest tests/test_sac_interface.py tests/test_hard_benchmark.py tests/test_semantic_experiment.py -q --junitxml=evidence/raw/sac-interface-junit.xml
.venv/Scripts/python.exe -X utf8 scripts/prepare-hard60.py
.venv/Scripts/python.exe -X utf8 scripts/verify-hard60.py --verify-source-ids
```

准备脚本每次下载上限 64 MiB，使用 Range 投影，避免把含完整文档正文的大问题表直接全量下载；它不是完整语料下载器。冻结输出发生变化时会拒绝覆盖，应显式新建版本，不能为了“跑通”删掉旧目录。重新跑测试会更新同名 JUnit，若需保留历史原件，先改为新的输出文件名。

现成 `.venv` 使用 Python 3.12。需要恢复依赖时，固定锁文件和 benchmark/probe/formats 组，不装本地模型：

```powershell
$env:UV_CACHE_DIR = Join-Path (Get-Location) '.cache/uv'
& '.bootstrap/uv-0.12.12/uv-0.12.12.data/scripts/uv.exe' sync --locked --group benchmark --group probe --group formats --python .venv/Scripts/python.exe
wsl.exe -d Ubuntu -- git -C /home/chuan/context-osv6 status --short
```

依赖恢复可能下载文件；不要为此改系统 PATH。启停脚本为 `scripts/start-prototype.ps1`、`scripts/stop-prototype.ps1`，当前服务状态未探测，不要接手后盲目启停。SAC 调用可能产生会话/用量记录，不是无副作用的裸检索；没有检查绑定与范围前不要试打生产接口。

后续若修改结构代码，先按主仓库 [代码索引指导](../agent/code-review-graph.md) 查询，再在同一轮更新图；本次文档无需图更新：

```powershell
wsl.exe -d Ubuntu -- /home/chuan/.local/bin/code-review-graph search --repo /mnt/c/Users/xingc/Documents/Codex/repository-tree --limit 8 create_agent
wsl.exe -d Ubuntu -- /home/chuan/.local/bin/code-review-graph update --repo /mnt/c/Users/xingc/Documents/Codex/repository-tree --base HEAD
wsl.exe -d Ubuntu -- /home/chuan/.local/bin/code-review-graph status --repo /mnt/c/Users/xingc/Documents/Codex/repository-tree
```

不要提交 `.env`、`.eval/`、`.code-review-graph/`、`.tgrep/` 或原始评测响应。主仓库只暂存本任务路径；读取相关目录的 `AGENTS.md`、E2E 门槛和运行规则后再做下一阶段代码/在线操作。

## 12. 继续阅读

- 当前设计：[PRD v0.3](2026-09-10-repository-tree-prd-v0.3.md)、[开发计划](2026-09-10-repository-tree-development-plan.md)。区分设计目标与本文件记录的已实现状态。
- 困难题集：[调研](2026-09-10-vgi-rag-hard-benchmark-research.md)、[准备记录](2026-09-10-vgi-rag-hard-benchmark-preparation.md)、[六组契约](C:/Users/xingc/Documents/Codex/repository-tree/eval/HARD-BENCHMARK.md)。
- 产线对照：[新增 SAC 接口](2026-09-10-vgi-rag-sac-baseline-interface.md)、[详细合同](C:/Users/xingc/Documents/Codex/repository-tree/eval/SAC-INTERFACE.md)。
- 工作规则：[本地提交纪律](../engineering/SOLO_DISCIPLINE.md)、[LLM 提示规则](../agent/llm-guidance.md)、[真实 E2E 门槛](../../avrag-rs/docs/e2e-gates.md)。

这些链接中的 Windows 绝对路径指向当前机器的独立原型；换机器时替换项目根路径，并单独恢复被忽略的数据和配置。独立工程根目录 `HANDOFF.md` 反向指向本文件，完整交接以本文件为准。
