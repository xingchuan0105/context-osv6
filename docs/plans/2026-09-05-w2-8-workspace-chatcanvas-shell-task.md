# W2.8 任务记录：Workspace 最小 Shell 内复用 ChatCanvas 与 Scope 增量叠加

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §5 (W2.8)
门禁目标：W2.8 切片完成

---

## 1. 任务目标与核心不变量

1. **唯一 ChatCanvas 复用**：
   - 在 `/dashboard/:workspace_id?` 挂载 Workspace 最小外壳，内部完全复用核心 `ChatCanvas` 与数据流，绝不重复发明一套独立的聊天组件。
2. **路由与 Query 参数恢复**：
   - 支持 `/dashboard/:workspace_id` 以及 `/dashboard/:workspace_id?session=:session_id` 历史恢复；
   - 快速切库时立即递增 `conversation_epoch` 并作废正在进行的旧流，绝无串流污染。
3. **全局最近会话区分归属**：
   - 会话侧栏明确标记工作区归属（展示 `workspace_name` 标签），与个人会话清晰区分。
4. **Scope 增量叠加 (`doc_scope`)**：
   - 提问请求携带 `workspace_id` 与可选 `doc_scope`，与个人 Session 文件及网络搜索（search）形成正交叠加。

---

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| W2.8.1 | 路由挂载：在 `App` 增加 `/dashboard/:workspace_id?` 路由与参数解析 | 已完成 | `app.rs`, `routes.rs` 单元测试通过 |
| W2.8.2 | 最小工作区外壳：展示当前工作区标识、返回个人对话快捷入口 | 已完成 | `chat_page.rs`，`workspace-banner` 呈现 |
| W2.8.3 | 会话列表归属标记：`session_label` 区分个人与工作区会话 | 已完成 | `[材料研发] 合金强度分析` 归属徽标呈现 |
| W2.8.4 | 切库防串流与生命周期测试：`chat_canvas_lifecycle_tests.rs` | 已完成 | `chat_canvas_lifecycle_tests.rs` 32/32 passed |
| W2.8.5 | Playwright 自动化：覆盖 `/dashboard/:ws?session=:sid` 恢复与归属隔离 | 已完成 | `chat-journey.spec.ts` 22/22 passed |
| W2.8.6 | 验证收敛、图谱更新与本地提交 | 已完成 | 图谱已更新 (14 files)，本地提交 `93533806` (W2.8 达成) |
