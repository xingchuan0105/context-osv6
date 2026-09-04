# Rust 前端 Phase 0：Tauri 垂直切片

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；验证摘要见文末「验证报告」 |
| 本任务终点 | `TauriIpcTransport` 接既有 `chat_stream` / `chat_cancel`；CSR 产物可构建；与 fixture/reducer 对等。不代表 Gate 0 通过 |
| 上游交接 | [`2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md`](2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md) |
| 权威设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) §10 的 **chat 流 / CSR** 子集 |
| live smoke | [`2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md`](2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md) |

## 0. 任务结果

`TauriIpcTransport` 已对接仓库既有 `chat_stream` / `chat_cancel`（wasm32 + `__TAURI__`）；native 无宿主仍 `Unavailable`；fixture mock 对等保留。CSR `mount_csr` + `dist/tauri` 产物已构建。未改 `tauri.conf.json`、Next、后端。Gate 0 仍 **NO-GO**。

## 1. 范围

### 1.1 做

- `TauriIpcTransport`：wasm32 经 `window.__TAURI__`（`withGlobalTauri`）调用既有 `chat_stream` / `chat_cancel`，监听 `chat://{request_id}`。
- 事件仍是 `contracts::chat::ChatEvent`；payload 用同一 serde，不写第二套 DTO。
- 桌面 host 要求的 `request_id` 作为 JSON 附加字段插入（与 Next `streamChatViaIPC` 相同），不是新契约类型。
- 复用 fixture → reducer 对等；native `new()` 无 mock 时仍 `Unavailable`。
- ChatPage：检测到 Tauri 运行时走 IPC，否则保持 Browser Fetch。
- CSR：`csr` feature 挂载 `App`；静态壳 + wasm 产物目录 `frontend_rust/dist/tauri`。
- `/` 重定向到 `/chat`，避免 CSR 打开 index 落在 404。

### 1.2 不做

- 不改 `frontend_next`、后端 API、`desktop/src-tauri/tauri.conf.json`（Phase 5 才切 `frontendDist`）。
- 不做 cloud session 登录页、上传、local stack、publish、updater、deep link。
- 不部署、不跑完整桌面安装包 E2E。
- 不把 IPC 事件当 SSE 再解码一遍（宿主已 emit 结构化 `ChatEvent`）。

## 2. 命令名（以仓库实情为准）

设计/交接写的 `chat_stream_start` 在本仓库不存在。现行 command 是 `chat_stream` 与 `chat_cancel`（`desktop/src-tauri/src/commands/chat_stream.rs`）。本切片对接这两个名字。

## 3. 验证门

1. `cargo test -p web-sdk` 与 `cargo test -p web-ui`（约 10s）。
2. `cargo check -p web-ui --target wasm32-unknown-unknown --no-default-features --features csr`（约 1min）。
3. 若 check 通过，再 `wasm-bindgen` 产出 `dist/tauri`（约 30s；`wasm-bindgen` 用 PATH 作用域 0.2.127）。
4. 结构改动后 `code-review-graph update`（约 2s）。
5. 不改 `tauri.conf.json`；不跑完整 `tauri build`。

## 4. 红线

- `frontend_next` 只读；`contracts::chat` 唯一 wire 真相。
- 不改后端 API；不切生产桌面前端产物。
- Gate 0 通过前禁止 Phase 1–6。
- 只提交本任务文件，本地提交不 push。

## 5. 验证报告（2026-09-04）

### 5.1 实现摘要

- `prepare_ipc_request` / `parse_ipc_event` / `map_ipc_error_status`：与 Next `streamChatViaIPC` 同一 host 约定（`request_id` 附加字段 + `chat://{id}`）。
- wasm32：`listen` 先于后台 `invoke("chat_stream")`，取消走 `chat_cancel`；事件按 `ChatEvent` serde，不再套 SSE decoder。
- ChatPage：`is_tauri_runtime()` 选 IPC，否则 Fetch。
- `/` → `/chat` 重定向（CSR 打开 index 不再落 404）。`AppRoute::parse("/")` 仍是 NotFound，根路径不是第二条完成路径。
- CSR：`mount_csr` + `crates/web-ui/csr/index.html` + `scripts/build-tauri-csr.sh` → `dist/tauri/`（wasm/js/css；目录 gitignore）。
- 未修改 `desktop/src-tauri/tauri.conf.json`。

### 5.2 命令与结果

| # | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 1 | `cargo test -p web-sdk` | 0 | 20 passed（原 17 decoder + 3 IPC payload） |
| 2 | `cargo test -p web-ui` | 0 | 30 passed（含 fixture/Tauri mock 对等 3） |
| 3 | `cargo check -p web-sdk --target wasm32-unknown-unknown` | 0 | 通过 |
| 4 | `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate` | 0 | 通过 |
| 5 | `cargo check -p web-ui --target wasm32-unknown-unknown --features csr` | 0 | 通过 |
| 6 | `scripts/build-tauri-csr.sh` | 0 | `dist/tauri/{index.html,web_ui.js,web_ui_bg.wasm,*.css}` |
| 7 | `code-review-graph update` | 0 | Incremental: 25 files / 71 nodes |

`contracts` `ts-rs` warning 仍在，不能宣称零 warning。

### 5.3 未做

- 未在真实 Tauri WebView / 安装包里点一轮（未改 `frontendDist`）。
- 未做 cloud session、上传、updater、deep link。
- Gate 0 仍为 **NO-GO**。
