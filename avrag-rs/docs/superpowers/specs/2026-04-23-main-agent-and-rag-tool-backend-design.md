> **⚠️ 术语过时**：本文档使用 "Main Agent" 术语，已被 `2026-05-12-architecture-baseline.md` 的 "UnifiedAgentService" 取代。三独立 Agent 架构已替代单体式 Main Agent。

# Main Agent 与 RAG 工具后端架构设计

> **状态**: Draft for Review; 2026-04-26 backend v1 boundary implemented for `Main Agent -> ExecutePlanRequest -> RAG API`.
> `clarify_needed` is legacy `RagPlan` display compatibility only and is not part of the execute-plan contract.
> **最新架构收口**: [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md)
> **来源**: [2026-04-23-rag-tool-backend-and-agent-control-discussion.md](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-23-rag-tool-backend-and-agent-control-discussion.md)
> **记忆层方案**: [2026-04-25-main-agent-memory-and-context-design.md](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-25-main-agent-memory-and-context-design.md)

## 1. 目标

在不改变“RAG 是产品核心能力”这一前提下，把当前后端里混合存在的：

- planning
- clarify
- retrieval
- answer

重新拆成两个清晰边界：

1. `Main Agent`
2. `RAG API`

目标不是继续优化当前 graphflow 的 prompt，而是把系统边界改成：

- `Main Agent` 决定“问还是查”
- `RAG API` 只负责“按计划执行检索”；它可以调用模型完成三元组抽取、query entity extraction、relation/path rerank、chunk rerank 等检索子任务，但这些是 bounded retrieval operators，不是用户级 agent 行为。

这样可以同时支持：

- 前端内置的产品级主控 agent

---

## 2. 现状与问题

当前聊天主链位于：

- [graphflow.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/chat/graphflow.rs)
- [graphflow_tasks_rag.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/chat/graphflow_tasks_rag.rs)
- [planner.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/planner.rs)
- [synthesizer.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/synthesizer.rs)

当前实现更像：

- `planner -> retrieval pipeline -> answer synthesizer`

问题在于：

1. `planner` 同时承担了：
   - 指代消解
   - 是否 clarify
   - 检索计划生成
2. `planner` 会吃到 session summary 和 recent messages，一旦历史失败状态进入上下文，就会被持续放大。
3. `clarify_needed=true` 会直接短路检索，导致错误判断扩散到整条链路。
4. 这种链路适合“后端自带 agent”，不适合“RAG 作为可复用检索工具后端”。

因此，当前问题不是局部 prompt 不够好，而是职责边界过宽。

---

## 3. 设计原则

本设计固定以下原则：

1. `RAG API` 不是用户级 agent，只是工具 backend；它可以包含模型辅助检索算子。
2. `clarify` 是 `Main Agent` 的自然语言回合，不是 RAG 契约的一部分。
3. `Main Agent` 自身承担 orchestration，不再单独引入一个“传话式 orchestrator”组件。
4. 内部前端 agent 与外部 assistant agent，统一向 `RAG API` 发送 `plan schema`。
5. workspace 内的指代消解，优先依赖 `docscope + doc metadata`，session working state 只做补充。
6. 用户偏好记忆只影响表达风格，不能把历史失败状态当作事实条件带回 planning。

---

## 4. 目标架构

### 4.1 角色划分

#### `Main Agent`

负责：

- 接收用户输入
- 加载对应 mode 的 skill
- 读取用户偏好、session working state、recent turns、docscope metadata
- 决定：
  - 直接 clarify
  - 直接普通 chat
  - 还是输出 `plan schema` 调用 `RAG API`
- 消费 `RAG API` 返回的 retrieval bundle
- 基于 answer skill 组织最终用户回复

#### `RAG API`

负责：

- 接收 `plan schema`
- 执行 BM25 / text dense / multimodal dense / graph relation retrieval
- 执行必要的 query entity extraction、relation/path rerank、chunk rerank、summary injection 与 citation packaging
- 返回 retrieval bundle

不负责：

- 用户自然语言理解
- clarify 决策
- 多轮对话状态管理
- session memory 管理
- 最终回答生成策略

#### 外部 provider agent

仅在 `websearch` 模式下使用。

其内部能力不由本设计约束，但如果需要调用站内知识检索，也必须走同一个 `plan schema -> RAG API` 契约。

### 4.2 运行形态

```text
frontend user
  -> Main Agent
      -> clarify to user
      -> or build plan schema
           -> RAG API
                -> retrieval bundle
           -> Main Agent answer skill
                -> final user answer
```

---

## 5. 模式与 skill 映射

前端模式不再对应“不同后端链路”，而是对应“同一个 Main Agent 的不同 skill profile”。

### 5.1 RAG 模式

- 输入到 `Main Agent`
- 加载 `plan skill`
- 如果需要检索，则输出 `plan schema` 给 `RAG API`
- 拿到 retrieval bundle 后，再由 `answer skill` 组织答案

### 5.2 Chat 模式

- 输入到 `Main Agent`
- 加载 `chat skill`
- 不默认走 `RAG API`
- 共享同一个用户级 Main Agent 和记忆体系

### 5.3 WebSearch 模式

- 走外部 provider agent
- 不作为 `RAG API` 的职责范围

---

## 6. RAG API 的职责边界

### 6.1 输入

`RAG API` 只接受结构化执行计划及执行上下文，例如：

- `plan_version`
- `doc_scope`
- `items`
- `bm25_keywords`
- `query_entities`
- `graph_hints`
- `summary_mode`
- 可选预算
- ACL / trace metadata

它不应再接受：

- session history
- session summary
- `clarify_needed`
- `clarify_message`

### 6.2 输出

`RAG API` 只返回 retrieval bundle，例如：

- `chunks`
- `citations`
- `relation_paths`
- `graph_supported_chunks`
- `summary_chunks`
- `score_breakdown`
- `coverage`
- `degrade_trace`
- `backend_trace`

它不应再输出：

- 是否要澄清
- 最终用户自然语言回答
- 对话级决策结论

### 6.3 设计意图

这样做的直接收益是：

1. `RAG API` 可被任意 agent 复用，而不强绑当前产品的对话策略。
2. 检索后端能保持确定性和可测性。
3. planning / answer 的升级不会反复侵入 retrieval backend。
4. vector graph rag 所需的模型调用被限定为检索算子，避免把第二个 agent 放回后端。

---

## 7. Main Agent 的定位

### 7.1 用户级 Main Agent

前端产品侧采用：

- 一个用户账号对应一个全局 `Main Agent`

它是用户在产品内的统一入口，持有：

- 极简用户偏好记忆
- 当前 session 的短期对话上下文
- 当前 workspace 的局部上下文
- 当前 mode 对应的 skill profile

### 7.2 Main Agent 的职责

`Main Agent` 负责以下四类能力：

1. `intent routing`
   - 当前输入走 chat、rag 还是 websearch
2. `clarify`
   - 在信息不足时直接自然语言追问
3. `planning`
   - 在信息足够时输出 `plan schema`
4. `answer`
   - 基于 retrieval bundle 生成最终回复

### 7.3 clarify 的处理方式

当 `Main Agent` 需要澄清时：

- 不调用 `RAG API`
- 不向用户暴露 JSON schema
- 直接返回自然语言问题

因此，`clarify` 是 Main Agent 的对话行为，不是工具协议。

---

## 8. Memory 与上下文控制

本设计不再引入 workspace 长期记忆或 `memvid` 作为 v1 主链依赖。

原因是产品侧已经提供显式、可管理的记忆能力：

- session 历史
- 聊天记录
- 内容源
- 笔记管理
- dashboard
- workspace 内搜索
- 全局搜索
- RAG 文档索引

因此，`Main Agent` 的记忆层只承担三个职责：

1. 保存少量用户偏好，用于改善表达风格和交互体验。
2. 组织当前会话上下文，用于多轮指代消解。
3. 每日增量 consolidation，把跨 workspace 的交互信号沉淀成少量长期偏好。

详细方案见：[Main Agent 记忆层与业务场景适配方案](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-25-main-agent-memory-and-context-design.md)。

### 8.1 指代消解优先级

workspace 内的解释优先顺序固定为：

1. 当前用户最新问题
2. 当前前端选中的 `docscope`
3. `docscope` 对应的 `document metadata`
4. 当前问题中的显式实体、文件名、时间范围
5. session working state
6. 最近 `3-4` 轮相关对话
7. 用户偏好记忆

这意味着：

- `docscope + metadata` 是主指代消解器。
- session working state 和 recent turns 只是补充信号。
- 用户偏好记忆不能参与事实判断。

### 8.2 用户偏好记忆

用户偏好记忆只保存稳定偏好，例如：

- 语言偏好
- 回答长短偏好
- 格式偏好
- 技术深度偏好
- 常用环境约束

写入规则：

- 用户明确要求“记住”或在设置页保存偏好时，可立即写入 active preferences。
- 默会偏好不按轮实时抽取，而是通过每日增量 consolidation 生成。
- consolidation 只读取上次运行后的新增跨 workspace 会话和现有偏好文件。
- consolidation 有新增偏好才追加；没有新增偏好则无输出。
- 不自动把文档事实、RAG evidence、assistant 回答或失败结论写入记忆。

长期偏好以文本文件保存，例如 `user-preferences.md`：

- `Active Preferences` 用于运行时注入，可重写以保持干净。
- `Daily Consolidation Log` 用于审计，append-only。
- 用户删除或禁用的偏好进入 blocked list，后续 consolidation 不得重新生成同义偏好。

### 8.3 session working state

session working state 是当前会话的轻量状态，不是长期记忆。

它可保存：

- 当前 topic
- 上一个明确文档
- 上一个明确实体
- 当前 unresolved question

它不保存：

- 旧失败结论
- 完整 assistant 回答
- RAG 文档事实
- 可由产品显式搜索或内容源系统管理的信息

### 8.4 mode-specific context

`RAG planning` 输入：

- 当前用户问题
- `docscope`
- `document metadata`
- session working state
- 最近相关用户消息
- `plan skill`

`RAG answer` 输入：

- 当前用户问题
- planning 阶段解析后的独立问题
- retrieval bundle
- `answer skill`
- 少量用户表达偏好

`Chat` 输入：

- 当前用户问题
- 用户偏好
- session working state
- 最近 `3-4` 轮对话
- `chat skill`

`Authoritative Context` 与 `Reference Context` 必须分区拼接。历史和记忆不能混入 RAG evidence。

---

## 9. 对当前后端的迁移含义

当前后端里与 agent 语义耦合最深的部分主要在：

- [graphflow.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/chat/graphflow.rs)
- [graphflow_tasks_rag.rs](/home/chuan/context-osv6/avrag-rs/crates/app/src/chat/graphflow_tasks_rag.rs)
- [planner.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/planner.rs)
- [response.rs](/home/chuan/context-osv6/avrag-rs/crates/rag-core/src/runtime/response.rs)

按本设计，后续需要逐步把这些职责拆开：

1. `planning` 从 RAG backend 中退出
2. `clarify` 从 RAG backend 契约中删除
3. `answer synthesis` 从“检索后端默认职责”变成“Main Agent 可选上层能力”
4. `RAG API` 只保留 plan execution 和 retrieval bundle 组装

这不是一次性推翻当前 graphflow，而是把当前链路逐步收缩为工具执行内核。

---

## 10. 非目标

本设计当前不解决：

1. `plan skill` 的具体 prompt、rewrite、subquery、keyword 细节
2. 外部 provider agent 的内部能力设计
3. 前端 UI 的 mode 交互细节
4. WebSearch provider 的供应商选型
5. `memvid` 或其他隐式长期记忆底座接入

这些都放到后续专项设计中单独处理。

但有一个前置约束已经确定：

- `plan skill` 的 v0 不从零重写，而是先从现有 [planner.rs](/home/chuan/context-osv6/avrag-rs/crates/llm/src/planner.rs) 中的 system prompt 提炼出来，再在新边界下逐步收缩和升级。

---

## 11. 成功标准

当以下条件成立时，可视为本架构收口完成：

1. `RAG API` 已不再接收 session history 或 clarify 语义输入。
2. 前端产品中的 `RAG` 模式与 `Chat` 模式均经过同一个 `Main Agent`。
3. `clarify` 已完全由 `Main Agent` 用自然语言处理。
4. workspace 内的主要指代消解已优先依赖 `docscope + metadata`。
5. 用户偏好记忆只影响表达风格，不影响检索范围、事实判断或 citation。
6. `memvid` 不作为 v1 主链依赖。
