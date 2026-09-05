# E0 任务记录：W2.4–W2.5 会话文件与 Scope 能力收尾

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §3
证据目录：`docs/engineering/_reports/2026-09-05-e0-w2-files-scope/`

---

## 1. 任务目标与交付范围

完成 `frontend_rust` 中会话文件托盘（Session File Tray）与能力切换（Scope Bar）的未提交改动收尾、定向测试、浏览器端点验、真实文件链路 smoke 与本地提交，达成出口门禁 **G0**。

### 涉及模块与文件
- **SDK 模型与接口**：
  - `frontend_rust/crates/web-sdk/src/scope.rs` (新增)
  - `frontend_rust/crates/web-sdk/src/session_files.rs` (新增)
  - `frontend_rust/crates/web-sdk/src/browser_rest.rs` (扩展 upload/delete/poll/capabilities)
  - `frontend_rust/crates/web-sdk/src/lib.rs` (导出)
  - `frontend_rust/crates/web-sdk/tests/scope_tests.rs` (新增)
  - `frontend_rust/crates/web-sdk/tests/session_files_tests.rs` (新增)
- **UI 组件与状态**：
  - `frontend_rust/crates/web-ui/src/components/chat/scope_bar.rs` (新增)
  - `frontend_rust/crates/web-ui/src/components/chat/session_file_tray.rs` (新增)
  - `frontend_rust/crates/web-ui/src/components/chat/chat_page.rs` (集成托盘与 scope 栏)
  - `frontend_rust/crates/web-ui/src/components/chat/chat_canvas.rs` (能力与文件状态联动)
  - `frontend_rust/assets/style/chat-poc.css` (样式支持)
  - `frontend_rust/crates/web-ui/tests/chat_canvas_lifecycle_tests.rs` (单元/生命周期扩展)
- **E2E / Browser 测试**：
  - `frontend_rust/tests/browser/chat-journey.spec.ts` (上传/删除/能力联动 E2E 用例)
  - `frontend_rust/tests/browser/fixture-sse-server.mjs` (Mock SSE 及文档上传接口)

---

## 2. 核心不变量与检查矩阵

1. **不暗建 Workspace**：新会话首次上传，仅创建一个个人 Conversation，不自动产生 Workspace。
2. **防竞态与生命周期**：
   - DELETE 操作完成后迟到 GET 不复活文件。
   - 会话 A 的文件轮询响应不可串扰会话 B。
   - 处于 `pending` / `enqueueing` / `processing` 的文件应阻止发送按钮（`files_block_send`）。
   - 解析失败状态（`failed`）允许重试或删除。
3. **RAG 自动与手动规则（`reconcile_session_rag`）**：
   - 当就绪文件数从 0 变为 ≥1 时，若用户未手动干预，自动勾选“知识库”。
   - 当就绪文件数降为 0 时，自动移除自动附带的“知识库”。
   - 用户手动切换后，保持用户显式偏好。
   - 网络搜索（Search）独立于文件与知识库状态。
4. **残留与干净工作区**：
   - 确认工作区无无关脏文件污染，`rebuild-playwright.log` 按规范保持现状不计入测试证据。

---

## 3. 执行时序与状态记录

| 子步骤 | 内容 | 状态 | 证据 / 退出码 |
|---|---|---|---|
| E0.1 | 核对 diff 与未跟踪文件，建立证据目录 | 已完成 | 目录已建立，diff 与新增文件核对完成 |
| E0.2 | 核验新会话首次上传与已有会话上传链路模型 | 已完成 | 通过单元测试与 Playwright 旅程验证，URL 落地，无暗建库 |
| E0.3 | 核验删除、轮询、重试与导航防竞态逻辑 | 已完成 | 通过单元与 E2E 验证，墓碑过滤生效，迟到请求防污染 |
| E0.4 | 核验自动 RAG 与手动能力组合规则 | 已完成 | 单元测试与 E2E 验证 0->1 自动附带、1->0 自动撤销、手动独立 |
| E0.5 | 运行 Rust 模型测试、check、Playwright 浏览器旅程与真机链路 | 已完成 | 83/83 unit passed, check 0, 16/16 browser passed, 2/2 live passed |
| E0.6 | 图谱更新 (`code-review-graph update`) 与本地提交 (G0) | 已完成 | 图谱已更新 (25 files)，已提交 commit `cd821a74` (G0 达成) |

---

## 4. 验证记录摘要
- **HEAD 基线**：`09e06411`
- **Cargo Test (web-sdk / web-ui)**：10 套件，83 passed / 0 failed (exit 0)
- **Cargo Check (SSR & Hydrate)**：`cargo check -p web-server --features ssr` (exit 0), `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate` (exit 0)
- **Cargo Leptos Build**：生成 wasm32 与 ssr 二进制成功 (exit 0)
- **Playwright Test**：16 passed / 0 failed (16.5s)
- **Live Smoke**：2 passed / 0 failed (5.4s against avrag-api :18081)
- **Graph Update**：已完成 (25 files, 49 nodes, 480 edges)
- **Commit SHA**：`cd821a74`

