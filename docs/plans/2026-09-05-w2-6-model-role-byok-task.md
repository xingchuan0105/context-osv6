# W2.6 任务记录：model_role 与 Quick Chat BYOK 状态读取与切换

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §5 (W2.6)
门禁目标：W2.6 切片完成

---

## 1. 任务目标与交付范围

实现 `model_role`、当前模型标牌与 Quick Chat BYOK 状态读取：
1. **个人会话默认 `quick_chat`**：
   - 新建个人会话（`/chat`）默认使用 `model_role = "quick_chat"`。
   - 工作区会话使用 `model_role = "agent"`。
2. **BYOK 状态读取 (`/api/v1/settings/provider-secrets`)**：
   - 在 `web-sdk` 中增加 `ProviderSecretRow`、`ProviderSecretsResponse` 与 `list_provider_secrets` 接口。
   - 提供 `has_quick_chat_byok` 判定逻辑。
3. **UI 模型标牌与状态指示**：
   - 在 `ChatPage` 头部或 Composer 展示当前会话的模型角色（如 `对话 · qwen3.8-flash` 及 BYOK / 官方默认状态）。
   - 来源与错误遵循后端响应，前端不脑补计费来源。
4. **历史恢复一致性**：
   - 切换或恢复历史会话时，`model_role` 严格尊重服务端返回事实，不被前端当前状态重算或污染。
5. **测试与验证**：
   - 单元测试：`web-sdk` provider-secrets 序列化与 `has_quick_chat_byok` 判定；`web-ui` 会话生命周期与历史恢复 `model_role` 对照。
   - 浏览器端 Playwright：断言模型角色标牌展示、BYOK 状态识别、个人默认 `quick_chat`。

---

## 2. 执行计划与时序

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| W2.6.1 | SDK 契约与 REST 扩展：`ProviderSecretsResponse` 与 `list_provider_secrets` | 已完成 | `web-sdk/src/providers.rs`，测试 5/5 通过 |
| W2.6.2 | UI 模型标牌与模型角色呈现：`ModelRoleBadge` | 已完成 | `web-ui/src/components/chat/model_badge.rs` 与 CSS 集成 |
| W2.6.3 | 历史恢复与切换一致性测试 | 已完成 | `chat_canvas_lifecycle_tests.rs` 31/31 passed |
| W2.6.4 | Playwright 浏览器旅程与 Mock SSE 扩展 | 已完成 | `chat-journey.spec.ts` 19/19 passed |
| W2.6.5 | 验证收敛、图谱更新与本地提交 | 进行中 | 待执行提交 |
