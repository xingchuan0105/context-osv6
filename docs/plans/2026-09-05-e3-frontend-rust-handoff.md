# Rust 前端与桌面宿主库交接文档 (E0–E3.4 交付与下一棒指引)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 状态 | **Active（现行有效交接文档）** |
| 当前 HEAD | `81de3587`（相对本地 origin 领先 60 个提交） |
| 权威计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) |
| 全量路由矩阵 | [`docs/design/ROUTE_MIGRATION_MATRIX.md`](../design/ROUTE_MIGRATION_MATRIX.md) (71 个真实端点) |
| 达成门禁 | **Gate 0 (G0)**、**Gate 1 (G1)**、**Gate 2 (G2)** 达成；**D0.1** 完成；**E3.1–E3.4** 完成 |
| 下一棒任务 | **E3.5**（`/admin/*` 管理后台运维 13 端点与应用内帮助 2 端点，冲刺 **Gate 3**） |

---

## 1. 进展与提交记录链

本次连续推进了 6 个大阶段、累计完成 9 次规范原子提交，各阶段对应任务文档与交付成果如下：

| 阶段 | 提交 SHA | 核心工作内容 | 对应记录文档 |
|---|---|---|---|
| **E0** | `cd821a74` | 会话文件托盘 (`SessionFileTray`)、能力标签 (`ScopeBar`)、多端口 `resolve_upload_url`、**G0 达成** | [`2026-09-05-e0-w2-files-scope-task.md`](2026-09-05-e0-w2-files-scope-task.md) |
| **E1** | `40614225` | 盘点全量 71 路由迁移矩阵、样式守卫扩展覆盖 `chat-poc.css`、静态产物 MIME/br/immutable 验证、Tiptap 方案明确、**G1 达成** | [`2026-09-05-e1-w1-acceptance-route-matrix-task.md`](2026-09-05-e1-w1-acceptance-route-matrix-task.md) |
| **W2.6** | `774c3b3a` | 模型角色标牌 (`ModelRoleBadge`)、`quick_chat` 个人默认角色与自备密钥 BYOK 识别 | [`2026-09-05-w2-6-model-role-byok-task.md`](2026-09-05-w2-6-model-role-byok-task.md) |
| **W2.7** | `b2ba620d` | 回答复制操作、Feedback 点赞/点踩真实持久化与错误警示、已删除来源卡标记与链接禁用 | [`2026-09-05-w2-7-feedback-actions-citations-task.md`](2026-09-05-w2-7-feedback-actions-citations-task.md) |
| **W2.8** | `93533806` | `/dashboard/:id?session=:sid` 路由、复用 ChatCanvas、工作区横幅、全局会话归属徽标、切库防串流 | [`2026-09-05-w2-8-workspace-chatcanvas-shell-task.md`](2026-09-05-w2-8-workspace-chatcanvas-shell-task.md) |
| **W2.9** | `5bd52c59` | 十二条 Chat-first 不变量逐一验收闭环、受限上下文隔离、跨轮次历史事实不可变、**G2 达成** | [`2026-09-05-w2-9-chat-first-gate2-task.md`](2026-09-05-w2-9-chat-first-gate2-task.md) |
| **D0.1** | `b3ce4ad7` | 抽离最小宿主库 `desktop/core`、复用 `web-sdk::SseDecoder` 根治多字节中文切块乱码、删除废弃许可门 | [`2026-09-05-d0-1-desktop-stream-parsing-task.md`](2026-09-05-d0-1-desktop-stream-parsing-task.md) |
| **E3.1** | `b2e71dfc` | 账号认证五页面 (`/login`, `/register`, `/reset-password/*`)、设置页与四大模型 BYOK 密钥录入与撤销闭环 | [`2026-09-05-e3-1-auth-settings-task.md`](2026-09-05-e3-1-auth-settings-task.md) |
| **E3.2** | `03bb02a6` | `/dashboard` 概览与弹窗建库、`/dashboard/:id` 工作台与右侧轨（持久资料+笔记）、分析与统计两端点 | [`2026-09-05-e3-2-dashboard-workspace-task.md`](2026-09-05-e3-2-dashboard-workspace-task.md) |
| **E3.3** | `65ab035a` | 工作区分享中心三端点、公开只读知识库问答 (`/shared/kb/:token`)、分享者公开主页与工作区邀请加入页面 | [`2026-09-05-e3-3-share-invite-task.md`](2026-09-05-e3-3-share-invite-task.md) |
| **E3.4** | `82076d9b` | 套餐定价对比 (`/pricing`)、钱包充值面板 (`#topup`)、拦截墙 (`/upgrade/paywall`)、成功回跳与桌面购买说明 | [`2026-09-05-e3-4-billing-pricing-task.md`](2026-09-05-e3-4-billing-pricing-task.md) |

---

## 2. 架构现状与已挂载路由清单

当前 `frontend_rust` 已挂载并实现 **24 个产品端点**，涵盖对话、工作区、分享、公开知识库、主页、邀请、认证、设置、定价与交易全链路：

```text
/                                   -> Redirect to /chat
/chat/:session_id?                 -> ChatPage (个人对话与流式画布)
/dashboard                          -> DashboardOverviewPage (工作区概览卡片与创建弹窗)
/dashboard/analytics                -> GlobalAnalyticsPage (全局分享访问统计)
/dashboard/:workspace_id            -> WorkspaceWorkbenchPage (工作区工作台，复用 ChatPage + 右轨资料与笔记)
/dashboard/:workspace_id/analyze    -> WorkspaceAnalyzePage (工作区切片健康度分析)
/dashboard/:workspace_id/share      -> WorkspaceSharePage (分享中心与公开链接生成)
/dashboard/:workspace_id/share/access-logs -> WorkspaceShareLogsPage (访问审计日志)
/dashboard/:workspace_id/share/analytics   -> WorkspaceShareAnalyticsPage (分享互动统计)
/shared/kb/:token                   -> SharedKbPage (公开只读知识库问答与受限画布)
/shared/u/:user_id                  -> SharedUserPage (分享者公开名片主页)
/invite/:workspace_id/:member_id    -> InvitePage (工作区受邀加入页面)
/login                              -> LoginPage (登录与 next 回跳)
/register                           -> RegisterPage (注册与条款勾选)
/reset-password                     -> ResetPasswordRequestPage (找回密码申请)
/reset-password/verify              -> ResetPasswordVerifyPage (验证码核验)
/reset-password/confirm             -> ResetPasswordConfirmPage (重设新密码)
/settings                           -> SettingsPage (设置主页与退出登录)
/settings?tab=providers             -> ProvidersPanel (四大模型 BYOK 密钥配置与撤销)
/settings/usage                     -> UsagePage (用量总览只读页)
/pricing                            -> PricingPage (套餐对比与钱包充值面板)
/upgrade/paywall                    -> PaywallPage (配额与高级会员拦截墙说明)
/upgrade/success                    -> UpgradeSuccessPage (支付成功回跳与订单查询)
/desktop/buy                        -> DesktopBuyPage (桌面客户端购买与说明)
```

### 核心分工体系
1. **`contracts`**：跨端统一协议源头（DTO 契约），保证前端、后端与桌面端零协议分叉。
2. **`web-sdk`**：平台中立客户端库（无 DOM、无 Leptos、无 Tauri），涵盖增量 `SseDecoder`、状态机 `reduce_chat_event`、以及完整的 `browser_auth`、`browser_rest`、`workspace_api`、`share_api`、`billing_api`。在 wasm32 下对接 Fetch API，在 native 下返回类型安全的 `Unavailable`。
3. **`web-ui`**：Leptos 0.8 SSR + WASM 水合界面层，统一提供 `ChatCanvasModel` 信号上下文，多页面共享同一个聊天画布组件，彻底消除双轨维护负担。
4. **`desktop/core`**：平台通用的桌面宿主核心库，供 Tauri 和未来 GPUI 桌面端共用流式传输与后端桥接。

---

## 3. 测试与验证门禁证据

所有交付均附带全量自动化验证证据，当前本地代码库处于 100% 绿态：

| 验证维度 | 命令与覆盖 | 结果 |
|---|---|:---:|
| **Rust 模型与集成测试** | `CARGO_BUILD_JOBS=2 cargo test -p web-sdk -p web-ui` | **14 个测试套件，101 passed / 0 failed** |
| **desktop-core 单元测试** | `CARGO_BUILD_JOBS=2 cargo test --manifest-path desktop/core/Cargo.toml` | **4 passed / 0 failed** (中文跨 chunk 切片测试通过) |
| **SSR 检查** | `cargo check -p web-server --features ssr` | **exit code 0** |
| **WASM Hydrate 检查** | `cargo check -p web-ui --target wasm32-unknown-unknown --features hydrate` | **exit code 0** |
| **Tauri 宿主检查** | `cargo +1.96.1 check --manifest-path desktop/src-tauri/Cargo.toml` | **exit code 0** |
| **设计系统样式守卫** | `cargo test -p web-ui --test style_baseline_guard` | **1 passed** (无字重 ≥500、无裸十六进制、无未授权阴影) |
| **Playwright 端到端旅程** | `pnpm exec playwright test (4 个 spec 文件)` | **全量 38 项测试 100% passed** (耗时 33.7s) |
| **Live Backend Smoke** | `LIVE_BACKEND=1 LIVE_API_BASE=http://127.0.0.1:18081 pnpm exec playwright test --config playwright.live.config.ts` | **2 passed / 0 failed** (真实 API 对接通过) |

---

## 4. 下一棒执行指南：E3.5 (管理后台运维与应用内帮助)

### 4.1 目标任务与范围
依据全量路由矩阵（`ROUTE_MIGRATION_MATRIX.md`），E3.5 负责交付剩余的 15 个端点，达成 **Gate 3 (G3)**：
1. **管理后台路由族 (13 端点)**：
   - `/admin`：管理后台概览
   - `/admin/users`：用户管理列表
   - `/admin/accounts` 与 `/admin/accounts/:owner_user_id`：账户详情与管理
   - `/admin/billing`：全平台计费账单
   - `/admin/usage`：全平台模型 Token 消耗监控
   - `/admin/health`：基础设施与服务健康检查
   - `/admin/rag-health`：RAG 检索质量与降级指标
   - `/admin/system/workers`：后台异步队列与 Worker 节点状态
   - `/admin/system/degradation`：服务降级策略配置
   - `/admin/broadcast`：全平台系统公告广播
   - `/admin/audit-logs`：管理审计操作日志
   - `/admin/feature-flags`：功能灰度开关配置
2. **应用内帮助路由族 (2 端点)**：
   - `/help`：应用内使用帮助中心
   - `/help/write`：长文与提示词编写建议指引

### 4.2 核心完成标准与门禁要求 (Gate 3)
- **非 Admin 权限拦截**：未登录用户导向 `/login?next=...`，非 admin 角色返回 403 友好无权访问提示；
- **空态与分页**：所有管理列表支持空态呈现与基本分页；
- **不留死路径**：管理后台所有入口形成闭环，达成 **Gate 3 (G3)** 后即可开启 **E4 (W4 公共 SSR / SEO / 双语)**。

### 4.3 常用命令与操作提示
- **构建工作目录**：`frontend_rust`
- **WASM bindgen CLI 锁定版本**：构建前必须确保使用 0.2.127：
  ```bash
  export PATH="$HOME/.local/opt/wasm-bindgen-cli-0.2.127:$PATH"
  ```
- **构建命令**：`CARGO_BUILD_JOBS=2 cargo leptos build`
- **测试命令**：
  - Rust 测试：`CARGO_BUILD_JOBS=2 cargo test -p web-sdk -p web-ui`
  - Playwright：`cd tests/browser && pnpm exec playwright test`
  - 样式守卫：`cargo test -p web-ui --test style_baseline_guard`
- **图谱维护**：结构性代码修改后，务必在根目录运行：
  ```bash
  code-review-graph update
  ```
- **Time-cost consent 准则**：在启动耗时编译或测试脚本前，先向用户呈报时间预算并取得授权。
