# SUPERSEDED — Workspace「快聊」标签页（历史稿，不实施）

> **已被取代（2026-09-02）**：现行定稿为 `docs/plans/2026-09-02-chat-first-conversation-workspace-design.md`。默认入口改为 Workspace 外的 `/chat`；本稿的 Workspace tab、`?pane=quickchat`、独立 QuickChat 页面/AgentKind 与附件 prompt 注入路径均不得实施。

# Workspace「快聊」标签页 — 原设计稿

**状态**: SUPERSEDED（未实施，仅作历史记录）
**日期**: 2026-08-31
**范围**: workspace 内新增轻量聊天模式（chatbot tab）：qwen3.8-flash、文件附件、深度思考开关、联网搜索开关
**视觉稿**: `docs/plans/mockups/2026-08-31-workspace-quickchat-tab.html`（浏览器直接打开，含空态/会话中两态 + 亮暗色切换）

---

## 0. 需求原文与命名

> 单独设计一个聊天模式，配 qwen3.8flash 模型，一个 chatbot 页面，可以上传文件、切换深度思考、添加网络搜索，单独做成一个标签页形式，嵌入到 Workspace 里面。

**建议对外名:「快聊」**（备选:「轻聊」「助手」，见 §9 决策点）。与现有「工作区对话」（知识库 grounded 对话）并列为中心面板的两个模式：

| | 工作区对话（现状） | 快聊（新增） |
|---|---|---|
| 定位 | 基于工作区资料的有据问答 | 轻量通用助手，快问快答 |
| 模型 | 主 agent 模型（deepseek 系） | **qwen3.8-flash**（官方模型，走余额） |
| 能力芯片 | 知识库 / 联网搜索 | 深度思考 / 联网搜索 / 上传附件（**无**知识库） |
| 资料来源 | workspace sources（doc_scope） | 会话级临时附件（不进资料库） |
| 右侧栏 | 资料源 + 笔记 | 隐藏（纯 chatbot 页面观感） |

## 1. 现状盘点（探索结论，2026-08-31）

前端（`frontend_next`）：

- Workspace 是三栏 shell（历史栏 | 中心聊天面板 | 右侧上下文栏），**没有 tab 系统**；中心面板即 `workspace-chat-pane.tsx`。
- Composer（`chat-composer.tsx`）已有能力芯片：`rag`（知识库）、`search`（联网搜索）——**联网开关现成**；无附件按钮、无深度思考开关（SSE 已有 `reasoning_summary_delta` 事件与「深度思考」进度文案，仅作展示）。
- 前端**不传 model**；面板状态持久化在 `lib/workspace/ui-store.ts`（zustand + persist）。
- 设计基线：tokens 全 400 字重、禁裸 hex、阴影仅 focus-ring 豁免清单；无现成 Switch 组件。

后端（`avrag-rs`）：

- `POST /api/v1/chat`（SSE）的 `ChatRequest` **无 model / thinking / attachments 字段**；`capabilities: ["rag"|"search"]` 是唯一开关，`agent_type: chat|general`。
- **纯 chat 路径现成**：空 capabilities → `AgentKind::Chat` → DirectAnswer 一轮直答（`mode_assemble.rs:89-100`，ProseOnly，max_iterations=2），不需要 workspace/doc_scope。
- 联网搜索 = `capabilities: ["search"]` → Lead+Workers web worker（`qwen_web` provider 默认），产品路径完整。
- 深度思考：dashscope 兼容层的顶层 `enable_thinking` 字段**已实现**（`openai_chat/request.rs`），但目前由 loop 按阶段强制（retrieve 关 / synthesis 开），**请求级不透传**。
- 附件：只有 workspace 文档全量 ingest 管道（presign → complete → 解析/嵌入/入 Milvus），**无轻量「塞 context」机制**。
- 模型：无集中清单，按 role 配置（`agent_llm`/`retrieve_llm`/`memory_llm`…，`ModelProviderConfig` 先例）；现役 flash 为 `qwen3.7-flash`，`qwen3.8-flash` 已在 e2e bench 出现。

**结论：快聊 = 纯 chat DirectAnswer 路径 + 服务端钉死 flash 模型 + 请求级 thinking 透传 + 新增会话级轻量附件；联网搜索直接复用产品 web 路径。不新建第二条联网/对话管线。**

## 2. IA 与布局

### 2.1 中心面板双模式（segmented control）

```text
Workspace shell（/dashboard/:id）
┌──────────┬───────────────────────────────────────┐
│ 历史栏    │  WorkspaceTopBar（不变）               │
│ (不变)    ├───────────────────────────────────────┤
│ 会话列表  │  [ 工作区对话 | 快聊 ]  ← segmented    │
│ 快聊badge │ ┌─────────────────────────────────┐   │
│           │ │  快聊面板（替代中心面板内容）      │   │
│           │ │  空态 hero / 消息流              │   │
│           │ │  composer（快聊变体）             │   │
│           │ └─────────────────────────────────┘   │
└──────────┴───────────────────────────────────────┘
     右侧资料/笔记栏：快聊模式隐藏，工作区对话模式照旧
```

- **为什么是中心面板内 tab 而不是顶栏/全局入口**：PRODUCT_IA §5 顶栏限额且禁业务塞入；该切换只改变中心面板内容模式，属页面内组件，不新增 route、不动 `nav-config.ts`（全局导航单一数据源与本功能无关）。
- **状态**：`ui-store` 增 `centerPane: "chat" | "quickchat"`（per-workspace 持久化，仿 `capabilities` 现状）；tab 定义建单一数据源 `lib/workspace/pane-tabs.ts`（仿 `settings-tabs.ts` 模式）。
- **深链（可选）**：`?pane=quickchat`，走 `session-url.ts` 同模式；实施时在 PRODUCT_IA §3.3 补一行（§9 变更流程）。
- **移动端**：segmented control 置于主面板顶部；抽屉结构不变。

### 2.2 历史栏与会话

- 快聊会话**复用现有 session 体系**持久化（`/api/v1/chat/sessions`），session 上标 `mode: "quickchat"`；不另起第二套会话存储（T7：不新增绕过 workspace 的产品容器，Chat 域内的 mode 标记即可）。
- 历史栏列表混排显示，快聊条目加小徽标；点击快聊会话 → 中心面板自动切到快聊模式并加载该会话。删除走现有会话删除。

## 3. 交互规格

### 3.1 Composer（快聊变体）

复用 `chat-composer.tsx` 的骨架（可拖高 textarea、Enter 发送、流式停止按钮），按模式注入差异 props：

| 控件 | 行为 |
|---|---|
| **深度思考** | 开关（默认关）。开 → 请求 `thinking: true`；SSE `reasoning_summary_delta` 渲染为消息内可折叠「思考过程」块（复用现有事件渲染） |
| **联网搜索** | 芯片，复用现有 `search` 芯片语义；开 → `capabilities: ["search"]` |
| **上传附件** | 回形针按钮 + 文件选择/拖拽；附件托盘显示在输入框上方（文件名 pill：上传中→就绪/解析失败，可移除）。**无**知识库芯片 |
| 模式说明行 | 「快聊 · qwen3.8-flash · 官方模型」（taxonomy：官方模型走余额） |

发送体：`{ agent_type: "quickchat", capabilities: [...] | [], thinking: bool, attachment_ids: [...] }`。

### 3.2 空态

hero：标题「快聊」+ 一句话说明（轻量对话，响应更快；可传文件、可开联网搜索）+ 3 条建议 prompt。底部弱提示：「需要基于工作区资料问答？切回工作区对话」——防混淆的发现路径。

### 3.3 附件生命周期

```text
选择文件 → 客户端校验(类型/大小) → POST /chat/attachments（multipart 直传，≤20MB）
  → 服务端解析文本（复用 ingestion 提取层：pdf/docx/md/txt/csv）→ 存 chat_attachment 行
  → 就绪 pill → 发送时随 attachment_ids 提交 → 服务端把附件文本注入本轮上下文
  → 失败：pill 标「解析失败」可移除；解析中：禁止发送
限制（v1）：每会话 ≤5 个文件、单文件 ≤20MB、注入文本总量预算 ~100k 字符（超限截断并在 pill 上提示）
存储：会话级、随会话保留用于历史回放展示；不入 Milvus、不出现在右侧资料栏、不计入 workspace sources
```

## 4. 后端设计

### 4.1 模型：服务端钉死，请求不选型

新增 config role `chatbot_llm`（完全仿 `memory_llm` 的 `ModelProviderConfig` 先例）：

```text
CHATBOT_LLM_BASE_URL / CHATBOT_LLM_API_KEY / CHATBOT_LLM_MODEL=qwen3.8-flash
  / CHATBOT_LLM_TIMEOUT_MS / CHATBOT_LLM_ENABLE_THINKING(默认随请求)
未配置 → 回退 agent_llm 客户端。
```

模型不进 `ChatRequest`（维持「前端不选型」现状；自定义模型仍走 BYOK 高级路径）。实施时确认 dashscope 精确模型 id（现役命名先例 `qwen3.7-flash` → `qwen3.8-flash`）。

### 4.2 请求契约（`contracts/src/chat.rs`）

```rust
ChatRequest {
    // 现有字段不动
    agent_type: String,          // 新枚举值 "quickchat"
    thinking: Option<bool>,      // 仅 quickchat 通道尊重；其余路径行为不变
    attachment_ids: Option<Vec<String>>,
}
Session 记录增 mode: Option<String>（"quickchat"）。
```

- `resolve_capabilities`：`quickchat` 且 capabilities 含 `rag` → **丢弃 rag**（快聊无知识库；前端不展示该芯片，服务端兜底），`search` 保留。
- 路由：`quickchat` 无 search → DirectAnswer 快聊通道；有 search → 复用 Lead+Workers web 产品路径（见 4.4）。
- 快聊不计 doc_scope，不触发 `validate_rag_doc_scope`。

### 4.3 快聊 DirectAnswer 通道

- `AgentKind::QuickChat`：管线同纯 chat（`tool_pool.clear()`、ProseOnly、`allow_content_early_stop`、max_iterations 沿用 chat 档）。
- LLM 客户端 = `chatbot_llm`；**thinking 尊重请求**：`with_enable_thinking(request.thinking)`——这是现有「loop 按阶段强制 thinking」规则中**唯一**的请求级旁路，仅限 quickchat 通道（retrieve 恒关、RAG/搜索合成行为不变的产品硬规则不动）。
- qwen 的 `enable_thinking` 兼容字段已在 openai_chat 协议层实现，无需改 provider 层。

### 4.4 联网搜索

`capabilities: ["search"]` → 现有 Lead+Workers web 路径，**v1 不改**：Lead/合成用主 agent 模型，保证联网质量与主对话一致。快聊模型钉死只作用于直答通道。（若后续要全链路 flash，属优化项，不进 v1。）

### 4.5 附件

- 新端点：`POST /api/v1/chat/attachments`（multipart 直传；小文件直传即可，不走 documents 的 presign 两段式）。
- 新表 `chat_attachments`（id, user_id, workspace_id, session_id, filename, mime, size_bytes, extracted_text, status, created_at）。**归属 user_id、挂 workspace_id**（T8 命名纪律），生命周期随会话。
- 解析：复用 `ingestion` crate 的文本提取层（pdf/docx/md/txt/csv），跳过嵌入/索引。图片 OCR 进 v2。
- 上下文注入：快聊请求组装时把附件文本以注册过的 host marker 块注入模型上下文——**先在 `react_loop/host_markers.rs` 注册**（AGENTS 硬规则），emitter 引用常量；本轮附件不存在时注入「附件未能读取」的第三人称 observation（prompts/loop 资产，不内联中文句子）。
- 计量：快聊对话按现有官方模型余额计量；附件本身 v1 不单独计费（存储临时、文本预算受限）。

## 5. 前端改动清单

| 动作 | 文件 |
|---|---|
| tab 单一数据源（新建） | `lib/workspace/pane-tabs.ts` |
| 持久化 | `lib/workspace/ui-store.ts`（`centerPane` 字段） |
| 中心面板切换渲染 + segmented 控件 | `components/workspace/workspace-surface.tsx`（panePanel 段）+ 新 `pane-tabs` 组件 |
| 快聊面板/空态/composer（新建） | `components/workspace/quickchat/`（`quickchat-pane.tsx`、`quickchat-composer.tsx`、module css；消息流复用 `chat-message-list.tsx`，thinking 折叠块复用 `reasoning_summary_delta` 渲染） |
| 附件 API 客户端 | `lib/workspace/client.ts`（uploadQuickAttachment / removeQuickAttachment） |
| 契约类型 | `lib/contracts/generated/contracts.ts` 重新生成（thinking / attachment_ids） |
| 历史栏 badge + 点击切换 | `workspace-history-pane.tsx`、`session-url.ts` |
| i18n | `lib/i18n/messages/workspace.ts`（快聊全部文案） |
| 测试 | `tests/workspace/`（surface 双模式、composer 芯片、附件托盘状态机）；style baseline 自动守护 |

桌面端（Tauri）无需特判：同一套 transport（IPC/Web 分流已封装）。

## 6. 遥测与文案

- 遥测：现有 chat 埋点增 `mode=quickchat`、`thinking`、`attachment_count`、`search` 字段；无新增用户可见披露脚注（用户信道规则：主气泡=模型 prose）。
- 文案遵守 taxonomy：官方模型 / 上传 / 联网搜索 / 深度思考；快聊内不出现「RAG」「doc_scope」「Agent」等内部词。

## 7. 实施切片（薄端到端优先，每片独立可验）

| 片 | 内容 | 验证门 |
|---|---|---|
| W0 | 后端 `quickchat` 枚举 + `chatbot_llm` 配置 + DirectAnswer 通道；前端 segmented control + 快聊面板（无芯片）+ 会话持久化带 mode | `cargo test -p app-chat -p app-bootstrap --lib`；`pnpm test`；手测发/收流式 |
| W1 | 深度思考：契约 `thinking` 透传 + composer 开关 + 思考过程折叠块 | 同上 + SSE thinking 用例 |
| W2 | 联网搜索芯片接入（`capabilities:["search"]`） | 现有 search 路径回归 |
| W3 | 附件：端点 + 表 + 解析 + marker 注册 + 注入 + composer 托盘 | 附件单测 + 注入 parity 测试（host_markers 规则） |
| 收尾 | PRODUCT_IA §3.3 补 `?pane=quickchat`；`code-review-graph update` | 文档同步 |

不并行开多片；W0 不通不合入（不做半成品替身）。

## 8. 边界与拒绝项

- **不做**：客户端选模型（BYOK 是唯一自定义模型路径）；快聊进知识库/RAG；第二套会话存储；第二套联网管线；兼容旧路由。
- 拒绝面：quickchat + `rag` → 服务端静默丢弃（前端无入口）；附件超限 → 客户端即时拒绝 + toast；解析失败 → pill 态可移除，不阻塞其余附件。
- 断网/流中断：复用现有错误处理（消息流 error 事件 → 重试入口）。

## 9. 决策点（请拍板）

1. **名称**：快聊（推荐）/ 轻聊 / 助手？
2. **联网时模型**：v1 复用产品 web 路径（Lead 用主模型）— 接受？还是要求全链路 flash？
3. **历史混排**：快聊会话与工作区会话混排 + 徽标（推荐）？还是独立分组/过滤 tab？
4. **BYOK 联动**：v1 快聊固定官方 qwen3.8-flash、不读用户 BYOK — 接受？
5. **附件计费**：v1 不单独计费、仅对话按余额计量 — 接受？
