# E3.1 任务记录：账号认证与设置路由族 (`/login`, `/register`, `/reset-password/*`, `/settings`)

日期：2026-09-05
负责人：Agent / Solo Trunk
关联计划：[2026-09-05-development-execution-plan.md](2026-09-05-development-execution-plan.md) §6 (E3.1)
门禁目标：E3.1 切片完成

---

## 1. 任务目标与交付范围

按全量路由迁移矩阵（`ROUTE_MIGRATION_MATRIX.md`）中的端点清单：
1. **账号认证路由族 (5 端点)**：
   - `/login`：支持邮箱/密码登录，记住凭据（同步写入 `avrag.auth.v1` localStorage 与 `avrag.auth.session` / `avrag.auth.persisted` Lax Cookie），支持登录后定向回跳（`redirect_url`）。
   - `/register`：支持注册新账号，条款同意（`terms_version` / `privacy_version`），注册后自动登录或引导。
   - `/reset-password`：申请重置密码验证码。
   - `/reset-password/verify`：核验验证码。
   - `/reset-password/confirm`：重设新密码。
   - 登出：一键清理本地 storage 与会话 cookie，重定向至 `/login`。
2. **设置与提供商路由族 (2 端点)**：
   - `/settings`：核心承载 `?tab=providers`（自有 Key 配置闭环），支持 `agent_llm` (DeepSeek), `parse_llm` (百炼), `quick_chat` (百炼 qwen3.8-flash), `siliconflow` (SiliconFlow embedding+rerank) 的密钥录入保存、掩码展示与吊销；严禁在日志中打印密钥明文。同时支持 profile / preferences tab。
   - `/settings/usage`：用量总览只读展示。
3. **安全与工程不变量**：
   - 敏感密钥（API Keys）仅通过受保护的 PUT 请求发送给 `/api/v1/settings/provider-secrets`，不入前端本地持久化，日志不打印明文。
   - 路由与 SSR 页面完整支持表单语义、ARIA 属性、焦点管理与无障碍。
   - 遵从设计系统样式（`--cos-*` token，字重 400，无裸十六进制颜色与未授权阴影）。

---

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E3.1.1 | SDK 认证与设置 REST 接口补齐 | 已完成 | `web-sdk/src/browser_auth.rs`, `browser_rest.rs` |
| E3.1.2 | 实现 Auth 页面族 (`/login`, `/register`, `/reset-password/*`) | 已完成 | `web-ui/src/components/auth/` |
| E3.1.3 | 实现 Settings 页面族 (`/settings?tab=providers`, `/settings/usage`) | 已完成 | `web-ui/src/components/settings/` |
| E3.1.4 | 挂载路由并补充单元与集成测试 | 已完成 | `app.rs`, `routes.rs` 单元测试通过 |
| E3.1.5 | Playwright 浏览器旅程全套件断言 | 已完成 | `auth-settings-journey.spec.ts` 5/5, `chat-journey.spec.ts` 23/23 |
| E3.1.6 | 验证收敛、图谱更新与本地提交 | 已完成 | 图谱已更新 (20 files)，本地提交 `b2e71dfc` (E3.1 达成) |
