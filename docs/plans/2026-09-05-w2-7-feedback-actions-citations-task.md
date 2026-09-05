# W2.7 任务记录：Feedback 状态持久化、回答操作与引用补齐

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §5 (W2.7)
门禁目标：W2.7 切片完成

---

## 1. 任务目标与交付范围

1. **Feedback 真实持久化与错误反馈**：
   - SDK 增加 `submit_message_feedback` 接口（`POST /api/v1/chat/sessions/{session_id}/messages/{message_id}/feedback`）。
   - 助手气泡提供点赞 (up) / 点踩 (down) 操作；失败时在 UI 呈现错误提示（失败可感知，而不是静默吃掉）。
2. **回答操作（复制等）**：
   - 助手气泡操作栏提供复制按钮（`data-testid="copy-answer-button"`），点击复制 Markdown 内容并给出反馈。
3. **引用来源卡与原文定位增强**：
   - 来源卡支持已删除来源状态标记（`is_deleted`）；
   - 来源卡预览片段与点击高亮对应文本 chip；
   - 代码块与表格样式对照。
4. **测试与自动化**：
   - SDK 单元测试：feedback URL 拼接与序列化；
   - Playwright 自动化：点赞成功、点踩失败可感知、回答复制操作、来源卡已删除标记。

---

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| W2.7.1 | SDK 契约与 REST 扩展：`submit_message_feedback` 与引用模型增强 | 已完成 | `web-sdk/src/conversation_api.rs` / `browser_rest.rs`，测试通过 |
| W2.7.2 | UI 组件：消息操作栏（复制、赞、踩）与已删除来源卡标记 | 已完成 | `web-ui/src/components/chat/message_actions.rs` 集成 |
| W2.7.3 | 样式支持：操作栏按钮、复制反馈提示、已删除来源样式 | 已完成 | `chat-poc.css` 变量遵循 `--cos-*`，字重 400 |
| W2.7.4 | Playwright 浏览器用例与 Mock 服务端扩展 | 已完成 | `chat-journey.spec.ts` 21/21 passed |
| W2.7.5 | 验证收敛、图谱更新与本地提交 | 进行中 | 待执行提交 |
