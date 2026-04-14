# context-osv6 Rust Frontend Design

> 状态：设计稿  
> 时间：2026-03-20  
> 说明：本文是在明确采用 Rust 前端方向的前提下，为 `context-osv6` 制定的新前端方案。它补充并部分替代 [PRD_RUST.md](../../PRD_RUST.md) 中第 16、17、20、29、30 章的前端实现建议；后端协议与业务边界仍以 `avrag-rs` 当前实现为准。

## 1. 设计结论

- 前端主方案选择：`Leptos SSR + Hydration + Axum + SSE`。
- 不选 `Dioxus Fullstack` 作为主线，不是因为它不能做，而是因为 `context-osv6` 当前更像“网页优先的 AI 工作台”，核心实时形态也是单向 token 流；Leptos 与现有 Axum/SSR/SSE 模型更贴合。
- Rust 前端不直接替换 `avrag-rs` 的 API 协议；它复用当前 `/api/v1/*`、`/api/auth/*`、`/api/shared/*`、`/webhooks/*`、`/docs`、`/openapi.json`、`/metrics`。
- Rust 前端必须完整继承 v5 已被验证的核心体验：
  - 工作区列表与收藏入口
  - Notebook 工作台
  - 文档上传与状态流转
  - 流式聊天
  - 引用查看
  - 分享、API Key、通知、计费、设置
- Rust 前端必须新增 v6 后端已具备但 v5 页面未完整承接的能力：
  - 密码重置全流程
  - 分享成员接受/拒绝
  - 分享统计与访问日志
  - 与实际 v6 管理接口对齐的 Admin 面板
  - 更明确的降级提示、Guard 报告与证据面板
- v5 的“笔记”能力不是后端能力，而是前端本地能力；Rust 版保留为“本地优先模块”，不假装它已经是 v6 服务端功能。

## 2. 为什么选 Leptos

选型依据在 2026-03-20 已用官方资料复核：

- Leptos 官方 Book 明确覆盖了 full-stack、SSR 与 hydration 路径，说明它适合做真正的 Web 应用，而不是只能做 CSR 小页面。
- `docs.rs` 当前 `leptos` 仍提供 `ssr`、`hydrate` 等特性，且 `leptos::hydration` 明确提供客户端水合能力。
- Dioxus 官方 0.7 文档同时覆盖了 `Fullstack`、`WebSocket`、`Streams and SSE`，说明它也可行；但它更偏“一套模型覆盖 web/desktop/mobile”，而 `context-osv6` 当前重点并不在跨端统一，而在 Web 工作台、SEO 友好首屏、与现有 Axum 服务共生。

因此这里的判断是：

- 如果目标是“最像 NotebookLM 的网页工作台”，选 Leptos。
- 如果目标是“未来同一套 Rust 组件同时跑桌面与移动”，再考虑 Dioxus 二期演进。

## 3. 现状评估：context-osv5 前端

### 3.1 v5 已有页面与交互资产

| 范围 | v5 页面/组件 | 已实现内容 | 迁移结论 |
|---|---|---|---|
| 应用入口 | `src/app/page.tsx` | 直接跳 `/dashboard` | 直接保留 |
| 认证 | `/(auth)/login`、`/(auth)/register` | 注册、登录、登出、自举登录态 | 保留并补齐重置密码 |
| 工作区列表 | `/dashboard` | Notebook 列表、卡片/列表视图、创建/编辑/删除、收藏分享工作区 | 高价值复用 |
| 工作区主界面 | `/dashboard/[id]` | 左侧文档/笔记栏、右侧聊天、上传、状态轮询、文档预览、源选择、笔记导入 KB | 作为主体验基线保留 |
| 全局壳层 | `/dashboard/layout` | 通知、分享、API Access、设置、移动菜单 | 保留，但结构要 Rust 化 |
| 分享页 | `/shared/kb/[token]` | 公开分享预览、登录后聊天、收藏/取消收藏 | 保留并增强 |
| 搜索页 | `/dashboard/search` + omnibar | 关键词搜索、按类型跳转 | 保留，但结果契约统一到 v6 |
| 设置 | `settings-drawer` | 主题、语言、资料、改密、计费 | 保留，去掉无后端支撑项 |
| 计费 | `billing-panel` | 套餐、当前订阅、用量、Checkout、Portal | 保留 |
| API 接入 | `api-access-modal` | Notebook API Key 创建/吊销、curl/OpenAI/MCP 示例 | 保留 |
| 通知中心 | `notification-center` | 通知列表、已读 | 保留 |
| 聊天 | `chat-panel`、`chat-input`、`chat-bubble`、`chat-trace-panel` | RAG/general/search 三模式、SSE、trace、citation lookup、提取到笔记 | 保留并强化 |
| 文档预览 | `document-viewer` | `content` / `parsed-preview` 预览、分页加载 | 保留并嵌入证据面板 |
| Admin | `/admin/*` | 有壳，但调用的是老接口，不匹配 v6 当前路由 | 必须重做 |

### 3.2 v5 前端最值得保留的设计

- 工作区页的“文档与笔记统一左栏 + 聊天主舞台”非常适合 AI 研究助手。
- 文档上传后的即时状态反馈、轮询与自动选源已经符合真实使用心智。
- 聊天气泡中的引用入口、trace 面板、模式切换、`@agent` 输入模型都已经验证可用。
- 分享、通知、计费、API Key 都已经有用户心智入口，不需要重新发明导航。

### 3.3 v5 前端对 v6 的主要缺口

| 缺口 | 现状 | 影响 |
|---|---|---|
| Admin 页面与 v6 实际接口不匹配 | 仍在调用旧的 `/api/v1/admin/*` 风格接口 | 管理后台基本不可直接迁移 |
| 工作区不是严格三栏 | 证据查看依赖 modal 或聊天内弹层 | 不利于 citation 深跳、trace 与 degrade 并列可视化 |
| `degrade_trace` 只有调试感，没有产品化告警条 | 用户不容易理解回答可信度变化 | 与 PRD 不一致 |
| 分享只有设置/成员，没有统计/访问日志页 | v6 已有后端能力未被使用 | 管理者缺少分享可观测性 |
| 密码重置全流程没有完整页面 | 后端已有 `/api/auth/reset/*` | 认证闭环缺失 |
| 笔记是本地 localStorage | 不是服务端模块 | 需要在 Rust 设计里明确标注为本地功能 |
| 文档重索引没有成体系的操作入口 | 接口有，主页面缺少显式承接 | 运维与修复链路不完整 |
| 管理后台信息架构偏“旧 CMS” | 与 v6 的 org/user/usage/health 域不一致 | 无法表达真正的 SaaS 后台 |

## 4. 现状评估：context-osv6 后端能力

### 4.1 当前后端已具备的功能域

| 功能域 | v6 路由/模块 | 前端承接状态 |
|---|---|---|
| Auth | `/api/auth/register` `/login` `/logout` `/me` `/profile` `/change-password` `/reset-*` | 部分承接，重置密码缺页 |
| Notebook | `/api/v1/notebooks*` | 已承接 |
| Document | `/api/v1/notebooks/{id}/documents` `/documents/*` `/reindex` | 已承接但需补重索引入口 |
| Chat | `/api/v1/chat` `/chat/sessions*` `/chat/citations/lookup` | 已承接 |
| SSE | `start` `trace` `planner_complete` `rag_trace` `rag_sources` `token` `citations` `done` `error` | 已承接但 UX 可提升 |
| API Key | `/api/v1/notebooks/{id}/api-keys*` | 已承接 |
| Notification | `/api/v1/notifications*` | 已承接 |
| Share | `/api/v1/notebooks/{id}/share*` `/members*` `/share/validate/{token}` `/api/shared/kb/{token}` | 部分承接 |
| Share Analytics | `/api/v1/notebooks/{id}/share/analytics` `/share/access-logs` | 未承接 |
| Billing | `/api/v1/billing/*` + `/webhooks/stripe` | 已承接用户端，运维面未显式体现 |
| Admin | `/api/v1/admin/organizations` `/organizations/{org_id}` `/users` `/usage` `/billing/block` `/health` | 几乎未承接 |
| Docs/Ops | `/docs` `/openapi.json` `/metrics` `/health` `/ready` | 只作为开发辅助存在 |

### 4.2 v6 后端没有的前端能力

目前没有看到服务端 Note API。现有 v5/v6 前端中的 `notesApi` 使用本地存储实现。

这意味着 Rust 前端要做两个明确决定：

- 保留“本地草稿笔记”作为纯前端模块。
- 不把它包装成“已同步到服务端”的产品能力。

## 5. Rust 前端总体方案

### 5.1 主技术栈

| 层级 | 方案 |
|---|---|
| UI 框架 | `leptos`（SSR + hydration） |
| 路由 | `leptos_router` |
| 头部/Meta | `leptos_meta` |
| Axum 集成 | `leptos_axum` |
| 服务承载 | 复用 `avrag-rs` 现有 `axum` 服务 |
| 实时流 | 原生 `EventSource` 封装，继续消费 `/api/v1/chat?stream=true` |
| HTTP 数据层 | Rust typed client，SSR 侧走 `reqwest`，hydrate 侧走浏览器 `fetch` 封装 |
| DTO 共享 | 复用 `crates/common`，必要时增加 `crates/web-sdk` 做前端兼容层 |
| 样式 | 复用现有设计 token，迁移为 Rust 前端可直接消费的 CSS 变量与 utility class；不引入 React 风格组件体系 |
| 本地持久化 | 浏览器本地存储适配层，承载 theme、locale、工作区偏好、笔记草稿 |
| 构建 | `cargo-leptos` 或等价 SSR/hydrate 构建链，产物由 `avrag-rs` 同进程服务 |
| E2E | Playwright 继续保留，新增 Rust 组件/状态测试 |

### 5.2 为什么不是“前端也走一套新的 Rust RPC”

不建议为了 Rust 前端再发明一套 UI 专属 RPC，原因是：

- v6 当前 REST/SSE 契约已经比较稳定。
- 现有 `transport-http` 已经是产品对外契约。
- 让 Rust 前端直接复用这些接口，可以减少与后端语义分叉。

因此采用双通道策略：

- SSR 首屏读取：页面 loader 直接使用 `AppState` 或 typed SDK 拉数据，避免无意义 loopback HTTP。
- 客户端交互：统一走当前 `/api/*`、`/api/v1/*`、`/api/shared/*` 与 SSE。

### 5.3 建议的代码组织

```text
avrag-rs/
  crates/
    web-ui/
      src/
        app.rs
        shell/
        routes/
          auth/
          dashboard/
          shared/
          admin/
        components/
          chat/
          workspace/
          document/
          share/
          billing/
          settings/
          admin/
          common/
        state/
        api/
        storage/
        sse/
      assets/
        app.css
        icons/
    web-sdk/
      src/
        auth.rs
        notebooks.rs
        documents.rs
        chat.rs
        share.rs
        billing.rs
        admin.rs
        notifications.rs
        sse.rs
```

设计原则：

- `web-ui` 负责视图、状态、页面组装。
- `web-sdk` 负责把现有后端契约翻译成前端可直接消费的 typed API。
- `common` 继续作为真正的跨层 DTO 真相源。

## 6. 信息架构与路由设计

### 6.1 用户端主路由

| 路由 | 页面定位 | 来源 |
|---|---|---|
| `/` | 入口重定向到 `/dashboard` | 保留 v5 |
| `/login` | 登录 | 保留 v5 |
| `/register` | 注册 | 保留 v5 |
| `/reset-password` | 发起重置请求 | v6 新增 |
| `/reset-password/verify` | 校验 ticket / code | v6 新增 |
| `/reset-password/confirm` | 设置新密码 | v6 新增 |
| `/dashboard` | Notebook 列表、收藏入口 | 保留 v5 |
| `/dashboard/search` | 全局搜索结果页 | 保留 v5 |
| `/dashboard/:notebook_id` | 主工作台 | v5 保留并升级 |
| `/dashboard/:notebook_id/share` | 分享与协作中心 | 从 v5 modal 升级为完整页 |
| `/dashboard/:notebook_id/share/analytics` | 分享统计 | v6 新增 |
| `/dashboard/:notebook_id/share/access-logs` | 分享访问日志 | v6 新增 |
| `/dashboard/:notebook_id/api-access` | Notebook API / MCP 面板 | 从 v5 modal 升级为可独立访问页 |
| `/shared/kb/:token` | 公开分享页 | 保留 v5 |
| `/invite/:notebook_id/:member_id` | 协作邀请接受/拒绝页 | v6 新增 |
| `/help` | 帮助文档 | 保留 |

### 6.2 Admin 路由

当前 v6 后端真正支持的路由，应对应这些页面：

| 路由 | 页面定位 | 说明 |
|---|---|---|
| `/admin` | Admin 首页，重定向到 `/admin/organizations` | 新定义 |
| `/admin/organizations` | 租户/组织列表 | 对接 `GET /api/v1/admin/organizations` |
| `/admin/organizations/:org_id` | 组织详情 | 对接 `GET /api/v1/admin/organizations/{org_id}` |
| `/admin/users` | 用户列表 | 对接 `GET /api/v1/admin/users` |
| `/admin/usage` | 平台用量总览 | 对接 `GET /api/v1/admin/usage` |
| `/admin/health` | 平台健康页 | 对接 `GET /api/v1/admin/health` |

说明：

- v5 的 `/admin/token-stats`、`/admin/admins`、`/admin/logs` 当前不是 v6 已实现能力，不应原样迁移。
- PRD 中的 `/admin/billing`、`/admin/rag-health`、`/admin/feature-flags`、`/admin/system/workers`、`/admin/system/degradation` 可以保留为未来导航占位，但在当前实现阶段不应出现“点击后 404”的死链接。

## 7. 页面与版面设计

### 7.1 Dashboard 列表页

保留 v5 的成熟结构：

- 顶部工具条：
  - 视图切换：卡片 / 列表
  - 新建 Notebook
- 主内容区：
  - 我的 Notebook
  - 收藏的分享 Notebook

Rust 版增强点：

- Notebook 卡片直接显示最近聊天时间、文档数、状态摘要。
- 收藏卡片区分“来自分享链接”与“已加入协作”。
- 页面首屏由 SSR 输出卡片骨架，hydrate 后补充交互。

### 7.2 主工作台页

主工作台采用“三栏式 Session 优先”架构，将 AI 工作台从“文档分析”进化为“长周期研究助手”。

#### 桌面版三栏布局

| 区域 | 内容 | 交互特性 |
|---|---|---|
| **左栏 (History)** | **历史对话记录 (Sessions)** | 支持多 Session 并行；上下文完全隔离；支持重命名与置顶管理。 |
| **中栏 (Chat)** | **对话主舞台 (Current Session)** | 沉浸式对话；采用 V5 风格轻量化 Citation 小标（非重型面板）。 |
| **右栏 (Assets)** | **内容源 (Sources) + 笔记 (Notes)** | **上下堆叠呈现**：上半部为文档列表，下半部为笔记列表，均具备独立滚动条与 Resize Handle。 |

#### 核心交互逻辑

1.  **轻量化溯源**: 点击聊天中的 Citation 小标，右栏对应的文档块产生高亮闪烁提示。
2.  **对话提取 (Export to Note/Source)**:
    *   聊天气泡提供“提取”操作，唤起 **Floating Rich Text Editor**。
    *   用户可选择“保存到笔记”或“保存到内容源”（导出为 .md 文件并重新索引）。
3.  **智能指令 (@Note)**:
    *   在输入框输入 `@笔记` 即时唤起新建笔记编辑器，保存后自动在对话流中插入记录确认。


### 7.3 分享与协作中心

v5 把分享做成 modal；Rust 版要保留 modal 快捷入口，但正式升级为独立中心页。

页面分成四个区块：

- `Link Share`
  - 生成链接
  - 权限：`partial / full`
  - 过期时间
  - 复制链接
- `Access Level`
  - `private / link / public`
- `Members`
  - 邀请成员
  - 成员角色与状态
  - 移除成员
- `Analytics`
  - share token 列表
  - 浏览次数
  - 最近访问时间
  - 访问日志

### 7.4 公开分享页

保留 v5 的基本布局：

- 顶部分享说明卡
- 文档列表摘要
- 登录后可聊天
- 收藏/取消收藏

Rust 版增强：

- 当 permission 为 `partial` 时，正文预览受限，但仍可看到文档标题与状态。
- 当 permission 为 `full` 时，右侧证据面板可正常打开，只是限制写操作。
- 分享访问会写入 access log，页面顶部给出访问来源和权限说明。

### 7.5 设置中心

从 v5 的 `SettingsDrawer` 继承，但重组信息架构：

- `Appearance`
  - 主题
  - 语言
- `Account`
  - 个人资料
  - 修改密码
  - 发起密码重置
- `Billing`
  - 当前套餐
  - 用量
  - 升级
  - 打开 Portal
- `Security`
  - 当前登录设备摘要
  - 退出登录

处理原则：

- 保留真实后端已支持的能力。
- 像 WeChat 绑定这类当前 v6 后端未暴露的功能，不进入默认 UI。

### 7.6 Admin 后台

Rust 版 Admin 不再沿用 v5 的旧 CMS 布局，而是重构为“组织中心 + 平台总览”。

#### Admin Shell

- 左侧导航
  - Organizations
  - Users
  - Usage
  - Health
- 右侧主内容

#### Organizations 页

- 表格字段：
  - org_id
  - 名称
  - 当前 plan
  - 用户数
  - notebook 数
  - 是否 blocked
- 操作：
  - 查看详情
  - Block/Unblock

#### Organization Detail 页

- 组织基本信息
- 当前订阅
- 关键用量
- 最近通知与分享活跃摘要
- “Block billing” 操作区

#### Users 页

- 按邮箱/组织筛选
- 展示用户角色、创建时间、最近活跃

#### Usage 页

- 平台用量总览
- 按组织维度查看 usage
- 支持时间窗口切换

#### Health 页

- `health` / `ready` / `metrics` 快速状态
- 关键降级率、超时率、失败率摘要

## 8. 交互逻辑设计

### 8.1 认证流程

- 应用启动时 SSR 优先读认证上下文。
- hydrate 后刷新 `/api/auth/me`，失败则回退登录页。
- 登录、注册、登出沿用现有后端。
- 补齐密码重置三步：
  1. `/reset-password`：提交邮箱
  2. `/reset-password/verify`：校验 code / token
  3. `/reset-password/confirm`：提交新密码

### 8.2 Notebook 与文档流程

Notebook：

- 列表页可创建、重命名、删除。
- 切换 Notebook 时必须清空当前 `doc_scope`、聊天输入态、引用选中态。

文档上传：

1. `POST /api/v1/notebooks/{id}/documents`
2. 对返回的 `upload_url` 或本地 dev upload 地址执行实际上传
3. 如后端要求，调用 `complete-upload`
4. 进入状态轮询
5. `completed` 后可自动加入 `doc_scope`

文档状态：

- 非终态每 2 秒轮询一次。
- 终态停止轮询。
- `failed` 给出重试或重索引入口。

### 8.3 聊天与 SSE 流

统一请求体：

- `query`
- `notebook_id`
- `session_id`
- `agent_type`
- `doc_scope`
- `source_type`
- `source_token`
- `stream`

状态机：

`idle -> submitting -> streaming -> done | error`

事件处理：

| SSE 事件 | UI 行为 |
|---|---|
| `start` | 初始化 assistant 占位消息 |
| `trace` | 追加 trace event |
| `planner_complete` | 更新 planner 面板 |
| `rag_trace` | 更新检索摘要 |
| `rag_sources` | 更新证据面板的 source list |
| `token` | 追加 assistant 文本 |
| `citations` | 更新引用列表 |
| `done` | 完结消息并刷新 session 摘要 |
| `error` | 终止流并显示错误态 |

### 8.4 引用与证据查看

点击 citation 后：

1. 调 `POST /api/v1/chat/citations/lookup`
2. 自动打开右栏 `Evidence`
3. 定位到对应 `doc_id + chunk_id`
4. 若有页码信息则同时切换文档页签

Rust 版不再只用 modal 展示 citation；modal 只作为移动端 fallback。

### 8.5 降级与 Guard 可视化

只要 `degrade_trace` 非空，就必须出现顶部告警条，内容至少包含：

- 降级阶段
- 原因
- 对引用或可信度的影响

如果存在 `guard_report`：

- 在右栏 `Trace` 中展示输入/输出护栏命中结果。
- 对高风险结果加醒目 badge。

### 8.6 分享与成员协作

- 分享中心默认展示当前 access level 与最近一个有效 token。
- 邀请成员后，列表即时出现 `invite_status=pending`。
- 被邀请者从专门的 `/invite/:notebook_id/:member_id` 页面接受或拒绝。
- 分享统计页直接消费：
  - `/share/analytics`
  - `/share/access-logs`

### 8.7 计费

- 用户端设置中心展示套餐、配额与当前订阅。
- `Upgrade` 触发 checkout。
- `Manage Billing` 触发 portal。
- 组织被 block 时，工作区顶部展示 billing 限制提醒，并在上传/聊天等动作前做前端预提示，但最终裁决仍以后端 quota 为准。

### 8.8 本地草稿笔记

这是明确的“本地优先”能力：

- 存储位置：浏览器本地持久化。
- 能力：创建、编辑、删除、导入为 Markdown 文档。
- 不与后端 `session`、`document`、`notification` 混用。
- UI 必须显示“仅当前浏览器可见”。

## 9. 状态管理设计

Rust 前端不需要照搬 React/Zustand 模式，建议按 Leptos 领域上下文组织：

| 领域 | 状态内容 |
|---|---|
| `AuthState` | user、token presence、auth bootstrap |
| `DashboardState` | notebook list、favorites、view mode |
| `WorkspaceState` | current notebook、documents、doc_scope、uploads、preview target |
| `ChatState` | sessions、messages、stream phase、active agent、trace、citations |
| `ShareState` | access level、members、tokens、analytics、access logs |
| `BillingState` | plans、subscription、usage |
| `AdminState` | org list、org detail、users、usage、health |
| `DraftState` | local notes、draft editor state |
| `UiPrefsState` | theme、language、panel widths、collapsed flags |

原则：

- 以 route-level context 为主，不做一个巨型全局 store。
- 需要跨页面持久化的最少状态才入本地存储。
- streaming 状态绝不写 localStorage。

## 10. 兼容与迁移策略

### 10.1 迁移顺序

1. `web-ui` 与现有 Next.js `frontend/` 并存
2. 先完成 Auth、Dashboard、Shared 页
3. 再完成 Workspace 三栏页
4. 接入 Chat SSE、citation、trace、degrade
5. 补齐 Share、Billing、API Access、Notifications
6. 重做 Admin
7. 切流并归档 Next.js 前端

### 10.2 与现有前端的兼容边界

- API 兼容层允许继续接受 `workspace`/`kb_id` 这种旧命名，但内部统一为 `notebook`。
- 聊天 agent 对外仍兼容 `knowledge_base -> rag` 的旧映射。
- Shared/favorite 的历史本地缓存可以迁移，但不保证长期保留旧 key。

## 11. 测试与验收

### 11.1 必测用户链路

- 注册 -> 登录 -> 创建 Notebook -> 上传文档 -> 等待完成 -> RAG 聊天 -> citation lookup
- General mode 聊天
- Search mode 聊天
- 分享链接生成 -> 未登录打开 -> 登录后聊天 -> 收藏/取消收藏
- API Key 创建/吊销
- Billing checkout/portal 跳转
- 通知已读
- Admin 组织列表 -> 组织详情 -> block 操作

### 11.2 组件与契约测试

- SSE parser 测试
- Chat 状态机测试
- Document status poller 测试
- Share analytics/access logs 映射测试
- 本地草稿存储适配测试

### 11.3 验收目标

- 继承 PRD 中对首屏、首 token、引用跳转、状态一致性的指标。
- Admin 页面不允许再出现指向旧接口的“伪可用”页面。
- 所有 v6 已存在后端能力必须有稳定入口。

## 12. 最终建议

这次 Rust 前端不是“把 Next.js 页面翻译成 Rust 语法”，而是做三件事：

- 保留 v5 真正有效的工作台交互。
- 用 Leptos 把 SSR、hydration、Axum 同进程能力吃满。
- 用真正的三栏工作台和补齐的后台/分享页面，把 v6 后端已经做出来的能力全部显性化。

一句话定稿：

- 主线选 `Leptos + Axum + SSE`。
- 工作台继承 v5，但升级为三栏证据化界面。
- Notes 保留本地优先。
- Share Analytics、Access Logs、Password Reset、Admin 全部补齐为一等页面。
