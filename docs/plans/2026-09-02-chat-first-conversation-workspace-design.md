# Chat-first Conversation × Workspace — 定稿设计

- **状态**: Accepted / 实施中
- **日期**: 2026-09-02
- **范围**: 登录后默认对话、全局最近、会话级文件、Conversation ↔ Workspace 关系、Quick Chat 模型角色、BYOK、引用与生命周期
- **权威 IA**: `docs/design/PRODUCT_IA.md` v2
- **关联**: `docs/plans/2026-08-11-lead-rag-web-workers-design.md` · `docs/agent/product-apps.md` T7/T8
- **实施交接**: `docs/plans/2026-09-02-chat-first-w0-w1-handoff.md`
- **取代**: `docs/plans/2026-08-31-workspace-quickchat-tab-design.md` 及其 HTML mockup
- **当前实施**: W0–W1 已完成并通过定向验证门；W2–W3 尚未开始

---

## 0. 已确认的产品决策

| # | 决策 | 定稿 |
|---|------|------|
| D1 | 首问是否需要 Workspace | 不需要。登录默认进入 `/chat`，普通 Conversation 的 `workspace_id` 可空 |
| D2 | 普通对话能力 | 支持无资料通用聊天，也支持独立开启网络检索 |
| D3 | 文件 | P0 必须包含会话级文件；上传后立即解析并进入会话级检索 |
| D4 | 文件保留 | P0 原件与解析成果随会话保留，不自动过期；用户删除时完整清理 |
| D5 | 默认模型 | `qwen3.8-flash` 是 `quick_chat` 官方默认模型 |
| D6 | BYOK | 设置中有独立 `quick_chat` 配置；模型角色与 Workspace、文件、联网开关解耦 |
| D7 | Workspace 定义 | 唯一可复用、可管理、可分享的持久知识容器；Session 文件不构成第二知识库 |
| D8 | 升级语义 | 「移动对话」与「将文件加入 Workspace」是两个显式、独立、幂等动作 |

一句话产品原则：

> 问一句就能开始；资料值得复用时，再给它一个 Workspace。

---

## 1. 问题与目标

### 1.1 现状问题

当前主路径是：

```text
登录 → /dashboard → 创建 Workspace → /dashboard/:id → 才能聊天
```

它把三个正交问题绑成一个前置流程：

1. Conversation 是否需要持久存在。
2. 文件是否只服务当前会话。
3. 资料是否值得跨会话复用、管理和分享。

结果是轻量用户被知识库流程拦在第一问之前，而 Workspace 内新增「快聊」tab 仍然无法解除门禁。

### 1.2 目标状态

```text
登录 → /chat → 立即提问
               ├─ 可选联网
               ├─ 可上传本会话文件
               └─ 需要沉淀时显式关联 Workspace
```

### 1.3 成功指标

P0 上线后至少观测：

- 新用户登录到首次发送的转化率与耗时。
- 首次回答成功率、首 token 延迟、流中断率。
- 普通对话中开启联网、上传文件、点击引用的比例。
- Session 文件解析成功率、等待时间、删除率与实际存储构成。
- 「移动到 Workspace」与「加入 Workspace 资料」的显式转化率。
- Workspace 新建率、资料复用率是否提升，而不是只看 Workspace 数量。
- 官方 quick_chat 与 BYOK quick_chat 的成功率、成本和配置错误率。

P0 不根据拍脑袋阈值自动提示升级；真实行为数据形成后再设计软提示。

---

## 2. 设计原则与硬不变量

1. **同一个 Chat Canvas。** 普通对话、Workspace 对话与公开分享复用消息流、Composer、SSE、反馈和引用渲染；差异在外层 shell 与 ContextScope。
2. **Quick 不是页面模式或 AgentKind。** 用户可见词是「对话 / 新对话」；`quick_chat` 仅是独立模型角色。
3. **上下文是加法，不是二选一。** 对话历史、Session 文件、Workspace 资料、Web 证据按本轮组合。
4. **模型与上下文解耦。** 移动对话、上传文件、打开联网不会偷偷改变主回答模型或 BYOK 选择。
5. **Workspace 只承载可复用知识。** Session 文件只有显式增加 Workspace binding 后，才对其他会话可见。
6. **每轮冻结事实。** 历史回答展示当时的模型、可用范围与实际引用，不用当前 UI 状态倒推。
7. **显式 promotion。** Host 或前端不自动搬家、不自动入库、不根据文件语义做隐私相关决定。
8. **一套文件处理管线。** Session 文件复用现有上传、解析、切片、向量、引用与清理能力；不另造「提取文本后塞 prompt」的临时管线。
9. **用户主气泡仍只有模型 prose。** 范围与引用由结构化 UI 展示，不把 host observation 或运行时诊断拼进回答正文。
10. **无兼容层。** 不保留 Workspace `quickchat` tab、`agent_type=quickchat`、平行附件表或第二套 Chat 页面。

---

## 3. IA、路由与 Shell

### 3.1 Canonical 路由

| 用户任务 | Canonical |
|---------|-----------|
| 新建普通对话 | `/chat` |
| 继续普通对话 | `/chat/:sessionId` |
| 工作区总览 | `/dashboard` |
| 进入工作区 | `/dashboard/:workspaceId` |
| 继续工作区对话 | `/dashboard/:workspaceId?session=:sessionId` |
| 移动当前对话 | 当前 Chat Canvas 的上下文胶囊 / 对话菜单 |
| 将当前文件加入 Workspace | Composer 文件托盘 / 会话文件抽屉 |

登录、注册、Web auth gate 与桌面云登录完成后的产品默认目的地统一为 `/chat`。

### 3.2 普通 Chat shell

```text
┌──────────────┬─────────────────────────────────────────┐
│ + 新对话      │ [ 本对话 ▾ ]                  通知 账户 │
│              ├─────────────────────────────────────────┤
│ 最近          │                                         │
│  合同问答 本对话│             Chat Canvas                │
│  Q3 竞品 产品组│                                         │
│              │                                         │
│ Workspaces   │  [文件托盘]                              │
│  产品调研     │  [联网]                         [发送]  │
└──────────────┴─────────────────────────────────────────┘
```

- 左栏最近按 `updated_at` 混排普通与 Workspace Conversation，并显示轻量来源标签。
- 右侧不出现持久资料栏；Session 文件从 Composer 托盘或轻抽屉查看。
- 点击 Workspace Conversation 进入其 canonical 深链，不在 `/chat` 内伪装 Workspace。

### 3.3 Workspace shell

```text
┌──────────────┬────────────────────────────┬──────────────┐
│ 当前 Workspace│ [ 文件夹 产品调研 ▾ ]      │ 工作区资料    │
│ Sessions     ├────────────────────────────┤ 本会话文件    │
│              │       同一个 Chat Canvas   │ Notes        │
│              │                            │              │
│              │ [资料范围] [联网]   [发送] │              │
└──────────────┴────────────────────────────┴──────────────┘
```

- 左栏只列当前 Workspace 的 Sessions。
- 右栏把「工作区资料」与「本会话文件」分区，避免上传位置不清。
- Workspace 内不新增「工作区对话 | 快聊」segmented control。

### 3.4 上下文胶囊

- 普通状态：`[ 本对话 ▾ ]`
- Workspace 状态：`[ 文件夹 产品调研 ▾ ]`

菜单允许：

- 最近 Workspace。
- 移动对话到已有 Workspace。
- 新建 Workspace 并移动对话。
- 从 Workspace Conversation「在普通对话中继续」（创建分支，不改写原会话历史）。

---

## 4. 领域模型

### 4.1 总览

```text
User
├─ Conversation[]
│  ├─ workspace_id?
│  ├─ model_role
│  ├─ Message[]
│  │  └─ TurnContextSnapshot
│  └─ DocumentBinding(scope=Session)
├─ Workspace[]
│  ├─ DocumentBinding(scope=Workspace)
│  ├─ Note[]
│  └─ associated Conversation[]
└─ DocumentArtifact[]
   ├─ immutable raw metadata / raw object
   ├─ ParseVersion[]
   ├─ chunks / blocks / assets / vectors / struct sidecars
   └─ DocumentBinding[]
```

### 4.2 Conversation

目标字段（概念契约，字段名在实现计划中再落）：

```text
id
owner_user_id
workspace_id?          // null = 普通对话
model_role             // 创建入口选定初始默认值，随后独立持久化
title
pinned
created_at
updated_at
```

不变量：

- `owner_user_id` 永远存在，是普通 Conversation 的 RLS 根。
- `workspace_id` 只表示该 Conversation 后续轮次可获得哪个 Workspace 的资料；不承担用户所有权。
- Conversation 从普通状态移入 Workspace 后，旧 Message 与旧 TurnContextSnapshot 不变。
- 创建时由入口显式选择初始默认值：global `/chat` 为 `quick_chat`，Workspace 内新建为 `agent`。写入后 `model_role` 是独立持久化事实，不再从当前 `workspace_id`、capabilities 或文件数量重算；移动后保持原值，除非用户显式改变模型配置。

### 4.3 DocumentArtifact 与 DocumentBinding

`DocumentArtifact` 表示一次不可变上传及其解析版本；`DocumentBinding` 表示可见范围。

Artifact 是内部存储身份，不是用户可浏览的“全局内容库”。产品 API 不提供按 owner 罗列全部 Artifact 的发现入口；读取、检索与操作必须从用户有权访问的 Conversation binding 或 Workspace binding 到达。

```text
DocumentArtifact
  id, owner_user_id
  original_filename, mime, original_size
  raw_state, raw_object_path
  parse_state, active_parse_version

DocumentBinding
  id, artifact_id, owner_user_id
  scope_kind = session | workspace
  conversation_id? XOR workspace_id?
  created_at
```

上面是领域层 sum type。物理 schema 优先使用 `conversation_document_bindings` 与 `workspace_document_bindings` 两张 typed table，让 `conversation_id` / `workspace_id` 都有真实外键、唯一约束与级联规则；不落无真实 FK 的 `(scope_kind, scope_id)` 多态表。

不变量：

- 同一 Artifact 可以同时有 Session binding 与 Workspace binding。
- 「加入 Workspace」新增 binding；不复制 raw、不重跑解析、不删除原 Session binding。
- 删除一个 binding 只撤销该 scope 的未来可见性。
- Artifact 没有任何 binding 后才进入完整 GC。
- 所有授权由服务端从 binding 推导；客户端传入的文档 ID 不能扩大可见范围。
- 同一 Artifact 同时落在本轮 Session 与 Workspace scope 时，检索按 `artifact_id + parse/index version` 去重，但 Evidence 保留它来自哪些可见 scope。

### 4.4 TurnContextSnapshot

每次发送时冻结：

```text
conversation_id
workspace_id_at_send?
conversation_history_boundary
session_binding_versions[]       // binding + artifact + parse/index version
workspace_binding_versions[]     // binding + artifact + parse/index version
web_enabled
thinking_enabled?
model_role
effective_provider
effective_model
credential_source = official | byok
created_at
```

Snapshot 是历史解释事实，不是重新执行请求。敏感凭据、API key、完整内部 prompt 不进入 Snapshot。

### 4.5 TurnEvidence / CitationRecord

执行结束后另写不可变的实际证据记录，不回填或改写发送时 Snapshot：

```text
turn_id
channel = rag | web | base_tool
source_scope = session | workspace | web
artifact_id? / parse_version? / index_version?
chunk_id? / asset_id? / page? / source_locator?
url? / fetched_at?
citation_status = available | source_deleted
```

- Snapshot 回答“本轮允许用什么、实际用了哪个模型”；TurnEvidence 回答“执行最终用了什么”。
- CitationRecord 不是 Document binding，不能阻止用户删除 Artifact。
- 来源被明确删除后，只保留文件名、页码、URL、时间等不可还原正文的最小墓碑；不保留 excerpt、完整 chunk 或 asset 副本。

---

## 5. ContextScope 与引用

### 5.1 本轮可用上下文

```text
TurnContext
= Conversation history
+ Ready Session artifacts selected for the turn
+ Enabled Workspace artifacts (workspace_id 非空时)
+ Optional Web evidence
```

### 5.2 发送前：可用范围

Composer 上方显示事实，例如：

```text
本对话 · 会话文件 2 · 联网已开启
产品调研 · 工作区资料 12 · 会话文件 1 · 联网关闭
```

- `uploading / parsing / failed / excluded` 文件不计入 Ready 数。
- 用户显式附加但尚未 Ready 的文件会阻止发送；用户可以移除或排除后继续。
- UI 不承诺所有可用资料都会被答案采用。

### 5.3 回答后：实际使用

回答下方按真实证据分组：

```text
本次引用：工作区资料 2 · 本会话文件 1 · 网页 3
```

- 文档引用保存 scope、artifact/parse version、chunk/asset、页码或 source locator。
- Web 引用保存 URL、标题、抓取时间与 Worker evidence。
- 无命中是合法结果；界面不得把「可用」冒充「已引用」。
- 历史引用无法打开时显示明确墓碑状态，不重新解释旧回答。

### 5.4 Web 与 thinking 的 turn 状态

- Web 是独立 turn capability。新 Conversation 默认关闭；在当前 Conversation 中保留最近一次 Composer 选择，但每次发送都显式提交并写入 Snapshot，服务端不从历史消息猜测。
- Thinking 与 ContextScope、Workspace 和 Web 解耦。只有 provider profile 明确通过 `enable_thinking` 契约验证时才显示控件；每轮显式提交并写入 Snapshot。
- Thinking 控件不是 Chat-first P0 的上线门；未验证 provider 时隐藏，不用兼容 fallback 或换模型模拟。

---

## 6. Conversation 生命周期

### 6.1 新建与恢复

- 访问 `/chat` 先呈现空 Canvas；首次发送或首次上传文件时再创建 Conversation。
- 普通 Conversation 的 `workspace_id=null`，不创建隐藏 Personal Workspace。
- `/chat/:sessionId` 恢复消息、文件 binding、模型角色与当前可用状态。
- 全局 Sessions API 支持用户级最近列表，并返回 `scope_kind / workspace_id / workspace_name` 以构造正确 URL。

### 6.2 移动到 Workspace（P1 / W4）

「移动对话到 Workspace」是单一 domain operation：

1. 验证目标 Workspace 属于同一 owner。
2. 更新 Conversation 的 `workspace_id`。
3. 写入 Conversation timeline event，说明后续轮次开始获得该 Workspace 上下文。
4. 不改变 model role。
5. 不自动给 Session 文件增加 Workspace binding。

跨 owner / shared Workspace 的 transfer 不在 P0；不得通过通用 PATCH 静默改变付费与 RLS 归属。

### 6.3 从 Workspace 回到普通对话（P1 / W4）

P0 不把原 Conversation 的 `workspace_id` 设回 null，因为历史轮次已使用 Workspace 资料。入口语义是「在普通对话中继续」：

- 新建普通 Conversation 分支。
- 复制用户选择的可见对话文本作为起点或建立 parent link。
- 不复制 Workspace binding。

---

## 7. 会话文件

### 7.1 上传与解析状态

```text
selected
→ uploading
→ uploaded
→ parsing
→ ready

失败：upload_failed | parse_failed
删除：deleting → deleted
```

交互要求：

- 每个文件有独立状态、重试和移除操作。
- 首次上传先幂等创建 Conversation，再创建 Session binding。
- `parse_failed` 保留原件供重试；用户显式删除才完整清理。
- P0 继承现有 Document 管线支持的格式、单文件大小和账户配额，不扩张新格式。
- 文件数量与累计解析量由服务端配额和可解释错误控制，不用「Quick 模式」硬编码另一套隐形限制。

### 7.2 处理策略

Session 文件复用现有：

- 对象存储上传与校验。
- 文本/结构/多模态解析。
- chunks、blocks、TOC、assets、struct sidecar。
- embedding、rerank 与检索索引。
- 文档引用与 viewer 的已有能力。
- 异步清理 worker。

P0 不把完整原文拼进请求 prompt，也不新建 `chat_attachments.extracted_text` 平行存储。

### 7.3 P0 保留策略（已确认）

```text
Raw original      随 Artifact / 会话保留
Derived text      随 Artifact / 会话保留
Vectors / graph   随 Artifact / 会话保留
Derived assets    随 Artifact / 会话保留
```

- 不自动 TTL，不自动删除 raw。
- 用户删除 Session binding 后，该文件不再进入未来轮次。
- Artifact 仍有 Workspace binding 时继续保留。
- Artifact 无任何 binding 时，异步完整清理 raw、assets、索引、chunks、parse runs 与 struct sidecar。
- 历史回答文字保持；引用只保留文件名、页码、URL、删除时间等不可还原正文的最小墓碑事实，不保留 excerpt、完整 chunk、asset 副本或其他可还原内容。

### 7.4 原件自动清理（明确不在 P0）

只有实际存储数据证明值得做时，才增加独立 raw 生命周期：

```text
raw:     available → purge_pending → purged
derived: processing → ready → deleting → deleted
```

不预设 raw 是主要空间来源；向量、结构数据和图片 assets 可能同样大或更大。是否进入这一层由 `raw / derived / assets / index` 的实际字节构成、下载/reindex 使用率与删除失败率共同决定。

届时至少需要：

- 独立 `purge_document_raw`，不得给现有完整 cleanup 加布尔分支。
- `raw_size_bytes / raw_purged_at / original_available` 等事实字段。
- 配额按实际 raw、derived、asset、index 分层计量。
- Viewer、下载、reindex、OCR 修复、页级审计的降级 UI。
- 隐私文案区分「原件已清理」与「内容及派生数据已永久删除」。
- parse-run/provenance 不再把已清理的 raw object path 当成可用 artifact；reindex 明确不可用或要求重新上传。
- orphan scanner 的引用白名单覆盖所有 derived asset storage path，避免 raw 清理后把仍被引用的图片资产误删。

---

## 8. Quick Chat 模型角色与 BYOK

### 8.1 官方配置

新增正式配置角色：

```text
AppConfig.quick_chat_llm
QUICK_CHAT_LLM_BASE_URL
QUICK_CHAT_LLM_API_KEY
QUICK_CHAT_LLM_MODEL=qwen3.8-flash
QUICK_CHAT_LLM_TIMEOUT_MS
QUICK_CHAT_LLM_API_STYLE
QUICK_CHAT_LLM_ENABLE_THINKING
```

不使用临时 `chatbot_llm` 命名，也不把该配置塞进 `agent_llm` 或 `retrieve_llm`。

### 8.2 BYOK

新增 account-scoped `ProviderSecretPurpose::QuickChat`：

```text
resolve(owner_user_id, purpose=quick_chat, scope=account_only)
```

设置页增加独立「Quick Chat · qwen3.8-flash」配置行。Resolver 规则：

```text
活动且完整的 quick_chat BYOK
  → BYOK qwen3.8-flash
未配置 quick_chat BYOK
  → 官方 QUICK_CHAT_LLM qwen3.8-flash
用户已明确启用 BYOK，但配置或调用失败
  → 配置错误；不静默切官方模型、不意外扣余额
```

每个 account + purpose 只能有一个明确活动配置；不得依赖多条 `llm` provider 记录的数据库返回顺序。

### 8.3 Context 与模型解耦

- `/chat` 新建 Conversation 默认 `model_role=quick_chat`。
- 上传文件、开启 Web、关联 Workspace 后仍使用 quick_chat Lead / 终答。
- Workspace 内新建的 Conversation 可继续使用现有 Workspace agent 默认角色。
- Conversation 被移动时保留 `model_role`；是否更换由显式用户操作决定。

### 8.4 Lead + Workers 边界

| 阶段 | 模型 / 凭据 |
|------|-------------|
| 无资料 DirectAnswer | quick_chat 官方或 quick_chat BYOK |
| Lead 指代消解、Brief、覆盖裁决 | quick_chat 官方或 quick_chat BYOK |
| 用户可见最终 prose | quick_chat 官方或 quick_chat BYOK |
| RAG Worker SaC / 检索回合 | 平台 `RETRIEVE_LLM_*` |
| Web Worker host 检索叶子（多 query / CRW） | 平台 `SEARCH_*` / SearchProvider；默认无独立 Worker LLM |
| embedding / rerank | 各自官方或独立 BYOK purpose |

Quick Chat BYOK key 不自动传给宿主搜索：BYOK 端点未必支持一致的 web_search、引用协议或 CRW。若未来支持用户自备搜索，另设 `search` purpose。

UI 必须说明：Quick Chat BYOK 只替代 Lead / 主回答模型；文件解析、RAG Worker、embedding/rerank 与联网服务仍可能使用平台余额。若未来某 SearchProvider 内部增加模型调用，该调用作为独立 UsageSegment 记录，不借用 Quick Chat BYOK。

### 8.5 上线前配置门

- qwen3.8-flash 官方价格表存在并由启动校验覆盖。
- 官方与 BYOK 的 API style、thinking、tool calling、流式输出、缓存和长上下文经过契约验证。
- BYOK provider profile 能选择正确协议，不一律强制普通 OpenAI Chat Completions。
- 每段 usage 明确 payer/credential source；BYOK 不把平台 Worker 和搜索错误地变成免费。

计费事实按每一次模型/工具调用落 `UsageSegment`，而不是整轮复用 `skip_wallet_debit`：

```text
turn_id, component_role
provider, model, credential_source
payer = platform_wallet | byok | included
tokens_or_units, rated_cost
```

quick_chat BYOK 只免该 key 承担的 DirectAnswer / Lead / synthesis；retrieve Worker、embedding、rerank、Web search、解析/OCR 分别按自己的 provider 与 payer 计量。

---

## 9. Agent lane 与 prompt 边界

### 9.1 路由

| 本轮能力 | 产品路径 |
|---------|----------|
| 无文件、无 Web | DirectAnswer / Chat |
| Web | Lead + Web Worker |
| Session 文件 | Lead + RAG Worker；doc scope 由 Session bindings 派生 |
| Workspace 资料 | Lead + RAG Worker；doc scope 由启用的 Workspace bindings 派生 |
| 文件 + Web | Lead + RAG/Web Workers，各通道 EvidencePack，Lead 合成 |

保持现行原则：Worker 不写用户终答，Lead 合成；无独立 verify LLM；Host 只做结构门与真实 tool Ok 计数。

### 9.2 Prompt 规则

- 不把附件原文或新的 LLM 指令体内联进 Rust/TypeScript。
- 新增的模型可见说明若确有必要，进入 `avrag-rs/prompts/**/*.md`，使用第三人称 observation voice。
- 若新增 host marker，先登记 `host_markers.rs` 再由 prompt asset/emitter 使用。
- 优先让 Session 文件通过现有 retrieval tool / EvidencePack 进入上下文，避免新造 prompt 注入旁路。
- 用户可见范围条是结构化 UI，不是 host 在回答末尾追加的披露脚注。

---

## 10. API 目标契约（概念）

### 10.1 Conversation

```text
GET    /api/v1/chat/sessions
POST   /api/v1/chat/sessions
GET    /api/v1/chat/sessions/:id
PATCH  /api/v1/chat/sessions/:id        // title / pinned，不承担 move
DELETE /api/v1/chat/sessions/:id

POST   /api/v1/chat/sessions/:id/move-to-workspace
```

`GET /chat/sessions` 无过滤时返回用户全部最近 Conversation，DTO 至少含：

```text
id, title, updated_at
workspace_id?, workspace_name?
scope_kind = personal | workspace
model_role
```

### 10.2 Session 文件

```text
POST   /api/v1/chat/sessions/:id/files
GET    /api/v1/chat/sessions/:id/files
DELETE /api/v1/chat/sessions/:id/files/:bindingId
POST   /api/v1/chat/sessions/:id/files/:bindingId/add-to-workspace
```

端点可在实现时复用现有 presign/complete 机制；路径表达的是 scope，不要求复制上传管线。

### 10.3 Chat execute

`ChatRequest.workspace_id` 不再是客户端授权源。服务端从 `session_id` 解析：

- owner。
- optional workspace。
- model role。
- 可见 Session bindings。
- 可见 Workspace bindings。

客户端可以选择允许范围内的文件/资料，但不能通过任意 ID 扩权。

### 10.4 Context / citation response

SSE 或完成事件需带结构化：

- `turn_context_snapshot_id`。
- 实际引用的 `source_scope`。
- artifact / parse version。
- Web URL / fetched_at。
- effective model role / provider source 的安全显示字段。

不得从前端当前 selectedSourceIds 反推历史消息。

---

## 11. 存储、RLS 与清理

### 11.1 Schema 目标

- `chat_sessions.workspace_id` 改为 nullable。
- `chat_sessions.owner_user_id` 保持必填并作为普通会话 RLS 根。
- 新增 `model_role`，不复用 `agent_type=quickchat`。
- 将原 `documents` 中 Artifact 与 Workspace scope 的耦合拆开为 Artifact + typed Binding；迁移完成后删除旧的双重真相字段/路径。
- TurnContextSnapshot 与 TurnEvidence 分开落不可变结构化表或受约束的 turn metadata；不得只存在浏览器状态，也不得共用一个会被执行过程反复覆盖的 JSON。

### 11.2 RLS 不变量

- 用户只能读写自己拥有的普通 Conversation 与 Artifact。
- Session binding 必须指向同 owner Conversation。
- Workspace binding 必须指向同 owner Workspace；P0 不支持跨 owner promotion。
- 检索前由服务端重算允许 artifact IDs；模型自报和客户端 doc IDs 不构成授权。
- 分享 surface 只读取 Workspace bindings；普通 Conversation 不直接分享。

### 11.3 清理

- 删除 Conversation → 删除其 Session bindings。
- 删除 Workspace → 删除其 Workspace bindings。
- Binding 删除后检查 Artifact 是否仍有引用。
- 零 binding Artifact → 入异步完整 cleanup；不得在 HTTP 请求内同步删多存储系统。
- cleanup 必须覆盖 raw、derived assets、vector/graph、chunks/blocks/parse-runs、struct sidecars。
- Snapshot / CitationRecord 不计入 binding 引用数；用户明确删除最后一个 binding 时，历史墓碑不能阻止 GC。
- 多存储清理通过 outbox/job 状态幂等重试；部分失败不会把 Artifact 伪装成已完整删除。

---

## 12. 前端组件边界

目标拆分：

```text
ChatShell (/chat*)
  ├─ GlobalConversationRail
  └─ ChatCanvas

WorkspaceSurface (/dashboard/:id)
  ├─ WorkspaceSessionRail
  ├─ ChatCanvas
  └─ WorkspaceContextRail

SharedWorkspaceSurface
  └─ ChatCanvas (locked context)
```

`ChatCanvas` 拥有：

- `useChatSession` / transcript / SSE。
- `ChatMessageList`。
- `ChatComposer`。
- 文件托盘与本轮范围条。
- progress、stop、retry、反馈、引用选择。

Shell 拥有：

- 路由与 session selection。
- 左右 rail。
- Workspace 加载与标题。
- 全局最近与来源标签。
- 上下文胶囊动作。

不要复用整个 `WorkspaceSurface` 来伪装普通 Chat，也不要复制一份 `QuickChatPane`。

---

## 13. 验收标准

### 13.1 普通对话

- [ ] 登录/注册成功默认进入 `/chat`。
- [ ] 无 Workspace、无文件时能创建、流式回答、持久化并跨设备恢复 Conversation。
- [ ] 联网开关可独立使用；引用可打开。
- [ ] 全局最近混排普通/Workspace Conversation，并路由到正确 canonical。
- [ ] 删除普通 Conversation 不影响 Workspace Conversation。

### 13.2 文件

- [ ] Composer 可上传现有 Document allowlist 内的文件。
- [ ] 每个文件展示 uploading / parsing / ready / failed 状态、重试与删除。
- [ ] 显式附加但未 Ready 的文件不会被静默忽略。
- [ ] Ready 文件仅对当前 Conversation 可见，后续轮次持续可用。
- [ ] 回答实际引用能区分 Session 文件、Workspace 资料与 Web。
- [ ] 删除 Session binding 后不再参与未来检索；零 binding Artifact 被完整异步清理。
- [ ] P0 无自动 raw TTL；原件与解析成果随会话保留。

### 13.3 Workspace 关系（P1 / W4）

- [ ] 移动 Conversation 只影响后续轮次，并记录 timeline event。
- [ ] 移动不改变 model role、不自动入库文件。
- [ ] 「加入 Workspace 资料」增加 binding，不重传、不重解析。
- [ ] Session / Workspace 双 binding 时删除一方不会误删 Artifact。

### 13.4 模型与计费（P0）

- [ ] 官方 quick_chat 默认 qwen3.8-flash。
- [ ] BYOK 设置中有独立 quick_chat 配置。
- [ ] BYOK 失败不静默回官方。
- [ ] 上传文件、联网不改变主回答模型。
- [ ] UI 正确披露 BYOK 主模型与平台 Worker/搜索/embedding 的费用边界。
- [ ] UsageSegment 按每次调用记录 credential source 与 payer；不存在整轮 BYOK 跳账。
- [ ] qwen3.8 官方价格与协议校验在上线前通过。

### 13.5 历史真实性（P0）

- [ ] 每轮分别保存发送时 TurnContextSnapshot 与执行后 TurnEvidence。
- [ ] 旧回答不会因后来移动、删除或重新选择资料而改变范围说明。
- [ ] 「可用范围」和「实际引用」分别展示。
- [ ] 来源删除后显示墓碑状态，不伪造仍可访问的原文。

---

## 14. 迁移与分层实施

一次性迁移规则：

- 现有 Workspace Conversation 保持原 `workspace_id`；只有新普通 Conversation 允许 null，不创建隐藏 Workspace。
- 现有 Workspace documents 转为 Artifact + Workspace typed binding；完成后删除旧直接关系与旧读写路径，不长期双读/双写。
- Sessions list/search、Cmd+K、RLS、analytics 与 desktop transport 都把 `workspace_id` 当可空事实；不得在边缘层补假 Workspace。
- 旧 quickchat tab、`mode/agent_type=quickchat`、`chatbot_llm` 与 `chat_attachments.extracted_text` 路径若存在则直接删除，不留兼容路由。

每一层保持产品可运行；实现 turn 在运行 cargo/pnpm/E2E 前按仓库规则先给时间估算并取得用户同意。

| 层 | 薄切片 | 验证门（实施时） |
|----|--------|------------------|
| W0（已完成） | nullable Conversation + official quick_chat config/resolver + 现有 Chat 执行 | contracts / storage / app-chat targeted tests（已通过） |
| W1（已完成） | `/chat` + 全局最近 + 共享 ChatCanvas；无文件通用 Chat 与 Web 可用 | frontend targeted tests / typecheck + desktop pre-start error targeted test + contracts / storage DTO tests（已通过） |
| W2 | Artifact/typed Binding + Session 文件上传、立即解析、会话级检索、Snapshot/Evidence 与完整删除 | ingestion/storage/worker/RAG/citation contract tests |
| W3 | 设置 quick_chat BYOK + fail-closed resolver + 逐调用 UsageSegment 与计费披露 | provider/billing/settings contract tests |
| W4（P1） | move Conversation / add Workspace binding + Workspace 本会话文件区 | auth/RLS/promotion + workspace frontend tests |
| 收尾 | 删除旧 quickchat tab 假设、同步生成 contracts、更新 code-review-graph | L1（经同意）+ `code-review-graph update/status` |

P0 指 W0–W3 全部完成；W0/W1 只是内部可验证切片，不作为缺文件或缺独立 BYOK 的对外半成品发布。W4 是“先聊后沉淀”的 P1，不改变 P0 已能长期保留会话文件的事实。

W0 验证记录（2026-09-02）：`app-core`、`app-chat`、`app`、`transport-http`、`storage-pg` 与 API-key security 定向测试均通过；`git diff --check` 通过，结构关系图已更新。

W1 验证记录（2026-09-02）：frontend typecheck 与完整 Vitest（109 files / 529 passed / 2 skipped）、desktop pre-start error、contracts、app-core/app-chat/app、storage-pg 与 HTTP security 定向测试均通过；未做真实 LLM / 完整 Playwright（按 mid-wave 非必需）。

---

## 15. 非目标

- Workspace 内「快聊」tab、`?pane=quickchat`、`agent_type=quickchat`。
- 两套 Chat 页面、两套联网管线、两套附件解析管线。
- 隐藏 Personal/Home Workspace。
- 自动把文件加入 Workspace、自动移动 Conversation。
- 基于文件名/类型/语义自动判断长期意图。
- P0 自动过期、自动删 raw、冷热分层。
- 基于文件数/轮次自动出现升级提示；先取得真实行为数据。
- Session 文件跨 Conversation 复用。
- 多 Workspace 同轮联合检索。
- 普通 Conversation 的公开分享、协作与跨 owner transfer。
- 全局内容库；持久可复用文件仍只属于 Workspace。
- 新增文件格式或新的 OCR/解析器。

---

## 16. 主要风险与对策

| 风险 | 对策 |
|------|------|
| nullable workspace 被误当成无 owner | owner_user_id 必填；T8 改为 user root + optional workspace scope |
| Workspace 成员仅凭 session_id 续聊时，owner-pays 重挂载无法先定位 owner-owned Conversation | canonical Workspace 路由与请求持续携带 workspace_id；计费层按 actor / conversation owner / payer 分离后再开放 session-only 成员续聊 |
| Session 文件越权进入别的会话 | 服务端从 binding 重算 allowed artifacts；客户端 ID 仅做子集选择 |
| move 与 promotion 半成功 | 两个独立 domain operation；各自幂等、审计；不由前端拼 PATCH |
| 历史回答范围漂移 | TurnContextSnapshot 与引用版本不可变 |
| BYOK 失败静默扣官方余额 | quick_chat resolver fail-closed；UI 显式切回官方 |
| BYOK 让平台 Worker/搜索免费 | usage 分段记录 credential source 与 payer，不以整 run 单一布尔跳账 |
| 文件删除误删 Workspace 资料 | Artifact/Binding 引用计数 + GC；双 binding 测试 |
| P0 文件处理过重 | 复用已有管线和配额，不做第二套短路；用真实耗时数据再优化 |
| 原件未来清理导致引用降级 | P0 不清理；后续独立 raw 状态与明确 UI |
| App shell 变成百科侧栏 | 左栏只承载新对话、最近、Workspace 发现；帮助仍为弱入口/弹窗 |
| Artifact 被做成第二个全局内容库 | Artifact 只能经 typed binding 到达；不提供 owner 全量发现 API |
| 同一文件双 binding 重复检索/计量 | effective scope 去重，Evidence 保留 scope；存储与解析只按 Artifact 计一次 |

---

## 17. 文档与规则同步

本设计接受后，以下 living references 必须保持一致：

- `docs/design/PRODUCT_IA.md`：Chat-first canonical IA。
- `docs/design/PRODUCT_IA_AUDIT.md`：v2 目标与实现状态。
- `AGENTS.md` / `docs/agent/product-apps.md`：T7/T8 精确定义。
- `docs/README.md`：现行 IA 版本与本设计入口。

结构代码开始实施前继续遵守：

- IA before pages。
- T1–T8 Product Apps 边界。
- prompts-in-md、第三人称 observation、host marker 注册。
- Lead + Workers 产品路径与用户主气泡边界。
- 结构改动后同 session 运行 `code-review-graph update`。
