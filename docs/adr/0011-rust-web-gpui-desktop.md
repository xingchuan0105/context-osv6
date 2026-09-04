# ADR 0011: 在线前端 Rust（Leptos），桌面全平台 GPUI

## Status

**Accepted** — 2026-09-04 业主口头拍板（在线 Rust；桌面 Windows / macOS / Linux 均 GPUI）。

**Amended 2026-09-04（晚，业主澄清；编排见 [`plans/2026-09-04-rust-web-first-gpui-parity-roadmap.md`](../plans/2026-09-04-rust-web-first-gpui-parity-roadmap.md)）：**

- 开发顺序：**在线 Leptos 优先**，GPUI 靠后。§5「第一刀」的桌面/在线两轨内容不变，顺序反转。
- GPUI 第一个可发布版本 = **现网 Tauri 全量对等**（本机栈 UI / 知识路径 / BYOK / 云登录 / Publish / 更新 / 深链 / MCP 入口）。删除 WebView UI 路径的门随之提高到全量对等；§3「登录 + 一轮真流 + 取消」降为中途里程碑。
- GPUI 后端 = 本机产品（sidecar，`127.0.0.1:18080`）；里程碑登录 = local session。
- GPUI 依赖线：`gpui-component`（`longbridge/gpui-kit`）git main 锁 rev；`gpui` 用它同一 rev 所锁的同一来源与版本（现为 crates.io `gpui-pre 0.3.x`，zed main 快照）。不单独追 zed main（类型不互通，`[patch]` 无法替换改名包）。
- Tauri CSR 路径（`TauriIpcTransport` / `web-ui` csr / `dist/tauri`）**立即删除**，不等宿主抽库（覆盖下文「宿主抽库之后删除」）。**Step 0.2 已删**（2026-09-04）：`tauri_transport.rs`、`csr` feature / `mount_csr` / `csr/index.html`、`scripts/build-tauri-csr.sh`、`chat_page` 的 `PocTransport` / `is_tauri_runtime()` 分流。覆盖改到 `FixtureTransport` 取消与 native `BrowserHttpTransport` → `Unavailable`。
- Windows GPUI 构建：本机 MSVC（`C:\dev\context-osv6`），不走 WSL MinGW 交叉。

**Supersedes（前端/桌面目标态，不是商业模式）：**

- [`docs/plans/2026-09-03-frontend-rust-migration-design.md`](../plans/2026-09-03-frontend-rust-migration-design.md) 中「Browser hydrate + Tauri CSR 共用 `web-ui`」、§10 Tauri 切 `frontendDist`、Phase 5 / Gate 5、完成定义里的「Browser SSE 与 Tauri IPC 共用 UI」
- Phase 0 曾落地的 `TauriIpcTransport` + `frontend_rust/dist/tauri` CSR：**不是**桌面产品路径；Step 0.2 已删除，不留 WebView 兼容层。历史任务文仍是时间点快照。

**Does not supersede：**

- Gate 0 **测量口径**与已归档数字（first 14.6% / complete 0.8%）。2026-09-04 19:40 起 Gate 0 **全部为观察项**，不再挡开发；不得把观察数字写成性能已通过
- T1–T8、`contracts::chat` 唯一 wire、生产 `/api/` 由 Nginx 直达 `avrag-api`
- ADR-0010 商业模式；现网 Tauri + Next 桌面在 GPUI 对等之前继续发货
- 独立 workspace / 独立 lockfile（`avrag-rs`、`frontend_rust`、`desktop` 不强并）

## Context

Phase 0 证明：Leptos 薄切片能跑真实 SSE；相对 Next 的流式墙钟没有 ≥20% 收益（first token 14.6%，complete 0.8%，夹具时钟封顶）。Rust 前端买到的是工程面（去 Node SSR、一份 `ChatEvent` / reducer），不是「用户等字更快」。

业主目标态不是「Web 停 Next、只改桌面」，而是：

1. **在线前端用 Rust**（Leptos + Axum）
2. **桌面用 GPUI，且 Windows / macOS / Linux 同一套原生 UI**

旧设计用 Tauri WebView 加载同一份 WASM CSR，是为了「三目标一份 UI」。GPUI 是原生 GPU UI，**不能**渲染 Leptos。继续养 Tauri CSR 等于第三套桌面，违反「不保留以后再换的过渡层」。

## Decision

### 1. 两套 UI，一份协议

```
contracts + 纯会话 reducer（无 DOM、无 GPUI、无 Leptos）
        ├── HTTP/SSE  → Leptos web-ui → Axum web-server（在线）
        └── 本机流     → GPUI 视图（Windows / macOS / Linux）
```

| 共用 | 不共用 |
|---|---|
| `contracts::ChatEvent`、纯 reducer、发流 / 取消 / 登录等宿主库 | 组件、样式、路由、WASM CSR、`view!`、GPUI Entity |
| 夹具与终态对等测试 | `TauriIpcTransport` 当产品 Transport |

禁止为桌面再写第二套 `ChatEvent` DTO，禁止 `#[server]` 另开协议。

### 2. 在线：Leptos，不是性能 GO

继续 Rust Web 是**栈选择**（生产页面去 Node），不是 Gate 0 翻盘。体积与 TTI 靠 gzip/br、`application/wasm`、islands / 按需 hydrate，不靠「换成 Rust」自动变快。

生产 `/api/` 仍 Nginx → `avrag-api`。`web-server` 只做 SSR 与静态，不反向代理 API。

全量替换 `frontend_next`、停发 Tauri、部署：仍须另批。Gate 0 不再挡在线/GPUI 开发。生产在对等之前仍是 Next + Tauri。

### 3. 桌面：全平台 GPUI，不再走 WebView UI

- 产品桌面 UI = GPUI，**同一 crate、三平台**。不为某一 OS 另留 Tauri WebView 或 Electron。
- 第一刀以 **Windows IME + 流式上屏** 为硬验收（中文输入），然后同一代码在 macOS / Linux 编过并点验，不叉出第二套视图。
- 许可、本地栈、sidecar、发布、更新、深链、单实例：**先从** `desktop/src-tauri` command **抽成库**，GPUI 与现网 Tauri 都调库。禁止在 GPUI 里再实现一份 `chat_stream`。
- 现网 Tauri + Next `frontendDist` **继续发货**，直到 GPUI 在「登录 + 一轮真流 + 取消」上不弱于现网。达到后删除 WebView UI 路径（含将来若存在的 Leptos CSR `frontendDist`），不长期双 UI。
- 安装包 / 更新通道随 GPUI 发货另立，不在第一刀改生产 `tauri.conf.json`。

### 4. 废掉的路径（计划立即废，代码按切片删）

- 「SSR + hydrate + Tauri CSR 同一 `web-ui`」作为桌面策略
- 为迁桌面而切 `tauri.conf.json` → `frontend_rust/dist/tauri`
- 把 GPUI 嵌进 Tauri WebView，或把 Leptos 当 GPUI 的兼容层
- 为迁前端强并 Cargo workspace

### 5. 第一刀（薄、可验收、互不阻塞）

| 轨道 | 做 | 不做 |
|---|---|---|
| 在线 | 保持现有 `/chat` 切片；需要时再加 WASM 压缩 / islands | 不开始 69 路由搬家；不部署；不改 `frontend_next` |
| 桌面 | 新独立 crate：夹具 `ChatEvent` → 共用 reducer → GPUI 列表 + composer；Windows 先跑通 IME 与流式 | 不接许可 / 本地栈 / 更新；不改生产 `tauri.conf.json`；不并进 `avrag-rs` workspace |

## Consequences

- 要维护两套产品 UI。换来的是：在线去 Node；桌面原生、无 WASM/WebView。
- `frontend_rust` 的 Tauri CSR 切片只作考古（代码已删）；不要再加回 `TauriIpcTransport` / CSR `frontendDist`。
- GPUI pre-1.0，跟 Zed 主仓可能 breaking；`gpui-component` 可用但不替代 `cos-tokens` 决策。三平台打包与自动更新是后续硬成本。
- Gate 0 数字仍记录「流式未达 ≥20%」。不得把观察项写成性能已通过。

## Non-goals

- 本 ADR 不批准删除 `frontend_next` 或停发 Tauri 安装包
- 不批准改后端 API、不批准第二聊天协议
- 不批准把 `desktop/src-tauri` 并入 `avrag-rs` workspace
