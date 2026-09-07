# PRODUCT_IA §5 checklist (E5.2)

| Shell | 落实 |
|---|---|
| Marketing chrome（pricing / desktop / legal） | `MarketingChrome`：定价 · 客户端 · 法律 · 语言 · 进入应用，active 态 |
| App top bar（chat / dashboard / workspace / settings） | `AppTopBar`：品牌→`/chat` · 分享组（访问/API/升级）· 通知 · 账户 |
| Chat shell Workspaces | 会话栏「工作区」区 + 「全部工作区」→ `/dashboard` |
| Dashboard 工具栏 | 「分享访问」+「客户端」 |
| 深层工具页 | analytics / share / usage / in-app help 使用 `ProductChrome`（App top bar + footer）；页内返回链保留为 breadcrumb |
| 公开 `/help/api-access*` | 仍用公开轻 chrome（IA 除外项） |
| 管理台 | 账户菜单探测后显示，不写入 nav-config |

壳内 in-app href 均来自 `SHELL_ENTRY_HREFS`，机械测试对齐 `nav-config.ts`。
