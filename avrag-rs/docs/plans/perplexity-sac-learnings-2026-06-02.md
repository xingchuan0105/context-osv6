# Perplexity Search-as-Code (SaC) 借鉴分析

> 参考资料：[Rethinking Search as Code Generation](https://research.perplexity.ai/articles/rethinking-search-as-code-generation)（Perplexity Research, 2026-06-01）
> 日期：2026-06-02
> 状态：调研文档（research note），非实施计划
> 范围：基于 `avrag-rs` 当前 RAG / WebSearch / Code Interpreter 实现，对照 SaC 架构评估可借鉴方向

---

## 一页架构视图（概览，非规范）

> SaC 三层模型与本项目对应物的对照速览，便于决策者快速扫读。具体状态 / 差距 / 待办见正文各章节。

```
┌────────────────────────────────────────────────────────────────────┐
│  Models（控制面）                                                  │
│  ─ LLM provider + ReAct state machine（SearchStrategy / RagStrategy）│
│  ─ 当前：通过状态机驱动 tool call；code-gen 编排暂不引入           │
└────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────────┐
│  Compute Sandboxes（确定性 compute）                               │
│  ─ avrag-code-interpreter（已：RLIMIT_AS 256MB / import 黑名单）   │
│  ─ ⚠️ 缺 session 抽象 + 跨调用持久化 → 方向 2 待办                │
└────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────────┐
│  Agentic Search SDK（原语层）                                      │
│  ─ RetrievalDataPlane: text_dense / bm25 / multimodal / graph      │
│  ─ search crate: Brave LLM Context / Brave News                   │
│  ─ Reranker + EvidenceGate + FocusMode                            │
│  ─ ⚠️ 通过 tool schema 暴露，缺编排教学 → 方向 3 待办             │
└────────────────────────────────────────────────────────────────────┘
```

---

## 摘要

SaC 的核心论断：**传统搜索的"黑盒端点"模型（query in / fully processed results out）在 agent 长任务场景下颗粒度过粗**。Perplexity 把搜索栈原子化、暴露为可编程原语（Agentic Search SDK），让模型在 sandbox 中**通过生成代码**而非"逐轮 function call"来编排这些原语。

其在 WANDR 基准上较次优系统提升 2.5×（0.386 vs 0.152），CVE 案例 token 用量降低 85%。本项目当前架构在 RAG 侧原语拆分较充分（`RetrievalDataPlane` 4 个后端），但暴露方式与状态管理能力与 SaC 存在结构性差距。以下三个改造方向按借鉴价值与本项目契合度排序。本文只讨论借鉴价值与可考虑的方向，**不**讨论实施路线优先级、具体时间表或工作拆解。

---

## 改造方向 1：Web Search 加工边界的可控性

### 1.1 现状

`crates/search/src/executor.rs` 中 `SearchProvider` 抽象是典型的"monolithic contract"：

```rust
async fn execute_search(
    &self,
    query: &str,
    vertical: Option<&str>,
) -> anyhow::Result<SearchResponse>;
```

`SearchResponse` 由 Brave LLM Context 端点预加工，内含 `synthesized_answer` 字段——即 Brave 自身 LLM 烤好的"现成答案"，加上 `results: Vec<SearchResult>`（标题 / URL / 摘要 / citation 索引）。调用方对该结果**只读不可干预**，无法：

- 观察候选生成阶段的 ranking signals
- 按 source / domain / 时间窗做二次过滤
- 调整召回-精度平衡
- 在 trajectory 中根据已收集证据反向调整 query

这是 SaC 文章点名的"query params 是唯一控制点"的典型形态。

### 1.2 SaC 可参考机制

SaC 把搜索栈拆为原子原语（retrieve / rerank / filter / dedupe / aggregate / render），AI 在 sandbox 内通过 Python 代码自由组合。这不是为了"展示技巧"，而是为了解决三个具体失败模式：

- **Coarse context**：当前端点对"需要 1 个精准事实"和"需要 100 个候选"返回同质化结果，迫使模型在 context 内做后处理
- **Failure to leverage domain knowledge**：模型在 trajectory 中得出的"应该优先 X 来源、混合 Y 信号"等洞察，无法回灌到搜索策略
- **Inefficient control flow**：fan-out / 异步 / 重试等非线性流程被迫序列化到 LLM turns 中，污染 context

### 1.3 实施路径

⚠️ **重要前置说明**：经与项目 owner 讨论，将 Brave `synthesized_answer` 完全替换为"原始 grounding 由我们自己的 LLM 加工"在工程上不成立——**Brave 端点承担的网页抓取 / 解析 / 反爬对抗成本不应转嫁到我方 LLM 链路**，且会显著拖慢 P95 延迟、降低首屏体验。

因此实施路径不是"原子化 Brave"，而是**重新划分加工边界**：

| 阶段 | 内容 | 改动量 | 风险 |
|:----:|------|:------:|:----:|
| 1 | 评估 Brave `synthesized_answer` 在现有 E2E 集上的"信息利用率"——是否被后续 LLM 大量改写、是否引入错误前导 | ~50 LOC（eval 脚本） | 低 |
| 2 | 在 `SearchResponse` 旁暴露 `grounding_raw: Vec<BraveGroundingItem>` 字段（已存在于 `BraveLlmContextResponse` 中），让上层 ReAct loop 可选地直接消费原始 grounding，跳过 `synthesized_answer` | ~80 LOC | 低 |
| 3 | 引入"加工模式"选择：根据 query 复杂度（query 长度、含实体数、是否触发多步意图）动态决定走 A（保留 Brave 加工）还是 B（自加工） | ~150 LOC | 中 |

> **不**做：完全去掉 `synthesized_answer`、把网页抓取 / 解析引入我方链路。SaC 的原子化假设其自身就是搜索引擎基础设施方，与本项目位置不同。

### 1.4 可能的效果（待验证）

- 短期：可能通过评估明确 `synthesized_answer` 的真实价值，避免过早重构
- 中期：可能改善复杂 web 任务（多步 / 跨域 / 需交叉验证）的可控性
- 长期：适合作为 wide research 类高价值场景的能力组件
- **未量化**：当前缺乏能直接对比"加工边界前移"效果的内部基准，需先建 benchmark

---

## 改造方向 2：长任务 AI 状态管理

### 2.1 现状

`crates/code-interpreter/src/lib.rs` 提供沙箱 Python 执行能力，已具备：

- 资源限制（`RLIMIT_AS` 256MB / `RLIMIT_CPU` 30s / 30s wall timeout）
- Import 黑名单（`os` / `subprocess` / `socket` / `sys` / `ctypes` 等 15 个模块）
- stdout / stderr / last expression 捕获

但**存在结构性缺失**：

- 每次 `execute()` 启动新 Python 进程，所有变量、临时文件、推理中间态全部丢失
- 无 session / workspace 抽象，跨调用无法保持状态
- 无文件级 read / write 工具引导，AI 缺乏"持久化中间产物"的标准路径

后果：当 agent 进入"先查 200 篇 → 抽样 30 篇看 → 决定深挖哪 5 篇"等多步任务时，所有中间证据要么塞回 context（贵、context pollution、撞上限），要么丢失（断链）。这是 SaC 文章用 WANDR 基准量化出来的核心瓶颈。

### 2.2 SaC 可参考机制

SaC 在 sandbox 内做"跨 turn 状态管理"，明确比较三种机制：

| 机制 | 描述 | 优势 | 劣势 |
|------|------|------|------|
| Token space（context） | 中间结果写回 LLM prompt | 显式，AI 总能看到 | 贵、context pollution、有上限 |
| REPL（in-memory） | 保留 Python 进程，变量不销毁 | 无 token 成本 | 长 trajectory 上 AI 自己搞不清留了什么（"100-cell Jupyter notebook 效应"） |
| **File-based serde** | **AI 显式 `write_file` / `read_file` 持久化** | **无 token 成本 + 显式结构 + 可审计** | **多几行 serde 代码** |

Perplexity 团队明确结论：**长 trajectory 上 file-based serde 最可靠**。他们引用原话：

> *"Requiring models to convey state declaratively rather than implicitly helps them manage that state more effectively."*

（让模型用"显式声明"传递状态，而不是"隐式残留"，能帮它更有效地管理状态。）

这与软件工程中"显式优于隐式"的原则同构——人靠交接文档跨天、跨人协作，AI 在跨步协作时靠的是同一种东西。

### 2.3 实施路径

| 阶段 | 内容 | 改动量 | 依赖 | 风险 |
|:----:|------|:------:|------|:----:|
| 1 | 新增 `SandboxSession { id, workspace_dir, history }` 抽象，`CodeInterpreter` 接受 session 参数（不传则维持当前单次执行语义） | ~120 LOC | 无 | 低 |
| 2 | 在沙箱 Python 镜像中预装 `avrag_sandbox_io` 包，提供 `read_file / write_file / append_log / list_files` helper | ~60 LOC + 镜像构建 | 1 | 低 |
| 3 | 写 `SKILL.md`（< 1500 tokens）教 AI："长任务中每步结束前用 `write_file` 持久化中间结果 + 推理日志，下一步用 `read_file` 取回；命名要带语义（`candidates_filtered.json` 而非 `data.json`）" | 文档 | 1, 2 | 低 |
| 4 | 在 `RagRuntime` / `SearchStrategy` 的 tool dispatch 中加入"建议使用沙箱"的条件提示（query 涉及多实体 / 多时间窗 / 多源时） | ~40 LOC | 1, 2, 3 | 中 |

> **不**做：REPL 模式。Perplexity 已实验表明 REPL 在长 trajectory 上劣于 file serde，且实现复杂度高、调试困难。先把 file serde 跑稳。
> **不**做：autoresearch loop 持续优化沙箱可消费性。该机制依赖成熟的 LLM-as-judge + 自动化评测基础设施，不在当前优先级。

### 2.4 可能的效果（待验证）

- **长链路任务（≥5 步）稳定性可能提升**——参考 Perplexity WANDR 2.5× 增益主要由该能力驱动
- **Token 成本可能降低**——中间产物不再每步重传，估算复杂任务可能降 30-60%（视 context 大小，未实测）
- **可能解锁 wide research 类高价值场景**——竞品监控 / 客户档案 / 文献综述 / 法务尽调等
- **可审计性可能增强**——出错时可追溯 AI 在哪一步 / 因何判断偏离
- **风险**：
  - 文件管理不当可能产生脏数据，需设计 session 清理 / TTL 策略
  - AI 可能写出"假 serde"（`json.dumps` 一个 lambda 等），需在 helper 层做基本校验

---

## 改造方向 3：Agent Skill 沉淀与 SDK 消费性

### 3.1 现状

`crates/rag-core/src/runtime.rs` 中 `RagRuntime::execute_tools` 配合 `tools::dispatch_all` 暴露了 `RetrievalDataPlane` 的 4 个后端：

- `text_dense`（向量检索）
- `bm25`（关键词检索）
- `multimodal`（图像 / caption 检索）
- `graph`（知识图谱）

加之 `SearchProvider::execute_search` 和 `RerankerClient`，AI 实际可用原语约 6 个。**这些原语的发现 / 调用完全依赖 tool schema**——系统 prompt 中列出每个 tool 的输入输出 JSON schema，AI 在 trajectory 中按 schema 选用。

**当前缺失**：关于**如何编排这些原语**的领域知识。AI 不知道：
- 查法律合同应该 bm25 优先还是 dense 优先
- 查公司股权结构应该走 graph + 文本扩展
- 模糊问题应该先宽召回、再按 domain 过滤、再重排
- 等等

这些经验目前沉淀在工程师头脑中 / 散落在 prompt 的零散 hint 里，未形成结构化资产。

### 3.2 SaC 可参考机制

Perplexity 的做法分三层：

1. **Agent Skills 文件**（< 2000 tokens 的 `SKILL.md`）教模型**如何组合原语**——重点不是"列出 API"，而是"组合模式 + few-shot"
2. **Autoresearch loop** 持续优化 SDK 的"可被模型消费性"（命名 / 默认值 / 错误消息的可读性等）
3. **约束 skill 大小**，避免 context 膨胀

其论证基础：自定义 SDK 不会出现在预训练数据中，仅靠 tool schema 不足以让模型掌握"组合模式"。

### 3.3 实施路径

| 阶段 | 内容 | 改动量 | 依赖 | 风险 |
|:----:|------|:------:|------|:----:|
| 1 | 撰写 `prompts/rag-agent-skill.md`（< 1500 tokens），结构：① 何时用哪个后端（决策表）② 三个典型组合模式（few-shot）：lexical+semantic 互补、图谱+文本扩展、多模态+文本 ③ 反例（不要做的事） | 文档 | 无 | 低 |
| 2 | 将 SKILL.md 注入到 RAG 策略的 system prompt 中（与 tool schema 并列，**不**替换） | ~20 LOC | 1 | 低 |
| 3 | 建立最小评测集：5-10 个典型 query，对比注入 skill 前 / 后的 tool 选择准确率与最终答案质量 | ~100 LOC（eval 脚本） | 1, 2 | 低 |
| 4 | 根据评测结果迭代 skill 内容（每月一次 review） | 文档 | 3 | 低 |

#### 阶段 1 产出示意：2 个可复用 skill 模式片段（建议/示例，非规范）

> 以下为方向 3 阶段 1 的**草稿级**示例，意图是让产品 / 模型团队立即看到 SKILL.md 的目标形态。可直接复制到 `prompts/rag-agent-skill.md` 试用；命名 / 案例细节可按业务域替换。

##### 模式 A：跨文档聚合查询

**适用场景**：用户要"列出 / 汇总 / 聚合"某实体在多个文档中的相关信息。
例：列出 XX 客户过去 3 年所有合同 / 纠纷 / 关键人。

**典型编排**（4 步）：

1. `graph` 抽取核心实体 + 关联文档 ID（限 org 内）
2. `bm25` 补全精确编号（合同号 / 案号 / 法规条款）
3. `text_dense` 兜底同义表达
4. RRF 合并三路候选 → `reranker` 精排 → `code_interpreter` 跨文档聚合

**反例**：

- 不要 `text_dense` + `reranker` 一把梭——会漏 `bm25` 能捞到的精确编号
- 不要 `graph` 一次跳 3 跳以上——成本爆炸

##### 模式 B：时间线整理

**适用场景**：用户要"按时间线 / 演进"梳理某事。
例：把 XX 项目从立项到当前的里程碑列出来。

**典型编排**（4 步）：

1. `graph` 锚定核心实体
2. `text_dense` 召回"时间锚"段落（含日期 / 季度 / 阶段词的）
3. `code_interpreter` 抽时间戳 + 排序 + 冲突检测
4. `code_interpreter` 写 markdown 时间线（持久化到 session 文件——见方向 2）

**反例**：

- 不要把所有文档全读全文——先用元数据 / 摘要筛
- 不要按文档创建时间排——用户要"事件时间"不是"入库时间"

> **不**做：autoresearch loop 自动优化 skill。该机制需要 LLM-as-judge 基建 + 大量评测样本，性价比不匹配当前阶段。
> **不**做：把 tool schema 拆得更细 / 更"原子化"。当前 4 后端 + 重排粒度合理，过度拆分反而增加 AI 选择负担。

### 3.4 可能的效果（待验证）

- **短期**：可能提升单步 RAG 任务的 tool 选择准确率（不再误用 multimodal 查纯文本）
- **中期**：可能改善多步 RAG 任务稳定性（AI 知道标准组合模式，不需要每任务从零摸索）
- **长期**：适合作为"业务知识 → AI 行为"的沉淀通道；新员工 / 新模型切换可复用
- **可量化**：阶段 3 评测脚本可直接出数字，决策有据
- **风险**：
  - skill 内容过时：业务变化后未及时更新，反而误导 AI
  - skill 过长：挤占有效 context，需严格控制 < 2000 tokens

---

## 跨方向考量

### 优先级

按借鉴价值与依赖关系（非时间承诺）：

1. **方向 3**（Skill 沉淀）— 短周期可验证，无外部依赖
2. **方向 2**（状态管理）— 中周期，需沙箱改造，是 wide research 类场景的能力底座
3. **方向 1**（加工边界）— 待评估数据出来后再决定，**不**作为当前迭代目标

具体落地时长由后续实施计划决定，本调研不预设。

### 共同设计原则

- **不破坏现有契约**：`CodeInterpreter` 默认单次执行语义保持不变，session 是 opt-in；`SearchProvider` 接口不变
- **不引入新隐式行为**：所有新增能力都通过显式 helper / 显式 SKILL.md 引导
- **不预先优化**：方向 2 第一阶段**只**做 file serde，不并行引入 REPL；方向 1 不做完全原子化
- **不预设时间表**：本调研不提供落地时长或工作拆解；具体时长由后续实施计划决定

### 与现有架构的关系

| SaC 概念 | 本项目对应物 | 差距 |
|---------|------------|------|
| Agentic Search SDK | `RetrievalDataPlane` + `search` + reranker | ✅ 已有，需补 Skill（方向 3） |
| Compute sandbox | `code-interpreter` | ⚠️ 缺 session / 状态持久化（方向 2） |
| Models as control plane | LLM provider + ReAct state machine | ⚠️ 状态机驱动 vs 代码生成驱动；暂不重构 |
| Skills | `prompts/` 目录 | ❌ 缺结构化 skill 文档（方向 3） |
| Autoresearch loop | 无 | **不**在当前路线（见下） |

> 关于 autoresearch：**现有架构评审（`docs/architecture-review-2026-06.md`）已明确"不引入自动评测作为常规机制，避免增加额外成本与不稳定因素"**。autoresearch 本质是自动评测的强化版，与该决策方向冲突，故本调研不将其纳入考虑；若未来该决策松动，可重新评估。

### 原语索引（轻量版，建议/示例，非规范）

> AI 当前可调用的 7 个原子能力概览。详细 schema / 参数约束请直接读 crate 源码，不在本调研范围。延迟 / 成本列为粗略量级估计，**未实测**，仅供排序决策参考。

| 原语 | 一句话用途 | 典型延迟 | 典型成本 |
|------|----------|---------|---------|
| `bm25`（`search_bm25`） | 关键词 / 编号 / 法律条款等精确匹配 | ~10-50ms | 低（Postgres FTS） |
| `text_dense`（`search_text_dense`） | 语义相似 / 同义改写 / 跨段落意图 | ~50-200ms | 中（embedding + Milvus） |
| `multimodal`（`search_multimodal`） | 图像 / 图表 / caption 检索 | ~200-500ms | 中高（CLIP 编码 + Milvus） |
| `graph`（`search_graph`） | 实体关系 / 股权 / 上下游链路 | ~100-500ms（hop 线性增长） | 高（图遍历 + 向量 + 文本） |
| `search`（`SearchProvider::execute_search`） | 公网 / 行业新闻 / 外部资料 | ~500ms-2s | 按调用付费 |
| `reranker`（`avrag_llm::RerankerClient`） | 多路召回融合后的精排 | ~100-300ms | 按调用付费 |
| `code_interpreter`（`avrag-code-interpreter`） | 自定义后处理 / 跨 turn 状态 / 计算 | 0 base + 执行时间 | ~0（本地 CPU） |

---

## 参考

- Perplexity Research (2026-06-01). *Rethinking Search as Code Generation*. https://research.perplexity.ai/articles/rethinking-search-as-code-generation
- Perplexity Research (2025-09). *Architecting and Evaluating an AI-First Search API*. https://research.perplexity.ai/articles/architecting-and-evaluating-an-ai-first-search-api
- Perplexity Research. *Designing, Refining, and Maintaining Agent Skills at Perplexity*. https://research.perplexity.ai/articles/designing-refining-and-maintaining-agent-skills-at-perplexity
- 项目内相关文档：
  - `docs/architecture-review-2026-06.md` — RAG / Chat / WebSearch 当前架构评审
  - `docs/dev-plan-2026-06.md` — Evidence Gate / focus mode 等当前实施计划
  - `crates/rag-core/src/evidence_gate.rs` — 现有证据门控层
  - `crates/code-interpreter/src/lib.rs` — 现有沙箱实现
