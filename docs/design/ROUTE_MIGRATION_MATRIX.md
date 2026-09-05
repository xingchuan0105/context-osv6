# 全量路由迁移矩阵 (Route Migration Matrix)

更新日期：2026-09-05  
规范依据：[PRODUCT_IA.md](PRODUCT_IA.md) 与 [2026-09-05-development-execution-plan.md](../plans/2026-09-05-development-execution-plan.md)  
全量端点总数：**71 个**（69 个 Page + 2 个 Route 处理器）

---

## 1. 核心对话路由族 (Chat-first Core) — 阶段：E0 / E2

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 1 | `/chat` | `app/(app)/chat/page.tsx` | 可选（未登录引导/已登录加载） | SSR + Hydrate | noindex | 已挂载 (`/chat`) | E0 / E2 |
| 2 | `/chat/:session_id` | `app/(app)/chat/[session_id]/page.tsx` | 必需（用户会话） | SSR + Hydrate | noindex | 已挂载 (`/chat/:session_id`) | E0 / E2 |

---

## 2. 账号与认证路由族 (Auth) — 阶段：E3.1

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 3 | `/login` | `app/(auth)/login/page.tsx` | 访客（已登录回跳） | SSR + Hydrate | noindex | 待挂载 | E3.1 |
| 4 | `/register` | `app/(auth)/register/page.tsx` | 访客（已登录回跳） | SSR + Hydrate | noindex | 待挂载 | E3.1 |
| 5 | `/reset-password` | `app/(auth)/reset-password/page.tsx` | 访客（重置申请） | SSR + Hydrate | noindex | 待挂载 | E3.1 |
| 6 | `/reset-password/verify` | `app/(auth)/reset-password/verify/page.tsx` | 访客（验证码核验） | SSR + Hydrate | noindex | 待挂载 | E3.1 |
| 7 | `/reset-password/confirm` | `app/(auth)/reset-password/confirm/page.tsx` | 访客（新密码重设） | SSR + Hydrate | noindex | 待挂载 | E3.1 |

---

## 3. 设置与用量路由族 (Settings) — 阶段：E3.1

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 8 | `/settings` | `app/(app)/settings/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.1 |
| 9 | `/settings/usage` | `app/(app)/settings/usage/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.1 |

---

## 4. 工作区与控制台路由族 (Dashboard & Workspace) — 阶段：E3.2

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 10 | `/dashboard` | `app/(app)/dashboard/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.2 |
| 11 | `/dashboard/:workspace_id` | `app/(app)/dashboard/[workspace_id]/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 (复用 Chat) | E3.2 |
| 12 | `/dashboard/:workspace_id/analyze` | `app/(app)/dashboard/[workspace_id]/analyze/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.2 |
| 13 | `/dashboard/:workspace_id/share` | `app/(app)/dashboard/[workspace_id]/share/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.2 |
| 14 | `/dashboard/:workspace_id/share/access-logs` | `app/(app)/dashboard/[workspace_id]/share/access-logs/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.2 |
| 15 | `/dashboard/:workspace_id/share/analytics` | `app/(app)/dashboard/[workspace_id]/share/analytics/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.2 |
| 16 | `/dashboard/analytics` | `app/(app)/dashboard/analytics/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.2 |

---

## 5. 邀请与分享路由族 (Share & Invites) — 阶段：E3.3

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 17 | `/invite/:workspace_id/:member_id` | `app/invite/[workspace_id]/[member_id]/page.tsx` | 需登录/注册接受 | SSR + Hydrate | noindex | 待挂载 | E3.3 |
| 18 | `/shared/kb/:token` | `app/shared/kb/[token]/page.tsx` | 公开带 Token | SSR + Hydrate | noindex | 待挂载 | E3.3 |
| 19 | `/shared/u/:userId` | `app/shared/u/[userId]/page.tsx` | 公开分享者主页 | SSR + Hydrate | indexable | 待挂载 | E3.3 |

---

## 6. 商业交易与支付路由族 (Billing & Paywall) — 阶段：E3.4

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 20 | `/pricing` | `app/(marketing)/pricing/page.tsx` | 公开 | SSR | indexable | 待挂载 | E3.4 |
| 21 | `/upgrade/paywall` | `app/(app)/upgrade/paywall/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.4 |
| 22 | `/upgrade/success` | `app/(app)/upgrade/success/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E3.4 |
| 23 | `/desktop/buy` | `app/(marketing)/desktop/buy/page.tsx` | 公开/需登录交易 | SSR + Hydrate | noindex | 待挂载 | E3.4 |

---

## 7. 管理后台路由族 (Admin Ops) — 阶段：E3.5

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 24 | `/admin` | `app/admin/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 25 | `/admin/accounts` | `app/admin/accounts/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 26 | `/admin/accounts/:owner_user_id` | `app/admin/accounts/[owner_user_id]/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 27 | `/admin/audit-logs` | `app/admin/audit-logs/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 28 | `/admin/billing` | `app/admin/billing/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 29 | `/admin/broadcast` | `app/admin/broadcast/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 30 | `/admin/feature-flags` | `app/admin/feature-flags/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 31 | `/admin/health` | `app/admin/health/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 32 | `/admin/rag-health` | `app/admin/rag-health/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 33 | `/admin/system/degradation` | `app/admin/system/degradation/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 34 | `/admin/system/workers` | `app/admin/system/workers/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 35 | `/admin/usage` | `app/admin/usage/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |
| 36 | `/admin/users` | `app/admin/users/page.tsx` | 必需 (admin) | SSR + Hydrate | noindex | 待挂载 | E3.5 |

---

## 8. 帮助文档与集成生态路由族 (Help & Integrations) — 阶段：E3.5 / E4

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 37 | `/help` | `app/(app)/help/page.tsx` | 公开/应用内 | SSR | indexable | 待挂载 | E3.5 |
| 38 | `/help/write` | `app/(app)/help/write/page.tsx` | 公开/应用内 | SSR | indexable | 待挂载 | E3.5 |
| 39 | `/help/faq` | `app/(open)/help/faq/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 40 | `/help/compare` | `app/(open)/help/compare/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 41 | `/help/api-access` | `app/(open)/help/api-access/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 42 | `/help/api-access/agents` | `app/(open)/help/api-access/agents/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 43 | `/integrations` | `app/(open)/integrations/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 44 | `/integrations/mcp` | `app/(open)/integrations/mcp/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 45 | `/integrations/claude-desktop` | `app/(open)/integrations/claude-desktop/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 46 | `/integrations/cursor` | `app/(open)/integrations/cursor/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |

---

## 9. 桌面客户端生态路由族 (Desktop Marketing & Setup) — 阶段：E4

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 47 | `/desktop` | `app/(marketing)/desktop/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 48 | `/activate` | `app/(desktop)/activate/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E4 |
| 49 | `/setup` | `app/(desktop)/setup/page.tsx` | 必需 | SSR + Hydrate | noindex | 待挂载 | E4 |

---

## 10. 法律条款中文路由族 (Legal - zh-CN) — 阶段：E4

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 50 | `/legal` | `app/(marketing)/legal/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 51 | `/legal/terms` | `app/(marketing)/legal/terms/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 52 | `/legal/privacy` | `app/(marketing)/legal/privacy/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 53 | `/legal/licenses` | `app/(marketing)/legal/licenses/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 54 | `/legal/licenses/project` | `app/(marketing)/legal/licenses/project/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 55 | `/legal/licenses/third-party` | `app/(marketing)/legal/licenses/third-party/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |

---

## 11. 英文公共站与法律路由族 (English Public Surface - en) — 阶段：E4

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 56 | `/en` | `app/en/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 57 | `/en/pricing` | `app/en/pricing/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 58 | `/en/desktop` | `app/en/desktop/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 59 | `/en/help/faq` | `app/en/help/faq/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 60 | `/en/help/compare` | `app/en/help/compare/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 61 | `/en/help/api-access` | `app/en/help/api-access/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 62 | `/en/help/api-access/agents` | `app/en/help/api-access/agents/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 63 | `/en/legal` | `app/en/legal/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 64 | `/en/legal/terms` | `app/en/legal/terms/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 65 | `/en/legal/privacy` | `app/en/legal/privacy/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 66 | `/en/legal/licenses` | `app/en/legal/licenses/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 67 | `/en/legal/licenses/project` | `app/en/legal/licenses/project/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |
| 68 | `/en/legal/licenses/third-party` | `app/en/legal/licenses/third-party/page.tsx` | 公开 | SSR | indexable | 待挂载 | E4 |

---

## 12. 根路径与静态/抓取 Route 处理器 — 阶段：E4

| # | 规范路径 (Canonical URL) | Next.js 源文件 | 鉴权 (Auth) | 渲染 (Render) | SEO / noindex | Rust 挂载状态 | 归属阶段 |
|---|---|---|---|---|---|---|---|
| 69 | `/` (首页/产品根) | `app/page.tsx` | 公开 (未登录展示，已登录导向 `/chat`) | SSR | indexable | PoC 临时重定向 `/chat` (E4 替换为产品首页) | E4 |
| 70 | `/llms.txt` | `app/llms.txt/route.ts` | 公开 | Static Text | noindex | 待挂载 | E4 |
| 71 | `/baidu_verify_codeva-THd6TRYMwv.html` | `app/baidu_verify_codeva-THd6TRYMwv.html/route.ts` | 公开 | Static Text | noindex | 待挂载 | E4 |

---

## 13. 阶段分布与生产所有权归属汇总

| 阶段 | 路由数量 | 涵盖业务 |
|---|---|---|
| **E0 / E2** | 2 | `/chat`, `/chat/:session_id` 个人与工作区核心流式问答 |
| **E3.1** | 7 | 认证注册重置 (`/login`, `/register`, `/reset-password/*`) 与 设置用量 (`/settings/*`) |
| **E3.2** | 7 | 工作区控制台 (`/dashboard/*`)，复用 ChatCanvas 画布 |
| **E3.3** | 3 | 工作区邀请与公开分享页面 (`/invite/*`, `/shared/*`) |
| **E3.4** | 4 | 交易计费、定价与购买流程 (`/pricing`, `/upgrade/*`, `/desktop/buy`) |
| **E3.5** | 15 | 管理后台运维 (`/admin/*`) 与应用内帮助 (`/help`, `/help/write`) |
| **E4** | 33 | 公开营销站、桌面下载激活、双语帮助、双语法律文本、首页与抓取协议 |
| **总计** | **71** | **100% 覆盖现有 `frontend_next` 的全部端点** |
