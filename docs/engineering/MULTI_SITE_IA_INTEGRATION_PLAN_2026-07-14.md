# 多站点 / 多页面信息架构与集成方案

**日期**: 2026-07-14  
**状态**: I0–I2 **landed**（2026-07-14：site-map、App 营销顶栏/页脚/帮助/定价交叉链、Landing+Why 家族导航、公网 deploy）  

**触发**: `/desktop` 页面与 NSIS 安装包已就绪，但用户从「网站」侧无法自然发现入口——**有页无链**。  
**关联**: [`STYLE_BASELINE.md`](../design/STYLE_BASELINE.md) §3.2、[`DESKTOP_WEB_DOWNLOAD_INSTALL_PLAN`](./DESKTOP_WEB_DOWNLOAD_INSTALL_PLAN_2026-07-14.md)、[`LOCAL_VPS_ALIGNMENT_PLAN`](./LOCAL_VPS_ALIGNMENT_PLAN_2026-07-14.md)、[`VISUAL_SYSTEM_AND_MULTI_SITE_UPGRADE_PLAN`](./VISUAL_SYSTEM_AND_MULTI_SITE_UPGRADE_PLAN_2026-07-10.md)

---

## 0. 问题一句话

> **功能面（Surface）已交付，发现图（Discovery Graph）未交付。**  
> 页面、域名、安装包各自存在，但**用户任务路径**上没有稳定、可重复的「从公网到目标面」的入口。

这不是 desktop 独有：任何「只写了路由、没写进导航与任务交接」的面都会复现。本方案把问题**泛化为多表面集成规范**，并以 desktop 为第一实例。

---

## 1. 现状拓扑（As-Is）

### 1.1 表面矩阵

| 表面 ID | 域名 / 宿主 | 主职责 | 运行形态 | 仓库 |
|---------|-------------|--------|----------|------|
| **hub** | `contextlm.top` / `www` | 品牌门户、产品家族索引 | 静态 export | `~/context-os-landing` |
| **app** | `app.contextlm.top` | SaaS 产品（登录后工作区） | Next standalone | monorepo `frontend_next` |
| **app-mkt** | 同 app 域 path | 营销/转化（pricing、desktop、legal） | 同 app | 同上 `(marketing)` |
| **auth** | 同 app 域 | 登录/注册/重置 | 同 app | `(auth)` |
| **blog** | `blog.contextlm.top` | 内容 / SEO | Ghost | 独立 |
| **why** | `whyimright.contextlm.top` | 趣味工具 | Next + Go | `~/whyiamright` |
| **canju** | `canju.contextlm.top` | 象棋工具 | 静态 + 引擎 | `~/cchess` |
| **desktop-client** | 本机 Tauri | 本地知识助手 | setup.exe | monorepo `desktop/` |
| **releases** | `app…/releases/desktop/*` | 安装包 CDN | nginx 静态 | VPS |

### 1.2 关键图（谁链到谁）

```text
                    ┌─────────────────┐
                    │  hub (landing)  │
                    │  contextlm.top  │
                    └────────┬────────┘
           应用/博客/Why/象棋│ CTA「进入应用」
         ┌─────────┬─────────┼─────────┬─────────┐
         ▼         ▼         ▼         ▼         ▼
       app:*     blog      why       canju    (无 desktop)
         │
         │  / → login | dashboard   （无 marketing home）
         │
    ┌────┴──────────────────────────────┐
    │  app 内 footer: 品牌·仪表盘·帮助  │
    │  ·定价·法律                       │  ← 无 /desktop
    │  login: 无桌面入口                │
    │  pricing: 无桌面档位入口          │
    └───────────────────────────────────┘

    /desktop + /desktop/buy + NSIS     ← 孤立：仅直链 / 书签可达
    客户端内 buy/help 仍有 app.avrag.com 残留
```

### 1.3 用户任务 vs 可达性（desktop 实例）

| 用户任务 | 期望路径 | 现状 |
|----------|----------|------|
| 「我想下 Windows 客户端」 | hub 或 app 导航 → `/desktop` → setup | **断**：家族导航无 Desktop |
| 「先逛官网再装」 | landing CTA → desktop | **断**：CTA 只到 app 根（再变 login） |
| 「已登录 SaaS，想装桌面」 | footer/settings → desktop | **断**：footer 无链 |
| 「买授权」 | `/desktop/buy` | 页存在；**发现断** |
| 「装完激活」 | deep link / buy | 有；依赖已装客户端 |
| 「从 why/canju 回主产品」 | 家族顶栏 | 有；**无 desktop 列** |

### 1.4 根因分层

| 层 | 问题 |
|----|------|
| **IA** | 家族导航只列「域名型产品」，未列「交付通道型」表面（Desktop = 交付形态，不是新域名） |
| **域策略** | `app` 根是**产品闸门**（/ → login/dashboard），不是**营销大厅**；营销页挂在 path 上却无 chrome 索引 |
| **任务交接** | 跨表面缺统一 `return` / `next` / UTM / 主 CTA 层级 |
| **配置漂移** | 客户端深链 `app.avrag.com` vs 生产 `app.contextlm.top` |
| **规范未落地** | `STYLE_BASELINE` §3.2 有家族导航示意，**未包含 Desktop，也未强制各仓同源配置** |

---

## 2. 问题泛化（General Problem）

### 2.1 定义：孤儿表面（Orphan Surface）

满足任一即可判为孤儿：

1. 路由/域名 **HTTP 200**，但在 **Hub 导航、产品 chrome、任务 CTA** 三处均不可达；或  
2. 仅被「同面内部链接」引用（如 `/desktop` ↔ `/desktop/buy`），无跨面入口；或  
3. 外链目标 **hostname 错误 / 过期**，导致任务在交接处 404 或进错产品。

### 2.2 反模式（应禁止）

| 反模式 | 表现 | 后果 |
|--------|------|------|
| **Build-only shipping** | 只做页/包/API，不做发现 | 用户找不到 |
| **Nav dump** | 顶栏塞满所有 URL | 认知过载，主任务被稀释 |
| **Auth-wall as home** | 公域 CTA 直达需登录根路径 | 未登录用户被吓退 |
| **Inconsistent family chrome** | 各仓手写不同链接表 | 品牌分裂、漏链 |
| **Deep link without install path** | 激活链完善、下载链缺失 | 转化漏斗断在中段 |
| **Same chrome for all modes** | 已登录工作区顶栏塞营销项 | 干扰生产任务 |

### 2.3 设计原则（人体工学 / UX）

对齐常见实践（Nielsen 可学习性、Krug「别让我思考」、任务导向 IA、跨设备 handoff）：

| # | 原则 | 操作含义 |
|---|------|----------|
| P1 | **任务优先于站点拓扑** | 导航按「我要做什么」组织，不按仓库目录 |
| P2 | **两级发现** | L1 家族/产品；L2 交付形态（Web / Desktop）与转化（定价/登录） |
| P3 | **枢纽清晰** | Hub 负责「选产品」；App 负责「用产品」；Marketing path 负责「买/装」 |
| P4 | **3±1 顶栏主链** | 公域顶栏主链 ≤4；更多进「更多 / 页脚」 |
| P5 | **情境相关入口** | 已登录：设置/帮助/页脚；未登录：hub + 营销 chrome |
| P6 | **稳定交接契约** | 跨域跳转带 `next`/`return`/`utm_source`；深链 hostname 单一配置 |
| P7 | **当前面高亮 + 退出路径** | 每面知道「我在哪」与「回 hub / 回 app」 |
| P8 | **孤儿门禁** | 新表面合并前必须登记发现矩阵（§4），CI/清单可勾选 |

---

## 3. 目标拓扑（To-Be）

### 3.1 角色分层

```text
L0  品牌枢纽 Hub          contextlm.top
      │  选：应用(SaaS) | 桌面客户端 | 博客 | 工具集
      ▼
L1a SaaS 产品 App         app.contextlm.top  （登录后）
L1b 交付 / 转化 App-Mkt   app…/desktop|/pricing|/legal|/login
L1c 内容 Blog / 工具 Why·Canju
L2  客户端 Desktop        安装包 + 本机壳
```

### 3.2 主用户路径（Happy paths）

**A. 获取桌面客户端（主修复）**

```text
Hub「下载桌面版」或家族「应用 ▾ → 桌面客户端」
  → https://app.contextlm.top/desktop
  → 下载 NSIS setup
  → （可选）/desktop/buy 购授权
  → 本机激活
```

**B. 使用云端 SaaS**

```text
Hub「进入应用」→ app.contextlm.top/login?next=/dashboard
  → 工作区
```

**C. 已登录用户装桌面**

```text
App 页脚 / 帮助 / 设置「桌面客户端」→ /desktop
```

**D. 工具站回流**

```text
Why/Canju 顶栏 → Hub 或 App（不变）
页脚可有一行「桌面客户端」弱链（可选，非 L1）
```

### 3.3 导航信息架构（推荐）

#### Hub + 公域家族顶栏（Landing / Why / Canju / Blog 主题）

| 槽位 | 文案 | 目标 | 说明 |
|------|------|------|------|
| Brand | Context-OS | hub | 唯一 mark |
| 1 | 应用 | `app…/login` 或 `app` 营销总述 | SaaS 入口；**勿**在未登录时只链 `/` 若会硬跳 dashboard 失败——应用 `login?next=/dashboard` |
| 2 | **桌面** | `app…/desktop` | **新增**；交付形态 |
| 3 | 博客 | blog | 内容 |
| 4 | 工具 ▾ 或 Why + 象棋 | why / canju | 可折叠以控数量 |
| CTA | 进入应用 | login | 主按钮 |

> 若顶栏超载：工具收入「更多」；**桌面保留 L1**（当前业务重点）。

#### App 营销 chrome（`/desktop`、`/pricing`、`/legal` 等未登录也可访问页）

轻顶栏（新建共享组件，避免与 workspace 顶栏混用）：

```text
[Mark→Hub]  定价  桌面  登录/注册     [进入应用]
```

#### App 产品 chrome（已登录 dashboard/settings）

- **不**把 Desktop 塞进主工作顶栏。  
- **必须**出现在：`ProductChromeFooter`、`/help` 一节、可选 Settings「关于 / 更多产品」。

### 3.4 单一真相：链接注册表

禁止各仓硬编码散落 hostname。建议 monorepo 维护：

```text
packages/cos-site-map/   或   frontend_next/lib/site-map.ts
  + 导出 JSON 供 landing/why 复制或构建时注入
```

最小 schema：

```ts
type SiteLink = {
  id: string;           // "desktop" | "app_login" | "hub" | ...
  label: { zh: string; en: string };
  href: string;         // absolute when cross-origin
  surface: "hub" | "app" | "blog" | "tool" | "desktop_mkt";
  discovery: Array<"family_nav" | "hub_cta" | "app_footer" | "help" | "pricing">;
  auth: "any" | "logged_out" | "logged_in";
};
```

**门禁**：`discovery` 非空才允许标为「已发布表面」。

---

## 4. 发现矩阵（DoD 模板）

每个表面上线前填表（desktop 示例已填）：

| 表面 | family_nav | hub_cta | app_footer | help | pricing | login_adjacent | 深链 hostname |
|------|:----------:|:-------:|:----------:|:----:|:-------:|:--------------:|:-------------:|
| `/desktop` | ❌→✅ | ❌→✅ | ❌→✅ | ❌→✅ | ❌→✅ 弱 | 可选 | 修 avrag.com |
| `/pricing` | 部分 footer | 可选 | ✅ | 可选 | — | 可选 | OK |
| `/legal` | footer | — | ✅ | — | — | — | OK |
| why | ✅ | ✅ | — | — | — | — | OK |
| canju | ✅ | ✅ | — | — | — | — | OK |

**规则**：主转化表面（desktop / pricing）在 **family_nav ∪ hub_cta ∪ app_footer** 中至少命中 **2** 格。

---

## 5. 实施波次（开发方案）

### Wave I0 — 契约与规范（0.5–1d）

| # | 任务 | 产出 |
|---|------|------|
| I0-1 | 落地 `site-map` 模块（URL + label + discovery） | `lib/site-map.ts`（或 package） |
| I0-2 | 更新 `STYLE_BASELINE` §3.2：加入 **桌面** 槽位与两级发现 | md |
| I0-3 | 本文件挂到 AGENTS / SOLO 一句「新表面须过发现矩阵」 | 可选一句 |
| I0-4 | 清单：替换客户端内 `app.avrag.com` → 配置化 `APP_PUBLIC_ORIGIN` | 代码搜索清单 |

### Wave I1 — App 域发现（P0，优先）

| # | 任务 | 验收 |
|---|------|------|
| I1-1 | `ProductChromeFooter` 增加「桌面客户端」→ `/desktop` | 已登录可见可点 |
| I1-2 | `/help` 增加 Desktop 一节 + 链到 `/desktop`、`/desktop/buy` | 帮助可达 |
| I1-3 | 营销页共享顶栏（desktop/pricing/legal）：Hub · 定价 · 桌面 · 登录 | 未登录可逛 `/desktop` |
| I1-4 | `/pricing` 增加「需要本地客户端？」→ `/desktop` 卡片/一行 | 交叉转化 |
| I1-5 | （可选）login 页脚弱链「下载桌面版」 | 未登录发现 |
| I1-6 | `deploy-frontend` 发布 app | 公网验证 |

### Wave I2 — Hub 与家族导航（P0）

| # | 任务 | 验收 |
|---|------|------|
| I2-1 | Landing Navbar + Footer + Hero：桌面入口 | hub 一点到 `/desktop` |
| I2-2 | Landing 主 CTA 分层：主「进入应用」/ 次「下载桌面」 | 双任务清晰 |
| I2-3 | Why 顶栏/页脚对齐同一 site-map（至少加 Desktop 或「更多」） | 一致 |
| I2-4 | Canju 页脚弱链（顶栏可不加） | 不抢象棋主任务 |
| I2-5 | `deploy-public-sites.sh landing why` | 公网 |

### Wave I3 — 交接与卫生（P1）

| # | 任务 | 验收 |
|---|------|------|
| I3-1 | 统一 `NEXT_PUBLIC_APP_ORIGIN` / brand home；修 Tauri `openInBrowser` 目标 | 无 avrag.com |
| I3-2 | Hub→App 使用 `/login?next=/dashboard`；Hub→Desktop 绝对 URL | 无错误闸门 |
| I3-3 | 跨链 `utm_source=hub|why|footer`（可选分析） | 可统计 |
| I3-4 | `/desktop` 页自身加「回官网」「登录 SaaS」出口 | P7 |
| I3-5 | 文档：发现矩阵加入发版 checklist | 防回归 |

### Wave I4 — 增强（P2，非阻断）

| # | 任务 |
|---|------|
| I4-1 | Hub 产品卡片区：SaaS / Desktop / Tools 三卡 |
| I4-2 | App 设置「关于」：版本 + 桌面下载 |
| I4-3 | 中间页 `/download` → 302 + 统计（可选） |
| I4-4 | 站点地图 `sitemap.xml` 含 marketing paths |
| I4-5 | 视觉：营销顶栏与 hub 同 tokens（已有 cos-tokens 则复用） |

---

## 6. 组件落点（实现指引）

| 组件 | 位置 | 用途 |
|------|------|------|
| `site-map.ts` | `frontend_next/lib/` | 链接单一真相 |
| `MarketingChrome` | `frontend_next/components/` | marketing layout 顶栏 |
| `(marketing)/layout.tsx` | 若无则建 | 挂 MarketingChrome + 页脚 |
| Landing `Navbar`/`Footer`/`HeroEntries` | 卫星仓 | 读同一 JSON 或手写对齐表 |
| Why `unified-navbar` / `footer` | 卫星仓 | 同上 |
| `ProductChromeFooter` | 已有 | 加 desktop 链 |

**不做**：把 Landing 并进 monorepo 前端（除非另开迁移）；本方案允许**配置同源、仓可分**。

---

## 7. 验收标准

### 功能

- [ ] 从 `https://contextlm.top` **不输入 URL** 可点到 `/desktop` 并下载 setup  
- [ ] 从已登录 `app` 页脚可到 `/desktop`  
- [ ] `/desktop` 有返回 Hub / 进入 SaaS 的出口  
- [ ] `latest.json` format=nsis 下载成功  
- [ ] 全库无面向用户的 `app.avrag.com` 生产链接  

### 体验

- [ ] 公域顶栏主链 ≤4 项（工具可折叠）  
- [ ] SaaS 工作区顶栏**无**营销堆砌  
- [ ] Desktop 在发现矩阵 ≥2 格  

### 工程

- [ ] site-map 变更可 code review  
- [ ] 发版：app 用 `deploy-frontend`；landing/why 用 `deploy-public-sites`  

---

## 8. 决策默认

| # | 决策 | 默认 |
|---|------|------|
| 1 | Desktop 页面宿主 | 保留 `app.contextlm.top/desktop`（与授权/账号同域） |
| 2 | Desktop 是否独立子域 | 否（除非日后 SEO 强需求） |
| 3 | 家族顶栏是否显示 Desktop | **是**（L1） |
| 4 | 工作区顶栏是否显示 Desktop | **否**（footer/help） |
| 5 | Hub 主 CTA | 进入应用；次 CTA 下载桌面 |
| 6 | 链接真相 | site-map 模块；卫星仓构建时对齐 |
| 7 | 本方案 vs 视觉统一 | 互补：视觉 = tokens；本方案 = **路径与发现** |

---

## 9. 风险

| 风险 | 缓解 |
|------|------|
| 顶栏过挤 | 工具下拉；Desktop 仍 L1 |
| 未登录访问 `/desktop` 被中间件踢登录 | 确认 marketing 路由在 auth gate 之外 |
| 卫星仓与 monorepo 双改漏同步 | site-map JSON + 发版 checklist |
| 用户混淆 SaaS vs Desktop | 文案：「云端应用」vs「Windows 桌面客户端」 |

---

## 10. 建议实施顺序（一个工作单元）

```text
1. I0 site-map + STYLE_BASELINE 补桌面槽
2. I1 App footer/help/marketing chrome + deploy-frontend
3. I2 Landing（+ Why）导航 + deploy-public-sites
4. I3 深链 hostname 清理 + /desktop 出口
5. 手测三条 happy path A/B/C
```

**下一步**：确认后按 Wave I0→I1 开工（最小闭环：App 页脚 + Hub 导航 + 营销顶栏）。

---

## 附录 A — 当前硬编码入口速查

| 位置 | 现有链 | Desktop |
|------|--------|---------|
| Landing Navbar | 应用/博客/Why/象棋 + 进入应用 | 无 |
| Landing Footer | 同上 | 无 |
| Why Navbar/Footer | 应用/博客/象棋 | 无 |
| App `ProductChromeFooter` | 品牌/仪表盘/帮助/定价/法律 | 无 |
| App `/` | → login \| dashboard | 无 |
| `/desktop` 自链 | buy, help | 自洽 |
| Tauri openInBrowser | `app.avrag.com/...` | 错误域 |

## 附录 B — 与「只有网页」体感的关系

用户体感「只有网页」来自：

1. Hub/CTA 把所有流量导向 **App 登录闸门**；  
2. App 登录后进入 **工作区**，营销 path 不在主路径上；  
3. Desktop 是 **path 级表面**，未进入 **域名级家族导航** 心智模型。

修复不是再做一个站，而是：**在既有站的发现层登记 Desktop 为正式产品形态**，并遵守情境相关、任务优先的导航纪律。
