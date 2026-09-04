# 在线 Rust + 桌面 GPUI 交接（下一棒任务入口）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **历史入口（2026-09-04 晚被取代）**。现行编排见 [`2026-09-04-rust-web-first-gpui-parity-roadmap.md`](2026-09-04-rust-web-first-gpui-parity-roadmap.md)：在线 Leptos 优先，GPUI 全量对等靠后；§5「下一刀 = GPUI Chat 薄切片」不再执行。§3 代码事实、§4 复现、§7 踩坑仍有效 |
| 权威栈 | [ADR-0011](../adr/0011-rust-web-gpui-desktop.md) |
| Gate 口径 | [charter](2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md)（19:40：全部观察，不挡开发） |
| Gate 数字 | [报告](2026-09-03-phase-0-benchmark-report.md) §2.4 / §2.5 / §2.6 |
| 在线切片证据 | [浏览器交接](2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md)（历史入口，不再当下一棒） |
| 本刀任务 | [GPUI 第一刀](2026-09-04-desktop-gpui-chat-slice-task.md) |

工作区里 ADR、charter、reducer 下沉、`desktop_gpui/` **多半尚未本地提交**。下一棒先 `git status`，需要时单独 commit，不 push。

---

## 1. 一句话现状

在线产品前端目标是 **Leptos + Axum**，桌面是 **全平台 GPUI**（Windows / macOS / Linux 同一 crate）。两套 UI，一份 `ChatEvent` + `reduce_chat_event`。

现网仍是 `frontend_next` + Tauri WebView，继续发货。Gate 0 流式没有 ≥20%（14.6% / 0.8%），30 分钟堆未判无界；**数字只观察，不停工**。不得写成性能已通过。

GPUI 已有独立工程：夹具能收到 `Done`，`cargo check --features ui` 已绿。窗口没在真机弹出过，没有 composer，没有按时间吐 token，没有真 API。

---

## 2. 目标形态

```
contracts + web-sdk::reduce_chat_event（无 DOM、无 GPUI、无 Leptos）
        ├── HTTP/SSE  → frontend_rust web-ui / web-server（在线）
        └── 本机流     → desktop_gpui（三平台）
```

| 共用 | 不共用 |
|---|---|
| `contracts::chat::ChatEvent`、`ChatTurnState`、`reduce_chat_event`、`FixtureTransport` | Leptos 组件、GPUI 视图、CSS、WASM CSR |
| 夹具与终态对等测试 | `TauriIpcTransport` 当产品桌面 UI |

禁止第二套 DTO，禁止 `#[server]` 另开协议，禁止把 GPUI 嵌进 Tauri WebView。

---

## 3. 已经落地（不要重做）

### 3.1 业主决定

| 何时 | 决定 |
|---|---|
| 18:56 | Gate 0 **护栏**改观察 |
| 约 19:16–19:22 | 在线 Rust；桌面 **全平台 GPUI**（不是 Windows-only，也不是 Web 停 Next） |
| 19:40 | Gate 0 **整闸**改观察，继续开发 |
| 之后 | 下一刀是 GPUI Chat 薄切片，不是 69 路由搬家 |

### 3.2 Gate 0 数字（release-like，观察）

| 指标 | Rust | Next | 备注 |
|---|---|---|---|
| first token p95 | 229.0ms | 268.0ms | +14.6%；差在无打字机 |
| complete p95 | 1527.8ms | 1540.7ms | +0.8%；夹具 97×15ms 封顶 |
| 传输 | 1.46MB | 0.56MB | Axum 未压 WASM |
| 30 分钟 CDP | 63 点，后 10 分钟 +41 KB/min | — | `unbounded=false` |

采集：`GATE0=1` 的 5+20；`GATE0_STRESS=1 GATE0_RUST_PROFILE=release` 且不要同时开 `GATE0=1`。`:8080` 是 Plane，不是 Next。

### 3.3 代码

- `reduce_chat_event` 在 `frontend_rust/crates/web-sdk/src/reducer.rs`；`web-ui/src/reducer.rs` 只 `pub use`。
- `FixtureTransport::events()` 可供同步收束。
- `desktop_gpui/`：独立 Cargo，**不是** `avrag-rs` / `frontend_rust` workspace 成员。默认 feature 不含 `gpui`；窗口二进制要 `--features ui`。
- `desktop_gpui::reduce_fixture_json_lines` 吃 JSONL / `data:` 行，与在线夹具同一解析。
- 当前窗口 `main.rs` 一次吃完 `stream-normal-long.json` 再画终态，**不是**流式上屏。
- `web-ui` 全套测试已绿（含 reducer fixture / canvas lifecycle）。`desktop_gpui` 无窗口测试已绿。`cargo check --features ui` 已绿。
- `code-review-graph update` 已在 reducer 下沉后跑过。

### 3.4 作废路径

- 「SSR + hydrate + Tauri CSR 同一 `web-ui`」不再是桌面策略。
- 不要再给 `TauriIpcTransport` / `frontend_rust/dist/tauri` 加产品功能（Step 0.2 已删该路径）。
- 不要切生产 `desktop/src-tauri/tauri.conf.json`。
- 旧设计 Phase 5 / Gate 5（Leptos CSR 进 Tauri）不执行。见 ADR-0011。

---

## 4. 复现

WSL 登录壳（`bash -lc`），`CARGO_BUILD_JOBS=2`。`wasm-bindgen` 用 `~/.local/opt/wasm-bindgen-cli-0.2.127` 再 `~/.cargo/bin`。

```bash
# 共用 reducer + 在线（约 15–40s）
cd /home/chuan/context-osv6/frontend_rust
cargo test -p web-sdk --lib
cargo test -p web-ui

# GPUI 无窗口（约 30s，已有 target 时更短）
cd /home/chuan/context-osv6/desktop_gpui
cargo test

# 类型检查窗口（约 1min；首次拉 gpui 5–15min）
cargo check --features ui

# 弹出夹具终态窗：在 Windows 本机编，不要指望无显示的 WSL
cargo run --features ui
```

`gpui` 0.2 必须 `use gpui::prelude::*;`（`Styled` / `ParentElement` / `AppContext`），只 `use gpui::{div, ...}` 会 E0599。

---

## 5. 下一刀（只做这一刀）

任务名建议：`docs/plans/2026-09-04-desktop-gpui-composer-stream-task.md`（下一棒开写）。

按顺序，做完再往下：

1. **Windows 弹出窗口**  
   `cargo run --features ui`。硬验收：夹具用户气泡可见（含「系统架构设计方案」一类正文），status 为 done。WSL 无 DISPLAY 不要死磕。

2. **Composer + 中文 IME**  
   输入框、回车发送。硬验收：拼音能上屏进框。优先 `gpui-component` 的 Input/Textarea，不要手写 IME。不接登录。

3. **夹具按时间流上屏**  
   仍用 `web-sdk` reducer + 现有 JSONL / `stream-long-3000`。token 逐条进 `ChatTurnState`，气泡变长，Done 后停。不要接 `avrag-api`，不要抽 Tauri `chat_stream`。

本刀不做：许可、本地栈、更新、深链、macOS/Linux 打包、在线 69 路由、WASM gzip/islands、删 Next、部署。

验证门（本刀结束才算完）：

| # | 门 | 估计 |
|---|---|---|
| 1 | `cargo test` in `desktop_gpui` + `cargo test -p web-ui`（reducer 未分叉） | 1–2 min |
| 2 | Windows 窗口：能输入中文、回车后夹具流能看完 | 人工 |
| 3 | 结构性改动后 `code-review-graph update` | ~2s |

耗时先报再跑。`cargo run --features ui` 首次可能 5–15 分钟。

---

## 6. 再往后（先不要做）

- 真 SSE / 登录：从 `desktop/src-tauri` command **抽库**，GPUI 与现网 Tauri 都调；禁止在 GPUI 里再实现一份 `chat_stream`。
- 同一 GPUI crate 在 macOS / Linux 编过并点验，不叉视图。
- 在线：WASM `gzip`/`br` + `application/wasm`、islands。不自动开 69 页搬家。
- GPUI 在「登录 + 一轮真流 + 取消」不弱于现网之后，再删 WebView UI。在那之前 Tauri + Next 继续发货。

---

## 7. 踩坑（下一棒别再踩）

1. **Gate 0 不是停工令，也不是性能通过。** 14.6% / 0.8% 是夹具时钟 + 打字机，不是 WASM 更快。
2. **`:8080` 是 Plane。** Next standalone `:3000`，API 用 `127.0.0.1:18081`。PoC 打真 API 用 CORS 白名单 `127.0.0.1:18080`。
3. **不要 `GATE0=1` 和 `GATE0_STRESS=1` 一起开**，否则先跑 5+20。
4. **`performance.memory` 10MB 分桶**，斜率只用 CDP `Runtime.getHeapUsage`。
5. **`route.continue` 改端口 = `ERR_BLOCKED_BY_CLIENT`。** 在线 Gate 0 用页内 fetch 补丁。
6. **Reducer 只此一份。** 改状态机改 `web-sdk`，不要在 `desktop_gpui` 或 `web-ui` 复制。
7. **GPUI 0.2 要 prelude。** Windows 主场；中文 IME 是硬验收，不是附带。
8. **独立 lockfile。** `desktop_gpui` 不并进 `avrag-rs`。`desktop/src-tauri` 也是独立 workspace。
9. **PowerShell 不要 `&&`、不要 heredoc 邮件头。** 命令走 `wsl.exe -d Ubuntu -- bash /绝对路径.sh`；CRLF 先转 LF。
10. **`nohup` 从短命 `wsl.exe` 脚本里起的进程会随脚本退出。** 长驻服务用长时间 background job。
11. JWT 若出现在 Playwright / gitignore 的 diag JSON，**不要**写进文档或 commit。

---

## 8. 红线

- `frontend_next` 只读；`contracts::chat` 唯一 wire。
- 不改后端 API；用户主气泡只有模型答案。
- 不部署、不动 Nginx/systemd；未另批不删 Next、不改生产 `tauri.conf.json`。
- 不为迁前端强并 Cargo workspace。
- 本地 commit 不 push，除非业主要。
