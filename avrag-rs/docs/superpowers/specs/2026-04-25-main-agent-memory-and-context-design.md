> **⚠️ 部分过时**：本文档 §4.4 "session working state"、§8.3 "session working state"、§10 "长期偏好生成机制" 等概念已被 `2026-05-12-architecture-baseline.md` 的三层模型取代。工作记忆层已完全移除。

# Main Agent 记忆层与业务场景适配方案

> **状态**: Draft for Review; 2026-04-26 minimal backend memory v1 implemented without `memvid`.
> Current implementation stores `AgentPreferenceMemory` in `user_profiles.custom_preferences.agent_memory` and keeps document facts/RAG evidence out of preference extraction.
> **最新架构收口**: [2026-04-26 Current Product Architecture](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md)
> **关联设计**: [2026-04-23-main-agent-and-rag-tool-backend-design.md](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-23-main-agent-and-rag-tool-backend-design.md)

## 1. 目标

本方案只解决产品内 `Main Agent` 的对话体验问题：

1. 多轮对话中能稳定理解“这个 / 上一个 / 刚才那份 / 继续”等指代。
2. `RAG`、`Chat`、`WebSearch` 三种模式能加载不同 skill 并执行不同动作。
3. 多种上下文拼接时，避免历史失败、旧回答、旧摘要污染当前任务。
4. `RAG` 模式能够稳定生成 `plan schema`，调用 `RAG API`，再基于 retrieval bundle 回答。
5. 仅保留极简用户偏好记忆，用于改善表达风格和交互体验。

本方案不把 `Main Agent` 设计成长时间自主执行任务的 agent，也不引入复杂长期记忆数据库。

---

## 2. 产品前提

Context OS 已经在产品层提供大量显式、可管理的记忆能力：

- session 历史
- 聊天记录
- 内容源
- 笔记管理
- dashboard
- workspace 内搜索
- 全局搜索
- RAG 文档索引

这些能力已经承担了“事实记忆”和“知识资产管理”的主要职责。

因此，agent 记忆层不应再复制一套隐式知识库。它只负责：

1. 保存少量用户偏好。
2. 组织当前会话上下文。
3. 辅助 mode-specific skill 执行当前任务。

存储安排：

- 需要权限、删除、审计或事务一致性的记忆是产品数据，使用 Postgres 作为真源。
- 需要语义召回的记忆可以同步一份到 Milvus，但 Milvus 不作为用户偏好、权限或审计真源。

---

## 3. 核心结论

`Main Agent` 的记忆层不是“记住更多历史”，而是一个上下文控制机制：

```text
Current Task
  + Mode
  + Authoritative Context
  + Reference Context
  + User Preference Memory
  + Mode Skill
  -> Main Agent action
```

其中：

- `Authoritative Context` 是权威上下文，例如 `docscope`、`document metadata`、`RAG Evidence`。
- `Reference Context` 只用于指代消解和对话连续性，例如最近几轮对话和 session working state。
- `User Preference Memory` 只用于表达风格和交互偏好，不参与事实判断。
- `Skill` 是执行方法，不是事实来源。

---

## 4. 上下文类型

### 4.1 Current Task

当前用户最新输入，是本轮最高优先级任务。

任何历史、记忆、skill 都不能覆盖当前用户的明确要求。

### 4.2 Mode

当前 mode 决定 `Main Agent` 的动作：

- `RAG`: 先加载 `plan skill` 生成计划，再加载 `answer skill` 回答。
- `Chat`: 加载 `chat skill` 做普通聊天互动。
- `WebSearch`: 走外部 provider agent，不作为 `RAG API` 的职责范围。

### 4.3 Authoritative Context

权威上下文用于决定事实边界。

`RAG planning` 阶段：

- 当前前端选中的 `docscope`
- `docscope` 对应的 `document metadata`
- 当前问题中的显式文档、实体、文件名、时间范围

`RAG answer` 阶段：

- `RAG API` 返回的 retrieval bundle
- chunks
- citations
- coverage
- backend trace

`Chat` 阶段一般没有事实权威上下文；如果用户要求文档事实，应引导进入 `RAG` 或明确说明当前没有证据。

### 4.4 Reference Context

参考上下文只用于多轮连续性，不作为事实证据。

v1 只保留：

- session working state
- 最近 `3-4` 轮对话
- 最近明确提到的文档、主题、实体

进入 `RAG planning` 前，应过滤重复失败模板和无关 assistant 回复。

### 4.5 User Preference Memory

用户偏好记忆只保存稳定偏好，例如：

- 语言偏好
- 回答长短偏好
- 格式偏好
- 技术深度偏好
- 常用环境约束

它只能影响表达风格，不能影响：

- 检索范围
- 是否调用 RAG
- 是否需要 clarify
- factual grounding
- citation 判断

### 4.6 Skill

Skill 是“怎么执行当前 mode”的方法说明。

Skill 不是事实来源，不保存用户事实，也不保存文档内容。

v1 固定三类 skill：

1. `plan skill`
2. `answer skill`
3. `chat skill`

---

## 5. 基础系统提示词规则

`Main Agent` 的基础系统提示词应短而稳定，只定义上下文身份和优先级。

示意规则：

```text
You are the Main Agent for Context OS.
Your job is to execute the current user task according to the current mode.

Context types:
- Current Task is the latest user message and has the highest priority.
- Mode decides whether you should chat, plan, answer, or use web search.
- Conversation History is for reference resolution and continuity only.
- User Preference Memory is for style and interaction preferences only.
- Skills describe how to perform the current mode; they are not evidence.
- RAG Evidence, when present, is the only source for grounded factual RAG answers.

Conflict rules:
- Follow the Current Task over history, memory, and skills.
- Trust docscope and document metadata over conversation history.
- Do not treat prior assistant failures as evidence that retrieval is impossible.
- In RAG answer mode, use only RAG Evidence for factual claims.
```

---

## 6. RAG 模式

`RAG` 模式分成两个 agent turn。

### 6.1 RAG Planning Turn

输入：

- `Current Task`
- `docscope`
- `document metadata`
- session working state
- 最近相关用户消息
- `plan skill`

输出：

- 可执行 `plan schema`
- 或自然语言 clarify

规则：

- 只生成检索计划，不回答用户。
- history 只用于指代消解。
- 不读取完整 assistant 历史回答。
- 不把旧失败、旧“找不到文档”、旧 clarify 当作当前事实。
- `docscope + document metadata` 是 workspace 内主指代消解器。

### 6.2 RAG Tool Call

输入：

- `plan schema`
- server-side validated `doc_scope`
- ACL / trace metadata

输出：

- retrieval bundle

`RAG API` 仍然是工具后端，不接收 session history、memory、clarify 语义或用户原始历史。它可以执行 bounded model-assisted retrieval，例如 query entity extraction、graph relation/path rerank 和 chunk rerank。

### 6.3 RAG Answer Turn

输入：

- `Current Task`
- planning 阶段解析出的独立问题
- retrieval bundle
- `answer skill`
- 少量用户表达偏好

输出：

- 最终自然语言回答

规则：

- 事实只来自 retrieval bundle。
- memory 只能影响语言、格式、长度等表达方式。
- evidence 不足时必须明确说明不足。
- 不暴露内部 plan、tool call 或 hidden reasoning。

---

## 7. Chat 模式

`Chat` 模式用于普通助手互动。

输入：

- `Current Task`
- 用户偏好
- session working state
- 最近 `3-4` 轮对话
- `chat skill`

规则：

- 可以使用 conversation history 保持自然连续性。
- 可以使用用户偏好调整表达方式。
- 不默认调用 `RAG API`。
- 如果用户问题需要 workspace 文档事实，应切换到 `RAG` 或说明当前没有文档证据。
- 不假装读取过未提供的文档。

---

## 8. WebSearch 模式

`WebSearch` 模式走外部 provider agent。

本设计只约束产品侧接收和展示结果：

- 搜索事实来自 web sources。
- 用户偏好只能影响表达方式。
- 如果外部 provider 需要调用站内知识库，仍必须走 `plan schema -> RAG API` 契约。

---

## 9. 上下文拼接顺序

所有 mode 使用统一 envelope，避免模型混淆：

```text
<System>
Main Agent base rules

<Mode>
rag_plan | rag_answer | chat | websearch

<Current Task>
用户最新消息

<Authoritative Context>
docscope / metadata / RAG evidence

<Reference Context>
session working state / recent turns

<User Preference Memory>
已筛选的稳定偏好

<Skill>
当前 mode 的执行规则

<Output Contract>
JSON schema 或自然语言回答要求
```

其中 `Authoritative Context` 与 `Reference Context` 必须分区，不能混写。

---

## 10. 长期偏好生成机制

v1 只生成用户偏好长期记忆，不生成 workspace 长期记忆。

长期偏好采用“每日睡眠式 consolidation”，而不是每轮对话结束后实时抽取。

### 10.1 生成节奏

白天的即时互动只使用：

- session working state
- 最近 `3-4` 轮对话
- 当前 mode skill
- LLM 自身的通用对话能力

每天固定时间运行一次 preference consolidation：

```text
Daily Preference Consolidation
  -> 读取上次 consolidation 之后的新增跨 workspace 会话
  -> 读取现有 user preference memory
  -> LLM 判断是否出现新的稳定交互偏好
  -> 有增量则追加到 user-preferences.md
  -> 无增量则不写
```

这里的“跨 workspace”只用于提取通用交互偏好，不用于抽取 workspace 事实。

### 10.2 输入范围

consolidation 只读取：

- 新增用户消息
- 必要的 assistant response summary
- mode metadata
- workspace metadata 的轻量标签
- 现有 `user-preferences.md`
- 用户已删除或禁用的偏好列表

consolidation 不读取：

- 文档事实
- RAG chunks
- RAG evidence 正文
- citations 内容
- web search source 正文
- 完整 assistant 长回答
- 检索失败结论或 bug 状态
- 临时任务状态
- 用户一次性要求

### 10.3 LLM 抽象任务

consolidation LLM 的任务边界必须很窄：

```text
Extract only durable interaction preferences.
Do not infer personality, emotion, motivation, sensitive traits, or private facts.
Do not extract project facts, document facts, bug state, or temporary task requirements.
If a signal is already covered by existing preferences, output no change.
If uncertain, output no change.
```

允许输出的偏好类型：

- response language
- response length
- response structure
- technical depth
- risk-control style
- planning / implementation sequencing preference
- clarification preference

禁止输出：

- 人格判断
- 心理状态判断
- 敏感属性推断
- 项目事实
- 文档事实
- workspace 状态
- 检索或工具失败结论

### 10.4 增量判断

LLM 必须对每个候选判断为以下之一：

```text
new_preference
strengthen_existing
conflict_or_override
no_change
```

写入规则：

- `new_preference`: 追加新的 active preference。
- `conflict_or_override`: 追加 override 记录，并将旧偏好标记为 superseded。
- `strengthen_existing`: 可追加 evidence log，但不新增运行时偏好。
- `no_change`: 不写文件。

### 10.5 显式偏好路径

用户明确表达偏好时，不必等待每日 consolidation：

1. 用户说“记住我喜欢……”
2. 用户说“以后都……”
3. 用户在设置页保存偏好

这类偏好可以直接写入 active preferences，并在当天 consolidation 时参与去重和合并。

### 10.6 文本文件格式

长期偏好记忆使用单一文本文件，例如 `user-preferences.md`。

推荐结构：

```md
# User Preference Memory

## Active Preferences

- [P-001] 用户偏好中文回答，除非当前任务明确要求其他语言。
  - category: language
  - scope: user_global
  - confidence: high
  - source: explicit
  - updated_at: 2026-04-25

- [P-002] 用户偏好先用业务语言解释方案，再进入技术实现。
  - category: response_structure
  - scope: user_global
  - confidence: medium
  - source: inferred_daily_consolidation
  - updated_at: 2026-04-25

## Superseded Preferences

- [P-000] ...

## Daily Consolidation Log

### 2026-04-25

Added:
- [P-002] 用户偏好先用业务语言解释方案，再进入技术实现。
  - evidence: 多次要求先做 to-be 业务方案，不做 as-is 代码 review
  - confidence: medium

No Change:
- P-001 already covers today's language preference signals.
```

`Active Preferences` 可重写，用于保持运行时 prompt 干净。
`Daily Consolidation Log` append-only，用于审计和解释来源。

### 10.7 冷启动

默认从用户启用偏好记忆当天开始记录，不回扫所有历史。

可选 one-time backfill：

- 必须用户授权。
- 只处理最近 `7` 天或最近 `N` 个 session。
- 只提取交互偏好。
- 不提取文档事实、项目事实或工具失败状态。

### 10.8 删除与可见性

- 用户偏好必须可在 UI 查看。
- 用户可以删除或修改偏好。
- 当前用户消息可以临时覆盖已保存偏好。
- 被删除或禁用的偏好必须进入 blocked list，后续 consolidation 不得重新生成同义偏好。

---

## 11. RAG Tool 稳定性约束

`Main Agent` 调用 `RAG API` 前后应有确定性校验：

1. `plan schema` 必须可结构化解析。
2. `doc_scope` 由系统填充或校验，不完全信任 LLM。
3. plan item 数量限制为 `1-4` 个。
4. item payload 只能是允许字段，例如 `query`、`bm25_terms`、`summary_mode`。
5. out-of-scope 文档必须拒绝或修正。
6. 空 plan 不调用 RAG，转自然语言 clarify。
7. retrieval bundle 为空时，answer skill 只能说明证据不足。
8. 每次回答保留 coverage、citations、trace，便于诊断。

---

## 12. memvid 结论

`memvid` 不作为 v1 记忆层。

原因：

1. 当前产品已经有显式、可管理的事实记忆系统。
2. v1 目标只是用户偏好和多轮指代消解，不需要额外的向量化长期记忆库。
3. 引入 `memvid` 会形成第二套隐式记忆，增加污染、解释和删除成本。

后续若需要跨 workspace 的可携带长期记忆，可单独评估 `memvid`。但它不得进入 `RAG planning` 的事实判断链路，也不得替代产品内的内容源、笔记、搜索和 RAG 索引。

---

## 13. 验收标准

1. `RAG planning` 能稳定用 `docscope + metadata + 少量 reference context` 消解指代。
2. 旧失败回答不会导致后续 planning 再次拒绝检索。
3. `RAG answer` 的事实性内容只来自 retrieval bundle。
4. `Chat` 模式能用最近上下文和用户偏好保持自然连续性。
5. 用户偏好可见、可删、可被当前消息临时覆盖。
6. 不引入 `memvid` 或其他隐式长期记忆作为 v1 主链依赖。
