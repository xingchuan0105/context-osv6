# W2.9 任务记录：会话/文件归属动作收尾与 Chat-first Gate 2 (G2) 达成

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §5 (W2.9 & G2)
门禁目标：Gate 2 (G2) 达成

---

## 1. 任务目标与 G2 十二条不变量核验矩阵

严格对照权威设计 §7 的十二条产品不变量，逐项映射验证证据：

| # | 不变量 (Invariants) | 验收归属与验证手段 | 状态 | 证据文件 |
|---|---|---|---|---|
| 1 | 普通会话不暗建库；文件随会话保留 | E0: 首次上传与发问仅创建个人会话，不调建库接口 | 已通过 | `chat-journey.spec.ts` (W2.4) |
| 2 | Workspace 共用聊天 (同一个 ChatCanvas) | W2.8: `/dashboard/:workspace_id?` 复用同一 `ChatCanvas` | 已通过 | `chat-journey.spec.ts` (W2.8) |
| 3 | 显式带入 (普通会话不暗载工作区) | W2.8: 个人会话 `workspace_id` 为空，工作区会话显式携带 | 已通过 | `chat_canvas_lifecycle_tests.rs` |
| 4 | 两类文件生命周期 (会话文件独立) | E0 / W2.4: Session 文件托盘仅绑定会话，删除不影响持久库 | 已通过 | `session_files_tests.rs` |
| 5 | 最近会话区分归属 | W2.8: 会话列表工作区条目前置展示 `[工作区名]` 徽标 | 已通过 | `chat-journey.spec.ts` (W2.8) |
| 6 | 模型角色独立 (`quick_chat` vs `agent`) | W2.6: 个人默认 `quick_chat`，工作区默认 `agent`，标牌清晰 | 已通过 | `chat-journey.spec.ts` (W2.6) |
| 7 | Quick Chat BYOK 独立 | W2.6: 读取 `quick_chat` 独立 key，区分自定义与官方默认 | 已通过 | `providers_tests.rs` |
| 8 | Web 独立 (网络搜索正交解耦) | W2.5: 网络搜索是独立能力 chip，不依赖知识库 | 已通过 | `chat-journey.spec.ts` (W2.5) |
| 9 | Scope 增量叠加 (`doc_scope`) | W2.8: 工作区 ID、Session 文件与搜索按本轮组合 | 已通过 | `chat_canvas_lifecycle_tests.rs` |
| 10 | 完整聊天交互 (停止/重试/复制/反馈) | W2.7: 进度推理折叠、复制回答、Feedback 持久化与错误提示 | 已通过 | `chat-journey.spec.ts` (W2.7) |
| 11 | 受限上下文测试 (防越权上下文外泄) | W2.9: 共享 ChatCanvas 只消费当前会话 messages 与 sources | 已通过 | `chat-journey.spec.ts` (用例 18) |
| 12 | Snapshot/Evidence 叠加不破坏会话 | W2.9: 历史恢复与 stream scope 隔离，断流与刷新不改写既有事实 | 已通过 | `chat_canvas_lifecycle_tests.rs` (用例 15) |

---

## 2. 执行步骤与结果

1. **W2.9.1 补充不变量 11 & 12 的端到端与生命周期断言**：已完成
2. **W2.9.2 运行全量 Rust 单元/集成测试与 SSR/WASM check**：
   - `cargo test -p web-sdk -p web-ui`: 11 个套件，91 passed / 0 failed (exit 0)
   - `cargo check -p web-server --features ssr`: exit 0
   - `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate`: exit 0
3. **W2.9.3 运行 Playwright 浏览器旅程全套件与 Live Smoke**：
   - `chat-journey.spec.ts`: 23 passed / 0 failed (18.1s)
   - `playwright.live.config.ts`: 2 passed / 0 failed (7.5s against avrag-api :18081)
4. **W2.9.4 更新知识图谱 (`code-review-graph update`)，生成 G2 报告，本地提交**：进行中

