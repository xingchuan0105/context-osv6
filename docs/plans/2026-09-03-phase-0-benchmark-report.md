# Phase 0 可行性验证基准评估修正报告（Gate 0 审计重置）

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-03（修正审计）；2026-09-04（真实性状态更新） |
| 状态 | **审计未通过 / 结论重置（INCONCLUSIVE / NO-GO）** |
| 关联设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) |
| 关联计划 | [`2026-09-03-frontend-rust-migration-implementation-plan.md`](2026-09-03-frontend-rust-migration-implementation-plan.md) |
| 修正结论 | **NO-GO（浏览器垂直切片已真实跑通，但仍无同机性能对照证据，不得据此进入全量迁移）** |

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
- 仍无 Next.js 同机、同浏览器、同网络条件的 LCP/Heap/掉帧/30 分钟压力对照数据；
- live backend smoke 已执行（2026-09-04）：真实 `/api/v1/chat` 一轮 + 401 对照经浏览器 Fetch 通过；未采集性能数字。配置端口 8080 当时被 Next HTML 占用，smoke 使用一次性 `127.0.0.1:18081`。证据见 [`2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md`](2026-09-04-frontend-rust-phase-0-live-backend-smoke-task.md) §6。

因此 **Gate 0 结论保持 NO-GO**：不得据此进入全量迁移或 Phase 1–6，本报告不含任何推测性能数字。

---

## 3. 最终审计结论

**Gate 0 结论修正为：NO-GO（证据不充分，不可进入全量迁移）**。
系统必须保持 `frontend_next` 稳定运行，后续工作严格限制在 PoC 范围内的深度测量与真实 Transport 补齐，严禁生产发布。
