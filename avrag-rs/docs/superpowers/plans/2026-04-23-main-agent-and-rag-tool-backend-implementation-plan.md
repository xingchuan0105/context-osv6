# Main Agent 与 RAG 工具后端改造实施方案

> **状态**: Draft for Review
> **2026-04-26 更新**: 本计划的边界仍有效，但最新目标架构增加 Milvus 统一检索数据面与 vector graph rag graph retrieval。见 [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md)。
> **关联设计**: [2026-04-23-main-agent-and-rag-tool-backend-design.md](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-23-main-agent-and-rag-tool-backend-design.md)
> **来源纪要**: [2026-04-23-rag-tool-backend-and-agent-control-discussion.md](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-23-rag-tool-backend-and-agent-control-discussion.md)

## 1. Goal

把当前“后端自带 planning + retrieval + answer”的聊天主链，逐步收缩成：

1. `Main Agent` 负责：
   - mode routing
   - clarify
   - planning
   - answer
2. `RAG API` 负责：
   - execute plan
   - return retrieval bundle
   - run bounded retrieval subroutines such as query entity extraction, relation/path rerank, and chunk rerank

本计划默认：

- 先收口架构边界
- 再落 memory v1
- `plan skill` 细化放到最后单独做，不在本次主改造里展开
- memory v1 收口为用户偏好记忆、每日增量 consolidation、session working state、recent turns 与 docscope metadata 装配，不接入 `memvid`

---

## 2. 成功标准

### 2.1 功能标准

- 内部前端 agent 与外部 assistant agent，统一通过 `plan schema` 调用 `RAG API`。
- `RAG API` 不再依赖 session history、session summary、clarify 状态。
- `clarify` 完全由 `Main Agent` 用自然语言处理。
- `Chat` 模式与 `RAG` 模式共享同一个用户级 `Main Agent`。
- workspace 指代消解优先依赖 `docscope + doc metadata`。

### 2.2 工程标准

- `RAG API` 的输入输出能通过类型和测试稳定约束。
- 旧链路在迁移期间可兼容，但新能力不再建立在 `clarify_needed` 上。
- memory v1 不引入复杂状态机，不接入隐式长期记忆底座。
- 所有阶段都以可验证的边界推进，不一次性大重写。

---

## 3. 当前代码映射

当前需重点迁移的链路位于：

- [graphflow.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/chat/graphflow.rs)
- [graphflow_tasks_rag.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/chat/graphflow_tasks_rag.rs)
- [planner.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/planner.rs)
- [synthesizer.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/synthesizer.rs)
- [planner.rs](/home/chuan/context-osv6/avrag-rs/crates/rag-core/src/runtime/planner.rs)
- [response.rs](/home/chuan/context-osv6/avrag-rs/crates/rag-core/src/runtime/response.rs)
- [response_utils.rs](/home/chuan/context-osv6/avrag-rs/crates/rag-core/src/runtime/response_utils.rs)

当前主要问题：

1. `planner` 与 `clarify` 深度耦合
2. RAG runtime 既做检索又做回答
3. session context 注入点位于后端，而不是主控 agent

---

## 4. 实施策略

采用 **五阶段推进**：

1. Phase 1: 冻结 `RAG API` 工具契约
2. Phase 2: 从后端链路中收缩 agent 语义
3. Phase 3: 引入 `Main Agent` 最小骨架与 mode 路由
4. Phase 4: 落地 memory v1
5. Phase 5: 兼容收尾与旧链路退场

`plan skill` 的细化与评测，不在本计划主线中展开，单独作为后续 Phase。

---

## 5. Phase 1: 冻结 RAG API 工具契约

**目标:** 先把 `RAG API` 的输入输出固定成检索工具语义，避免后续边做边改边界。

### 5.1 任务

1. 定义 `execute-plan` 请求结构
2. 定义 retrieval bundle 响应结构
3. 明确哪些字段不再属于 RAG 契约

### 5.2 设计要求

请求侧至少明确：

- `plan_version`
- `doc_scope`
- `items`
- `summary_mode`
- 可选预算
- ACL / trace metadata

响应侧至少明确：

- `chunks`
- `citations`
- `summary_chunks`
- `coverage`
- `degrade_trace`
- `backend_trace`

明确移出契约的字段：

- `clarify_needed`
- `clarify_message`
- 用户原始问题
- session history
- session summary

### 5.3 建议代码落点

- `crates/common`
- `crates/rag-core`
- `crates/app`

优先新增独立 DTO，而不是在现有 `RagPlan` 上继续叠补丁。

### 5.4 验收

- 新 DTO 能通过单元测试约束字段边界
- `execute-plan` 契约可在文档和类型层稳定表达
- 旧 `clarify_needed` 语义被标记为过渡态，而不是未来方向

---

## 6. Phase 2: 从后端链路中收缩 agent 语义

**目标:** 在不立即引入完整 Main Agent 运行时的前提下，先把当前后端里的 agent 语义剥离出去。

### 6.1 任务

1. 把当前 retrieval 核心收敛为“按计划执行”
2. 把 `clarify` 从 backend 流程里剥离
3. 把 answer synthesis 从 backend 默认职责改成可迁移能力

### 6.2 具体改造点

#### `crates/app/src/chat/graphflow.rs`

- 保留现有 graph 骨架用于兼容
- 逐步拆出：
  - planning 节点
  - answer synthesize 节点
- 让 retrieval 部分可以被单独调用

#### `crates/app/src/chat/graphflow_tasks_rag.rs`

- 现有：
  - `rag_call_planner`
  - `rag_apply_summary_policy`
  - `rag_build_answer_context`
  - `rag_answer_synthesize`
- 目标：
  - 把 `rag_call_planner` 从“后端入口职责”降为兼容适配层
  - 把 retrieval 执行和 bundle 组装收成独立可复用路径

#### `crates/rag-core/src/runtime/*`

- 把 runtime 中强依赖 session 和回答生成的部分逐步迁走
- 保留：
  - plan execution
  - retrieval
  - rerank
  - summary injection
  - citation packaging

### 6.3 验收

- 后端可以仅凭 plan DTO 执行 retrieval 并返回 bundle
- 不再需要 `clarify_needed=true` 才能驱动分支
- retrieval 结果在无 answer synthesize 的情况下也能单独测试

---

## 7. Phase 3: 引入 Main Agent 最小骨架与 mode 路由

**目标:** 为产品侧建立真正的 Main Agent 入口，而不是继续让聊天后端兼任 agent。

### 7.1 任务

1. 建立用户级 `Main Agent` 概念
2. 建立 mode -> skill profile 映射
3. 明确 `RAG` / `Chat` / `WebSearch` 三种分流

### 7.2 目标行为

#### `RAG` 模式

- Main Agent 读取 plan skill
- 需要检索时输出 `plan schema`
- 调用 `RAG API`
- 消费 retrieval bundle
- 由 answer skill 生成用户回复

#### `Chat` 模式

- Main Agent 读取 chat skill
- 默认不进入 `RAG API`

#### `WebSearch` 模式

- 走外部 provider agent

### 7.3 实施建议

- 先做最小骨架和接口层，不急着做复杂 skill 系统
- mode routing 可以先落在 `app` 层服务边界中
- 先把“Main Agent 是谁、接什么输入、产什么输出”做清楚

### 7.4 验收

- `Chat` 与 `RAG` 模式已共享同一用户级 Main Agent 入口
- `clarify` 已在 Main Agent 层自然语言返回
- `RAG API` 不再承担 agent 路由

---

## 8. Phase 4: 落地 memory v1

**目标:** 用最小上下文控制设计解决连续性和指代消解，不再依赖后端 planner 吃 session 污染上下文。

### 8.1 任务

1. 落地用户偏好记忆
2. 落地每日增量 preference consolidation
3. 落地 session working state
4. 落地 recent turns 装配规则
5. 确立 `docscope + metadata` 作为主指代消解器

### 8.2 实现范围

#### session working state

- 当前 topic
- 上一个明确文档
- 上一个明确实体
- 当前 unresolved question

#### recent turns

- 最近 `3-4` 轮相关对话
- `RAG planning` 前过滤重复失败 assistant 回复
- 只用于指代消解和连续性，不作为事实证据

#### 用户偏好记忆

- 语言偏好
- 回答长短偏好
- 格式偏好
- 技术深度偏好
- 常用环境约束
- 用户明确要求“记住”或在设置页保存时可立即写入
- 默会偏好通过每日增量 consolidation 生成

#### 每日增量 consolidation

- 固定时间运行一次
- 只读取上次运行后的新增跨 workspace 会话
- 载入既有 `user-preferences.md` 做对比
- 有新增偏好才追加；无新增偏好则无输出
- 只抽象交互偏好，不抽取项目事实、文档事实或工具失败状态
- `Active Preferences` 可重写以保持运行时 prompt 干净
- `Daily Consolidation Log` append-only，用于审计

#### 指代消解顺序

1. 当前用户最新问题
2. `docscope`
3. `doc metadata`
4. 当前问题显式实体、文件名、时间范围
5. session working state
6. recent turns
7. 用户偏好记忆

### 8.3 非目标

memory v1 不做：

- workspace 长期记忆
- `memvid` 接入
- assistant 回答入长期记忆
- 检索失败结论入记忆
- 全量历史反复扫描
- 每轮对话结束实时偏好抽取

### 8.4 验收

- 在已勾选 `docscope` 的 workspace 内，绝大多数“这份/上一个/刚才那份”问题无需额外靠 summary 才能解释
- 重复失败模板不会连续污染 planning
- 用户偏好只影响表达风格，不影响检索范围、事实判断或 citation
- 每日 consolidation 只对新增会话做增量处理，并能在无新增偏好时保持文件不变

---

## 9. Phase 5: 兼容收尾与旧链路退场

**目标:** 在新边界稳定后，逐步关闭旧的“后端自带 planner/answer”默认路径。

### 9.1 任务

1. 标记旧 `clarify_needed` 契约为 deprecated
2. 清理 backend 中默认的 session summary 注入依赖
3. 将旧 answer synthesis 路径收成兼容层或移除

### 9.2 风险控制

- 不要求一步删除所有旧代码
- 但不允许在新功能上继续扩张旧契约
- 所有新增能力必须站在 `Main Agent -> plan schema -> RAG API` 这条新边界上

### 9.3 验收

- 新旧链路边界清晰
- 新开发默认不再接入 `clarify_needed/items` 老模式
- 旧后端 planner 不再是产品主链核心

---

## 10. 延后项

以下事项明确延后，不在本次主实施方案中展开：

1. `plan skill` 的 prompt、rewrite、subquery、keyword 设计
2. `plan skill` 的 golden set 与评测框架
3. answer skill 的细粒度风格迭代
4. websearch provider 侧能力细节

这些内容等主边界和 memory v1 收口后，再单独做专项方案。

其中 `plan skill` 的起步方式也已提前确定：

- `plan skill v0` 先从现有 [planner.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/planner.rs) 的 system prompt 中摘取和收敛，而不是重新从空白 prompt 设计。
- 后续迭代只在这个基线上逐步调整 rewrite、keyword、subquery 和 clarify 策略。

---

## 11. 建议执行顺序

1. 先冻结 `RAG API` 契约，避免边界继续漂移。
2. 再把后端里的 agent 语义往外剥，先收 retrieval 内核。
3. 然后引入 `Main Agent` 最小骨架和 mode 路由。
4. 再落 memory v1。
5. 最后再做 `plan skill` 的专项打磨。

这个顺序的原因很简单：

- 先改 `plan skill`，会继续绑死在旧边界上
- 先收边界，后续 skill 才能真正独立迭代

---

## 12. 2026-04-26 实现状态快照

### 已实现

- `ExecutePlanRequest` 已成为新 RAG 工具入口，校验包含空 items、超 4 items、payload 二义性、priority 和 budget zero。
- `/api/v1/rag/execute-plan` 在执行前校验 `doc_scope` UUID、可访问性和文档 ready 状态；非法、越权或未 ready 统一映射为 `invalid_doc_scope`。
- `ExecutePlanBudget.total_candidate_budget` 驱动召回候选预算，`final_chunk_budget` 驱动最终 bundle chunk 上限，并写入 `backend_trace.retrieval_trace`。
- 产品 chat 主链路通过 `MainAgentRagPlanDecision::{Execute, Clarify}` 分流；clarify 不调用 RAG API，直接走自然语言回复。
- RAG planning 和 general chat 已使用 `MainAgentContext` envelope，区分 Authoritative Context、Reference Context 和 User Preference Memory。
- `AgentPreferenceMemory` 存放在 `user_profiles.custom_preferences.agent_memory`，并暴露 `GET/PUT/DELETE /api/auth/agent-preferences`。
- 显式偏好写入只处理“记住 / 以后都 / remember that”等明确表达；working memory 不再从 assistant answer 写入 `gathered_facts`。
- worker 已接入每日 agent preference consolidation job；该 job 只读取会话摘要和既有偏好，只抽明确交互偏好，无新增偏好时不写库。
- 未挂图的 `rag_load_session_context` 任务已移除。

### 兼容保留

- `RagPlan` 只作为 legacy display/compat 类型保留，用于 `planner_output` 和旧测试展示。
- `clarify_needed`、`clarify_message` 只属于 legacy `RagPlan` 展示兼容，不属于 `ExecutePlanRequest`，也不进入 `/api/v1/rag/execute-plan` JSON。
- `RagRuntime::plan` 与旧 `synthesize_answer_text*` 已标记为 legacy compatibility；产品 chat 主链不再依赖它们作为默认 planner/answer 路径。

### 延后

- 不接入 `memvid` 或 workspace 长期记忆。
- 不新增本地 `.run/.env.runtime` 配置处理。
- 不做完整前端偏好管理 UI；当前只保证后端 API 可消费。
- `plan skill` 的 golden set、rewrite/subquery 策略和 answer skill 风格细化继续作为后续专项。
