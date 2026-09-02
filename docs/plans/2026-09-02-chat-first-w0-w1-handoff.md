# Chat-first W0–W1 实施交接（W0–W1 已完成，P0 尚未上线）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-02 |
| 状态 | Handoff；W0–W1 已实现并通过验证，尚未提交或部署 |
| 分支基线 | `master` @ `e3af8c096350`；工作树包含本轮及既有未提交改动 |
| 设计真相 | [`2026-09-02-chat-first-conversation-workspace-design.md`](2026-09-02-chat-first-conversation-workspace-design.md) |
| IA 真相 | [`../design/PRODUCT_IA.md`](../design/PRODUCT_IA.md) |
| 下一阶段 | W2：Session 文件、Artifact/Binding、Snapshot/Evidence、完整删除 |

## 0. 当前状态一句话

**用户现在可以不建 Workspace，直接从 `/chat` 开始普通聊天或开启 Web；个人与 Workspace Conversation 共用一套 Chat Canvas，Conversation 的 owner/scope/model role 已成为服务端持久化事实。W0–W1 仍在未提交工作树中，P0 尚缺 W2 文件闭环和 W3 独立 Quick Chat BYOK/逐调用计费，当前不可作为完整 P0 发布。**

## 1. 接手前阅读顺序

1. 根 [`AGENTS.md`](../../AGENTS.md)：设计原则、T1–T8、prompt、验证与工作树纪律。
2. [`../design/PRODUCT_IA.md`](../design/PRODUCT_IA.md)：登录后 canonical 路由与 Shell。
3. [`2026-09-02-chat-first-conversation-workspace-design.md`](2026-09-02-chat-first-conversation-workspace-design.md)：产品与领域定稿，尤其 §4、§7、§8、§10–§14。
4. [`../design/PRODUCT_IA_AUDIT.md`](../design/PRODUCT_IA_AUDIT.md)：哪些 Chat-first 项已完成、哪些仍 open。
5. [`2026-08-11-lead-rag-web-workers-design.md`](2026-08-11-lead-rag-web-workers-design.md)：Session 文件进入 RAG 后必须遵守的 Lead + Workers 边界。

冲突时以上游文件和根 `AGENTS.md` 为准。本交接是 2026-09-02 的实施快照，不替代设计真相。

## 2. 分层完成度

| 层 | 状态 | 已交付 / 缺口 |
|---|---|---|
| W0 | ✅ 完成 | nullable Conversation、owner 作用域、`model_role`、官方 `QUICK_CHAT_LLM_*` 路由、存储/HTTP 契约 |
| W1 | ✅ 完成 | `/chat`、全局最近、共享 `ChatCanvas`、认证/品牌/搜索入口、桌面前置错误语义 |
| W2 | ⬜ 未开始 | Session 文件、typed Binding、服务端 scope、Snapshot/Evidence、零 binding 完整 GC |
| W3 | ⬜ 未开始 | `quick_chat` BYOK purpose、设置 UI、fail-closed、逐调用 UsageSegment 与费用披露 |
| W4 / P1 | ⬜ 未开始 | move Conversation、add-to-Workspace binding、Workspace「本会话文件」区 |

`P0 = W0 + W1 + W2 + W3`。W4 是 P1，不是文件 P0 的前置条件。

## 3. 已锁定的产品决策

| 主题 | 不变量 |
|---|---|
| 默认入口 | 登录、注册和认证完成后进入 `/chat`，不能要求先创建 Workspace |
| 普通对话 | 无文件时可通用聊天；Web 是独立 turn capability |
| Workspace | 唯一可复用、可管理、可分享的持久知识容器；Session 文件不是第二个全局知识库 |
| 文件 P0 | 上传后立即解析，ready 后持续参与该 Conversation；原件与派生结果随绑定保留，不自动过期 |
| 模型角色 | `/chat` 创建默认 `quick_chat`；Workspace 内创建默认 `agent`；写入后不因文件、Web 或后续 workspace 关系重算 |
| 官方模型 | `quick_chat` 官方默认 `qwen3.8-flash` |
| BYOK | `quick_chat` 必须是独立 purpose；不能借用通用 `llm` BYOK，也不能把 Quick Chat key 传给 Web search |
| 升级语义 | 「移动 Conversation」和「将文件加入 Workspace」是两个显式、独立、幂等动作 |
| 删除 | 用户删掉最后一个 binding 后完整清理 raw、derived、assets、index 与 sidecar；Citation 只留不可还原正文的最小墓碑 |

明确不要引入：隐藏 Personal Workspace、Workspace 内 Quick Chat tab、`agent_type=quickchat`、第二套附件解析管线、自动 move、自动入库、P0 raw TTL、兼容 shim 或长期双读双写。

## 4. W0 已落地内容

### 4.1 数据库与领域契约

- [`../../avrag-rs/migrations/0084_chat_first_conversations.up.sql`](../../avrag-rs/migrations/0084_chat_first_conversations.up.sql)
  - `chat_sessions.workspace_id` 改为 nullable。
  - 新增非空 `model_role`，数据库约束只允许 `agent | quick_chat`。
- [`../../avrag-rs/migrations/0085_chat_session_owner_recent_index.up.sql`](../../avrag-rs/migrations/0085_chat_session_owner_recent_index.up.sql)
  - 新增 `(owner_user_id, updated_at DESC, created_at DESC)`，服务全局最近列表。
- [`../../contracts/src/workspaces.rs`](../../contracts/src/workspaces.rs)
  - `ConversationScopeKind = personal | workspace`。
  - `ChatSession` 包含 `owner_user_id / workspace_id? / scope_kind / workspace_name? / model_role`。
  - `CreateChatSessionRequest.agent_type` 可省略，服务端默认 `chat`。
- [`../../frontend_next/lib/contracts/generated/contracts.ts`](../../frontend_next/lib/contracts/generated/contracts.ts) 已由共享 Rust 契约再生。

Wire 真值：响应中的 `workspace_id`、`workspace_name`、`title` 为 `None` 时省略字段，不发送 `null`；生成 TypeScript 因此使用可选字段。Rust 输入仍能反序列化显式 `null`，这是输入容忍，不是服务端响应的第二种格式。

### 4.2 owner、RLS 与存储

- `owner_user_id` 是所有 Conversation 的权限根；个人 Conversation 不借 Workspace 表达所有权。
- Workspace-bound 创建仍验证 Workspace 属于同 owner。
- exact GET、update、delete、message/citation 都执行 owner-scoped 查找；越权 ID 对外表现为 not found。
- `workspace_name` 由 owner-bound `LEFT JOIN workspaces` 计算；mapper 缺列时 fail-fast，不静默伪造 `None`。
- PostgreSQL 与 memory adapter 都遵守相同列表排序：
  - 全局列表：`updated_at DESC, created_at DESC`，忽略 pinned。
  - Workspace 列表：`pinned DESC, updated_at DESC, created_at DESC`。
  - memory search 与 PostgreSQL search 都按最近排序并限制 50 条。

主要路径：

- [`../../avrag-rs/crates/storage-pg/src/lib_impl/repository_sessions.rs`](../../avrag-rs/crates/storage-pg/src/lib_impl/repository_sessions.rs)
- [`../../avrag-rs/crates/storage-pg/src/lib_impl/repository_search.rs`](../../avrag-rs/crates/storage-pg/src/lib_impl/repository_search.rs)
- [`../../avrag-rs/crates/storage-pg/src/lib_impl/errors_and_mappers.rs`](../../avrag-rs/crates/storage-pg/src/lib_impl/errors_and_mappers.rs)
- [`../../avrag-rs/crates/app-core/src/adapters/memory_chat_persistence.rs`](../../avrag-rs/crates/app-core/src/adapters/memory_chat_persistence.rs)
- [`../../avrag-rs/crates/rag-core-ports/src/chat_persistence.rs`](../../avrag-rs/crates/rag-core-ports/src/chat_persistence.rs)

### 4.3 Quick Chat 官方模型路径

- [`../../avrag-rs/crates/app-core/src/config.rs`](../../avrag-rs/crates/app-core/src/config.rs) 新增独立 `quick_chat_llm` 配置，默认模型是 `qwen3.8-flash`。
- [`../../avrag-rs/.env.example`](../../avrag-rs/.env.example) 登记 `QUICK_CHAT_LLM_BASE_URL/API_KEY/MODEL/TIMEOUT_MS/TEMPERATURE/ENABLE_THINKING/API_STYLE`。
- 未显式设置 `QUICK_CHAT_LLM_API_KEY` 时，官方路径可复用已配置的 `DASHSCOPE_API_KEY`。
- [`../../avrag-rs/crates/app-chat/src/sessions.rs`](../../avrag-rs/crates/app-chat/src/sessions.rs) 只在创建时按入口选择初始角色；随后以存储值为准。
- [`../../avrag-rs/crates/app-chat/src/chat/pipeline_steps.rs`](../../avrag-rs/crates/app-chat/src/chat/pipeline_steps.rs) 将持久化的 `session.model_role` 放入本轮 agent metadata。
- [`../../avrag-rs/crates/app-chat/src/agents/unified/mod.rs`](../../avrag-rs/crates/app-chat/src/agents/unified/mod.rs) 根据 metadata 选择 `quick_chat_llm_client`，与 Chat/RAG/Search capability kind 解耦。

当前刻意状态：`uses_quick_chat` 时不会解析通用 `ProviderSecretPurpose::Llm`。这避免错误借用，但独立 `quick_chat` BYOK 尚未实现，属于 W3。

## 5. W1 已落地内容

### 5.1 路由与 Shell

| 用户任务 | Canonical URL |
|---|---|
| 新建个人 Conversation | `/chat` |
| 恢复个人 Conversation | `/chat/:sessionId` |
| Workspace 总览 | `/dashboard` |
| Workspace Conversation | `/dashboard/:workspaceId?session=:sessionId` |
| BYOK 设置 | `/settings?tab=providers` |

新路由：

- [`../../frontend_next/app/(app)/chat/layout.tsx`](<../../frontend_next/app/(app)/chat/layout.tsx>)
- [`../../frontend_next/app/(app)/chat/page.tsx`](<../../frontend_next/app/(app)/chat/page.tsx>)
- [`../../frontend_next/app/(app)/chat/[session_id]/page.tsx`](<../../frontend_next/app/(app)/chat/[session_id]/page.tsx>)

[`../../frontend_next/components/chat/chat-shell.tsx`](../../frontend_next/components/chat/chat-shell.tsx) 持有全局最近、Workspace 入口、deep-link 校验和路由选择；[`../../frontend_next/components/chat/global-conversation-rail.tsx`](../../frontend_next/components/chat/global-conversation-rail.tsx) 只负责 rail 展示。

登录、注册、Auth gates、品牌 CTA、top bar、Cmd/Ctrl+K、dashboard 全局搜索和 desktop static deep link 已统一到 canonical `/chat`。导航目的地仍只在 `frontend_next/lib/navigation/nav-config.ts` 定义。

### 5.2 共享 Chat Canvas

[`../../frontend_next/components/chat/chat-canvas.tsx`](../../frontend_next/components/chat/chat-canvas.tsx) 是唯一 Chat Canvas，复用于：

- 个人 `/chat`。
- Workspace Conversation。
- Shared Workspace 的 locked-context surface。

旧 `frontend_next/components/workspace/workspace-chat-pane.tsx` 已删除，没有兼容 wrapper。Canvas 接受 nullable `workspaceId`；Shell 持有路由与 rail，Canvas 持有 transcript、SSE、Composer、progress、stop/retry、反馈和引用交互。

已加的关键状态保护：

1. `/chat/:sessionId` 先 owner-scoped exact GET，确认是 personal 后才挂载 Canvas。
2. deep link 指向 Workspace Conversation 时跳转其 Workspace canonical URL。
3. exact lookup 失败是终态，Composer 不可发送；同 SID 可原地重试。
4. 首轮 SSE 分配真实 `session_id` 时不卸载/重挂 Canvas，避免误触发 history hydration。
5. 外部 session 切换会 abort 当前 stream；streaming 时 rail、新对话和 capability 切换禁用。
6. `/chat` 原地“新对话”通过 `resetEpoch` 清空草稿和 Search 选择。
7. history hydration 或失败期间 Composer 禁用，避免丢失上下文后继续发送。

### 5.3 全局最近与全局搜索

- [`../../frontend_next/lib/chat/client.ts`](../../frontend_next/lib/chat/client.ts) 直接复用生成的 `ChatSession`，不维护平行 DTO。
- rail 和 Cmd/Ctrl+K 使用 owner-global Session 列表。
- 个人来源显示“本对话”；Workspace 来源显示 `工作区 · {workspace_name}`。
- API 搜索返回空 Workspace 命中时，不回退并混入本地全部 Workspace。
- Dashboard 查询变化后会清除上一轮结果，Enter 不会误打开 stale result。

### 5.4 Desktop 前置失败

[`../../desktop/src-tauri/src/commands/chat.rs`](../../desktop/src-tauri/src/commands/chat.rs) 与 [`../../desktop/src-tauri/src/commands/chat_stream.rs`](../../desktop/src-tauri/src/commands/chat_stream.rs) 已删除“随机 session ID + Start/AnswerStart/Error/Done”伪流。

License failure 或上游在 SSE `Start` 前返回非 2xx 时，只发一个 `ChatEvent::Error`，且 wire 中没有 `session_id`。成功 SSE 代理路径未改变。

## 6. 当前 HTTP / DTO 契约

实际 W1 router：

```text
GET    /api/v1/chat/sessions
POST   /api/v1/chat/sessions
GET    /api/v1/chat/sessions/:id
PUT    /api/v1/chat/sessions/:id       # title / pinned
DELETE /api/v1/chat/sessions/:id
GET    /api/v1/chat/sessions/:id/messages
POST   /api/v1/chat
```

`GET /chat/sessions?workspace_id=...` 是 Workspace-scoped pinned-first；不带过滤是 owner-global recent。个人 Conversation 的典型响应不含 Workspace 字段：

```json
{
  "id": "…",
  "owner_user_id": "…",
  "scope_kind": "personal",
  "agent_type": "chat",
  "model_role": "quick_chat",
  "pinned": false,
  "created_at": "…",
  "updated_at": "…"
}
```

Workspace Conversation 会额外包含：

```json
{
  "workspace_id": "…",
  "scope_kind": "workspace",
  "workspace_name": "…",
  "model_role": "agent"
}
```

注意：定稿 §10.1 的目标方法写作 `PATCH`，现有 router/client 仍使用 `PUT`。功能与部分更新语义已工作，但这是一个明确的协议命名偏差。后续处理时应一次性将 router、client 与测试切到一个 canonical 方法，不能同时保留 PUT/PATCH 两条兼容路由。

## 7. W2 的删除与保留红线

### 7.1 不要把 raw purge 当成文档删除

现有 `delete_document` 是文档级完整删除，会清：

- raw object。
- derived asset objects。
- vector / graph index。
- chunks、blocks、parse runs、TOC、多模态行。
- struct-store / sidecar。

因此它不能被复用为“只删除原文件、省空间”。P0 已决定：raw 与 derived 随会话保留，不做自动 TTL；只有用户明确删除最后一个 binding 后才调用完整、异步、可重试的 cleanup。

### 7.2 W2 必须采用 typed bindings

领域层是 `scope=session | workspace`，物理层优先两张有真实外键的表：

```text
conversation_document_bindings(conversation_id, artifact_id, ...)
workspace_document_bindings(workspace_id, artifact_id, ...)
```

不要落 `(scope_kind, scope_id)` 这种数据库无法保证真实 FK 的多态表。现有 Workspace document 关系迁移完成后应删除旧双重真相路径，不长期双写。

### 7.3 必须保留的恢复/引用事实

- worker 首次解析与 reindex 当前都需要 raw object；提前删 raw 会失去重解析、OCR 修复和原页审计。
- derived assets 需与 derived text 同寿命；图片引用会实时读取 asset object。
- `document_parse_runs.artifact_path` 当前可能仍指 raw object，未来 raw purge 层必须先修 provenance。
- orphan scanner 在设计 raw/asset 分层前必须确认白名单覆盖 `document_assets.storage_path`，否则 asset 可能被误判为孤儿。
- “原件已清理”不能写成“内容已删除”：chunks、OCR、assets、embeddings、KG 与历史引用仍可能包含敏感内容。

## 8. 已执行验证

以下均在 2026-09-02 执行，Rust 使用 `1.96.1`、`jobs=2`。

| 范围 | 命令 / 结果 |
|---|---|
| Frontend 类型 | `cd frontend_next && pnpm typecheck`；通过 |
| Frontend 单测 | `cd frontend_next && pnpm test`；109 files，529 passed，2 skipped |
| Shared contracts | `cd avrag-rs && cargo +1.96.1 test -p contracts --jobs 2`；35 passed，1 ignored |
| app-core | `cargo +1.96.1 test -p app-core --lib --jobs 2 -- --test-threads=1`；41 passed |
| app-chat | `cargo +1.96.1 test -p app-chat --lib --jobs 2`；87 passed |
| App domain face | `cargo +1.96.1 test -p app --test delegate_contract --jobs 2`；15 passed |
| HTTP/API-key | `cargo +1.96.1 test -p transport-http --test api_key_security_contract --jobs 2`；16 passed |
| PostgreSQL | `avrag-storage-pg` tenant isolation、personal scope、update owner guard、conversation search 与双排序反例均通过 |
| Desktop | `cd desktop/src-tauri && cargo +1.96.1 test --lib commands::chat::tests --jobs 2`；2 passed |
| Contract generation | `cd frontend_next && pnpm generate:contracts`；通过 |
| Diff | `git diff --check`；通过；新增文件亦无行尾空白 |
| 结构图 | `code-review-graph update/status`；17,160 nodes / 185,444 edges |

PostgreSQL 定向复跑需要静默复用 `avrag-rs/.env`：

```bash
cd /home/chuan/context-osv6/avrag-rs
set -a
source .env
set +a
cargo +1.96.1 test -p avrag-storage-pg --lib tenant_isolation --jobs 2
AVRAG_MIGRATION_ROLE_ONLY=true cargo +1.96.1 test -p avrag-storage-pg --lib search_sessions_matches_assistant_message_body_when_database_available --jobs 2
```

### 8.1 非阻塞验证事实

- `app-core` 并行全库曾因既有环境变量测试互相污染，让 `api_keys_csv_builds_multi_key_primary_member` 读到其他测试的 key；该测试精确单线程复跑通过，完整 app-core 单线程 41/41 通过。继续复跑该 crate 时优先 `--test-threads=1`。
- 仓库级 `cargo fmt --all -- --check` 当前会报告大量既有、与本轮无关的格式基线差异；没有为此重排全仓。W0/W1 backend/contracts 本次触及文件的定向 `rustfmt --check` 通过。
- Desktop 两个触及文件的 file-level rustfmt 仍会指出 3 个未改行的既有布局差异；desktop 定向测试通过，未扩散无关格式化。
- 编译仍会出现若干既有 unused/private-interface/ts-rs attribute warnings，本轮没有用 drive-by 修改处理。

## 9. 尚未验证 / 尚未交付

- 未运行真实 `qwen3.8-flash` 对话与 Web 联合路径。
- 未运行完整 Playwright、真实浏览器 deep-link 恢复或 desktop 真机旅程。
- 未运行 `scripts/test-l1.sh` 或 full E2E；需要新的时间估算与用户同意。
- 未提交、未 push、未部署；生产状态未改变。
- 个人 Chat 尚无 Session 文件托盘、上传/list/delete 端点和会话级检索。
- TurnContextSnapshot、TurnEvidence、source scope/versioned citation 尚未持久化。
- 独立 `ProviderSecretPurpose::QuickChat`、设置 UI、fail-closed resolver 与逐调用 UsageSegment 尚未落地。
- move Conversation / add-to-Workspace binding 是 W4/P1，尚未落地。

## 10. 建议下一刀：W2 分五个可工作薄片

每一片开始前先用 code-review-graph 查结构与影响半径；运行 migration、cargo、pnpm 或 E2E 前按仓库规则先估时并取得用户同意。任一验证门失败时停在该片，不继续叠下一层。

### W2a — Artifact + typed Binding schema

1. 盘点现有 `documents`、assets、chunks、parse runs、index 与 cleanup 的真实 FK/状态。
2. 建立 Artifact 身份以及 Conversation/Workspace 两张 typed binding 表。
3. 让现有 Workspace documents 一次性迁移到 Workspace binding。
4. 补 owner/RLS、唯一约束、双 binding 和零 binding 判定测试。
5. 在 W2 完成前删除旧 scope 真相，不留永久双读/双写。

### W2b — Session 文件 API 与 UI 状态

1. 首次上传先幂等创建 Conversation。
2. 复用现有 presign/complete/ingest 管线，只新增 Conversation binding。
3. 实现 list/delete/retry 与 `uploading → parsing → ready | failed`。
4. 显式附加但未 ready 的文件阻止发送；用户可移除或排除后继续。

### W2c — 服务端 ContextScope 与 RAG

1. 从持久化 `session_id` 解出 owner、optional Workspace 和 model role。
2. 服务端重算 allowed Session/Workspace artifact IDs；客户端 doc IDs 只做允许范围内的选择，不能授权。
3. ready Session artifacts 复用现有 RAG Worker / EvidencePack；Web 仍走独立 Web Worker leaf。
4. 文件、Web 或两者组合都不改变持久化 `model_role`。

### W2d — Snapshot 与 Evidence

1. 发送时写不可变 TurnContextSnapshot：模型角色、capability、可用 scope、artifact/version 与 provider source。
2. 执行后另写 TurnEvidence：真实使用的 channel/source scope/chunk/asset/page/URL/fetched_at。
3. 历史 UI 分开显示“当时可用范围”和“本次实际引用”，不能从当前 Composer 状态反推。

### W2e — 完整删除与墓碑

1. 删除 binding 后先停止未来检索。
2. 仍有另一类 binding 时保留 Artifact 及全部派生物。
3. 零 binding 时通过 outbox/job 完整清理 raw、assets、index、chunks/blocks/parse runs 与 sidecar。
4. 部分失败保持可重试状态，不伪装成已完全删除。
5. 历史回答文字保留；引用只留不可还原正文的最小墓碑。

## 11. W3 接手提示

当前 `ProviderSecretPurpose` 只有 `llm | embedding | rerank`，Quick Chat 分支又明确跳过通用 `llm` secret。W3 应一次性完成：

1. 新增独立 `quick_chat` purpose、数据库约束与唯一活动配置语义。
2. Settings provider tab 增加 Quick Chat 配置；默认推荐 qwen3.8-flash，但协议/profile 可独立选择。
3. 活动且完整时使用 BYOK；未配置时走官方 `QUICK_CHAT_LLM_*`；用户明确启用但配置/调用失败时 fail-closed，不能静默回官方扣钱包。
4. DirectAnswer、Lead、synthesis 使用同一 Quick Chat credential；RAG Worker、SearchProvider、embedding/rerank 各自计费。
5. 用每次调用的 UsageSegment 记录 provider/model/credential_source/payer，不再用整轮 `skip_wallet_debit` 表示全部免费。

## 12. 工作树与交接纪律

- 当前是 dirty `master`，且包含本轮之外的既有用户改动；不要 `git reset --hard`、`git checkout --` 或批量格式化。
- 先用 `git status --short` 和 `git diff -- <path>` 确认目标，再做手术式编辑。
- `workspace-chat-pane.tsx` 的删除是有意替换，不要恢复兼容 wrapper。
- 共享 contracts 改动后必须重新运行 `pnpm generate:contracts`，不能手改生成 TS 作为第二真相。
- 结构性代码改动结束后必须 `code-review-graph update`，不要提交 `.code-review-graph/`。
- prompt 若需变化，只能落在 `avrag-rs/prompts/**/*.md`，并遵守第三人称 observation voice 与 host marker 注册规则。
- 默认不 push、不建 PR、不部署；部署只能走 `scripts/deploy-*.sh`。

接手完成 W2 前，最小验收不是“文件能上传”，而是：**同 owner Session 文件能上传并立即解析，ready 后只在该 Conversation 的服务端授权范围内参与 RAG；删除 binding 后不再召回，最后一个 binding 删除后完整异步清理；历史 Snapshot/Evidence 与引用墓碑仍能解释旧回答。**
