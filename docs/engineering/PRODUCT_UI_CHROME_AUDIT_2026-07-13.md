# Product UI Chrome 系统审计（Dashboard / Workspace / Settings）

**日期**: 2026-07-13  
**范围**: `frontend_next` 主产品面 + 账单门户 + 法律/帮助/品牌导航  
**状态**: 审计 + 部分落地；**执行计划**见 [PRODUCT_UI_CHROME_AND_BILLING_DEV_PLAN_2026-07-13.md](./PRODUCT_UI_CHROME_AND_BILLING_DEV_PLAN_2026-07-13.md)  
**关联**: [STRIPE_BILLING_REMOVAL_2026-07-13.md](./STRIPE_BILLING_REMOVAL_2026-07-13.md)、[VISUAL_SYSTEM_AND_MULTI_SITE_UPGRADE_PLAN_2026-07-10.md](./VISUAL_SYSTEM_AND_MULTI_SITE_UPGRADE_PLAN_2026-07-10.md)、AGENTS.md §8（`workspace` sole truth）

---

## 0. 一句话结论

| 议题 | 结论 |
|------|------|
| 同图标不同名/功能 | **系统性问题**，不限于账户按钮：`+` 与 **用户图标** 均多义 |
| 管理订阅无弹页 | **Stripe 已产品弃用**（见 [STRIPE_BILLING_REMOVAL_2026-07-13.md](./STRIPE_BILLING_REMOVAL_2026-07-13.md)）；管理订阅 = **应用内方案列表 + `/pricing`**（Creem/支付宝） |
| 法律/文档/开源 | 页面**已有**（`/legal/*`、`/help`），但 **Dashboard/Workspace 无常驻入口** |
| 品牌主页 | 营销站 `contextlm.top` 与 App 脱节；App `/` 仅跳登录/Dashboard |
| 产品词 | 禁止用户可见「笔记本」；统一 **工作区 / workspace** |

---

## 1. 触发样本：账户按钮（截图 9.png）

| 面 | 文案 | 交互 | 问题 |
|----|------|------|------|
| Dashboard | `dashboardAccountLink` → **账户** | `Link` → `/settings?tab=profile` | 无登出 |
| Workspace | `dashboardProfileLink` → **账号信息** | 菜单：资料 + 登出 | 同图标、不同名、半能力 |

**目标态**: 全局 **账户菜单**（文案统一「账户」、含资料/账单/登出）。

---

## 2. 「+」多义（更严重）

| 位置 | 文案 | 创建对象 |
|------|------|----------|
| Dashboard 工具栏/卡片 | 新建工作区 | Workspace |
| Workspace 顶栏 | ~~新建笔记本~~ → **新建工作区**（已修硬编码） | Workspace |
| History | 新建会话 | Session |
| Sources | 添加内容源 | Source |
| Notes | 新建笔记 | Note |

**原则**: 一层上下文一个实心「创建」；全局顶栏禁止裸 `+` 无标签。

---

## 3. 动作 Taxonomy（推荐）

| ID | 语义 | 图标 | 文案 zh/en | 放置 |
|----|------|------|------------|------|
| G-CREATE-WS | 新建工作区 | `+` / folder-plus | 新建工作区 / New workspace | **仅 Dashboard 主 CTA**；Workspace 降权 |
| C-CREATE-SESSION | 新建会话 | `+`+文字 | 新建会话 | 仅左 History |
| C-ADD-SOURCE | 添加资料 | `+` 或 file-plus | 添加资料 | 右栏 Sources |
| C-CREATE-NOTE | 新建笔记 | `+` 或 note-plus | 新建笔记 | 右栏 Notes |
| G-ACCOUNT | 账户菜单 | user | **账户** / Account | 全局顶栏统一组件 |
| G-APPEARANCE | 主题语言 | theme | 外观 | 顶栏快捷 **或** 仅 Settings |
| W-SHARE | 分享 | share | **分享**（弃用默认「传播」） | Workspace 次要 |
| W-API | API | code | API | Workspace 次要 |
| W-ANALYZE | 分析 | chart | 分析 | 有路由则挂；否则删死文案 |

### 放置图

```
Global Chrome: Brand(官网) | Title | … | Share API Analyze? | Appearance | Account▾
  Dashboard 独有主 CTA: [+ 新建工作区]
  Workspace: 不把「再建工作区」做成实心主按钮

History: [+ 新建会话] [搜索]
Rail:    Sources [+ 添加资料]  Notes [+ 新建笔记]
Composer: 模式 | 输入 | 发送
Footer (产品页): 品牌官网 · 工作台 · 帮助 · 定价 · 法律中心 · 协议 · 隐私 · 开源
```

---

## 4. 管理订阅（诊断 + 落地）

### 4.1 根因（历史）

点击「管理订阅」曾依赖外部 Customer Portal；后端一度 stub，且产品已 **弃用 Stripe**，Creem/支付宝亦无对等门户 → 用户感知为「没弹出」。

### 4.2 目标行为（现行）

1. **不使用**外部账单门户（Stripe 已删除）。  
2. 「管理订阅」→ **应用内方案列表**。  
3. 「更换方案」→ `/pricing`（Creem / 支付宝结账）。  

详见 **[STRIPE_BILLING_REMOVAL_2026-07-13.md](./STRIPE_BILLING_REMOVAL_2026-07-13.md)**。

### 4.3 代码

| 文件 | 变更 |
|------|------|
| `settings-billing-panel.tsx` | 管理订阅展开方案列表；更换方案 → `/pricing` |
| `billing` crate | 无 Stripe client；portal API 固定 unavailable |

### 4.4 后续

- Creem merchant 自助链接若产品需要，单独 ADR，**不得**回退 Stripe。  

---

## 5. 法律 / 帮助 / 开源 / 品牌（诊断 + 落地）

### 5.1 已有路由（代码存在）

| 路径 | 内容 |
|------|------|
| `/legal` | 法律中心 |
| `/legal/terms` | 用户协议 |
| `/legal/privacy` | 隐私政策 |
| `/legal/licenses` (+ project / third-party) | 开源声明 |
| `/help`, `/help/api-access`, `/help/write` | 产品帮助 |
| `/pricing` | 定价 |
| 营销站 | `https://www.contextlm.top`（多站点计划） |

### 5.2 缺口

| 缺口 | 说明 |
|------|------|
| Dashboard / Settings 无页脚 | `LegalFooterLinks` 仅 marketing/pricing/登录链 |
| Workspace 无法律入口 | 顶栏无 help/legal |
| 品牌主页 | Dashboard 品牌不可点官网；`/` 只做 auth 跳转 |
| 产品文档 | Help 是应用内 FAQ，非独立 docs 站；需在页脚标明「产品帮助」 |

### 5.3 已改代码

| 文件 | 变更 |
|------|------|
| `components/product-chrome-footer.tsx` | 统一页脚：官网 / 工作台 / 帮助 / 定价 / 法律 / 协议 / 隐私 / 开源 |
| Dashboard / Settings | 挂载 `ProductChromeFooter` |
| `dashboard-header.tsx` | Mark → 品牌官网（`NEXT_PUBLIC_BRAND_HOME_URL` 或默认 `https://www.contextlm.top`）；标题 → `/dashboard`；品牌名 **Context-OS** |
| `workspace-top-bar.tsx` | 消灭「新建笔记本」硬编码 → `workspaceCreateDialogLabel` |

### 5.4 配置

```bash
# frontend_next .env — 可选覆盖品牌官网
NEXT_PUBLIC_BRAND_HOME_URL=https://www.contextlm.top
```

---

## 6. 优先修复清单

### P0

| # | 项 | 状态 |
|---|----|------|
| 1 | 顶栏「新建笔记本」→ 工作区 | **Done** |
| 2 | 账户文案/交互统一 | Open（菜单组件） |
| 3 | 管理订阅 = 应用内方案 + `/pricing`（无 Stripe） | **Done** |

### P1

| # | 项 | 状态 |
|---|----|------|
| 4 | Workspace 顶栏「+」降权 | Open |
| 5 | Dashboard/Settings 页脚法律/帮助 | **Done** |
| 6 | 品牌官网入口 | **Done**（header + footer） |
| 7 | Workspace 挂页脚或账户菜单含法律 | Open |
| 8 | API Access / 硬编码中文 | Open |

### P2

- 「传播」→「分享」  
- Analyze 入口或删文案  
- notebook testid 清理  
- Admin/e2e 残留  

---

## 7. 验证建议

1. **Settings → 账单 → 管理订阅**  
   - 展开应用内方案列表（**不**跳外部门户）。  
   - 「更换方案」进入 `/pricing`。  
2. **Dashboard 底脚**：法律中心 / 帮助 / 开源可点。  
3. **Dashboard Logo**：外链品牌站；标题回工作台。  
4. **Workspace 顶栏**：「新建工作区」文案（非笔记本）。

---

## 8. 关键文件

| 角色 | 路径 |
|------|------|
| 本审计 | `docs/engineering/PRODUCT_UI_CHROME_AUDIT_2026-07-13.md` |
| 账单面板 | `frontend_next/components/settings/settings-billing-panel.tsx` |
| Portal 后端 | `avrag-rs/crates/billing/src/service.rs` / `handlers.rs` |
| 产品页脚 | `frontend_next/components/product-chrome-footer.tsx` |
| Dashboard 顶栏 | `frontend_next/components/dashboard/parts/dashboard-header.tsx` |
| Workspace 顶栏 | `frontend_next/components/workspace/workspace-top-bar.tsx` |
