# Rust 前端 Phase 0：Gate 0 release-like 再采

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；Gate 0 仍 **NO-GO** |
| 本任务终点 | `cargo leptos build --release` 后对同一 SSE 夹具采 Rust 5 冷 + 20 热（能复跑则顺带 Next）。不改 charter，不自动 GO |
| 冻结口径 | [`2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md`](2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md) |
| 上游交接 | [`2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md`](2026-09-04-frontend-rust-phase-0-browser-slice-handoff.md) |

## 1. 范围

### 1.1 做

- `PATH` 先 `wasm-bindgen` 0.2.127，再 `cargo leptos build --release`。
- 采集器改读 `GATE0_RUST_PROFILE=release` → `target/release/web-server` + 当前 `target/site`；`reuseExistingServer=false`，避免误连 leftover debug。
- 采满 Rust 5+20；Next 若仍在 `:3000` 则同跑。写入审计报告 §2.4。
- Gate 0 默认仍 NO-GO（本切片不跑 30 分钟）。

### 1.2 不做

- 不改 `frontend_next`、后端、`tauri.conf.json`、部署脚本、`.env`、冻结 charter。
- 不宣布 GO。不进入 Phase 1–6。
- 不实现 `GATE0_STRESS=1`（下一棒）。

## 2. 验证门

1. `target/release/web-server` 存在且 `/healthz` 200。
2. `GATE0=1 GATE0_RUST_PROFILE=release …` 采满 Rust 5+20，或记 Blocked。
3. 更新 `2026-09-03-phase-0-benchmark-report.md`；本地提交不 push。

## 3. 红线

- 看到数字后不得改 charter。
- debug 数字不得当作 GO。
- `frontend_next` 只读。
- 只提交本任务文件。

## 4. 验证报告（2026-09-04）

### 4.1 做了什么

- `cargo leptos build --release`（wasm-bindgen 0.2.127 在 PATH 前；约 45s 增量）。
- 采集器支持 `GATE0_RUST_PROFILE=release`，web-server 不 reuse leftover debug。
- Rust release + Next standalone 各 5+20，discarded = 0。数字写入报告 §2.4。
- 未改 `frontend_next`、后端、charter。未宣布 GO。未跑 30 分钟。

### 4.2 命令

| # | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 1 | `cargo leptos build --release` | 0 | `target/release/web-server` 18MB；`web_ui.wasm` 1.4MB |
| 2 | `GATE0=1 GATE0_RUST_PROFILE=release GATE0_NEXT_BASE=… GATE0_API_BASE=…` | 0 | 1 passed / 1 skipped；约 1.6min |
| 3 | 核心 p95 | — | first：229.0 / 268.0（14.6%）；complete：1527.8 / 1540.7（0.8%） |

### 4.3 判定

**NO-GO**。核心未达 ≥20%；体积 1.46MB vs 0.56MB 护栏失败；无 30 分钟。
