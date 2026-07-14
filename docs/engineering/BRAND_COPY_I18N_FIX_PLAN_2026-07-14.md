# 品牌壳 · 文案 · 多语言修复计划

**日期**: 2026-07-14  
**状态**: N1–N4 **landed**（2026-07-14：横排 lockup、客户端文案、去 AVRag 用户面、Client 产物名、营销/Hub/Why/Canju 英中切换）  

**依据截图**: `E:\OneDrive\桌面\16.png`（`/desktop` 营销顶栏 + 产品页）  
**相关**: [`BRAND_MARK_FAVICON_AUDIT`](./BRAND_MARK_FAVICON_AUDIT_2026-07-14.md)、[`MULTI_SITE_IA_INTEGRATION_PLAN`](./LOCAL_VPS_ALIGNMENT_PLAN_2026-07-14.md)（发现路径）、[`STYLE_BASELINE`](../design/STYLE_BASELINE.md)

---

## 1. 核实结论（As-Is）

### 1.1 Logo「顶着边 / 不好看」— **已证实**

截图中左上角是：

```text
  [Mark]          ← 方标单独一行
  Context-OS      ← 字在下方
```

**根因**（不是「字太长放不下」）：

| 层 | 问题 |
|----|------|
| CSS | `.app-auth-brand-link` 默认 **`flex-direction: column`**（登录卡竖排设计） |
| 误用 | 营销顶栏 `MarketingChrome` 品牌链 **复用了 `app-auth-brand-link`**，内联虽写 `align-items: center`，**竖排仍生效** → mark 与字上下叠 |
| 观感 | 竖叠后占高 ≈ 28px+字号，顶栏 56px 内显得顶边、拥挤；用户建议「左右排列」正是正确布局 |

**正确态**（登录卡除外）：

```text
[Mark] Context-OS     ← row + gap 8–12px，垂直居中
```

| 表面 | 当前 brand 布局 | 应改为 |
|------|-----------------|--------|
| App 营销 chrome（/desktop 等） | 竖叠（bug） | **横排** |
| App 登录 AuthFrame | 竖排（有意） | 保持竖排或横排二选一写进规范 |
| App 工作区顶栏 | 已横排 | 保持 |
| Landing / Why / Canju | 已横排 | 保持；统一 gap/尺寸 |

### 1.2 产品名「AVRag」— **用户面应退场**

| 位置 | 现状 | 用户可见？ |
|------|------|------------|
| `/desktop` 大标题 | **AVRag Desktop** | ✅ 是（截图中央） |
| `/desktop/buy`、激活页、授权页 | AVRag Desktop / 在 AVRag Desktop 中激活 | ✅ |
| `tauri.conf.json` `productName` / 窗口 title | AVRag Desktop | ✅ 安装后窗口标题、开始菜单名 |
| `package-desktop-release` 文件名 | `AVRag-Desktop_*.exe` | ⚠️ 下载文件名；改名要版本波次 |
| `latest.json` `product` | AVRag Desktop | ⚠️ 清单字段 |
| 代码/crate 名 avrag-rs、identifier `com.avrag.desktop` | 技术 id | ❌ 可不改（内部） |
| 品牌官网主标题 | 已是 Context-OS | ✅ OK |

**决策（本计划默认）**：

| 层 | 名称 |
|----|------|
| **用户可见产品名** | **Context-OS**；客户端全称 **Context-OS 客户端** / EN: **Context-OS Client**（或 Windows Client） |
| **短标签（导航）** | 中文：**客户端**；英文：**Client**（勿用「桌面」作导航主文案） |
| **技术/包名** | 本波可不改 `avrag-rs`、`com.avrag.desktop`；安装包文件名可波次 2 再改，避免断链 |

### 1.3 「桌面」文案 — **品牌导航应改为「客户端」**

| 位置 | 现中文 | 问题 | 建议中文 | 建议英文 |
|------|--------|------|----------|----------|
| Landing 顶栏 | 桌面 | 直译感 | **客户端** | Client |
| Landing 次链 | 下载桌面 | 同上 | **下载客户端** | Download client |
| Landing Hero | 桌面客户端 / 下载桌面版 | 「桌面」多余 | **客户端** / **下载客户端** | Client / Download client |
| Why / Canju 顶栏 | 桌面 | 同上 | **客户端** | Client |
| Canju 页脚 | 桌面客户端 | 可接受，可统一为「客户端」 | **客户端** | Client |
| site-map `desktop` | 桌面 | 同上 | **客户端** | Client |
| productChrome.desktop | 桌面客户端 | 偏长但可用 | **客户端** 或保留「桌面客户端」仅 App 内 | Desktop client / Client |
| 营销 chrome 导航 | 走 productChrome.desktop → 桌面客户端 | 截图「桌面客户端」 | 导航用短：**客户端** | Client |
| `/desktop` 页内 | 下载 Windows 版 | OK（指平台） | 可保留；标题改为 Context-OS 客户端 | Download for Windows |

路径 URL `/desktop`、`releases/desktop/` **本波不改**（避免破坏下载与深链）；只改**可见文案**。

### 1.4 英文版 — **缺口大**

| 表面 | 中文 | 英文 | 语言切换 |
|------|------|------|----------|
| **App（frontend_next）** | 消息字典较全 | 同字典 en 键较全 | ✅ 工作区内主题菜单可切 zh/en |
| **App 营销页** | 大量硬编码中文（/desktop 正文、buy 页） | 未走 i18n | ⚠️ 不随 locale 变 |
| **Landing** | 全站硬编码中文 | **无** | **无**；`lang=zh-CN` 写死 |
| **Why** | 硬编码中文 | **无** | **无** |
| **Canju** | 硬编码中文 | **无** | **无** |
| **Blog/Ghost** | 内容侧 | 取决于主题/文章 | 另案 |

**用户要求**：「品牌、各站点，都要有英文版本」→ 至少 **Hub + App 营销 + Why + Canju** 具备 en 文案与可切换入口（或 `?lang=` / 路径前缀 / cookie）。

---

## 2. 目标态（To-Be）

### 2.1 品牌壳布局

```text
Marketing / Family chrome:
  [Mark 28px]  Context-OS          ← flex-row, gap 8–12px, items-center
  右侧：定价 | 客户端 | … | 登录 | 进入应用

Auth card (optional keep column):
  [Mark 56px]
  品牌回首页
```

独立 class：`cos-brand-lockup`（横排），**禁止**营销壳复用 `app-auth-brand-link` 的 column。

### 2.2 命名表（对外）

| 场景 | zh | en |
|------|----|----|
| 品牌 | Context-OS | Context-OS |
| SaaS | 应用 / 云端应用 | App / Cloud app |
| 安装版产品 | Context-OS 客户端 | Context-OS Client |
| 导航短标签 | 客户端 | Client |
| 下载 CTA | 下载客户端 / 下载 Windows 客户端 | Download client / Download for Windows |
| 购买 | 购买客户端授权 | Buy client license |
| 禁止用户面 | AVRag、AVRag Desktop（除历史文件名过渡期） | same |

### 2.3 多语言最低标准

| 站 | 机制（推荐） | 切换入口 |
|----|--------------|----------|
| App | 现有 `useUiPreferences` + messages；营销页改走字典 | 营销顶栏增加 中/EN（未登录也可） |
| Landing | 轻量：`messages.{zh,en}.ts` + cookie/`?lang=` 或 `/en` 前缀 | 顶栏右 中文 \| EN |
| Why / Canju | 同 Landing 模式（共享文案包或各仓一份 map） | 顶栏右 中文 \| EN |
| URL | 波次 1 用 query/cookie 即可；波次 2 再考虑 `/en/...` SEO | — |

---

## 3. 实施波次

### Wave N0 — 决策冻结（0.5h，开工前勾选）

| # | 决策 | 默认 |
|---|------|------|
| N0-1 | 用户可见名 | Context-OS / Context-OS Client |
| N0-2 | 导航中文 | **客户端**（不用「桌面」） |
| N0-3 | 导航英文 | **Client**（不用 Desktop 作短标签；英文页长文可用 Desktop client） |
| N0-4 | URL `/desktop` | 本波保留 |
| N0-5 | 安装包文件名 AVRag-Desktop_*.exe | 波次 N2 随版本 bump 改为 `Context-OS-Client_*.exe` 或并行双名 |
| N0-6 | 英文落地范围 | Hub + App 营销 + Why + Canju（Blog 内容另案） |

### Wave N1 — 壳布局 + 中文文案纠偏（P0，1 日）

| # | 任务 | 文件/范围 | 验收 |
|---|------|-----------|------|
| N1-1 | 新增 `cos-brand-lockup` 横排样式；营销 chrome **去掉** `app-auth-brand-link` | `globals.css`、`marketing-chrome.tsx` | `/desktop` 顶栏 mark 与字左右排，垂直居中，不顶边 |
| N1-2 | 审计所有误用 `app-auth-brand-link` 作横导航的地方 | page-frame 仅 auth 保留 column | 无竖叠 brand |
| N1-3 | 文案：桌面→客户端（site-map、Landing Navbar/Footer/Hero、Why、Canju） | 卫星仓 + monorepo site-map | 顶栏无孤立「桌面」二字 |
| N1-4 | 用户面去 AVRag：/desktop 标题、buy、activate、licenses、i18n 句 | frontend_next | 截图中央为 **Context-OS 客户端** |
| N1-5 | productChrome / desktop messages 中英同步 | messages/*.ts | 中文客户端 / 英文 Client |
| N1-6 | `deploy-frontend` + landing/why/canju | scripts | 公网截图过关 |

### Wave N2 — 客户端产物命名（P1，与发版绑）

| # | 任务 | 验收 |
|---|------|------|
| N2-1 | `tauri.conf.json` productName / window title → Context-OS Client | 窗口标题正确 |
| N2-2 | package 脚本输出文件名 + latest.json product 字段 | 新版本下载名与清单一致 |
| N2-3 | 兼容：旧 URL 文件可保留一代；latest 只指新名 | 下载不 404 |
| N2-4 | 标识符 `com.avrag.desktop` | **不改**（系统注册稳定）除非另开迁移 |

### Wave N3 — 英文版（P0/P1，2–3 日）

| # | 任务 | 说明 |
|---|------|------|
| N3-1 | **App 营销页 i18n**：desktop/page、buy、硬编码段落进 messages | 随 locale 切换 |
| N3-2 | **营销顶栏** 语言切换（中 / EN） | 写 cookie，与工作区 locale 同源 |
| N3-3 | **Landing**：抽 `lib/copy.ts` 中英；Navbar/Hero/Footer/Features；`lang` 属性随切换 | 默认 zh，可 EN |
| N3-4 | **Why**：navbar/footer/主文案中英 map + 切换 | |
| N3-5 | **Canju**：chrome + 主 UI 关键串（模式/分析等）中英 | 棋盘术语可第二优先 |
| N3-6 | 文档：STYLE_BASELINE 增加「对外命名 + 导航用词」 | |
| N3-7 | 部署与冒烟：zh/en 各点一遍导航「客户端/Client」 | |

### Wave N4 — 硬化（P2）

| # | 任务 |
|---|------|
| N4-1 | 共享 `@cos/copy` 或 monorepo `packages/cos-site-copy` 供卫星仓引用 |
| N4-2 | 视觉回归：marketing chrome lockup 快照 |
| N4-3 | 搜索全库用户串 `AVRag Desktop` / `桌面` 导航义，CI 禁回归（allowlist 技术注释） |

---

## 4. 文案对照速查（改前 → 改后）

### 中文

| 改前 | 改后 |
|------|------|
| AVRag Desktop | Context-OS 客户端 |
| 在 AVRag Desktop 中激活 | 在 Context-OS 客户端中激活 |
| 桌面（导航） | 客户端 |
| 下载桌面 / 下载桌面版 | 下载客户端 |
| 桌面客户端（导航长） | 客户端（导航）；正文可用「Windows 客户端」 |
| 桌面端设置 | 客户端设置 |

### English

| 改前 | 改后 |
|------|------|
| AVRag Desktop | Context-OS Client |
| Desktop（nav short） | Client |
| Download for Windows | 可保留（平台明确）或 Download client |

---

## 5. 风险与范围边界

| 风险 | 处理 |
|------|------|
| 改安装包文件名导致旧文档/外链失效 | N2 与版本 bump 同步；保留旧文件 1 个大版本 |
| Why/Canju 全 UI 英文化工作量大 | N3 先 chrome + 首页关键 CTA，棋盘术语可二期 |
| 「Desktop」在英文技术圈常用 | 导航短标签用 Client；长文可写 “desktop client” |
| 法律/协议中的 AVRag | 单独法务核对，本计划不自动改协议正文 |

---

## 6. 验收清单

### 布局

- [ ] `/desktop` 顶栏：mark 与「Context-OS」**左右排列**，垂直居中，左右 padding 充足  
- [ ] 不再出现 mark 在上、字在下的竖叠（营销壳）  
- [ ] mark 不超出 56px 顶栏高度  

### 命名

- [ ] 用户可见主标题无 **AVRag**  
- [ ] 导航中文无孤立「桌面」；统一「客户端」  
- [ ] 英文导航为 Client  

### 多语言

- [ ] App 营销页随 locale 切换中/英  
- [ ] Landing / Why / Canju 可切到完整英文 chrome + 主 CTA  
- [ ] `html lang` 随语言更新  

---

## 7. 建议开工顺序（一个最小闭环）

```text
1. N1-1/N1-2  营销顶栏横排 lockup          → 立刻改善截图问题
2. N1-3/N1-4/N1-5  客户端文案 + 去 AVRag   → 品牌感
3. N1-6  部署 App + 公域站
4. N3-1/N3-2  App 营销 i18n + 顶栏语言切换
5. N3-3~N3-5  Landing / Why / Canju 英文
6. N2  随下次客户端发版改 productName/文件名
```

**下一步**：确认 N0 默认后，从 **N1（横排 + 客户端文案 + 去 AVRag 用户面）** 开工。

---

## 附录 A — 截图问题与修复一一对应

| 截图现象 | 修复项 |
|----------|--------|
| Logo 顶边、拥挤 | N1-1 横排 lockup，勿 column |
| 大标题 AVRag Desktop | N1-4 → Context-OS 客户端 |
| 导航「桌面客户端」 | N1-3 短标「客户端」 |
| 安装步骤仍写 AVRag | N1-4 i18n |
| 整页仅中文 | N3 |

## 附录 B — 关键代码锚点

| 主题 | 路径 |
|------|------|
| 竖排元凶 | `frontend_next/app/globals.css` `.app-auth-brand-link` |
| 误用处 | `frontend_next/components/marketing-chrome.tsx` |
| AVRag 标题 | `app/(marketing)/desktop/page.tsx`、`buy/page.tsx`、activate、licenses |
| 导航「桌面」 | Landing Navbar/Footer/Hero；Why navbar/footer；Canju App；`lib/site-map.ts` |
| 产品名配置 | `desktop/src-tauri/tauri.conf.json`、`scripts/package-desktop-release.sh` |
| App 已有 i18n | `lib/i18n/messages/*`、`useUiPreferences` |
| 卫星站无 i18n | `context-os-landing`、`whyiamright`、`cchess` |
