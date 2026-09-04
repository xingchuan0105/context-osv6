# Phase 0 可行性验证基准评估修正报告（Gate 0 审计重置）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-03（修正审计）；2026-09-04（真实性状态更新 + 首次同机采集） |
| 状态 | **审计未通过 / 结论保持 NO-GO**（已有 Rust debug 浏览器数字；无 Next 对照、无 30 分钟、非 release-like） |
| 关联设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) |
| 关联计划 | [`2026-09-03-frontend-rust-migration-implementation-plan.md`](2026-09-03-frontend-rust-migration-implementation-plan.md) |
| 修正结论 | **NO-GO（已有 Rust debug 浏览器 5+20 数字，仍无 Next 对照 / 30 分钟 / release-like，不得进入全量迁移）** |

---

## 1. 现状与审计纠偏

在初期执行中，我们采集了 `web-ui` 内存中 Reducer 和 JSON 反序列化的耗时（单轮 ~60 µs），但必须客观指出：
1. **测试对象偏差**：仅测了 Native Rust 纯内存操作，**没有包含真实浏览器环境下的 DOM 绘制、WASM 运行时、LCP/FCP 以及事件循环延迟**。
2. **测试夹具不足**：初始夹具为 4~9 行微小 JSON（约 1KB），**并非计划所要求的 3000+ 字复杂长文本与嵌套代码块的真实长会话**。
3. **缺少 Next.js 同条件客观对照**：报告中引用的 Next.js 耗时属于行业推断估计，**没有提供真实 Next.js 在同等软硬件基准下的录制跑数与原始审计数据**。
4. **缺少真实 Transport 支持**：`BrowserHttpTransport` 与 `TauriIpcTransport` 尚未打通底层 Fetch 与 Invoke，没有真实网络吞吐证据。

---

## 2. Gate 0 真实判定状态（全部归零重核）

根据设计文档 §3.3 的刚性要求重新审计：

- [ ] **协议正确性**：仅在简化内存 Mock 下通过；真实 Fetch/SSE 与 Tauri IPC 网络流尚未打通。
- [ ] **信道隔离验证**：纯 Reducer 逻辑隔离通过，但在实际 UI 渲染组件层尚未挂载验证。
- [ ] **平台编译可行性**：本轮 `web-sdk` + `web-ui` WASM check 与 `web-server` Native check 已通过；但真实 SSR、Hydrate、Tauri CSR 三个可运行产物仍未形成，因此平台 gate 仍未通过。Cargo 输出另有一条来自共享 `contracts` 的 `ts-rs` 属性解析 warning。
- [ ] **真实性能收益**：缺乏浏览器真实 LCP、绘制掉帧、长会话真实 Heap 采样证据。
- [ ] **生产就绪度**：当前仅为 PoC 骨架，绝不可作为全量上线依据。

---

## 2.1 真实性状态更新（2026-09-04 浏览器垂直切片任务后）

以下条目从"未验证"变为"已真实验证"，证据见
[`2026-09-04-frontend-rust-phase-0-browser-vertical-slice-task.md`](2026-09-04-frontend-rust-phase-0-browser-vertical-slice-task.md)
文末验证报告：

- **真实 Browser Fetch/SSE 已打通**：`BrowserHttpTransport` 经真实 Fetch/ReadableStream 消费 SSE，自动化浏览器测试证明非对齐网络分块（113 字节切片、UTF-8 跨包）、真实取消（服务端观测到连接中止）、401 与坏 JSON typed error、retry 新流隔离，全部穿过浏览器边界。
- **Leptos SSR/hydration `/chat` 与 `/chat/:sessionId` 已真实运行**：SSR 输出含表单语义，hydration 无重复 root、无 hydration 错误；dev（`cargo leptos build`）与 release-like（`cargo leptos build --release`，含 wasm-opt）产物均已实际构建、启动，SSR smoke 与浏览器旅程通过。
- **夹具已达标**：新增脱敏、确定性、3013 字长会话 SSE 分块夹具（97 个显式 chunk，含 CRLF/keepalive/EOF 无尾空行），decoder→reducer 与 fixture transport→reducer 终态一致。
- **信道隔离已在组件层验证**：浏览器旅程断言答案主气泡不含 activity/trace 文案，activity/reasoning/citations 各自独立区域，错误 `role="alert"`。

以下阻断项**不变**：

- Tauri IPC adapter 与 CSR 产物已在 `frontend_rust` 构建（未改生产 `tauri.conf.json`，无 WebView 点验）；
- 已有 Rust debug 同机 5+20（§2.2）；仍无 Next.js 对照、无 30 分钟、无 release-like；
- live backend smoke 已执行（2026-09-04）：真实 `/api/v1/chat` 一轮 + 401 对照经浏览器 Fetch 通过；未采集性能数字。配置端口 8080 当时被 Next HTML 占用，smoke 使用一次性 `127.0.0.1:18081`。证据见 [`2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md`](2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md) §6。

因此 **Gate 0 结论保持 NO-GO**：不得据此进入全量迁移或 Phase 1–6。Rust 侧数字见 §2.2；Next 对照未采到，禁止用推断值填空。

---

## 2.2 首次同机采集（2026-09-04）

冻结口径：[`2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md`](2026-09-04-frontend-rust-phase-0-gate0-benchmark-charter.md)（见数后未改指标/阈值）。
任务与采集器：[`2026-09-04-frontend-rust-phase-0-gate0-perf-task.md`](2026-09-04-frontend-rust-phase-0-gate0-perf-task.md)。

### 环境

| 项 | 实际值 |
|---|---|
| 视口 | 1440×900 |
| 浏览器 | Playwright Chromium 1.53（`frontend_rust/tests/browser`） |
| 网络 | localhost，无 CPU/网络 throttle |
| 夹具 | `stream-long-3000.chunks.json`，`CHUNK_DELAY_MS=15`，未用 `/bytes/:n` |
| Rust 产物 | **debug** `target/debug/web-server` + `target/site`（hydrate），`http://127.0.0.1:3200` |
| Next 产物 | 本轮未采（`GATE0_NEXT_BASE` unset）。同日一次 stub 尝试：`http://127.0.0.1:8080` 是 nginx→Next（`lang=zh-CN`），`workspace-chat-composer` 20s 内不可见，auth stub 不够，**无任何 Next 数字** |
| 计时 | 页内 `performance.now()`（点击与 MutationObserver/rAF 同文档）；增量间隔 <800ms 或首绘 >2000 字的样本丢弃重试。本轮 discarded = 0 |
| 样本 | Rust 冷 5 + 热 20。采集时刻 `2026-09-04T08:20:33Z` |
| 30 分钟 | 未跑（`GATE0_STRESS` 未开；采集器仍 `test.fail`） |

早期两次采集（`08:08` / `08:10`）用 Node `Date.now()` + Playwright locator，出现负 `first_token` 与 ~400ms 假 complete，**作废，不进入下表**。

### 核心（仅 Rust；无 Next，不能算 ≥20%）

| id | n | min | median | p95 | max |
|---|---|---|---|---|---|
| `stream_first_token_ms` | 25 | 219.0 | 223.5 | 235.5 | 236.4 |
| `stream_complete_ms` | 25 | 1509.1 | 1534.3 | 1545.9 | 1546.3 |

`complete − first` 约 1.3s，与 97×15ms 夹具一致。首绘可见文本长度均为 242 字（前几枚 token 同一帧上屏，不是整篇积压）。

### 护栏（仅 Rust；无 Next，不能判回退）

| id | n | min | median | p95 | max | 备注 |
|---|---|---|---|---|---|---|
| `nav_lcp_ms` | 5 | 84 | 88 | 100 | 100 | 冷启动 `/chat` |
| `encoded_transfer_bytes` | 5 | 6856790 | 6856790 | 6856790 | 6856790 | debug WASM，5 次相同 |
| `heap_used_after_stream` | 25 | 10000000 | 10000000 | 10000000 | 10000000 | Chromium `performance.memory` 按 10MB 分桶，**不能做斜率** |
| `long_frame_count` | 25 | 0 | 0 | 0 | 0 | 发送→收束 rAF 间隔 >50ms |

LCP / 体积优于完整 Next **不算**核心收益（charter §3）。debug 6.8MB 也不得当作 release 体积。

### 冷启动原始记录（LCP / 传输）

| # | `nav_lcp_ms` | `encoded_transfer_bytes` |
|---|---|---|
| 1 | 100 | 6856790 |
| 2 | 88 | 6856790 |
| 3 | 84 | 6856790 |
| 4 | 88 | 6856790 |
| 5 | 92 | 6856790 |

### 流式原始记录（5 冷 + 20 热）

| # | kind | `stream_first_token_ms` | `stream_complete_ms` | heap | long_frames | first_paint_chars |
|---|---|---|---|---|---|---|
| 1 | cold | 236.4 | 1539.1 | 10000000 | 0 | 242 |
| 2 | cold | 230.0 | 1532.8 | 10000000 | 0 | 242 |
| 3 | cold | 227.8 | 1541.6 | 10000000 | 0 | 242 |
| 4 | cold | 232.3 | 1545.6 | 10000000 | 0 | 242 |
| 5 | cold | 235.5 | 1545.8 | 10000000 | 0 | 242 |
| 6 | hot | 231.7 | 1537.8 | 10000000 | 0 | 242 |
| 7 | hot | 222.0 | 1534.7 | 10000000 | 0 | 242 |
| 8 | hot | 226.0 | 1539.7 | 10000000 | 0 | 242 |
| 9 | hot | 227.7 | 1545.9 | 10000000 | 0 | 242 |
| 10 | hot | 222.8 | 1537.0 | 10000000 | 0 | 242 |
| 11 | hot | 223.1 | 1528.8 | 10000000 | 0 | 242 |
| 12 | hot | 223.6 | 1533.4 | 10000000 | 0 | 242 |
| 13 | hot | 224.2 | 1539.0 | 10000000 | 0 | 242 |
| 14 | hot | 224.6 | 1546.3 | 10000000 | 0 | 242 |
| 15 | hot | 223.1 | 1540.1 | 10000000 | 0 | 242 |
| 16 | hot | 223.5 | 1534.1 | 10000000 | 0 | 242 |
| 17 | hot | 224.1 | 1534.3 | 10000000 | 0 | 242 |
| 18 | hot | 220.4 | 1521.3 | 10000000 | 0 | 242 |
| 19 | hot | 220.6 | 1523.3 | 10000000 | 0 | 242 |
| 20 | hot | 223.5 | 1528.0 | 10000000 | 0 | 242 |
| 21 | hot | 223.5 | 1521.4 | 10000000 | 0 | 242 |
| 22 | hot | 219.1 | 1533.0 | 10000000 | 0 | 242 |
| 23 | hot | 221.2 | 1509.1 | 10000000 | 0 | 242 |
| 24 | hot | 219.0 | 1510.7 | 10000000 | 0 | 242 |
| 25 | hot | 222.9 | 1521.3 | 10000000 | 0 | 242 |

### 本轮仍未满足的 GO 条件

1. 无 Next 同机 5+20，无法谈核心 p95 ≥20%。
2. 无护栏对照，无法谈回退 >10%。
3. 30 分钟堆斜率未跑；且现用 `usedJSHeapSize` 被分桶，即使跑了也要换测法。
4. Rust 为 debug，不是 charter 要求的 release-like。
5. 未改 charter、未部署、未进 Phase 1–6。

---

## 3. 最终审计结论

**Gate 0 结论保持：NO-GO（Rust debug 数字不足以下结论，不可进入全量迁移）**。
系统必须保持 `frontend_next` 稳定运行。下一棒只补 Next 对照、release-like 再采、以及可用的 30 分钟堆测法；严禁生产发布。
