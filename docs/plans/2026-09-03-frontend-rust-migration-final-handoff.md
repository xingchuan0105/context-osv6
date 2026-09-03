# 前端全栈 Rust 迁移 PoC 现状审计与整改报告

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-03 |
| 状态 | **Phase 0 PoC 骨架；本轮静态修正与代码级验证通过；Gate 0 仍为 NO-GO** |
| 关联权威设计 | [`2026-09-03-frontend-rust-migration-design.md`](2026-09-03-frontend-rust-migration-design.md) |
| 实施编排计划 | [`2026-09-03-frontend-rust-migration-implementation-plan.md`](2026-09-03-frontend-rust-migration-implementation-plan.md) |
| 性能实测报告 | [`2026-09-03-phase-0-benchmark-report.md`](2026-09-03-phase-0-benchmark-report.md)（Gate 0 NO-GO） |

## 1. 不变红线

1. 当前工程不是“已完成迁移”，只是一组 Phase 0 数据流、状态机和测试 seam。
2. 严禁生产部署，严禁删除 `frontend_next`；现行 Next.js 仍是唯一生产前端。
3. 已删除的发布脚本与 systemd 服务不得恢复。

## 2. 本轮修正后的代码事实

### 2.1 流身份与终态

- 每次 `prepare_user_turn` 生成本地 `StreamScope`，同时取消并失效上一条活动流。
- Transport 消费者必须把该 scope 随每个事件传回 `ChatCanvasModel::on_event`；scope 与 Conversation epoch 在进入 reducer **之前**校验。
- 服务端 `Start.request_id` 只负责已接受流内部的协议关联，旧流迟到的 `Start` 无法占用新 turn。
- 新建或切换 Conversation 通过 `ChatCanvasModel` 完成：先取消旧流、清空共享 `live_turn`，再切换会话。
- `Error` 可以作为唯一 SSE 事件进入终态；进入 `Done`、`Error` 或 `Cancelled` 后，活动 scope 与取消句柄立即清理。
- `cancel` 只允许把 `Streaming` 改为 `Cancelled`，不会覆盖已经到达的 `Done` 或 `Error`。

### 2.2 Done 权威收束

- `Done.payload` 按共享契约 `contracts::chat::ChatResponse` 反序列化；格式不合法时进入显式 `invalid_done_payload` 错误，禁止静默展示半成品。
- 最终 `answer` 即使为空也会覆盖 token 草稿；`answer` 为空时从 `answer_blocks` 的文本块生成主气泡正文。
- 结构化 `answer_blocks` 写入最终 Conversation message，不再在收束时丢失。
- Done 引用为空时保留早先 SSE 引用；Done 引用非空时以其结构为准，并从同一 `chunk_id` / `citation_id` 的流式引用补回被服务端裁剪的 `content` 与 `preview`。

### 2.3 路由与 Transport 实情

- `AppRoute` 只把 `/chat` 与 `/chat/:sessionId` 识别为 Chat；根路径 `/` 不再成为第二条聊天完成路径。
- Axum PoC 壳同时承接 `/chat` 与 `/chat/{session_id}`；`/healthz` 保持独立。
- `CancellationToken` 当前只支撑本地句柄传播与 fixture/mock 的协作式取消。
- Browser Fetch/SSE 和真实 Tauri IPC 尚未实现；两个 adapter 在无 fixture 时明确返回 `TransportError::Unavailable`，不再以空流伪装成功，也不宣称已具备真实 I/O abort。

### 2.4 回归用例源码

本轮新增或加强了以下边界用例：旧流 `Start` 抢占、新 turn 自动取消旧 token、切换 Conversation 前置隔离、Error-only、Error 后 cancel、重复 Done、空最终答案、`answer_blocks` 回退、引用正文保留、session_id 跨轮传递与 canonical route。

## 3. 本轮验证证据

- `cargo test --manifest-path frontend_rust/Cargo.toml`：25 个单元/集成测试通过，0 失败。
- `cargo check --manifest-path frontend_rust/Cargo.toml -p web-sdk -p web-ui --target wasm32-unknown-unknown`：通过。
- `cargo check --manifest-path frontend_rust/Cargo.toml -p web-server`：通过。
- 三次 Cargo 运行均出现同一条共享 `contracts` 的 `ts-rs` warning：无法解析 `#[serde(default, skip_serializing_if = "Vec::is_empty")]`；本轮未越界修改共享契约，不能宣称“零 warning”。
- `code-review-graph` clean rebuild：全图 2356 files / 18331 nodes / 197591 edges；直接 `frontend_rust` 子树 19 个源码文件 / 118 个节点；已删除的 `static_assets.rs` 节点计数为 0。
- `git diff --cached --check`：通过。

## 4. Gate 0 仍为 NO-GO

以下阻断项仍未完成：

- 可运行的 Leptos SSR/hydration/CSR UI；
- 真实 Browser Fetch/SSE 与 Tauri IPC adapter，包括真实取消；
- 3000+ 字复杂会话与真实异常网络夹具；
- Headless 浏览器 LCP、绘制延迟、掉帧、Heap 与无障碍数据；
- 与优化后 Next.js 在同机、同浏览器、同网络条件下的对照记录。

因此当前代码不得部署生产，也不得据此启动 Phase 1–6。
