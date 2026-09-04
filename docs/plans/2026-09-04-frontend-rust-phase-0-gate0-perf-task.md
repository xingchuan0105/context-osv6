# Rust 前端 Phase 0：Gate 0 同机性能对照（第一采集）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；Gate 0 仍 **NO-GO** |
| 本任务终点 | 冻结 charter；用同一 SSE 夹具在浏览器里采 Rust（及可达时的 Next）5 冷 + 20 热。不自动 GO |
| 冻结口径 | [`2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md`](2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md) |
| 上游交接 | [`2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md`](2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md) |

## 1. 范围

### 1.1 做

- 冻结 charter（核心两项 = 流式 first token / complete；LCP/体积/堆是护栏）。
- Playwright Gate 0 采集器：Rust `/chat` 打既有 fixture；Next `/chat` 用 `route.continue` 把 POST 转到同一 fixture（保分块）。
- 写出原始 JSON + 中位数/p95。更新审计报告。Gate 0 默认仍 NO-GO，除非 charter 全条满足。

### 1.2 不做

- 不改 `frontend_next`、后端、`tauri.conf.json`、部署脚本。
- 不默认跑 30 分钟压力（`GATE0_STRESS=1` 才跑）。
- 不因 debug 产物或缺 Next 数字宣布 GO。
- 不进入 Phase 1–6。

## 2. 验证门

1. charter 已落盘且不含测量数字。
2. `GATE0=1` Playwright 对 Rust 采满 5 冷 + 20 热（约 8–12 分钟；需已有 `target/debug/web-server`）。
3. Next：`GATE0_NEXT_BASE` 可访问则对照；否则记 Blocked，不编造。
4. 更新 `2026-09-03-phase-0-benchmark-report.md`；本地提交不 push。

## 3. 红线

- 看到数字后不得改 charter。
- `frontend_next` 只读。
- 只提交本任务文件。

## 4. 验证报告（2026-09-04）

### 4.1 做了什么

- 冻结 charter（核心 = first token / complete；LCP/体积/堆/掉帧是护栏）。见数后未改口径。
- Playwright Gate 0 采集器：`gate0-harness.ts` + `gate0-perf.spec.ts` + `playwright.gate0.config.ts`。
- 计时改页内 `performance.now()`；空画布（`chat-empty` + 无 `live-answer`）才发送；非增量样本丢弃重试。
- Rust debug：5 冷 + 20 热采满，discarded = 0。数字写入 [`2026-09-03-phase-0-benchmark-report.md`](2026-09-03-phase-0-benchmark-report.md) §2.2。
- Next：本轮 unset。同日 stub `127.0.0.1:8080` 未见到 `workspace-chat-composer`，无 Next 数字。
- 30 分钟采集器未实现（`GATE0_STRESS=1` 仍 fail）。
- 未改 `frontend_next`、后端、`tauri.conf.json`、部署脚本。

### 4.2 命令

| # | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 1 | `GATE0=1` Playwright `--config playwright.gate0.config.ts`（无 Next） | 0 | 1 passed / 1 skipped；约 46s |
| 2 | 核心 p95 | — | first token 235.5ms；complete 1545.9ms（仅 Rust） |

### 4.3 判定

**NO-GO**。缺 Next、缺 30 分钟、debug 非 release-like、堆指标被 Chromium 分桶。
