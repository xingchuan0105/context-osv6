# Rust 前端 Phase 0：浏览器垂直切片交接文档（下一棒任务入口）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | 浏览器切片、live smoke、Tauri 垂直切片、会话列表 + 历史、Gate 0 **Rust + Next 对照**已完成；Gate 0 仍 **NO-GO** |
| 完成提交 | `1f39b4a3`（feat(frontend_rust): Phase 0 browser vertical slice） |
| 上游任务 | [`2026-09-04-frontend-rust-phase-0-browser-vertical-slice-task.md`](2026-09-04-frontend-rust-phase-0-browser-vertical-slice-task.md)（§10 验证报告） |
| 权威设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) |
| 现状审计 | [`2026-09-03-frontend-rust-migration-final-handoff.md`](2026-09-03-frontend-rust-migration-final-handoff.md) |
| Gate 报告 | [`2026-09-03-phase-0-benchmark-report.md`](2026-09-03-phase-0-benchmark-report.md)（NO-GO 不变） |

## 1. 一句话现状

`frontend_rust` 现在是一个**真实可运行的浏览器薄切片**：Leptos 0.8 SSR + hydration 的 `/chat` 与
`/chat/:sessionId`，经真实 Fetch/ReadableStream 消费 SSE（唯一契约 `contracts::chat::ChatEvent`），
发送/流式/停止/错误/重试全旅程有自动化浏览器证据。`TauriIpcTransport` 已接既有
`chat_stream` / `chat_cancel`，CSR 产物可构建。侧栏只读会话列表 + `/chat/:id` 历史恢复已接入
同一 `ConversationManager` / reducer（浏览器 Fetch；桌面 REST IPC 未做）。**它不是产品前端**，
不得部署；未切 `tauri.conf.json`。Gate 0 已冻结 charter，并采到 Rust debug 与 Next standalone
各 5 冷 + 20 热；核心 p95 未达 ≥20%，缺 30 分钟 / release-like，结论仍 **NO-GO**。

## 2. 架构地图（改造后）

```
frontend_rust/
├── Cargo.toml                 # 配置唯一源：[[workspace.metadata.leptos]]（Leptos.toml 已删除）
├── crates/
│   ├── web-sdk/               # 平台中立客户端；无 DOM/Leptos 依赖
│   │   ├── src/sse_decoder.rs       # 增量 SSE decoder + events_from_byte_stream 消费回路
│   │   ├── src/transport.rs         # ChatTransport / ChatEventStream（Send 按 target 收窄）/ TransportError / Cancellation
│   │   ├── src/browser_transport.rs # wasm32: 真实 Fetch+ReadableStream+AbortController；native: Unavailable
│   │   ├── src/conversation_api.rs  # 只读 sessions/messages URL + parse
│   │   ├── src/browser_rest.rs      # wasm32 GET JSON；native: Unavailable
│   │   ├── src/fixture_transport.rs # 确定性测试用
│   │   └── src/tauri_transport.rs   # wasm32: __TAURI__ invoke/listen；native: mock 或 Unavailable
│   ├── web-ui/                # Leptos 0.8 应用 + 纯状态模型（crate-type = ["cdylib","rlib"]）
│   │   ├── src/app.rs               # App（Router，App 级 provide model/token 上下文）+ shell()
│   │   ├── src/components/chat/chat_page.rs   # ChatPage：composer/消息/活动/推理/引用/错误区域
│   │   ├── src/components/chat/chat_canvas.rs # ChatCanvasModel + StreamScope + on_transport_error
│   │   ├── src/reducer.rs           # reduce_chat_event（终态/request 关联守卫）
│   │   └── src/session.rs           # ConversationManager
│   └── web-server/            # leptos_axum SSR：/chat、/chat/:sessionId、/healthz、静态产物
├── assets/style/chat-poc.css  # PoC 布局（只引用 Token；design-tokens.css 走 style-file 管线）
└── tests/
    ├── fixtures/stream-long-3000.{chunks.json,events.jsonl}   # 3013 字确定性分块夹具
    └── browser/               # Playwright：fixture 套件 + gated live smoke（playwright.live.config.ts）
```

事件流（浏览器旅程断言的链路）：

```
DOM → spawn_chat_stream → BrowserHttpTransport → Fetch(POST /api/v1/chat)
    → ReadableStream 逐块 → SseDecoder → ChatEvent
    → ChatCanvasModel::on_event(StreamScope 校验) → RwSignal → DOM
```

关键不变量：用户主气泡只有模型答案；activity/trace/error 各自区域；终态唯一
（Done 权威收束一次）；旧流事件/失败经 scope + epoch 双重拒绝；`capabilities: Some([])`。

## 3. 复现指南

### 3.1 环境前提（已就位，勿全局安装）

- `wasm-bindgen` CLI 必须与 lockfile 的 0.2.127 精确匹配。全局是 0.2.114（不匹配，勿动）；
  使用 PATH 作用域副本：`export PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$PATH"`。
- `wasm-opt` version_123 已由 cargo-leptos 自动装入 `~/.cache/cargo-leptos`。
- Playwright 依赖：`cd frontend_rust/tests/browser && pnpm install`（chromium 已在 `~/.cache/ms-playwright`）。

### 3.2 常用命令（cwd = `frontend_rust`）

```bash
# 单元/集成测试（约 10s）
cargo test -p web-sdk && cargo test -p web-ui && cargo test -p web-server

# wasm 目标检查
cargo check -p web-sdk --target wasm32-unknown-unknown
cargo check -p web-ui --target wasm32-unknown-unknown --no-default-features --features hydrate

# 完整产物（wasm hydrate + native ssr；release 加 --release）
cargo leptos build            # 产出 target/debug/web-server + target/site/pkg/*
cargo leptos build --release  # 含 wasm-opt；产出 target/release/web-server

# 启动（env 覆盖 [[workspace.metadata.leptos]]）
LEPTOS_SITE_ADDR=127.0.0.1:3001 LEPTOS_SITE_ROOT=$PWD/target/site ./target/debug/web-server

# 浏览器验收（会先自动拉起 web-server:3200 与 fixture:3201；需先 cargo leptos build）
cd tests/browser && pnpm exec playwright test

# live backend smoke（gated；PoC 必须绑 18080，因默认 CORS 不含 3001/3200）
# 要求本机 avrag-api 可访问；8080 是 Plane，用空闲端口另起 API 并设 LIVE_API_BASE
cd tests/browser && LIVE_BACKEND=1 LIVE_API_BASE=http://127.0.0.1:<api-port> \
  pnpm exec playwright test --config playwright.live.config.ts

# Gate 0 采集（需已有 target/debug/web-server；约 1 分钟；不自动 GO）
GATE0=1 bash scripts/run-gate0-perf.sh
# Next 对照（standalone :3000；API :18081。不要用 :8080，那是 Plane）
GATE0=1 GATE0_NEXT_BASE=http://127.0.0.1:3000 GATE0_API_BASE=http://127.0.0.1:18081 \
  bash scripts/run-gate0-perf.sh

# Tauri CSR 产物（不改 desktop tauri.conf.json）
bash scripts/build-tauri-csr.sh   # 产出 dist/tauri/
```

WSL 纪律：`jobs=2`，不要叠加并发全量 cargo 运行；长时间任务后台 + 日志。

### 3.3 已知 warning（如实保留，勿宣称零 warning）

- 共享 `contracts` 的 `ts-rs` 属性解析 warning（`skip_serializing_if = "Vec::is_empty"`）仍存在；
  未越界修改共享契约。

## 4. 已验证 vs 未验证

已验证（证据：切片任务 §10 + Playwright 7/7；live smoke 任务 §6 + Playwright 2/2）：

- decoder 覆盖 §4.1 全部 10 类输入；3000+ 字夹具 decoder→reducer 与 fixture→reducer 终态一致；
- 浏览器侧 113 字节乱序分块（UTF-8 跨包）完整渲染；真实取消被服务端观测；401/坏 JSON 为
  `role="alert"` typed error；retry 新流隔离；SSR 双路由表单语义；hydration 无重复 root/无错误；
- dev 与 release-like 产物均实际构建并启动。
- live backend：无 token 的真实 401 → `unauthorized`；有 JWT 的一轮 Quick Chat 流式收束、URL 落地。
- Gate 0 第一采集（debug）：Rust 5 冷 + 20 热，first token p95 235.5ms，complete p95 1545.9ms。
- Gate 0 Next 对照（standalone :3000）：两侧 5+20；first p95 Rust 234.8 / Next 262.7（10.6%）；complete 1531.6 / 1538.6（0.5%）；30 分钟未采。

未验证/未完成：

- 真实 Tauri WebView 点验；桌面会话列表（需 REST IPC）；Session files、RAG/Web/Workspace/Share/BYOK；
- Markdown 富渲染/代码高亮/虚拟列表；
- Gate 0 仍缺：`cargo leptos build --release` 再采、30 分钟堆斜率（且 `performance.memory` 10MB 分桶不可用）；
- Nginx/systemd/部署脚本（Phase 6 之前禁止）。

## 5. 实施期踩坑记录（下一棒别再踩）

1. **路由切换会 dispose 页面级响应节点**。`/chat` → `/chat/:id` 是两条 Route，导航即重建
   ChatPage。长生命周期任务（流消费）内**只允许**访问 App 级 signal、Router 级 `use_navigate`
   句柄（其内部只捕获 RouterContext）与 `window.location`；读页面级 Memo 会在 dispose 后
   panic，流静默停摆。修复见 `chat_page.rs::spawn_chat_stream` 注释。
2. **Effect 内读 model 必须 untracked**。会话绑定 Effect 若订阅 model，新流确立 session id
   的瞬间（URL 未更新）会被误判为"切回旧会话"，清空消息并作废当前流。Effect 只应响应路由
   参数；`model.with_untracked` 读取。这是本切片最难定位的 bug（retry 后页面被重置）。
3. **cargo-leptos 0.3.6 配置在 workspace 元数据**：`[[workspace.metadata.leptos]]` 且必须是
   Cargo.toml 最后一个 section（`leptos_config` 从该 header 读到文件尾）；lib 包需要
   `crate-type = ["cdylib","rlib"]`。
4. **release 构建 view 类型深度溢出**：未擦除组件模式下嵌套 view! 的查询深度超默认上限，
   需 `#![recursion_limit = "512"]`（`web-ui/src/lib.rs`）。组件继续变大时优先拆组件。
5. **web-sys 0.3.103 的 `RequestInit` builder 风格已废弃**且 `signal()` 签名易错；用
   `set_method/set_headers_headers/set_body_opt_str/set_signal`。
6. **Playwright on Node 24**：测试目录 `package.json` 需 `"type": "module"`（否则 config 加载
   报 `exports is not defined`）；`addInitScript` 只对未来导航生效，改当前页用
   `page.evaluate`；闭合 `<details>` 内的输入框不可见，先点 `summary` 再 `fill`；fixture 服务器
   的计数器用 `/admin/reset` 在 `beforeEach` 清零，避免跨用例污染。
7. **base_url 不能带 query**（transport 直接 `base + "/api/v1/chat"`）；fixture 的字节切片模式
   走路径前缀 `/bytes/:n/api/v1/chat`。
8. **本机 8080 经常既不是 avrag-api 也不是 Next**。本机 `:8080` 是 **Plane**
   （`application-name=Plane`）。Next standalone 在 `:3000`；API 用 `:18081`。
   live smoke 不要改 `.env`；另起空闲端口并设 `LIVE_API_BASE`。PoC 页用 `127.0.0.1:18080`
   （默认 CORS 白名单），不要用 3001/3200 打真实 API。
9. **填 token 会 GET 会话列表**。fixture 必须对带 `/bytes/:n`、`/case/:name` 前缀的
   `GET .../api/v1/chat/sessions` 与 `.../messages` 回 200，否则 Playwright 的
   `console.error` 收集会红。桌面 WebView 不能 Fetch `127.0.0.1`（PNA），历史只走浏览器。
10. **历史 apply 必须带 epoch**。迟到的 GET 不得覆盖新会话或正在 streaming 的 turn。
    URL 回显仍不得 `switch_*`。`/chat` 仅在「未在流式且已有 session id」时 `new_personal_chat`。
11. **Gate 0 计时必须在页内 `performance.now()`**。Node `Date.now()` + Playwright locator
    会在 `/chat` → `/chat/:id` 重挂时读到上一轮 `live-answer`，出现负 first_token / ~400ms
    假 complete。发送前必须 `chat-empty` 且无 live-answer；非增量样本丢弃。
    Done 后 live-answer **仍在**（status ≠ Idle），热路径必须先「新对话」。
12. **不要把 `:8080` 当 Next，也不要只 stub cookie**。`app/(app)/chat/page.tsx` 是
    `return null`，UI 在 layout。要真实登录 + `legal-acceptance`。
    Chromium `performance.memory` 按 10MB 分桶，不能做 30 分钟斜率。
13. **`route.continue` 改到另一端口 = `ERR_BLOCKED_BY_CLIENT`**。Gate 0 用页内 `fetch` 补丁
    把 `POST /api/v1/chat` 发到夹具（与 Rust 3200→3201 同一模式）。
14. **Next `/chat` 首发会 `replace` 到 `/chat/:id` 并 abort SSE**。流式计时必须落在已有
    session 路由（夹具是 `sess-900`）。
15. **夹具末事件无结尾空行时 Next 解析不到 `done`**（残留行不进 `data` 字段，
    `data-pending` 一直为 true）。夹具服务在最后一块后补 `\n\n`。关页前 `unrouteAll`。

## 6. 建议的下一任务切片（按依赖排序）

1. **补齐 Gate 0 剩余条件（仍不许 GO）**：
   - Next 对照已采（§2.3）；debug 核心改善 10.6% / 0.5%，体积护栏失败。
   - Rust release-like：`cargo leptos build --release` 后再采一套，debug 数字不得 GO。
   - 30 分钟：实现 `GATE0_STRESS=1`；不要用分桶的 `usedJSHeapSize` 当斜率。
   - 未达标即按设计 §3.3 停迁移、转优化 Next。
2. **真实 Tauri WebView 点验**（可选，Phase 5 之前）：用临时 `frontendDist` 或独立 window
   验证 IPC 一轮；不要把生产 `tauri.conf.json` 切走 Next。桌面会话列表需要 REST IPC 才能做。
3. 之后才是 Phase 1 固化（route manifest、SEO/security/style 基线测试、Token 同步校验）。

Gate 0 第一采集证据见
[`2026-09-04-frontend-rust-phase-0-gate0-perf-task.md`](2026-09-04-frontend-rust-phase-0-gate0-perf-task.md) §4
与 [`2026-09-03-phase-0-benchmark-report.md`](2026-09-03-phase-0-benchmark-report.md) §2.2。
Next 对照见
[`2026-09-04-frontend-rust-phase-0-gate0-next-contrast-task.md`](2026-09-04-frontend-rust-phase-0-gate0-next-contrast-task.md) §4
与报告 §2.3。
charter：[`2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md`](2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md)（已冻结）。
复跑：`GATE0=1 GATE0_NEXT_BASE=http://127.0.0.1:3000 GATE0_API_BASE=http://127.0.0.1:18081 bash frontend_rust/scripts/run-gate0-perf.sh`。

会话列表 + 历史证据见
[`2026-09-04-frontend-rust-phase-0-session-history-task.md`](2026-09-04-frontend-rust-phase-0-session-history-task.md) §5。

Tauri 垂直切片证据见
[`2026-09-04-frontend-rust-phase-0-tauri-vertical-slice-task.md`](2026-09-04-frontend-rust-phase-0-tauri-vertical-slice-task.md) §5。
仓库 command 实名是 `chat_stream` / `chat_cancel`，不是设计稿里的 `chat_stream_start`。

live backend smoke 已完成，证据见
[`2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md`](2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md) §6。
踩坑：配置端口 8080 可能被 Next HTML 占用；PoC 页必须用 CORS 白名单 origin（`127.0.0.1:18080`），不要用 3001/3200 打真实 API。

## 7. 红线（任何后续任务不得突破）

- `frontend_next` 只读；`contracts::chat` 是唯一 wire 真相（禁止第二 DTO/别名/解析忽略）；
- 不改后端 API；不把协议残片/host 标签拼进用户主气泡；
- 不动 Nginx/systemd/部署脚本，不部署生产；Gate 0 通过前禁止 Phase 1–6；
- 任何 compile/test 先报耗时获同意；结构性改动后 `code-review-graph update`；
- 只提交本任务文件，本地提交不 push。
