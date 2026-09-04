# Rust 前端 Phase 0：live backend smoke

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；验证摘要见文末「验证报告」 |
| 本任务终点 | 现有浏览器薄切片对本地 `avrag-api` 走一轮真实 Quick Chat（含 401 对照）；不代表 Gate 0 通过 |
| 上游交接 | [`2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md`](2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md) |
| 权威设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) |
| 现状审计 | [`2026-09-03-frontend-rust-migration-final-handoff.md`](2026-09-03-frontend-rust-migration-final-handoff.md) |
| Gate 报告 | [`2026-09-03-phase-0-benchmark-report.md`](2026-09-03-phase-0-benchmark-report.md) |

## 0. 任务结果

现有 Leptos `/chat` 薄切片已对本地 `avrag-api` 走通一轮真实 Quick Chat（含无 token 的 401 对照）。产品 Rust 代码未改；只新增 gated Playwright 与文档。完成本任务后 Gate 0 仍保持 **NO-GO**。

## 1. 范围

### 1.1 做

- 本机 `avrag-api` + 现有 PoC token 框 + 一轮 `capabilities: Some([])` Quick Chat。
- 无 token 的 401 对照，用来区分 CORS/网络失败 vs 真打到 API。
- 把 CORS / 鉴权 / 事件 / 主气泡分离写成可复现证据。

### 1.2 不做

- 不改 `frontend_next`、后端 API/CORS 代码、Auth UI。
- 不伪造 `TRUST_PROXY_AUTH` / 代理头。
- 不做 Tauri、会话历史、Gate 0 性能对照、部署。
- 不新增 API base 输入框：继续用 `__POC_CHAT_API_BASE__` 测试缝。
- 产品 Rust 代码默认零改动；只加 gated Playwright 与文档。

## 2. 拓扑

- PoC 页绑在 `http://127.0.0.1:18080`（默认 CORS 白名单已含此 origin）。
- API 默认 `http://127.0.0.1:8080`（以 `avrag-rs/.env` 的 `AVRAG_API_ADDR` 为准，地址不写入文档）。
- 空 `base_url` 会打到 web-server 同源 `/api/v1/chat`（无反代 → 404），因此必须注入 `__POC_CHAT_API_BASE__`。
- `:8081` 是 worker health，不能当作 API 是否在跑的依据。

## 3. 鉴权

- 静默读 `avrag-rs/.env`；不向用户要密钥；不把 token/密码写入文档或提交。
- `POST /api/auth/login`（公开路由）。
- 账号优先复用 `frontend_next/e2e/fixtures/test-user.ts` 的 E2E 夹具；`.env` 的 `E2E_TEST_USER_*` 可覆盖。
- `account_not_registered` 时走公开 `POST /api/auth/register`（法律版本 `2026-06-13`），再 login。
- JWT 只进内存态 `#poc-token-input`。

## 4. 验证门

1. `GET /health`；未监听则启动 `avrag-api`（不启 worker，不 `docker-compose up`）。
2. 若缺 `frontend_rust/target/debug/web-server` 或 `target/site/pkg`，才 `cargo leptos build`。
3. login / 必要时 register。
4. `LIVE_BACKEND=1 pnpm exec playwright test --config playwright.live.config.ts`：
   - 401 对照（无 LLM）：alert 含 `unauthorized`。
   - 真实一轮：主气泡有模型文本；activity/trace 不进主气泡；URL `replace` 到 `/chat/:sessionId`；状态「已完成」。
5. 不跑现有 7 条 fixture。不改 Rust 则不做 `code-review-graph update`。

任一门失败：停在该门，保留命令/退出码/响应摘要，标 Blocked。

## 5. 红线

- `frontend_next` 只读；`contracts::chat` 是唯一 wire 真相。
- 不改后端 API；偏差只记证据。
- 不动 Nginx/systemd/部署脚本；Gate 0 通过前禁止 Phase 1–6。
- 只提交本任务文件，本地提交不 push。

## 6. 验证报告（2026-09-04）

### 6.1 环境事实

- `.env` 中 `AVRAG_API_ADDR` 的端口是 **8080**，但 `GET http://127.0.0.1:8080/health` 返回约 6KB 的 Next.js HTML（`text/html`），`POST /api/auth/login` 与 `POST /api/v1/chat` 均为 `{"error": "Page not found."}`。8080 被前端占用，不是 `avrag-api`。
- `:8081` 无监听（worker 未运行）。上一棒用 8081 判断“后端未运行”不能代表 API。
- 未改 `.env`、未改后端源码、未改 CORS。用已有 `target/debug/avrag-api` 以进程环境 `AVRAG_API_ADDR=127.0.0.1:18081` 另起一份 API（`/health`=200，日志 `avrag api listening addr="127.0.0.1:18081"`）。
- `frontend_rust/target/debug/web-server` 与 `target/site/pkg` 已在，未重建。
- 登录：`POST /api/auth/login` 对仓库 E2E 夹具账号返回 200；`.env` 无 `E2E_TEST_USER_*` 覆盖；未走 register。JWT 只写入临时文件并注入内存态 token 框，未入库、未入文档。

### 6.2 命令与结果

| # | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 1 | `GET /health` on :8080 | 0 | HTTP 200，但是 HTML，不是 API |
| 2 | 启动 `./target/debug/avrag-api`（`AVRAG_API_ADDR=127.0.0.1:18081`） | 0 | listening |
| 3 | `POST /api/auth/login` | 0 | HTTP 200，已有 E2E 用户 |
| 4 | `LIVE_BACKEND=1 LIVE_API_BASE=http://127.0.0.1:18081 pnpm exec playwright test --config playwright.live.config.ts` | 0 | **2 passed / 0 failed**（8.9s） |

未跑现有 7 条 fixture；未改 Rust，未做 `code-review-graph update`。

### 6.3 浏览器证据

1. **401 对照（552ms，无 LLM）**：PoC 源 `http://127.0.0.1:18080`，`__POC_CHAT_API_BASE__` 指向 18081，不填 token。`role="alert"` 含 `unauthorized`，主气泡为空。证明 CORS 预检通过且请求打到真实 API（不是网络/同源 404）。
2. **真实一轮（7.4s）**：展开 PoC token 框填入 JWT，query「只回答：pong」。URL `replace` 到 `/chat/:sessionId`；`live-answer` 非空；状态「已完成」；主气泡不含 `[retrieval_summary]` / `DSML`；无 pageerror。

### 6.4 偏差（只记录，未改后端/Next）

- 本机配置端口 8080 当前被 Next.js HTML 占用，live smoke 不能按文档默认地址直打。一次性把 API 绑到 18081 仅用于本任务，不是产品配置变更。
- 后端无 token 时 chat 路径 JSON `error` 为 `login_required`；PoC transport 把所有 HTTP 401 映射为 `TransportError::Unauthorized`，页面 code 仍是 `unauthorized`。与 fixture 401 用例一致，不算第二契约。

### 6.5 未做

- Tauri、会话历史、Gate 0 同机性能对照、部署。
- 未恢复或改写 8080 上的占用进程。
- Gate 0 仍为 **NO-GO**。
