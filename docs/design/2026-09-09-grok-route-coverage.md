# Rust 布局路由对应表

2026-09-09。按当前 app.rs 提取 68 条路由声明；W5 已将参数路由实例化并补充 4 个设置 query 入口，完成 72 × 4 布局检查；详见 `2026-09-09-grok-w5-matrix.md`。参数值仅覆盖测试夹具实例，不代表全部业务数据。

| 路由 | 布局实现 | W5 全站验收 |
|---|---|---|
| `/` | PublicLayout | 布局矩阵通过 |
| `/chat/:session_id?` | NavigationRail + ChatPage | 布局矩阵通过 |
| `/dashboard` | ApplicationLayout | 布局矩阵通过 |
| `/dashboard/analytics` | ApplicationLayout | 布局矩阵通过 |
| `/dashboard/:workspace_id/analyze` | ApplicationLayout | 布局矩阵通过 |
| `/dashboard/:workspace_id/share/access-logs` | ApplicationLayout | 布局矩阵通过 |
| `/dashboard/:workspace_id/share/analytics` | ApplicationLayout | 布局矩阵通过 |
| `/dashboard/:workspace_id/share` | ApplicationLayout | 布局矩阵通过 |
| `/dashboard/:workspace_id` | ApplicationLayout | 布局矩阵通过 |
| `/shared/kb/:token` | PublicLayout | 布局矩阵通过 |
| `/shared/u/:user_id` | PublicLayout | 布局矩阵通过 |
| `/invite/:workspace_id/:member_id` | PublicLayout | 布局矩阵通过 |
| `/login` | PublicLayout | 布局矩阵通过 |
| `/register` | PublicLayout | 布局矩阵通过 |
| `/reset-password` | PublicLayout | 布局矩阵通过 |
| `/reset-password/verify` | PublicLayout | 布局矩阵通过 |
| `/reset-password/confirm` | PublicLayout | 布局矩阵通过 |
| `/settings` | ApplicationLayout | 布局矩阵通过 |
| `/settings/usage` | ApplicationLayout | 布局矩阵通过 |
| `/admin` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/accounts` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/accounts/:owner_user_id` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/users` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/usage` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/billing` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/health` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/rag-health` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/system/workers` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/system/degradation` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/broadcast` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/audit-logs` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/admin/feature-flags` | ApplicationLayout + AdminShell | 布局矩阵通过 |
| `/help` | ApplicationLayout | 布局矩阵通过 |
| `/help/write` | ApplicationLayout | 布局矩阵通过 |
| `/help/faq` | PublicLayout | 布局矩阵通过 |
| `/help/compare` | PublicLayout | 布局矩阵通过 |
| `/help/api-access` | PublicLayout | 布局矩阵通过 |
| `/help/api-access/agents` | PublicLayout | 布局矩阵通过 |
| `/integrations` | PublicLayout | 布局矩阵通过 |
| `/integrations/mcp` | PublicLayout | 布局矩阵通过 |
| `/integrations/claude-desktop` | PublicLayout | 布局矩阵通过 |
| `/integrations/cursor` | PublicLayout | 布局矩阵通过 |
| `/desktop` | PublicSiteHeader | 布局矩阵通过 |
| `/activate` | PublicLayout | 布局矩阵通过 |
| `/setup` | PublicLayout | 布局矩阵通过 |
| `/legal` | PublicSiteHeader | 布局矩阵通过 |
| `/legal/terms` | PublicSiteHeader | 布局矩阵通过 |
| `/legal/privacy` | PublicSiteHeader | 布局矩阵通过 |
| `/legal/licenses` | PublicSiteHeader | 布局矩阵通过 |
| `/legal/licenses/project` | PublicSiteHeader | 布局矩阵通过 |
| `/legal/licenses/third-party` | PublicSiteHeader | 布局矩阵通过 |
| `/en` | PublicLayout | 布局矩阵通过 |
| `/en/pricing` | PublicSiteHeader | 布局矩阵通过 |
| `/en/desktop` | PublicSiteHeader | 布局矩阵通过 |
| `/en/help/faq` | PublicLayout | 布局矩阵通过 |
| `/en/help/compare` | PublicLayout | 布局矩阵通过 |
| `/en/help/api-access` | PublicLayout | 布局矩阵通过 |
| `/en/help/api-access/agents` | PublicLayout | 布局矩阵通过 |
| `/en/legal` | PublicSiteHeader | 布局矩阵通过 |
| `/en/legal/terms` | PublicSiteHeader | 布局矩阵通过 |
| `/en/legal/privacy` | PublicSiteHeader | 布局矩阵通过 |
| `/en/legal/licenses` | PublicSiteHeader | 布局矩阵通过 |
| `/en/legal/licenses/project` | PublicSiteHeader | 布局矩阵通过 |
| `/en/legal/licenses/third-party` | PublicSiteHeader | 布局矩阵通过 |
| `/pricing` | PublicSiteHeader | 布局矩阵通过 |
| `/upgrade/paywall` | PublicLayout | 布局矩阵通过 |
| `/upgrade/success` | PublicLayout | 布局矩阵通过 |
| `/desktop/buy` | PublicLayout | 布局矩阵通过 |

重定向保留：`/` → 登录或聊天，`/activate` → `/desktop`，`/setup` → `/settings?tab=providers`，工作区 `/analyze` → 分享中心。

旧 ProductChrome / AppTopBar / ShareAccessMenu 已移除；PublicFooter 仅用于公共定价页。
