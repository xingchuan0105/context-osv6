# 在线 Rust 优先 + 桌面 GPUI 全量对等 — 路线图与下一刀

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04（晚） |
| 状态 | **现行下一棒入口**。取代 [`2026-09-04-rust-web-gpui-desktop-handoff.md`](2026-09-04-rust-web-gpui-desktop-handoff.md) 的「下一刀 = GPUI Chat 薄切片」 |
| 权威栈 | [ADR-0011](../adr/0011-rust-web-gpui-desktop.md)（含本日晚间修订） |
| 设计真相 | 在线：[`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) §5–§9、§11（Phase 5 不执行）；桌面：ADR-0011 §3 |
| 审计依据 | 本文 §1（2026-09-04 审核：完成度 / BUG / drift / gap） |

---

## 0. 业主决定（2026-09-04 晚）

| 问题 | 决定 |
|---|---|
| 「Rust 前端」指什么 | **在线 Leptos 优先**，GPUI 靠后 |
| GPUI 第一个可发布版本 | **现网 Tauri 全量对等**：本机栈 UI、知识路径、BYOK、云登录、Publish、更新、深链、MCP 入口 |
| GPUI 后端 | **本机产品**：GPUI 自己拉起 PG/Redis + `avrag-api`/worker sidecar，连 `127.0.0.1:18080` |
| 对等门的「登录」 | **local session**（自动本地 JWT，无云账号）；云登录属全量对等后段 |
| GPUI 依赖线 | **git main 锁 rev**。约束见 §4.1：`gpui` 版本由 `gpui-component` 锁定，不单独追 zed main |
| Windows 构建 | 本机已有 VS Build Tools + MSVC 工具链；在 `C:\dev\context-osv6` 跑 cargo |
| Tauri CSR 死路径 | **立即删除**，不等宿主抽库 |
| 未提交工作区 | 分两笔本地 commit（GPUI/ADR/reducer/在线 一笔；博客/微信/nginx llms.txt 一笔）再开工 |

---

## 1. 审核结论（2026-09-04）

### 1.1 完成度

- **在线 `frontend_rust` Phase 0 — 可用薄切片。** web-sdk（SSE decoder / BrowserHttpTransport / reducer / 会话 REST）、web-ui（Leptos SSR+hydrate `/chat`、`/chat/:id`、会话列表与历史、stop/retry）、web-server（leptos_axum）。web-sdk 20 / web-ui 30 / Playwright 7+2 绿。Gate 0 数字归档（first 14.6% / complete 0.8%；30 min 堆 `unbounded=false`），全部观察项。
- **`desktop_gpui/` — 约 30% 的第一刀。** 6 个文件（lib.rs 30 行 / main.rs 60 行）。夹具一次吃完收束到 `ChatTurnState`，画静态文本；窗口未在真机弹过，无 composer / 流式 / 真 API / 登录 / 取消。相对现网 Tauri 客户端能力 ≈ 0。
- 工作区 26 个文件未提交，含无关改动（`docs/posts/`、`scripts/wechat-publish.py`、`scripts/deploy-blog-post.sh`、`deploy/nginx/app-contextlm.conf` llms.txt）。

### 1.2 BUG

| # | 位置 | 现象 | 处置 |
|---|---|---|---|
| B1 | `desktop/src-tauri/src/commands/chat_stream.rs` `proxy_chat_to_local_api` | 手写 SSE 解析：`String::from_utf8_lossy(&chunk)` 逐 chunk 解码，多字节 UTF-8（中文）跨 chunk 变 U+FFFD；坏帧 `warn` 后忽略（设计 §5.1 明令禁止）。仓库已有正确的 `web-sdk::SseDecoder` | **D0 首个子切片**（可插队，见 §3.0）：改用 `web-sdk::SseDecoder`，坏帧 → 一条 `Error` 终态 |
| B2 | `desktop_gpui/Cargo.toml` `gpui = "0.2"` | crates.io 0.2.2（2025-10 快照）；`gpui-component` 0.6 依赖 `gpui-pre 0.3.x`，两者不能同链；handoff 第 2 步「用 gpui-component Input 做 IME」按现依赖走不通 | D1 切依赖线（§4.1） |
| B3 | `chat_stream.rs` `license_allows_chat` 门 | `license/service.rs::is_dev_mode()` 恒 `true`（ADR-0010 免费客户端），死门 | D0 抽库时删除 |

### 1.3 Drift

- ADR-0011 判死的 Tauri CSR 路径：**Step 0.2 已删**（`tauri_transport.rs` / `csr` feature / `mount_csr` / `build-tauri-csr.sh` / `PocTransport`）。覆盖在 `web-ui/tests/transport_adapter_tests.rs`。
- 迁移设计 §4 构建矩阵 / §10 / Phase 5 / 完成定义、实施计划 §0「≥20% 刚性 Go/No-Go」仍是旧口径，只靠文首横幅作废。→ 本路线图为现行编排；旧文不再逐段改写（时间点快照规则）。
- handoff §4「`cargo run --features ui` 在 Windows 本机编」未写工具链：GPUI Windows 只支持 MSVC，不能 WSL MinGW 交叉、不能在 `\\wsl.localhost` UNC 跑 cargo。→ §4.2。
- `desktop_gpui/src/main.rs` `include_str!("../../frontend_rust/tests/fixtures/…")` 跨工程相对路径。→ D1 改为 `desktop_gpui/fixtures/` 软链或构建期复制（与在线夹具同源，不分叉内容）。

### 1.4 Gap（ADR 要求、未开工）

- **宿主抽库 0%**：`chat_stream` / `cloud_session` / `local_stack` / `native_stack` / `local_product` / `local_session` / `publish` / `documents` / updater / deep link 全绑 `tauri::AppHandle`（`app.emit`、`app.path().app_data_dir()`）。
- GPUI 侧：窗口 / IME / 流式上屏 / 取消 / 登录 / 会话列表 / 历史 / Markdown / 引用 / 进度区，全无。
- macOS / Linux 未编过；打包与更新通道无设计。
- 对等门没有可执行验收清单（`docs/desktop/SMOKE_CHECKLIST.md` 是 Tauri 的）。

---

## 2. 目标形态

```
contracts + web-sdk（SseDecoder / reducer / 会话 REST；无 DOM、无 GPUI、无 Leptos）
        ├── HTTP/SSE → web-ui（Leptos）→ web-server（Axum SSR/静态）        ← 在线，优先
        └── desktop-core（宿主库：本机栈 / 会话 / 流 / 云 / 发布；无 tauri、无 gpui）
                ├── desktop/src-tauri（现网薄壳，继续发货直到全量对等）
                └── desktop_gpui（Windows / macOS / Linux 同一 crate）       ← 桌面，靠后
```

禁止：第二套 `ChatEvent`、第二份 SSE decoder、第二份 reducer、`#[server]` 另开协议、把 GPUI 嵌 WebView。

---

## 3. 轨道 W — 在线 Leptos（优先）

分层生长：每层结束时 `/chat` 仍然能跑真流。Gate 失败停在当层。

### Step 0（本刀，已批）

1. 分两笔本地 commit（§0）。
2. **已做。** 删 Tauri CSR 死路径（§1.3 第一条）；取消与 native `Unavailable` 覆盖改到 `transport_adapter_tests.rs`。ADR-0011 已注记。
3. **已做。** `cargo test -p web-sdk` 23 绿；`cargo test -p web-ui` 38 绿（含 `transport_adapter_tests` 2）；`cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate` 通过；`code-review-graph update` 已跑。

### W1 基础平台（只做 W2 需要的）

| 做 | 验证门 |
|---|---|
| 浏览器凭据 adapter：与 Next `avrag.auth.v1` / `avrag.auth.session` / `avrag.auth.persisted` 同键；bootstrap `GET /api/auth/me`（3s）；去掉 PoC token 框 | Playwright：同键存储水合后 `/chat` 出现会话列表（`chat-journey` W1） |
| `web-server` 静态：启动时预压缩 `.br/.gz`、`/pkg` 长缓存；`hash-files = true` | gate0 harness `encoded_transfer_bytes` 复采（观察，另跑） |
| route manifest：`ROUTE_FAMILIES`（render / auth / noindex）；`nav_parity_tests` 对 `nav-config.ts` | `cargo test -p web-ui` |
| cos-tokens 同步已进 `sync.sh`；`style_baseline_guard.rs` 守卫保持 | 已有 |

### W2 完整 Chat-first 垂直切片（设计 §7 十二条不变量）

按序叠加，每项一个可单测的 reducer/模型缝 + 一个浏览器旅程：Markdown（`pulldown-cmark` + `ammonia` allowlist，恶意 fixture）→ 引用 marker 与来源卡 → 进度 / 推理区终态折叠 → Session files（signed upload）→ scope bar（Session / Workspace / Web 增量叠加）→ `model_role` 与 quick-chat BYOK 读取 → feedback → `/dashboard/:workspace_id` 内复用同一 ChatCanvas。
**Gate 2**：Chat-first 验收矩阵（fixture Playwright + live smoke）全绿；`stream-long-3000` 流式对照复采（观察）。

### W3 应用与交易面 → W4 公共 SSR / SEO / 双语 → W5 切流与清理

按设计 §11 Phase 3 / 4 / 6 逐 route family。**W5（部署、Nginx route owner、删 `frontend_next`）仍须另批**，本文不批。

---

## 4. 轨道 D — 桌面 GPUI 全量对等（靠后；D0 可插队）

### 4.1 依赖线约束（业主选 git main 的可执行形式）

`gpui-component`（仓库已更名 `longbridge/gpui-kit`）main 的 workspace 依赖是 `gpui = { package = "gpui-pre", version = "0.3.x" }`（zed main 当日快照，crates.io，2026-09-03 起发布，默认含 `windows-manifest`）。若 `desktop_gpui` 直接 `gpui = { git = zed }`，与 gpui-component 内的 `gpui-pre` 是两份不同 crate，类型不互通，且 Cargo `[patch]` 无法用改名包替换。因此：

- `gpui-component = { git = "https://github.com/longbridge/gpui-kit", rev = "<pin>" }`
- `gpui` / `gpui_platform` 写成 **gpui-component 同一 rev 所锁的同一来源与版本**（当前即 `package = "gpui-pre"` 0.3.x）
- 升级只动 rev，一起动；`Cargo.lock` 提交。

### 4.2 Windows 构建路径

- 本机 MSVC：`wsl -d Ubuntu -- bash /home/chuan/context-osv6/scripts/sync-windows-dev.sh` 同步到 `C:\dev\context-osv6`，在 `C:\dev\context-osv6\desktop_gpui` 跑 `cargo run`。首次拉 gpui 依赖 5–15 min。
- WSL 只跑无窗口 `cargo test`。不改 `scripts/build-windows.sh`（那是 Tauri 交叉编）。

### D0 宿主抽库 `desktop/core`（crate `desktop-core`；无 tauri、无 gpui）

从 `desktop/src-tauri/src/commands/*` 抽出，`AppHandle` 换成注入的 `DataDir` + 事件回调：

| 子切片 | 内容 | 顺手修 |
|---|---|---|
| D0.1（可插队） | `chat_stream`：`proxy_chat_to_local_api` → `web-sdk::SseDecoder`；`FnMut(&ChatEvent)` 回调不变 | **B1**、**B3** |
| D0.2 | `local_session` / `local_stack` / `native_stack` / `local_product` / `lifecycle`（本机栈生命周期） | |
| D0.3 | `documents` / `api` / `upload_bytes` / `system`（REST 直通、上传、目录） | |
| D0.4 | `cloud_session` / `publish` / `license`（残余）/ deep link 解析 | |

每个子切片：`desktop/src-tauri` 改为调库并 `bash scripts/dev-windows-hotswap.sh shell-only` 点验一次，行为不变。`desktop-core` 自带 `cargo test`。

### D1 GPUI Chat（Windows 硬验收）

切依赖线（§4.1）→ 窗口弹出 → composer（gpui-component `Textarea`）+ **中文 IME 拼音上屏、无重复字符**（zed #56149/#56327 类 WM_KEYDOWN 与 composition 冲突为已知风险，出现即记录、不自写 IME）→ 夹具按时间流上屏（`web-sdk` reducer，`stream-long-3000`）→ 接 `desktop-core`：local session JWT + 真流 `:18080` + 取消 → 会话列表 / 历史（`web-sdk::conversation_api`）。
运行时：`desktop-core` 内一条 tokio runtime 线程，`futures::channel::mpsc` 把 `ChatEvent` 送进 GPUI `cx.spawn`。
**里程碑 M1** = ADR-0011 §3「登录 + 一轮真流 + 取消」不弱于现网（不是删 WebView 的门）。

### D2–D5 全量对等（按 `SMOKE_CHECKLIST.md` 段落）

D2 本机数据面 UI（S1–S6：栈状态 / 启动并迁移 / 产品进程 / 日志目录 / 退出收摊）→ D3 知识路径（K1–K4：workspace、上传、入库任务、引用）→ D4 设置（BYOK、模型角色、云登录 + 钱包、Publish）→ D5 系统集成（updater、deep link、单实例、`open_in_browser`、MCP/CLI 入口）。
交付物：`docs/desktop/GPUI_PARITY_CHECKLIST.md`（逐项对照 SMOKE）。

### D6 三平台与发布

同一 crate 在 macOS / Linux 编过并点验（不叉视图）→ 安装包 / 更新通道另立设计 → **全量对等达成后另批**删除 Tauri WebView UI 路径与 `frontend_next` 桌面产物。

---

## 5. 排序与插队规则

1. Step 0 → W1 → W2 → （D0.1 插队：现网 bug）→ W3 → W4 → D0.2–D0.4 → D1 → D2–D5 → W5（另批）→ D6（另批）。
2. 每刀开工前写 `docs/plans/<date>-<slice>-task.md`，只做该刀；结束时 `--lib`/Playwright 绿 + `code-review-graph update`。
3. 任何编译 / 测试 / 脚本运行先报耗时再跑；WSL `CARGO_BUILD_JOBS=2`。

---

## 6. 红线

- `frontend_next` 只读；`contracts::chat` 唯一 wire；`web-sdk` 唯一 SSE decoder 与 reducer。
- 不改后端 API；用户主气泡只有模型答案。
- 不部署、不动 Nginx/systemd；未另批不删 Next、不改生产 `tauri.conf.json`。
- 不为迁前端强并 Cargo workspace；`desktop-core`、`desktop_gpui`、`frontend_rust`、`desktop/src-tauri` 各自 lockfile。
- Gate 0 数字只观察；不得写成性能已通过。
- 本地 commit 不 push。
