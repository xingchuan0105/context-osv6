# Rust 布局路由对应表

2026-09-09。按当前 app.rs 提取 68 条路由声明；参数路由与 query 入口仍需在 W5 展开验收。这里只记录布局对应，不代表全部路由已完成浏览器验收。

| 路由 | 布局实现 | W5 全站验收 |
|---|---|---|
| `/` | PublicLayout | 待验证 |
| `/chat/:session_id?` | NavigationRail + ChatPage | 待验证 |
| `/dashboard` | ApplicationLayout | 待验证 |
| `/dashboard/analytics` | ApplicationLayout | 待验证 |
| `/dashboard/:workspace_id/analyze` | ApplicationLayout | 待验证 |
| `/dashboard/:workspace_id/share/access-logs` | ApplicationLayout | 待验证 |
| `/dashboard/:workspace_id/share/analytics` | ApplicationLayout | 待验证 |
| `/dashboard/:workspace_id/share` | ApplicationLayout | 待验证 |
| `/dashboard/:workspace_id` | ApplicationLayout | 待验证 |
| `/shared/kb/:token` | PublicLayout | 待验证 |
| `/shared/u/:user_id` | PublicLayout | 待验证 |
| `/invite/:workspace_id/:member_id` | PublicLayout | 待验证 |
| `/login` | PublicLayout | 待验证 |
| `/register` | PublicLayout | 待验证 |
| `/reset-password` | PublicLayout | 待验证 |
| `/reset-password/verify` | PublicLayout | 待验证 |
| `/reset-password/confirm` | PublicLayout | 待验证 |
| `/settings` | ApplicationLayout | 待验证 |
| `/settings/usage` | ApplicationLayout | 待验证 |
| `/admin` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/accounts` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/accounts/:owner_user_id` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/users` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/usage` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/billing` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/health` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/rag-health` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/system/workers` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/system/degradation` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/broadcast` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/audit-logs` | ApplicationLayout + AdminShell | 待验证 |
| `/admin/feature-flags` | ApplicationLayout + AdminShell | 待验证 |
| `/help` | ApplicationLayout | 待验证 |
| `/help/write` | ApplicationLayout | 待验证 |
| `/help/faq` | PublicLayout | 待验证 |
| `/help/compare` | PublicLayout | 待验证 |
| `/help/api-access` | PublicLayout | 待验证 |
| `/help/api-access/agents` | PublicLayout | 待验证 |
| `/integrations` | PublicLayout | 待验证 |
| `/integrations/mcp` | PublicLayout | 待验证 |
| `/integrations/claude-desktop` | PublicLayout | 待验证 |
| `/integrations/cursor` | PublicLayout | 待验证 |
| `/desktop` | PublicSiteHeader | 待验证 |
| `/activate` | PublicLayout | 待验证 |
| `/setup` | PublicLayout | 待验证 |
| `/legal` | PublicSiteHeader | 待验证 |
| `/legal/terms` | PublicSiteHeader | 待验证 |
| `/legal/privacy` | PublicSiteHeader | 待验证 |
| `/legal/licenses` | PublicSiteHeader | 待验证 |
| `/legal/licenses/project` | PublicSiteHeader | 待验证 |
| `/legal/licenses/third-party` | PublicSiteHeader | 待验证 |
| `/en` | PublicLayout | 待验证 |
| `/en/pricing` | PublicSiteHeader | 待验证 |
| `/en/desktop` | PublicSiteHeader | 待验证 |
| `/en/help/faq` | PublicLayout | 待验证 |
| `/en/help/compare` | PublicLayout | 待验证 |
| `/en/help/api-access` | PublicLayout | 待验证 |
| `/en/help/api-access/agents` | PublicLayout | 待验证 |
| `/en/legal` | PublicSiteHeader | 待验证 |
| `/en/legal/terms` | PublicSiteHeader | 待验证 |
| `/en/legal/privacy` | PublicSiteHeader | 待验证 |
| `/en/legal/licenses` | PublicSiteHeader | 待验证 |
| `/en/legal/licenses/project` | PublicSiteHeader | 待验证 |
| `/en/legal/licenses/third-party` | PublicSiteHeader | 待验证 |
| `/pricing` | PublicSiteHeader | 待验证 |
| `/upgrade/paywall` | PublicLayout | 待验证 |
| `/upgrade/success` | PublicLayout | 待验证 |
| `/desktop/buy` | PublicLayout | 待验证 |

重定向保留：`/` → 登录或聊天，`/activate` → `/desktop`，`/setup` → `/settings?tab=providers`，工作区 `/analyze` → 分享中心。

旧 ProductChrome / AppTopBar / ShareAccessMenu 已移除；PublicFooter 仅用于公共定价页。
