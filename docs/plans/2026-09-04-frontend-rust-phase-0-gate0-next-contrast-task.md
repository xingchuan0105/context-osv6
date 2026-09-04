# Rust 前端 Phase 0：Gate 0 Next 同机对照（第一采集）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；Gate 0 仍 **NO-GO** |
| 本任务终点 | 真实登录后对 Next `/chat` 采 5 冷 + 20 热（同一 SSE 夹具）。不改 charter，不自动 GO |
| 冻结口径 | [`2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md`](2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md) |
| 上游交接 | [`2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md`](2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md) |

## 1. 范围

### 1.1 做

- 对已占用端口上的 Next `/chat` 做真实 API 登录（E2E 夹具账号；不编 JWT）。
- Playwright 把 `POST /api/v1/chat` 经页内 `fetch` 补丁打到同一 fixture（保分块；`route.continue` 改端口会被拦）；其余 `/api/*` 由采集器转到本机 `avrag-api`（`:8080` 是 Plane，不是 Next）。
- 法律门：登录后 `legal-acceptance`，避免 `LegalReacceptanceGate` 挡住画布。
- 采满 Next 5 冷 + 20 热；能复跑则顺带再采一套 Rust debug。写入审计报告。
- Gate 0 默认仍 NO-GO（本切片不做 release-like、不跑 30 分钟）。

### 1.2 不做

- 不改 `frontend_next`、后端、`tauri.conf.json`、部署脚本、`.env`。
- 不改冻结 charter。
- 不宣布 GO。不进入 Phase 1–6。

## 2. 验证门

1. Next 画布出现 `workspace-chat-composer`（不是 login / 法律门）。
2. `GATE0=1 GATE0_NEXT_BASE=… GATE0_API_BASE=…` 采满 Next 5+20，或记 Blocked（不编造）。
3. 更新 `2026-09-03-phase-0-benchmark-report.md`；本地提交不 push。

## 3. 红线

- 看到数字后不得改 charter。
- `frontend_next` 只读。
- 不把 JWT / 密码写入文档或提交。
- 只提交本任务文件。

## 4. 验证报告（2026-09-04）

### 4.1 做了什么

- 确认 `:8080` 是 Plane，不是 Next。standalone Next 起在 `:3000`；`avrag-api` 用已有进程、只改监听 `:18081`。
- 真实登录 + `legal-acceptance` 后进画布。`POST /api/v1/chat` 走页内 `fetch` 补丁到夹具（`route.continue` 改端口会被拦）。
- 流式在 `/chat/sess-900` 上采（`/chat` 首发会 abort SSE）。夹具服务给末事件补 `\n\n`，否则 Next 解析不到 `done`。
- Next 与 Rust 各 5 冷 + 20 热，discarded = 0。数字写入 [`2026-09-03-phase-0-benchmark-report.md`](2026-09-03-phase-0-benchmark-report.md) §2.3。
- 未改 `frontend_next`、后端、charter。未宣布 GO。

### 4.2 命令

| # | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 1 | `GATE0=1 GATE0_NEXT_BASE=http://127.0.0.1:3000 GATE0_API_BASE=http://127.0.0.1:18081` Playwright gate0 | 0 | 1 passed / 1 skipped；约 1.6min |
| 2 | 核心 p95 | — | first：Rust 234.8 / Next 262.7（10.6%）；complete：1531.6 / 1538.6（0.5%） |

### 4.3 判定

**NO-GO**。核心未达 ≥20%；debug WASM 体积护栏失败；无 30 分钟；非 release-like。
