# Gate 0 benchmark charter（冻结口径；闸门已改观察）

| 字段 | 内容 |
|---|---|
| 冻结日期 | 2026-09-04 |
| 状态 | **口径冻结**；2026-09-04 18:56 护栏改观察；**2026-09-04 19:40 业主修订：Gate 0 全部强制性改为观察项，不再挡开发** |
| 权威来源 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) §3.2–§3.3（目标态已被 [ADR-0011](../adr/0011-rust-web-gpui-desktop.md) 修订） |
| 实施细则 | [`2026-09-03-frontend-rust-migration-implementation-plan.md`](2026-09-03-frontend-rust-migration-implementation-plan.md) |

数字怎么采、怎么比，仍按本文，见数后不得改测量定义。是否继续开发 **不再** 由本文否决。

## 1. 对照对象

| 侧 | 页面 | 流 |
|---|---|---|
| Rust PoC | `frontend_rust` `/chat`（Leptos SSR + hydrate） | 同一 `stream-long-3000` SSE 夹具 |
| Next 基线 | 现网 `frontend_next` `/chat`（只读，不改源码） | 浏览器把 `POST /api/v1/chat` `continue` 到同一夹具，保留分块 |

不得拿空白页对完整产品；也不得用内存 reducer µs 数字充当浏览器证据。

## 2. 环境（首次跑数前填写，之后只追加机器差异，不改规则）

| 项 | 冻结值 |
|---|---|
| 视口 | 1440×900 |
| 浏览器 | Playwright Chromium（与 `frontend_rust/tests/browser` 锁定的 1.53 同一套） |
| 网络 | localhost，**不**加 CPU/网络 throttle（夹具已在本机；throttle 会淹没实现差异） |
| 流夹具 | `tests/fixtures/stream-long-3000.chunks.json`，`CHUNK_DELAY_MS=15`，**不用** `/bytes/:n` 乱序切片 |
| Rust 产物 | 优先 `cargo leptos build --release`；debug 必须在报告标明 |
| Next 产物 | 记录 `next dev` / `next start` / 已占用端口上的进程 |
| 样本 | 每侧：冷启动 **5**、热路径流式 **20**。报告给中位数、p95、min/max、全部原始记录 |
| 30 分钟压力 | 单独开关 `GATE0_STRESS=1` |

## 3. 核心 p95（观察项；2026-09-04 19:40 起不再否决）

选**流式交互**，不选 LCP/体积当核心收益——后者会把「PoC 页更瘦」误当成迁移收益。

相对 Next ≥20% 只表示「有性能收益可讲」，**未达到不得写成已通过，也不得因此停开发**。

| id | 定义 | 起点 | 终点 |
|---|---|---|---|
| `stream_first_token_ms` | 输入到首绘 | 点击发送 | 助手可见文本长度 > 0（Rust：`#live-answer` 或 assistant `chat-message`；Next：assistant `chat-message`） |
| `stream_complete_ms` | 输入到流式收束 | 点击发送 | 夹具尾部 marker「退化成早已演练过的常规操作。」已出现，且 Rust 状态行「已完成」或 Next assistant `data-pending=false` |

已采（release-like）：first **14.6%**，complete **0.8%**。归档见报告 §2.4。

## 4. 护栏（观察项；2026-09-04 18:56 起不再否决）

| id | 定义 |
|---|---|
| `nav_lcp_ms` | 冷启动 `/chat` 的 Largest Contentful Paint（PerformanceObserver，buffered） |
| `encoded_transfer_bytes` | 该次导航 `navigation` + `resource` 的 `transferSize` 之和 |
| `heap_used_after_stream` | 一轮 3000 字夹具收束后的堆（报告同时记 `performance.memory` 与 CDP `Runtime.getHeapUsage`；斜率只用后者） |
| `long_frame_count` | 发送到收束期间 `rAF` 间隔 > 50ms 的次数（掉帧代理） |

相对 Next 回退 >10% **只归档，不否决**。

## 5. 稳定性（观察项）

30 分钟内每 30s 采一次 CDP `heap_used`。后 10 分钟斜率持续为正且无平台 → 记 `unbounded`，**不挡开发**。已采：`unbounded=false`（报告 §2.6）。

## 6. 观察结论，不是 Go / No-Go 闸

~~同时满足才进入 Phase 1，否则停迁移。~~ **已撤销（2026-09-04 19:40）**。

1. 核心 p95 ≥20%：观察。未达到 = 没有流式性能故事，不是停工令。
2. 护栏：观察。
3. 30 分钟堆：观察。已采，未判无界。
4. 协议/产品正确性仍按既有切片验收，不靠本 charter 开闸。
5. 无双协议或功能降级换票：仍是产品红线，与 Gate 0 数字无关。

开发按 [ADR-0011](../adr/0011-rust-web-gpui-desktop.md)：在线 Leptos，桌面全平台 GPUI。删除 `frontend_next`、改生产 `tauri.conf.json`、部署：仍须单独批准，不因本文自动执行。
