# Phase 0 可行性验证基准评估修正报告（Gate 0 审计重置）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-03（修正审计）；2026-09-04（真实性状态更新 + Rust 第一采集 + Next 同机对照） |
| 状态 | **审计未通过 / 结论保持 NO-GO**（已有 Rust debug 与 Next standalone 5+20；核心 p95 未达 ≥20%；无 30 分钟；非 release-like） |
| 关联设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) |
| 关联计划 | [`2026-09-03-frontend-rust-migration-implementation-plan.md`](2026-09-03-frontend-rust-migration-implementation-plan.md) |
| 修正结论 | **NO-GO（Next 对照已采；debug Rust 核心改善不足 20%，体积护栏失败，不得进入全量迁移）** |

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
- 已有 Rust debug 同机 5+20（§2.2）与 Next standalone 对照（§2.3）；仍无 30 分钟、无 release-like；核心 p95 未达 ≥20%；
- live backend smoke 已执行（2026-09-04）：真实 `/api/v1/chat` 一轮 + 401 对照经浏览器 Fetch 通过；未采集性能数字。配置端口 8080 当时被 Next HTML 占用，smoke 使用一次性 `127.0.0.1:18081`。证据见 [`2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md`](2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md) §6。

因此 **Gate 0 结论保持 NO-GO**：不得据此进入全量迁移或 Phase 1–6。Rust 第一采集见 §2.2；同机 Next 对照见 §2.3。

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

## 2.3 同机 Next 对照（2026-09-04）

任务：[`2026-09-04-frontend-rust-phase-0-gate0-next-contrast-task.md`](2026-09-04-frontend-rust-phase-0-gate0-next-contrast-task.md)。
见数后未改 charter。采集时刻 `2026-09-04T09:15:44Z`。两侧 discarded = 0。

### 环境

| 项 | 实际值 |
|---|---|
| 视口 / 浏览器 / 网络 / 夹具 | 与 §2.2 相同；夹具服务在最后一块之后补 `\n\n`（见下） |
| Rust 产物 | **debug** `target/debug/web-server` + `target/site`，`http://127.0.0.1:3200` |
| Next 产物 | **standalone** `node frontend_next/.next/standalone/server.js`，`HOSTNAME=127.0.0.1 PORT=3000`。不是 `next dev`，也不是 `:8080` |
| `:8080` | **Plane**（`application-name=Plane`），不是 Next，也不是 `avrag-api` |
| API | 已有 `avrag-api`，`AVRAG_API_ADDR=127.0.0.1:18081`（只改监听，未改 `.env`） |
| Next 流式路由 | `/chat/sess-900`（夹具 `session_id`）。`/chat` 首发会 `replace` 到 `/chat/:id` 并 abort SSE |
| POST 夹具 | 页内 `fetch` 补丁把 `POST /api/v1/chat` 发到 `http://127.0.0.1:3201`。`route.continue` 改端口会被 Chromium `ERR_BLOCKED_BY_CLIENT` |
| 计时 | 页内 `performance.now()` |

### 核心 p95（可算比值；debug 不得 GO）

| id | Rust n / median / p95 | Next n / median / p95 | Rust 相对 Next |
|---|---|---|---|
| `stream_first_token_ms` | 25 / 223.3 / **234.8** | 25 / 250.8 / **262.7** | 快 **10.6%**（未达 ≥20%） |
| `stream_complete_ms` | 25 / 1507.1 / **1531.6** | 25 / 1518.2 / **1538.6** | 快 **0.5%**（未达 ≥20%） |

`complete − first` 两侧都约 1.3s，与 97×15ms 夹具一致。Next 首绘 8 字（打字机一跳）；Rust 首绘 242 字（第一枚 token 整块上屏）。

### 护栏

| id | Rust p95 | Next p95 | 判定 |
|---|---|---|---|
| `nav_lcp_ms` | 116 | 316 | Rust 未回退 |
| `encoded_transfer_bytes` | 6_856_790 | 563_891 | **Rust debug WASM 比 Next standalone 大约 12×**，护栏失败（预期；不得 GO） |
| `heap_used_after_stream` | 10_000_000（分桶） | 18_200_000 | Next 有 15.2–19.3MB 档；Rust 仍锁在 10MB 桶，不能比斜率 |
| `long_frame_count` | 0 | 0 | 未回退 |

### Rust 冷启动（本轮）

| # | `nav_lcp_ms` | `encoded_transfer_bytes` |
|---|---|---|
| 1 | 96 | 6856790 |
| 2 | 84 | 6856790 |
| 3 | 108 | 6856790 |
| 4 | 96 | 6856790 |
| 5 | 116 | 6856790 |

### Next 冷启动 `/chat`

| # | `nav_lcp_ms` | `encoded_transfer_bytes` |
|---|---|---|
| 1 | 288 | 563891 |
| 2 | 316 | 563890 |
| 3 | 260 | 563891 |
| 4 | 248 | 563890 |
| 5 | 312 | 563890 |

### Rust 流式（本轮 5 冷 + 20 热）

| # | kind | first | complete | heap | long | chars |
|---|---|---|---|---|---|---|
| 1 | cold | 239.6 | 1536.8 | 10000000 | 0 | 242 |
| 2 | cold | 233.9 | 1531.6 | 10000000 | 0 | 242 |
| 3 | cold | 227.7 | 1510.7 | 10000000 | 0 | 242 |
| 4 | cold | 234.8 | 1507.1 | 10000000 | 0 | 242 |
| 5 | cold | 228.2 | 1512.3 | 10000000 | 0 | 242 |
| 6 | hot | 233.0 | 1514.9 | 10000000 | 0 | 242 |
| 7 | hot | 221.2 | 1497.4 | 10000000 | 0 | 242 |
| 8 | hot | 219.9 | 1489.6 | 10000000 | 0 | 242 |
| 9 | hot | 223.3 | 1502.6 | 10000000 | 0 | 242 |
| 10 | hot | 218.8 | 1499.4 | 10000000 | 0 | 242 |
| 11 | hot | 220.9 | 1502.7 | 10000000 | 0 | 242 |
| 12 | hot | 220.4 | 1500.3 | 10000000 | 0 | 242 |
| 13 | hot | 223.6 | 1512.5 | 10000000 | 0 | 242 |
| 14 | hot | 219.4 | 1494.5 | 10000000 | 0 | 242 |
| 15 | hot | 220.3 | 1503.5 | 10000000 | 0 | 242 |
| 16 | hot | 220.8 | 1498.8 | 10000000 | 0 | 242 |
| 17 | hot | 221.0 | 1514.3 | 10000000 | 0 | 242 |
| 18 | hot | 221.9 | 1514.0 | 10000000 | 0 | 242 |
| 19 | hot | 224.1 | 1507.1 | 10000000 | 0 | 242 |
| 20 | hot | 223.7 | 1499.0 | 10000000 | 0 | 242 |
| 21 | hot | 220.6 | 1493.3 | 10000000 | 0 | 242 |
| 22 | hot | 223.3 | 1523.8 | 10000000 | 0 | 242 |
| 23 | hot | 224.5 | 1515.7 | 10000000 | 0 | 242 |
| 24 | hot | 224.0 | 1526.0 | 10000000 | 0 | 242 |
| 25 | hot | 222.0 | 1517.1 | 10000000 | 0 | 242 |

### Next 流式（5 冷 + 20 热，`/chat/sess-900`）

| # | kind | first | complete | heap | long | chars |
|---|---|---|---|---|---|---|
| 1 | cold | 261.3 | 1537.9 | 19300000 | 0 | 8 |
| 2 | cold | 259.7 | 1544.0 | 16100000 | 0 | 8 |
| 3 | cold | 261.2 | 1529.9 | 18200000 | 0 | 8 |
| 4 | cold | 261.5 | 1535.3 | 18200000 | 0 | 8 |
| 5 | cold | 262.9 | 1527.9 | 16100000 | 0 | 8 |
| 6 | hot | 262.7 | 1538.6 | 15200000 | 0 | 8 |
| 7 | hot | 252.9 | 1512.4 | 15200000 | 0 | 8 |
| 8 | hot | 247.9 | 1498.3 | 15200000 | 0 | 8 |
| 9 | hot | 245.7 | 1518.2 | 15200000 | 0 | 8 |
| 10 | hot | 248.0 | 1509.9 | 15200000 | 0 | 8 |
| 11 | hot | 244.6 | 1510.6 | 15200000 | 0 | 8 |
| 12 | hot | 249.5 | 1525.4 | 15200000 | 0 | 8 |
| 13 | hot | 250.7 | 1517.2 | 15200000 | 0 | 8 |
| 14 | hot | 251.5 | 1533.7 | 15200000 | 0 | 8 |
| 15 | hot | 251.0 | 1525.9 | 15200000 | 0 | 8 |
| 16 | hot | 250.2 | 1505.8 | 15200000 | 0 | 8 |
| 17 | hot | 244.7 | 1501.4 | 15200000 | 0 | 8 |
| 18 | hot | 252.3 | 1519.7 | 15200000 | 0 | 8 |
| 19 | hot | 251.4 | 1513.2 | 15200000 | 0 | 8 |
| 20 | hot | 248.8 | 1536.0 | 15200000 | 0 | 8 |
| 21 | hot | 251.6 | 1527.1 | 15200000 | 0 | 8 |
| 22 | hot | 244.1 | 1514.7 | 15200000 | 0 | 8 |
| 23 | hot | 244.7 | 1503.9 | 15200000 | 0 | 8 |
| 24 | hot | 250.8 | 1515.2 | 15200000 | 0 | 8 |
| 25 | hot | 245.2 | 1504.4 | 15200000 | 0 | 8 |

### 采集器必须记住的事实

1. **`:8080` 是 Plane**，不能当 `GATE0_NEXT_BASE`。
2. Next standalone 的 rewrite 仍默认把 `/api` 指到 `:8080`；采集器必须自己登录 `:18081` 并代理非 chat 的 `/api/*`。
3. **`route.continue` 改到另一端口会被拦截**；页内 `fetch` 补丁才能打到夹具。
4. **`/chat` 首发会因 `replace` 中止 SSE**；流式必须在已落地的 `/chat/sess-900` 上采。
5. 夹具末事件故意无结尾空行。Next 的 SSE 解析不会把无换行残留当 `data`，`done` 不落地、`data-pending` 一直为 true。夹具服务在最后一块后补 `\n\n`（chunk 内容未改）。
6. 关页前必须 `unrouteAll({ behavior: "ignoreErrors" })`，否则代理 `page.request.fetch` 会在 context 已关时把测试打挂。

### 本轮仍未满足的 GO 条件

1. 核心两项相对 Next 均未 ≥20%（10.6% / 0.5%）。
2. debug WASM 体积护栏失败。
3. 30 分钟堆斜率未跑；`usedJSHeapSize` 仍不可用。
4. Rust 为 debug，不是 release-like。
5. 未改 charter、未部署、未进 Phase 1–6。

---

## 3. 最终审计结论

**Gate 0 结论保持：NO-GO（已有同机对照，但 debug 数字不能支持迁移）**。
系统必须保持 `frontend_next` 稳定运行。下一棒是 `cargo leptos build --release` 再采，以及可用的 30 分钟堆测法；严禁生产发布。
