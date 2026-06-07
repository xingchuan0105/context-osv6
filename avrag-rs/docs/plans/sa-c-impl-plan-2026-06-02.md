# SaC 改造实施计划（开发计划版）

> 来源研究：[Perplexity Search-as-Code 借鉴分析](./perplexity-sac-learnings-2026-06-02.md)
> 日期：2026-06-02
> 状态：待执行
> 范围：基于 `avrag-rs` 当前 RAG / WebSearch / Code Interpreter 实现，对照 SaC 架构落地 3 个工作包
> 本文件**不**替换上游研究 doc；研究 doc 保留作为决策溯源

---

## 1. 目标与范围

### 1.1 目标

本计划要达成 3 个具体目标：

1. **提升 RAG 工具选择准确率** — 把分散在工程师头脑中的"何时用哪个后端"经验沉淀为 Agent Skill，让 AI 在 4 个 RAG 后端 + 重排上做出更准确的选择。
2. **支撑长任务（≥5 步）执行** — 给现有 `code_interpreter` 加 session 抽象与 file-based 持久化，让多步任务能跨 turn 保持状态。
3. **明确 Web Search 加工边界** — 在不引入新外部依赖的前提下，定义 Brave `synthesized_answer` 与自加工之间的切换策略。

### 1.2 范围

**包含**：

- WP1：Agent Skill 沉淀（≈ Sprint N+1）
- WP2：沙箱 session 抽象与持久化（≈ Sprint N+2）
- WP3：Brave 加工边界评估与可选切换（≈ Sprint N+3，评估先行）

**不包含（约束）**：

| 约束 | 理由 | 来源 |
|------|------|------|
| 不引入 autoresearch loop | 与"不引入自动评测作为常规机制"决策冲突 | `architecture-review-2026-06.md` §一 |
| 不做完全原子化 Brave | parsing 失败 / 延迟代价（已在研究 doc 中与项目 owner 对齐） | 研究 doc §1.3 |
| 不做 REPL 模式 | Perplexity 实验结论 file serde 优于 REPL | 研究 doc §2.3 |
| 不拆更细的 tool schema | 当前 4 后端粒度合理 | 研究 doc §3.3 |
| 不引入抽样 judge 作为常规评测 | 与现有评测策略冲突 | `architecture-review-2026-06.md` §一 |
| 不修改 Evidence Gate / focus mode 主链路 | 与现有架构评审决策冲突 | `architecture-review-2026-06.md` §一 |

### 1.3 与现有架构评审的关系

本计划不修改 `architecture-review-2026-06.md` 的核心决策：

- Evidence Gate 保留为在线证据门控层
- RAG / WebSearch 继续采用"检索后直接 grounded answer"的主链路
- focus mode 作为可选优化层保留
- 评测以人工 E2E 回归为主

本计划的所有改动都是**在现有架构内的能力补强**，不涉及主链路重构。

---

## 2. 现状与差距

### 2.1 模型控制面（Models）

当前 LLM 通过 ReAct 状态机（`SearchStrategy` / `RagStrategy`）驱动 tool call。决策粒度限于"在哪个状态调哪个 tool"，无法在 trajectory 中根据中间结果动态调整策略。**差距**：复杂任务的 in-flight 调整能力不足；编排经验沉淀缺失。

### 2.2 计算沙箱（Sandbox）

`avrag-code-interpreter` 已具备 rlimit、import 黑名单、单次执行能力。**结构性缺失**：

- 无 session 抽象：每次 execute 启动新进程，变量 / 文件 / 中间态全部丢失
- 无 file-based 持久化引导：AI 没有"显式 write_file 持久化中间产物"的标准路径
- 无跨 turn 状态管理

后果：多步任务中所有中间证据要么塞回 context（贵、context pollution、撞上限），要么丢失（断链）。

### 2.3 原语层（SDK / Primitives）

`RetrievalDataPlane` 已暴露 4 个后端（text_dense / bm25 / multimodal / graph），`search` crate 暴露 Brave，`RerankerClient` 已可用。AI 当前共 7 个原子能力：

| 原语 | 一句话用途 | 典型延迟 | 典型成本 |
|------|----------|---------|---------|
| `bm25`（`search_bm25`） | 关键词 / 编号 / 法律条款等精确匹配 | ~10-50ms | 低（Postgres FTS） |
| `text_dense`（`search_text_dense`） | 语义相似 / 同义改写 / 跨段落意图 | ~50-200ms | 中（embedding + Milvus） |
| `multimodal`（`search_multimodal`） | 图像 / 图表 / caption 检索 | ~200-500ms | 中高（CLIP 编码 + Milvus） |
| `graph`（`search_graph`） | 实体关系 / 股权 / 上下游链路 | ~100-500ms（hop 线性增长） | 高（图遍历 + 向量 + 文本） |
| `search`（`SearchProvider::execute_search`） | 公网 / 行业新闻 / 外部资料 | ~500ms-2s | 按调用付费 |
| `reranker`（`avrag_llm::RerankerClient`） | 多路召回融合后的精排 | ~100-300ms | 按调用付费 |
| `code_interpreter`（`avrag-code-interpreter`） | 自定义后处理 / 跨 turn 状态 / 计算 | 0 base + 执行时间 | ~0（本地 CPU） |

> 延迟 / 成本列为粗略量级估计，**未实测**。

**本阶段默认方案**（3 个工作包共用）：

| 原语 / 组件 | 默认用法 | 选型理由 |
|------------|---------|---------|
| RAG 4 后端 | 按 `RetrievalDataPlane` trait 暴露，**不**重新拆分 | 4 后端粒度合理，符合"删浅层抽象"原则 |
| Web Search | 以 Brave `synthesized_answer` 为主链路 | 解析 / 反爬成本不应转嫁到我方 LLM（与 owner 对齐） |
| Reranker | 多路召回融合后的标准精排步骤 | 已有 `RerankerClient`，无需新引入 |
| Code Interpreter | 单次执行为默认，session 是 opt-in | 不破坏现有契约 |
| tool schema 粒度 | 维持现状 | 过度拆分增加 AI 选择负担 |

---

## 3. 工作包拆解

### 3.1 工作包 1：Agent Skill 沉淀（WP1）

**对应原研究方向**：方向 3

**目标**：把分散在工程师头脑中的"何时用哪个后端 / 怎么组合"经验沉淀为 Agent Skill（< 2000 tokens），让 AI 在 4 个 RAG 后端 + 重排上做出更准确的选择。

**Owner**：`prompts/` 目录 owner + `RagRuntime` 模块 owner（角色占位，下同）

**依赖**：无

**交付物**：

| ID | 交付物 | 说明 |
|----|--------|------|
| WP1-D1 | `prompts/rag-agent-skill.md` v1 | < 2000 tokens，含 5+ 编排模式 + 反例 + 工具选择决策表 |
| WP1-D2 | skill 注入代码 | 把 SKILL.md 内容拼入 RAG / Search 策略的 system prompt（与 tool schema 并列，**不**替换） |
| WP1-D3 | 最小评测集 | 5-10 个典型 query，对比注入前 / 后的人工评估结果 |
| WP1-D4 | 灰度发布说明 | 适用场景 / 灰度范围 / 回滚方式 |

**起始模式**（v1 内容起点，引用自研究 doc §3.3 阶段 1）：

- 模式 A：跨文档聚合查询
- 模式 B：时间线整理

完整定义见 `perplexity-sac-learnings-2026-06-02.md` §3.3 末尾。v1 需补充至 5+ 模式。

**阶段**：

| 阶段 | 内容 | 改动量 | 风险 |
|:----:|------|:------:|:----:|
| 1 | 撰写 SKILL.md v1 草稿（5+ 模式） | 文档 | 低 |
| 2 | 注入代码 + 灰度开关 | ~50 LOC | 中 |
| 3 | 5-10 query 人工 E2E 评估 | 评测 | 低 |
| 4 | 灰度 + 监控 + 收尾 | 部署 | 低 |

**风险与缓解**：见 §5 风险表。

---

### 3.2 工作包 2：长任务状态管理（WP2）

**对应原研究方向**：方向 2

**目标**：给 `avrag-code-interpreter` 加 session 抽象与 file-based 持久化，让多步任务能跨 turn 保持状态。

**Owner**：`code-interpreter` 模块 owner + `RagRuntime` 模块 owner

**依赖**：WP1（软依赖；WP1 的 skill 可提及"持久化到 session 文件"，会用到 WP2 的能力；WP2 可独立启动）

**交付物**：

| ID | 交付物 | 说明 |
|----|--------|------|
| WP2-D1 | `SandboxSession { id, workspace_dir, history }` 抽象 | 接受 session 参数（不传则维持当前单次执行语义） |
| WP2-D2 | `avrag_sandbox_io` Python 包 | 提供 `read_file / write_file / append_log / list_files` helper |
| WP2-D3 | 沙箱镜像构建流程 | 把 `avrag_sandbox_io` 打包进镜像 |
| WP2-D4 | `prompts/sandbox-session-skill.md` | < 1000 tokens，教 AI 使用 file serde 模式 |
| WP2-D5 | Tool dispatch 条件提示 | 在 `RagRuntime` / `SearchStrategy` 中加入"建议使用沙箱"的条件提示 |

**阶段**：

| 阶段 | 内容 | 改动量 | 风险 |
|:----:|------|:------:|:----:|
| 1 | `SandboxSession` 抽象落地 | ~120 LOC | 低 |
| 2 | `avrag_sandbox_io` 包 + 镜像构建 | ~60 LOC + 镜像 | 低 |
| 3 | `sandbox-session-skill.md` v1 | 文档 | 低 |
| 4 | Tool dispatch 条件提示 | ~40 LOC | 中 |
| 5 | 5-step 长任务 E2E 回归 | 测试 | 中 |

**风险与缓解**：见 §5 风险表。

---

### 3.3 工作包 3：Web Search 加工边界（WP3）

**对应原研究方向**：方向 1

**目标**：在不引入新外部依赖的前提下，明确 Brave `synthesized_answer` 与自加工之间的切换策略。

**Owner**：`search` crate 模块 owner

**依赖**：WP1、WP2 完成后启动（不在当前 Sprint 路线）

**交付物**：

| ID | 交付物 | 说明 |
|----|--------|------|
| WP3-D1 | Brave `synthesized_answer` 评估报告 | 在现有 E2E 集上评估"信息利用率"——是否被后续 LLM 大量改写、是否引入错误前导 |
| WP3-D2 | 决策建议 | 基于评估数据决定走 A（保留 Brave 加工）/ B（自加工）/ C（混合）哪条路径 |
| WP3-D3 | （若选 B/C）`SearchResponse` 扩展 | 暴露 `grounding_raw: Vec<BraveGroundingItem>` 字段 |
| WP3-D4 | （若选 B/C）加工模式选择逻辑 | 根据 query 复杂度动态切换 A / B |

**阶段**：

| 阶段 | 内容 | 改动量 | 风险 |
|:----:|------|:------:|:----:|
| 1 | 评估脚本 + 报告 | ~50 LOC | 低 |
| 2 | 决策评审 | 评审会 | — |
| 3 | （按决策）落地 | 视路径 | 视路径 |

**约束**：2 周时间盒内必须出评估报告，逾期强制决策（即使数据不明确）。

**风险与缓解**：见 §5 风险表。

---

## 4. 资源与依赖

### 4.1 模块 Owner（角色占位）

| 角色 | 涉及的 WP |
|------|----------|
| `prompts/` 目录 owner | WP1, WP2 |
| `RagRuntime` 模块 owner | WP1, WP2 |
| `SearchExecutor` 模块 owner | WP3 |
| `CodeInterpreter` 模块 owner | WP2 |
| LLM 策略 owner | WP1, WP2, WP3（统一调度） |

> 具体人映射由项目 lead 在 Sprint 计划会上完成。

### 4.2 评审资源

| 评审类型 | 涉及内容 | 频率 |
|---------|---------|------|
| 沙箱变更 review | WP2 的 session / 镜像 / 工具变更 | 每次 PR |
| Prompt 变更 review | WP1 / WP2 的 SKILL.md 变更 | 每次 PR |
| 架构变更 review | 任何触及 `architecture-review-2026-06.md` 决策的改动 | 按需 |

### 4.3 测试资源

- 现有 E2E 集（人工回归为主，不引入自动 judge）
- 5-10 query 最小评测集（WP1 自建）
- 5-step 长任务测试集（WP2 自建）
- 内部 baseline 数据（用于"无 skill / 无 session"对比基线）

### 4.4 跨包依赖

- WP1 完成后 WP2 可启动（WP2 的 skill 引用 WP1 的 prompt 注入机制）
- WP2 完成后 WP3 可启动（评估与决策可借助 WP2 的沙箱能力）
- 实际 Sprint 安排按团队容量与依赖确定

---

## 5. 风险与应对

| ID | 风险 | 涉及 WP | 概率 | 影响 | 缓解措施 |
|----|------|---------|------|------|---------|
| R1 | skill 内容过时误导 AI | WP1 | 中 | 中 | 每月 review + owner 维护节奏；建立"过时标记"机制 |
| R2 | skill 挤占有效 context | WP1 | 中 | 中 | 硬性约束 < 2000 tokens；超长模式拆成多文件 |
| R3 | 沙箱 session 脏数据 | WP2 | 中 | 中 | session TTL + 清理任务；按 org 隔离 workspace |
| R4 | AI 假 serde（`json.dumps` lambda 等） | WP2 | 中 | 低 | `avrag_sandbox_io` helper 层做基本校验 |
| R5 | 沙箱镜像构建复杂度 | WP2 | 低 | 中 | 复用现有 image 流程，不引入新的构建系统 |
| R6 | 长任务 E2E 测试不稳定 | WP2 | 中 | 中 | 复用现有 E2E 框架；不稳定用例标记 skip |
| R7 | 与现有 `CodeInterpreter::execute` 不兼容 | WP2 | 低 | 高 | 默认行为不变，session 是 opt-in；现有调用点不需修改 |
| R8 | Brave 自加工 parsing 失败 | WP3 | 高 | 高 | **不**走全自加工路径；与项目 owner 已对齐 |
| R9 | Brave 自加工 P95 延迟回归 | WP3 | 高 | 高 | 走混合路径，默认保留 Brave 加工；自加工仅在特定 query 类型触发 |
| R10 | 评估数据不支持任何路径 | WP3 | 中 | 中 | 2 周时间盒强制决策 |
| R11 | Brave 服务条款风险 | WP3 | 低 | 中 | 选 B/C 路径前法务 / 合规 review |

---

## 6. 里程碑与时间表

> 以下为 Sprint 相对时间占位。**Sprint N+1** 等表述需在 Sprint 计划会上由项目 lead 映射为具体日历日期。

| 里程碑 | WP | 起点 | 完成标志 |
|--------|----|------|---------|
| M1 | WP1 阶段 1 | Sprint N+1 起点 | `prompts/rag-agent-skill.md` v1 草稿 PR 合入 |
| M2 | WP1 阶段 2-3 | Sprint N+1 中 | skill 注入 + 5-10 query 评估完成 |
| M3 | WP1 阶段 4 | Sprint N+1 末 | 灰度发布上线 |
| M4 | WP2 阶段 1-2 | Sprint N+2 起点 | `SandboxSession` 抽象 + 沙箱镜像 |
| M5 | WP2 阶段 3-4 | Sprint N+2 中 | skill + tool dispatch 条件提示 |
| M6 | WP2 阶段 5 | Sprint N+2 末 | 5-step 长任务 E2E 回归通过 |
| M7 | WP3 阶段 1 | Sprint N+3 起点 | 评估脚本 + 报告 |
| M8 | WP3 阶段 2-3 | Sprint N+3 末 | 决策评审 + （按决策）落地 |

**关键依赖**：

- M2 依赖 M1
- M4 可与 M2 并行（WP1 与 WP2 解耦，WP2 不硬等 WP1）
- M5 依赖 M4
- M7 依赖 M3 + M6

---

## 7. 验收标准

### 7.1 WP1 验收

**必达**：

- `prompts/rag-agent-skill.md` v1 合入，且内容 < 2000 tokens
- 注入代码合入，含关闭开关
- 5-10 query 人工 E2E 评估：**tool 选择准确率相比无 skill baseline 提升 ≥ 10pp（绝对值）**
- 现有 E2E 集无回归
- 现有 P95 延迟不恶化 > 5%

**可选**：

- 灰度期间无 P0/P1 事故
- 灰度 1 周后 review 通过

### 7.2 WP2 验收

**必达**：

- `SandboxSession` 抽象合入，默认行为不变（不传 session 维持单次执行）
- 沙箱镜像构建流程合入 CI
- 5-step 长任务 E2E 回归通过率 ≥ 80%
- 现有 E2E 集无回归
- 现有 P95 延迟不恶化 > 5%

**可选**：

- session TTL 清理任务上线
- `sandbox-session-skill.md` v1 人工评估通过

### 7.3 WP3 验收

**必达**：

- 评估报告合入（含数据 + 结论）
- 决策评审会议纪要合入（明确 A / B / C 路径选择，含不同意路径的理由）

**可选**：

- （按决策）落地完成 + 灰度上线

### 7.4 全局回归（每个 WP 落地前必跑）

- 现有 E2E 集无回归
- 现有 P95 延迟不恶化 > 5%
- 现有 token 成本不恶化 > 10%

---

## 8. 后续优化方向

### 8.1 短期（WP1-WP3 完成后 1-2 个 Sprint）

- 评测集扩展：从 5-10 query 扩展到 20-30 query
- skill 模式库扩展：从 5 个模式扩展到 10+ 个
- 长任务 query 集扩展：从 5-step 扩展到 10-step
- Web Search 加工边界的进一步探索（基于 WP3 评估数据）

### 8.2 中期（3-6 个月）

- 持续 SKILL.md 迭代（基于人工评估反馈）
- 与 `dev-plan-2026-06.md` 中其他工作（如 Evidence Gate 调优、focus mode 触发条件）的联动
- 评测流程的轻量化（仍是人工为主，但可考虑半自动辅助标注）

### 8.3 长期 / 待决策

> 以下项目前**不**在路线上，列出仅为决策溯源。如未来相关约束松动，需重新评估。

- **autoresearch loop**：如未来 `architecture-review-2026-06.md` 中"不引入自动评测"决策松动，可重新评估。参见研究 doc §2.3 / §3.3 的"不做"说明。
- **完全原子化 Brave**：与项目 owner 在研究 doc §1.3 已对齐，**不**走此路径
- **REPL 模式**：Perplexity 实验结论 file serde 优于 REPL，**不**走此路径
- **更细粒度 tool schema**：4 后端粒度合理，**不**做
- **状态机 → 代码生成驱动的策略重构**：是大方向但风险 / 投入比当前不合适，远期观察

---

## 参考

- 上游研究：[perplexity-sac-learnings-2026-06-02.md](./perplexity-sac-learnings-2026-06-02.md)
- 架构约束：[architecture-review-2026-06.md](../architecture-review-2026-06.md)
- 当前实施计划：[dev-plan-2026-06.md](../dev-plan-2026-06.md)
- SaC 原文：[Rethinking Search as Code Generation](https://research.perplexity.ai/articles/rethinking-search-as-code-generation)
- 相关 Perplexity 文章：
  - [Architecting and Evaluating an AI-First Search API](https://research.perplexity.ai/articles/architecting-and-evaluating-an-ai-first-search-api)
  - [Designing, Refining, and Maintaining Agent Skills at Perplexity](https://research.perplexity.ai/articles/designing-refining-and-maintaining-agent-skills-at-perplexity)
