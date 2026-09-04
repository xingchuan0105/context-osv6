# Gate 0 benchmark charter（冻结）

| 字段 | 内容 |
|---|---|
| 冻结日期 | 2026-09-04 |
| 状态 | **已冻结**。本文件在看到任何性能数字之后不得改口径、不得改核心指标或阈值 |
| 权威来源 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) §3.2–§3.3 |
| 实施细则 | [`2026-09-03-frontend-rust-migration-implementation-plan.md`](2026-09-03-frontend-rust-migration-implementation-plan.md) Step 0.4–0.5 |

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
| Rust 产物 | 优先 `cargo leptos build --release`；若本轮只用 debug，报告必须标明，且不得据此 GO |
| Next 产物 | 记录 `next dev` / `next start` / 已占用端口上的进程；dev 不得据此 GO |
| 样本 | 每侧：冷启动 **5**、热路径流式 **20**。报告给中位数、p95、min/max、全部原始记录 |
| 30 分钟压力 | 单独开关 `GATE0_STRESS=1`；未跑则 Gate 0 稳定性项保持未满足 |

## 3. 核心 p95（至少两项相对 Next 改善 ≥ 20% 才谈性能收益）

选**流式交互**，不选 LCP/体积当核心收益——后者会把「PoC 页更瘦」误当成迁移收益。

| id | 定义 | 起点 | 终点 |
|---|---|---|---|
| `stream_first_token_ms` | 输入到首绘 | 点击发送 | 助手可见文本长度 > 0（Rust：`#live-answer` 或 assistant `chat-message`；Next：assistant `chat-message`） |
| `stream_complete_ms` | 输入到流式收束 | 点击发送 | 夹具尾部 marker「退化成早已演练过的常规操作。」已出现，且 Rust 状态行「已完成」或 Next assistant `data-pending=false` |

## 4. 护栏（任一项相对 Next 回退 > 10% 即失败）

| id | 定义 |
|---|---|
| `nav_lcp_ms` | 冷启动 `/chat` 的 Largest Contentful Paint（PerformanceObserver，buffered） |
| `encoded_transfer_bytes` | 该次导航 `navigation` + `resource` 的 `transferSize` 之和 |
| `heap_used_after_stream` | 一轮 3000 字夹具收束后 `performance.memory.usedJSHeapSize`（Chromium） |
| `long_frame_count` | 发送到收束期间 `rAF` 间隔 > 50ms 的次数（掉帧代理） |

Rust 体积/LCP 优于完整 Next **不算**核心收益，只作护栏：Rust 更差才否决。

## 5. 稳定性

30 分钟内每 30s 采一次 `heap_used`。失败：后 10 分钟斜率持续为正且无明显平台。未跑本项则不得 GO。

## 6. Go / No-Go（与设计 §3.3 相同，此处只引用不改写）

同时满足才进入 Phase 1，否则停迁移、转优化 Next：

1. 上表两项核心 p95 相对 Next ≥ 20%，且是 5+20 样本而非单次最佳值。
2. 护栏无 >10% 回退。
3. 30 分钟无持续无界堆增长。
4. 协议/产品正确性与三目标 release-like 产物已在既有切片满足（本 charter 不重开）。
5. 无双协议或功能降级换票。

缺 Next 对照、缺 30 分钟、或 Rust 非 release-like：**保持 NO-GO**，只归档数字。
