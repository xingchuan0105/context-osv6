# Rust 前端 Phase 0：Gate 0 护栏改观察 + 整体对照 + 30 分钟堆

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-04 |
| 状态 | **Done（2026-09-04）**；30 分钟 CDP `unbounded=false`；Gate 0 仍 NO-GO |
| 本任务终点 | 业主放开护栏否决后写出整体对照；实现 `GATE0_STRESS=1`（CDP 堆）并采 30 分钟。不自动 GO |
| 业主决定 | 护栏改为观察项；核心 p95 ≥20% 与 30 分钟稳定性未改 |
| 冻结口径 | [`2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md`](2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md)（已记修订） |

## 1. 范围

- 修订 charter §4 / §6.2：护栏不否决。
- 报告写整体对照（流式、LCP、体积、掉帧），不把体积当否决。
- 30 分钟：每 30s 一轮夹具，CDP `Runtime.getHeapUsage`；后 10 分钟斜率 >50KB/min 且无平台 → unbounded。
- 不进 Phase 1。核心仍未达 ≥20%，GO 仍不成立。

## 2. 红线

- 不改核心阈值。不改 `frontend_next`。不部署。本地提交不 push。
