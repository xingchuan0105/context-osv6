# Rust 前端 Phase 0：会话列表 + 历史加载

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；验证摘要见文末「验证报告」 |
| 本任务终点 | 只读会话列表 + `/chat/:sessionId` 恢复历史；与现有 reducer 同一管线。不代表 Gate 0 通过 |
| 上游交接 | [`2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md`](2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md) |
| 权威设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) §3.1 会话列表 / 消息历史子集 |

## 0. 任务结果

只读会话列表与 `/chat/:sessionId` 历史恢复已接入同一 `ConversationManager` / reducer。
浏览器走 Fetch GET；native REST 仍 `Unavailable`；桌面端不拉列表（无 REST IPC）。
Gate 0 仍 **NO-GO**。

## 1. 范围

### 1.1 做

- 只读 API：`GET /api/v1/chat/sessions`、`GET /api/v1/chat/sessions/{id}`、`GET /api/v1/chat/sessions/{id}/messages`。
- wire 类型只用 `contracts` 的 `ChatSession` / `ChatSessionListResponse` / `ChatMessage` / `ChatMessageListResponse`，不发明第二套 DTO。
- `/chat/:sessionId` 恢复历史到同一 `ConversationManager.messages`；发送/流式仍走现有 reducer。
- 侧栏会话列表；点一项只 `navigate(/chat/:id)`；「新对话」只 `navigate(/chat)`。
- 路由 Effect 只订阅路由（和 token）；读 model 必须 untracked。URL 回显不得 `switch` 打断流。
- 历史 apply 在 epoch 不匹配、session 不匹配、或正在 streaming 时拒绝。
- Playwright fixture 对 GET sessions/messages 返回 200，避免填 token 后 404 污染 console。

### 1.2 不做

- 不改 `frontend_next`、后端 API、Auth UI、CORS、`.env`。
- 不做 create/update/delete session、session files、RAG/Web/Workspace/Share/BYOK。
- 不做桌面 REST IPC（Tauri WebView 打 `127.0.0.1` 受 PNA 限制；历史仍是浏览器 Fetch）。
- 不跑 Gate 0 性能对照；不部署；不切 `tauri.conf.json`。

## 2. 验证门

1. `cargo test -p web-sdk` 与 `cargo test -p web-ui`（约 10–15s）。
2. `cargo check -p web-sdk --target wasm32-unknown-unknown` 与
   `cargo check -p web-ui --target wasm32-unknown-unknown --no-default-features --features hydrate`（约 1min）。
3. 结构改动后 `code-review-graph update`（约 2s）。
4. 不跑完整 Playwright / `cargo leptos build`（除非现有 fixture 套件被本切片改坏且用户同意重建）。

## 3. 红线

- `frontend_next` 只读；`contracts` 是唯一 wire 真相。
- 不改后端 API；不把协议残片拼进用户主气泡。
- Gate 0 通过前禁止 Phase 1–6。
- 只提交本任务文件，本地提交不 push。

## 4. 已知限制

- 桌面 CSR 本切片不拉会话列表/历史（无通用 REST IPC）。
- PoC token 仍是内存框；空 token 不发 GET。
- 历史行的 `reasoning` 不从 `turn_metadata` 猜测，保持空。

## 5. 验证报告（2026-09-04）

### 5.1 实现摘要

- `conversation_api`：`GET /api/v1/chat/sessions` / `{id}` / `{id}/messages` 的 URL 与 parse。
- `BrowserRestClient`：wasm32 Fetch GET + 同一 Bearer；native `Unavailable`。
- `ChatCanvasModel::switch_to_session` / `replace_session_list`（按 `updated_at` 近到远）/
  `apply_history`（epoch / session / streaming 守卫）。
- `ChatMessage` → `ConversationMessage`：只映射 user/assistant；`reasoning` 不猜。
- ChatPage：侧栏列表 + 「新对话」；路由 Effect 仍 untracked 读 model；URL 回显不 `switch`；
  历史加载期间锁 composer，避免先发后到的 GET 覆盖当前 turn。
- Playwright fixture 对带前缀的 GET sessions/messages 回 200 空列表。
- 未改 Next、后端、`tauri.conf.json`、Auth UI。

### 5.2 命令与结果

| # | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 1 | `cargo test -p web-sdk` | 0 | 26 passed（原 20 + 6 REST parse/native） |
| 2 | `cargo test -p web-ui` | 0 | 39 passed（原 30 + 9 history/list） |
| 3 | `cargo check -p web-sdk --target wasm32-unknown-unknown` | 0 | 通过 |
| 4 | `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate` | 0 | 通过 |
| 5 | `code-review-graph update` | 0 | Incremental: 33 files / 163 nodes |

`contracts` `ts-rs` warning 仍在。本切片未跑 Playwright / `cargo leptos build`。

### 5.3 未做

- 桌面 REST IPC / 真实 WebView 点验。
- create/update/delete session、session files。
- Gate 0 性能对照。
