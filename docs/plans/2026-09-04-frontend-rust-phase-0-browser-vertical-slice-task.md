# Rust 前端 Phase 0：Browser Fetch/SSE + Leptos `/chat` 垂直切片任务

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；验证摘要见文末「验证报告」 |
| 本任务终点 | 完成 **Phase 0 的浏览器垂直切片**；不代表 Gate 0 通过 |
| 代码起点 | 编写本文时仓库 HEAD 为 `4b9f8c3f`；`frontend_rust` 最近基线为 `1b6660c8` |
| 权威设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) |
| 现状审计 | [`2026-09-03-frontend-rust-migration-final-handoff.md`](2026-09-03-frontend-rust-migration-final-handoff.md) |
| Gate 报告 | [`2026-09-03-phase-0-benchmark-report.md`](2026-09-03-phase-0-benchmark-report.md) |

## 0. 任务结果

在现有 `frontend_rust` PoC 骨架上交付一个真实可运行的浏览器薄切片：

1. Axum 为 `/chat` 与 `/chat/:sessionId` 输出真实 Leptos SSR 壳；
2. 浏览器 WASM 成功 hydration；
3. 用户可发送一轮纯 Quick Chat 请求；
4. `BrowserHttpTransport` 通过真实 Fetch 读取 SSE 字节流，并把唯一契约 `contracts::chat::ChatEvent` 交给现有 reducer / `ChatCanvasModel`；
5. 页面可展示用户消息、流式回答、独立进度/错误区域，并支持 stop 与 retry；
6. 受控浏览器测试证明网络分块、取消和终态行为经过真实浏览器边界，而不是直接调用 fixture transport。

完成本任务后，Gate 0 仍保持 **NO-GO**。真实 Tauri IPC、完整 Chat-first 功能、同机性能对照和 Gate 0 决策分别由后续任务承接。

## 1. 执行前读取顺序

实施 Agent 开始修改前完整读取：

1. 仓库根 `AGENTS.md`；
2. 权威设计的 §3、§4、§5、§6、§11；
3. 现状审计全文；
4. `contracts/src/chat.rs` 中 `ChatRequest`、`ChatEvent`、`ChatResponse`；
5. 当前 `frontend_rust/crates/web-sdk`、`web-ui`、`web-server`；
6. Next 的现行只读参考：
   - `frontend_next/lib/workspace/stream.ts`；
   - `frontend_next/hooks/chat-session/use-chat-stream.ts`；
   - `frontend_next/components/chat/chat-canvas.tsx`。

随后用 `code-review-graph` 查询 `BrowserHttpTransport`、`ChatTransport`、`ChatCanvasModel` 与 `web-server::create_app` 的最小上下文和影响半径。当前工作树可能继续变化，以执行时的 HEAD 为准；保留用户已有改动，不做 reset、checkout 或批量格式化。

**Gate A 完成条件：** Agent 能列出每个待改模块的责任、调用方和测试面，并确认没有通过复制 Next 代码或复制 wire type 建立第二真相源。

## 2. 范围边界

### 2.1 允许修改

- `frontend_rust/Cargo.toml`、`Cargo.lock`、`Leptos.toml`；
- `frontend_rust/crates/web-sdk/**`；
- `frontend_rust/crates/web-ui/**`；
- `frontend_rust/crates/web-server/**`；
- `frontend_rust/tests/**`；
- 本文档、现状审计和 Gate 报告中的事实状态；
- 如需要浏览器验收脚本，仅放在 `frontend_rust` 自己的测试目录中，并优先复用仓库已有 Playwright/浏览器工具链。

### 2.2 Next 端边界

`frontend_next` 已闭环，是本任务的**只读产品基线与行为参考**：

- 不修改 Next 页面、组件、测试、依赖或构建配置；
- 不把 Rust PoC 接入 Next 构建；
- 若发现 Next 与契约存在疑似偏差，只在交接报告记录可复现证据，不在本任务顺手修复；
- 不把 Next 的 React hook、Zustand store 或 TypeScript SSE parser 逐行翻译进 Rust。

### 2.3 明确留给后续任务

- 真实 `TauriIpcTransport` 和桌面打包；
- 登录、注册、退出及完整浏览器 Auth UI；
- 会话列表、服务端历史加载、Session files、RAG、Web Search、Workspace、Share、BYOK 设置；
- Markdown 富渲染、代码高亮、复杂引用卡片和虚拟列表；
- Next/Rust 的正式 LCP、Heap、掉帧和 30 分钟压力对照；
- Nginx、systemd、部署脚本、生产流量、`desktop/src-tauri/tauri.conf.json`；
- Phase 1–6 的任何组件或兼容层。

## 3. 不变产品契约

### 3.1 请求

本切片只发纯个人 Quick Chat：

- `workspace_id = None`；
- 新会话 `session_id = None`，恢复路由可带现有 `session_id`；
- `agent_type = "chat"`；
- `capabilities = Some([])`，明确表达纯 Chat；
- `stream = true`；
- 客户端不发送 `model_role`，服务端按个人 Conversation 创建入口持久化 `quick_chat`；
- 客户端不伪造服务端 `request_id`。

`ChatRequest` 必须直接来自共享 `contracts`。如果共享契约在 WASM 上形成真实阻塞，先记录最小失败证据，再按权威设计讨论更小的 wire crate；本任务不复制 DTO。

### 3.2 事件

`contracts::chat::ChatEvent` 是唯一事件类型。SSE 的 `event:` 字段属于 framing；其值必须在反序列化前合入对应 JSON 对象。至少处理：

- `start`：锁定服务端 `request_id/session_id`；
- `operation_guide`：不进入用户主气泡；
- `activity`：进入独立进度区域；
- `answer_start`：记录消息和 agent 信息；
- `trace`：进入诊断/进度状态，不拼入答案；
- `token`：增量答案；
- `reasoning_summary_delta`：进入独立摘要区域；
- `citations`：更新引用状态；
- `done`：以 `ChatResponse` 权威收束一次；
- `error`：进入可恢复终态。

不得新增旧事件别名、兼容 DTO 或“解析失败即跳过”的成功路径。

### 3.3 用户信道

用户主气泡只显示模型答案。Activity、trace、协议错误、HTTP 错误和运行时诊断进入各自状态区域。Transport 或 host 不向答案正文追加披露脚注。

## 4. Gate B：SSE decoder

先把 SSE framing 从浏览器 API 中分离为 `web-sdk` 内部的增量 decoder；decoder 输入字节分块，输出 `ChatEvent` 或 typed error。它不依赖 DOM、Leptos、路由或浏览器 storage。

### 4.1 必测输入

1. UTF-8 汉字跨 byte chunk；
2. `\n` 与 `\r\n`；
3. 一个事件多行 `data:`；
4. comment/keepalive；
5. 一个 chunk 包含多个完整事件；
6. 事件跨多个 chunk；
7. EOF 前没有最后空行；
8. 缺少 `event:`、空 `data:`、未知事件、坏 JSON；
9. reader 中途失败；
10. 正常 `done`、单独 `error`、终态后迟到事件交由 reducer 拒绝。

现有 `stream-normal-long.json` 不满足 3000+ 字与真实 SSE framing 要求。本任务至少新增一组**脱敏、确定性、3000+ 字**的分块样本，并保留原始 byte/chunk 边界；不得把产品 prompt、真实用户内容、密钥或 golden answer 放入 fixture。

### 4.2 Error 语义

复用并收敛 `TransportError`，至少能区分：

- 网络失败；
- 401、402、403、429；
- 其他非 2xx（保留 status 和经过长度限制的 body）；
- 空响应体；
- framing 错误；
- JSON/contract 错误；
- 中途断流；
- 客户端取消。

**Gate B 完成条件：** 所有上述输入有确定性测试；坏帧不会消失为成功；合法帧序列得到与 fixture transport 相同的 reducer 终态。

## 5. Gate C：真实 Browser Fetch/SSE transport

实现 `BrowserHttpTransport` 的 WASM 路径：

1. 空 `base_url` 表示同源 `/api/v1/chat`；显式 base URL 只用于隔离测试；
2. 请求头包含 `Accept: text/event-stream` 和 `Content-Type: application/json`；token 非空时才加 Bearer；
3. HTTP 非 2xx 在进入 SSE decoder 前映射为 typed error；
4. 响应体以流读取，不能先缓冲完整响应；
5. `Cancellation` 必须触发底层 Fetch abort，并令消费者得到唯一 `Cancelled` 结果；
6. Stream 被丢弃时释放 reader/网络资源；
7. native 非浏览器路径继续明确返回 `Unavailable`，不伪造空流成功；
8. 如果浏览器流对象不是 `Send`，按目标平台收窄 stream 类型约束；不要用无意义 wrapper 假装线程安全。

优先采用成熟、维护活跃且支持当前 WASM 工具链的依赖。新增依赖前先检查现有 workspace、类型能力和生成后的 transitive surface；只引入完成 Fetch/stream/hydration 所需的最小集合。

### 5.1 浏览器网络证明

建立受控的同源 SSE 测试端点或测试服务器，并让真实浏览器页面经过：

```text
DOM → BrowserHttpTransport → Fetch → HTTP response body → SSE decoder
    → ChatEvent → ChatCanvasModel/reducer → DOM
```

至少证明：

- 响应被拆成非对齐网络 chunk 后仍正确渲染；
- stop 在终态前真正中止 reader；
- abort 后的迟到字节不再修改 UI；
- HTTP 401 与坏 JSON 在页面显示为错误状态；
- 重试创建新的本地 stream scope，旧流无法污染新流。

测试使用内存中的非秘密 token。真实后端 smoke 若需要现有凭据，只从 `avrag-rs/.env` 读取并保持静默，不写入源码、HTML、日志、fixture 或报告；缺少可用凭据时记录“live backend 未验证”，不得改造 Auth 或伪造通过。

**Gate C 完成条件：** 至少一项自动化浏览器测试确实穿过 Fetch/ReadableStream 边界；纯 Rust 单测或直接调用 decoder 不能替代这一证据。

## 6. Gate D：Leptos SSR/hydration `/chat`

将当前硬编码 HTML 壳替换为真正的 Leptos 应用，保持组件边界简单：

- `web-sdk`：请求、Fetch、SSE、typed error；
- `web-ui`：Leptos view、页面状态、现有 reducer / `ChatCanvasModel`；
- `web-server`：SSR、路由、静态资源、`/healthz`。

### 6.1 最小 UI

页面至少具有：

- canonical `/chat` 和 `/chat/:sessionId`；
- 有明确 label 的 textarea；
- Send、Stop、Retry；
- 用户消息列表与 assistant 流式正文；
- 与答案分离的 activity/status、reasoning summary、citation 区域；
- 错误 `role="alert"`；流式状态使用合适的 `aria-live`；
- 新会话收到服务端 session id 后，地址更新为 `/chat/:sessionId` 且不重置当前流；
- stop/retry 后焦点回到 composer；
- 加载基础 Token 样式，不扩建完整设计系统。

SSR HTML 必须包含可读的页面骨架和表单语义，不能继续只输出 `Context-OS Chat PoC` 占位 div。Hydration 后 DOM 无 panic、无重复 root、无 hydration mismatch。

### 6.2 凭据输入

完整 Auth UI 不在本任务内。根组件可接收一个窄的、内存态 PoC token 输入；Transport 不负责读取或持久化浏览器凭据。不得新增 URL token、硬编码 token、独立登录页或新的持久化 key。

**Gate D 完成条件：** `/chat` 与 `/chat/test-session` 的 SSR smoke 均返回含表单语义的 HTML；真实浏览器 hydration 后可完成发送、流式、停止、错误和重试旅程。

## 7. Gate E：验证与交接

仓库要求任何 compile、test、构建或脚本运行前先向用户给出预计耗时并获得同意。获得同意后，按从窄到宽的顺序运行；不得在前一门失败时叠加后一门：

1. `web-sdk` decoder/transport 定向测试；
2. `web-ui` reducer、模型和 view 定向测试；
3. Browser 自动化 smoke；
4. `web-sdk` + `web-ui` 的 `wasm32-unknown-unknown` hydrate check；
5. `web-server` native SSR check/test；
6. release-like Leptos build 和实际启动 smoke；
7. `git diff --check`；
8. 结构修改完成后 `code-review-graph update`，再检查变更图谱。

如果运行环境没有 `cargo-leptos` 或所需浏览器 runner，先报告安装内容、预计时间和磁盘影响，再取得用户同意。不得用 `cargo check` 代替“可运行 SSR/hydrate 产物”的结论。

验证报告逐条记录：命令、退出码、通过数量、warning、跳过项和未验证项。共享 `contracts` 现有 `ts-rs` 属性 warning 若仍出现，必须如实记录，不得声称零 warning。

完成后更新：

- `2026-09-03-frontend-rust-migration-final-handoff.md`：把 Browser vertical slice 移入已完成，并保留 Tauri、完整功能和 benchmark 阻断项；
- `2026-09-03-phase-0-benchmark-report.md`：只更新真实性状态，不填推测性能数字，结论继续保持 NO-GO；
- 本文档顶部状态改为 Done 或 Blocked，并附验证摘要。

最后仅暂存本任务文件，保留所有无关工作树改动；按 solo trunk 纪律创建本地提交，不 push。

**Gate E 完成条件：** 代码、浏览器证据、三份文档状态和 code-review graph 一致；没有把静态检查、fixture 单测或提交说明冒充真实浏览器运行。

## 8. 停止条件

出现任一情况即停在当前 Gate，保留证据并请求决策：

- 需要修改 `contracts::ChatEvent` 或后端 API 才能完成浏览器流；
- 需要修改 Next 端才能让 PoC 工作；
- 需要通过第二套 DTO、双协议或解析失败忽略来绕过契约；
- SSR 与 hydrate 只能依赖两套不同状态模型；
- 真实取消无法终止底层 Fetch/reader；
- 浏览器工具链要求未经批准的全局安装或大规模依赖升级；
- 任一验证门失败且根因尚未定位。

Blocked 不是失败伪装：文档应写清已完成到哪一步、最小复现、错误原文、影响范围和下一次需要的具体决定。

## 9. 最终验收清单

- [ ] Next 保持只读，生产与部署配置未改变；
- [ ] `ChatRequest` / `ChatEvent` 只来自共享 `contracts`；
- [ ] 真实增量 SSE decoder 覆盖 UTF-8、CRLF、多行 data、keepalive、EOF 和坏帧；
- [ ] Browser Fetch 逐块消费响应体，非 2xx 与断流为 typed error；
- [ ] Cancellation 真正 abort 网络读取；
- [ ] `/chat` 与 `/chat/:sessionId` 为真实 Leptos SSR + hydrate；
- [ ] DOM 旅程覆盖 send、stream、stop、error、retry 和迟到流隔离；
- [ ] 用户答案与 activity/trace/error 信道分离；
- [ ] 自动化浏览器测试穿过真实 Fetch/ReadableStream；
- [ ] release-like SSR/hydrate 产物实际构建并启动；
- [ ] 所有 warning、跳过项和 live backend 限制如实记录；
- [ ] Gate 0 仍标记 NO-GO；
- [ ] 结构图谱已更新；
- [ ] 只提交本任务相关文件，未 push、未部署。

## 10. 验证报告（2026-09-04，实施 Agent 记录）

### 10.1 实现摘要

- **Gate A**：code-review-graph 确认 `BrowserHttpTransport`/`ChatTransport` 仅在 `web-sdk`、`ChatCanvasModel::on_event` 调用方全为既有测试、`web-server::create_app` 仅被 `main` 调用；无第二真相源（wire type 唯一来自 `contracts::chat`）。
- **Gate B**：新增 `web-sdk/src/sse_decoder.rs`（平台中立增量 decoder + `events_from_byte_stream` 消费回路）；`TransportError` 增加 `EmptyBody`/`Interrupted`；新增脱敏确定性 3013 字夹具 `tests/fixtures/stream-long-3000.chunks.json`（97 个显式 chunk，含 CRLF 帧、keepalive 注释、末事件无尾空行）与对应 `stream-long-3000.events.jsonl`。
- **Gate C**：`BrowserHttpTransport` wasm32 路径实现真实 Fetch + ReadableStream 逐块消费（不缓冲全响应）；非 2xx 在 decoder 前映射 typed error（401/402/403/429/其他，body 截断 4096 字符）；`Cancellation` 驱动 `AbortController.abort()`，消费者得到唯一 `Cancelled`；流提前 drop 时守卫同样 abort；`ChatEventStream`/`ChatTransport` 按 target 收窄 Send 约束（wasm 不伪装线程安全）；native 继续返回 `Unavailable`。
- **Gate D**：`web-ui` 成为真实 Leptos 0.8 应用（`App`/`shell`/`ChatPage`），`web-server` 改为 `leptos_axum` SSR（`/chat`、`/chat/:sessionId`、`/healthz`、静态产物）；composer 有 label，Send/Stop/Retry、用户/助手消息、独立 activity/reasoning/citations 区域、错误 `role="alert"`、流式区 `aria-live`；session id 落地后 `replace` 到 `/chat/:id` 且不打断流；stop/retry 后焦点回 composer；PoC token 仅内存态（App 级上下文，不持久化、不进 URL）。
- **配置**：cargo-leptos 0.3.6 要求 workspace 元数据，`Leptos.toml` 已并入 `frontend_rust/Cargo.toml` 的 `[[workspace.metadata.leptos]]`（单一配置源，运行时由 `get_configuration(Some("Cargo.toml"))` 读取，`LEPTOS_*` 环境变量可覆盖）。
- **修复的实现期缺陷**（均有浏览器证据）：路由切换 dispose 页面级 Memo 导致流任务 panic（改为任务内只访问 App 级 model / Router 级 navigate / `window.location`）；会话绑定 Effect 误订阅 model 造成"新流 session 落地前被当作切回旧会话"的清空竞态（改为 untracked 读取，仅响应路由参数）。

### 10.2 验证命令与结果（从窄到宽，逐条执行）

| # | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 1 | `cargo test -p web-sdk` | 0 | 17 passed / 0 failed（§4.1 全部 10 类输入 + 字节级双切分穷举 + 3000 字夹具） |
| 2 | `cargo test -p web-ui` | 0 | 30 passed / 0 failed（reducer/生命周期 19、fixture 3、decoder parity 1、benchmark 2、style guard 1、tauri 3、lib 1） |
| 3 | `cargo check -p web-sdk --target wasm32-unknown-unknown` | 0 | 通过 |
| 4 | `cargo check -p web-ui --target wasm32-unknown-unknown --no-default-features --features hydrate` | 0 | 通过 |
| 5 | `cargo test -p web-server` | 0 | 0 tests（编译通过） |
| 6 | `cargo leptos build`（dev：wasm hydrate + native ssr + wasm-bindgen） | 0 | 通过（约 1m30s） |
| 7 | `pnpm exec playwright test`（`frontend_rust/tests/browser`，chromium） | 0 | **7 passed / 0 failed**（详见 10.3） |
| 8 | `cargo leptos build --release`（含 wasm-opt）+ 启动 smoke | 0 | 通过（`/chat`、`/chat/test-session` SSR 含表单语义；`/pkg/web_ui.js|.wasm|.css`、`/style/chat-poc.css` 均 200；`/healthz`=ok） |
| 9 | `git diff --check` | 0 | 通过 |

### 10.3 浏览器证据（Gate C/D 收口，全部穿过真实 Fetch/ReadableStream 边界）

1. SSR smoke：`/chat` 与 `/chat/test-session` 返回含 `for="chat-composer-input"`、`<textarea`、发送/停止/重试按钮与 `/pkg/web_ui.js` 接线的 HTML，不再是 `Context-OS Chat PoC` 占位壳。
2. hydration：单一 `chat-canvas` root、无 pageerror、无 hydration console error。
3. send→stream：fixture 以 113 字节切片（UTF-8 跨包）发送 3013 字答案，页面完整渲染（尾部 marker 断言 + 长度 ≥3000）；activity/reasoning/citations 各自区域可见且文案不进入主气泡；URL `replace` 到 `/chat/sess-900` 且流不中断；`Authorization: Bearer poc-test-token` 仅在 token 非空时注入（fixture 端断言）。
4. stop：终态前点击停止后状态为「已停止」；**服务端观测到连接被 abort**（证明真实取消穿越网络边界）；800ms 内 UI 文本零变化（迟到字节不改写）；焦点回到 composer。
5. 401：页面 `role="alert"` 显示 `unauthorized`，答案主气泡为空。
6. 坏 JSON：`role="alert"` 显示 `framing` typed error（坏帧不消失为成功）。
7. retry：停止慢流后重试创建新 stream scope，完整渲染新答案，旧流 token 不污染；fixture 端确认恰好 2 次真实网络请求。

### 10.4 Warning / 跳过项 / 未验证项（如实记录）

- 共享 `contracts` 的 `ts-rs` 属性 warning 仍存在（`#[serde(default, skip_serializing_if = "Vec::is_empty")]` 无法解析；未越界修改共享契约），在 cargo 输出中可见；**不能宣称零 warning**。
- `wasm-bindgen` CLI：全局为 0.2.114，与 lockfile 的 0.2.127 不匹配；按任务要求未做全局安装，改用 PATH 作用域的 `~/.local/opt/wasm-bindgen-cli-0.2.127`（官方预编译二进制）。复现构建：`PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$PATH" cargo leptos build [--release]`。
- `wasm-opt` version_123 由 cargo-leptos 自动装入其用户缓存（`~/.cache/cargo-leptos`），未动全局环境。
- release 构建需要 `#![recursion_limit = "512"]`（leptos 未擦除组件的 view 类型深度；已加在 `web-ui/src/lib.rs`）。
- **live backend smoke 未执行**：本机 8081 后端未运行；未改造 Auth、未伪造通过。所有浏览器证据来自受控 fixture SSE 服务器与内存态非秘密 token。
- Tauri IPC、完整 Chat-first 功能、Next/Rust 同机性能对照：按 §2.3 留给后续任务，Gate 0 保持 **NO-GO**。
