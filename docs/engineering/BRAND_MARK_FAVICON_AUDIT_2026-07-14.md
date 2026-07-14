# 品牌 Mark / 标签页 Icon 全站审计清单

**日期**: 2026-07-14  
**状态**: Audit only（问题清单；修复另波）  
**触发**: 品牌官网未用规范 SVG LOGO；部分页无 tab icon；`/desktop` 等页 mark 比例溢出框体  
**规范源**: [`docs/design/STYLE_BASELINE.md`](../design/STYLE_BASELINE.md) §3.1 — 唯一 mark = **ContextOsMark**（双弧 + 规范填色）  
**关联**: 多站 IA 计划、视觉基准 Monochrome Ink

---

## 0. 问题泛化（根因模型）

| 根因 ID | 名称 | 表现 | 正确做法 |
|---------|------|------|----------|
| **R1 Glyph Drift** | 字形漂移 | 各仓手写「简化双弧」与 React 全量 mark（含中轴/圆点）并存 | **单一 SVG 源**（`public/brand/context-os-mark.svg` 或 monorepo 规范文件）→ 各表面只引用 |
| **R2 Hardcoded Geometry** | 硬编码几何 | `width="90" height="90"` 写死在组件上，与 CSS 容器抢尺寸 | 组件 **不设默认像素宽高**，或 `width/height=100%` + 容器 CSS 控尺寸 |
| **R3 Context-blind Stroke** | 描边不随主题 | path `stroke="white"` 固定；浅色顶栏上 fill 深色时笔画仍白 → 糊/脏 | 笔画用 `currentColor` 反相策略，或 **双 token**（plate / ink） |
| **R4 Slot Overflow** | 槽位溢出 | 顶栏 56px 槽内塞 80–90px mark | 槽位表：nav 24–28px、auth 40–48px、hero 64–80px；**CSS 强制 max** |
| **R5 Missing Favicon Contract** | 无 favicon 契约 | layout 未声明 `icons` / 无 `icon.svg` / 无 `favicon.ico` | 每站强制：`icon.svg`（SVG）+ optional `apple-touch-icon` + HTML link |
| **R6 Dead Asset Path** | 死链资源 | `apple-icon` 301 → 不存在的 png | 审计 200；生成真实文件 |
| **R7 MetadataBase Wrong** | OG/绝对 URL 错 | `metadataBase` 指向 localhost | 生产 `APP_ORIGIN` |
| **R8 Inline Duplicate** | 内联复制 | Navbar/Footer 各自粘贴 SVG path | 共享组件或 `<img src="/brand/...">` |

---

## 1. 规范 vs 实际（字形三态）

当前至少 **三套** 图形在跑：

| 变体 | 内容 | 出现位置 |
|------|------|----------|
| **A. Full mark** | 双弧 + 中轴 + 横线 + 圆点 | `frontend_next/components/context-os-mark.tsx`、`app/icon.svg` |
| **B. Dual-arc only** | 仅双弧，无中轴圆点 | `public/brand/context-os-mark.svg`（app/landing/why/canju 拷贝）、Landing/Why/Canju 内联顶栏 |
| **C. Dual-arc inverted plate** | 深底 `#171717` + 浅弧 | Why `icon.svg`、部分 favicon 意图 |

**决策待确认（修复前必须定）**：

- [ ] **D1** 品牌唯一字形 = A Full 还是 B Dual-arc？（STYLE_BASELINE 写「双弧」但组件是 Full）  
- [ ] **D2** 浅色/深色底的 plate 色：浅底深标 vs 深底浅标（两套 asset 还是 CSS `currentColor`）

---

## 2. 表面 × Mark × Favicon 矩阵

| 表面 | 域名 / 路径 | 页内 Mark | 与规范一致？ | 尺寸/溢出 | Tab icon | 备注 |
|------|-------------|-----------|--------------|-----------|----------|------|
| **Hub 官网** | contextlm.top | 内联 **B**（Navbar 28×28 浅底深弧） | ❌ 未用 Full A；未读 `public/brand` 文件 | 顶栏 OK | ❌ **无** `icons` metadata；`/favicon.ico` **404** | 用户点名：官网未用品牌 SVG |
| Hub 静态 brand | `/brand/context-os-mark.svg` | 文件存在 **B** | 半成品 | — | 未挂到 layout | 资源在、未接线 |
| **App 营销 chrome** | /desktop /pricing /legal | `ContextOsMark` **A** + class `app-auth-mark` | 字形 A | ❌ **严重溢出**：header **3.5rem(56px)**，mark CSS **5rem(80px)**，SVG 属性 **90×90** | ✅ `/icon.svg` | 用户点名 desktop logo 出框 |
| **App 登录** | /login | Full A + `app-auth-mark` 5rem | 字形 OK | 卡片内 5rem 可接受 | ✅ icon.svg | stroke 固定白：浅主题时依赖 fill 深色 + 白笔，可能对比怪异 |
| **App 激活** | /activate (Tauri) | Full A inline style **4rem** | 字形 A | 居中大标 OK | 继承 app | — |
| **App 工作区顶栏** | /dashboard/* | Full A + `.topBarMark` ~ brand×2.4 | 字形 A | 一般 OK（需 CSS 覆盖硬编码 90） | ✅ | `width=90` 若 CSS 未 `!important`/未覆盖可能撑开 |
| **App Dashboard header** | dashboard | Full A + `.dashboard-brand-mark` | 字形 A | 查 globals ~ brand 比例 | ✅ | — |
| **App 页脚** | product chrome | 无 mark，仅文字链 | — | — | — | 可选小标 |
| **Why** | whyimright… | 顶栏内联 **B** 24px；icon **C** dual-arc dark plate | ❌ 非 Full A；与 app 不一致 | 顶栏 OK | ✅ `/icon.svg`；favicon.ico 404（有 svg 可接受） | — |
| **Canju** | canju… | 顶栏内联 **B** 24px 深底浅弧 | ❌ | OK | ⚠️ 线上 favicon.ico 200 但源仓 **无** public/favicon.ico（构建产物或旧文件） | 页脚无独立 mark 文件引用 |
| **Blog/Ghost** | blog… | Ghost 主题 | 未审计 | — | ✅ favicon.ico 声明 | 主题是否 ContextOsMark 未知 |
| **Desktop 安装包** | NSIS / 壳 | Tauri icons 目录 | 独立 ICO | — | 壳图标 | 应与 Full A 导出一致 |

---

## 3. 尺寸 / 溢出问题细项（R2+R4）

| # | 位置 | 证据 | 影响 |
|---|------|------|------|
| S1 | `ContextOsMark` 默认 `width={90} height={90}` | 组件源码 | 任何未覆盖的 class 即 90px 大方块 |
| S2 | `/desktop` 营销顶栏 | header `height: 3.5rem` + `.app-auth-mark { width/height: 5rem }` + SVG 90 | **logo 超出顶栏框**（用户截图现象） |
| S3 | 营销顶栏误用 auth 样式 | `className="app-auth-mark"` 为登录卡设计（大标） | 槽位语义错用 |
| S4 | 工作区 `.topBarMark` 设宽高 | 依赖 CSS 压过 90 | 若 CSS 失效则溢出 |
| S5 | activate `style={{ width: "4rem", height: "4rem" }}` | 内联 | OK，但组件仍应无硬编码 |

**槽位建议（修复标准）**

| 槽位 | 显示尺寸 | CSS class 建议 |
|------|----------|----------------|
| Family / Marketing nav | 24–28px | `cos-mark--nav` |
| App workspace top bar | 28–32px | `cos-mark--shell` |
| Auth card | 48–64px | `cos-mark--auth` |
| Favicon / apple | 32 / 180 | 静态文件 |
| Hero / 空状态 | 64–96px | `cos-mark--hero` |

---

## 4. Favicon / 标签页清单（R5/R6）

| 站 | 期望 | 实测 | 优先级 |
|----|------|------|--------|
| contextlm.top | icons in layout + `/icon.svg` or `/favicon.ico` | layout **无 icons**；`/favicon.ico` **404**；brand svg 200 未挂 | **P0** |
| app.contextlm.top | icon.svg + apple | icon.svg **200**；apple-icon **301** → `/apple-icon.png`（桌面导出兼容桩，**需确认 png 是否存在**） | P1 |
| whyimright | icon.svg | **200**（简化 dual-arc） | P1 对齐字形 |
| canju | favicon | 线上 200；源码 public 需核对 | P1 |
| blog | Ghost favicon | 有声明 | P2 对齐品牌 |
| app 生产 OG | absolute app origin | HTML 中出现 `localhost:3000/opengraph-image`（**metadataBase 错**） | **P0** SEO/分享 |

---

## 5. 填色 / 对比度清单（R3）

| # | 问题 | 位置 |
|---|------|------|
| C1 | Full mark 笔画 **写死 white**，plate `currentColor` | ContextOsMark：浅色顶栏 plate=深色时白笔 OK；若 plate 浅则白笔不可见 |
| C2 | Landing 浅灰 plate `#f5f5f5` + 深弧 | 暗站 OK；与 app Full mark 视觉不一致 |
| C3 | Canju 顶栏深 plate `#171717` + 浅弧 | 另一套反相 |
| C4 | brand svg `stroke="#fff"` 固定 | 作 img 时不可随主题 |

---

## 6. 工程 / 源文件清单

| 文件 | 问题 |
|------|------|
| `frontend_next/components/context-os-mark.tsx` | 硬编码 90；笔画白；Full 字形 |
| `frontend_next/public/brand/context-os-mark.svg` | Dual-arc only；与组件不一致 |
| `frontend_next/app/icon.svg` | Full；与 brand svg 不一致 |
| `frontend_next/app/apple-icon.tsx` | 301 到可能不存在的 png |
| `frontend_next/components/marketing-chrome.tsx` | 误用 `app-auth-mark` 大尺寸 |
| `frontend_next/app/globals.css` `.app-auth-mark` | 5rem 仅适 auth |
| `context-os-landing/app/layout.tsx` | 无 icons |
| `context-os-landing/app/sections/Navbar.tsx` | 内联简化 mark，未用 brand 文件 |
| `whyiamright/.../unified-navbar.tsx` | 本地复制 ContextOsMark 简化版 |
| `cchess/frontend/src/App.tsx` | 内联简化 mark |
| `*/public/brand/context-os-mark.svg` ×4 | 四份拷贝易漂移 |

---

## 7. 验收检查表（修复后勾选）

### 字形

- [ ] 全站页内 mark 与 favicon **同一 path 集合**（A 或 B 二选一，已定稿）  
- [ ] 无「仅双弧」与「Full」混用（除非文档声明 favicon 可简化）  
- [ ] monorepo `public/brand/context-os-mark.svg` = 唯一源；卫星仓同步或 CDN  

### 尺寸

- [ ] `ContextOsMark` 无硬编码 90；尺寸仅由 class/slot 控制  
- [ ] 营销顶栏 mark ≤ 28px，不超出 56px header  
- [ ] 登录 auth mark 48–64px，不撑破卡片  
- [ ] 工作区 top bar mark ≤ 32px  

### Favicon

- [ ] hub / app / why / canju：`link rel=icon` 可访问 **200**  
- [ ] hub 不再依赖默认 `/favicon.ico` 404  
- [ ] apple-touch 指向真实 png/svg，无空 301  

### 主题

- [ ] 浅色/深色底对比度 WCAG 可读  
- [ ] 笔画与 plate 使用 token，无死白死黑（除固定 favicon 文件）  

### 元数据

- [ ] 生产 HTML 中 OG/twitter 图 **不含 localhost**  
- [ ] `metadataBase` = `https://app.contextlm.top`（或 env）  

---

## 8. 建议修复波次（非本清单范围，供开工）

| Wave | 内容 |
|------|------|
| **M0** | 决策 D1/D2；导出唯一 SVG（含 favicon 用固定色版） |
| **M1** | 修 `ContextOsMark`（尺寸 props、currentColor）；营销 chrome 用 `cos-mark--nav`；部署 app |
| **M2** | Landing layout icons + Navbar/Footer 用 brand 文件；部署 hub |
| **M3** | Why/Canju 同步 mark + favicon；部署 public-sites |
| **M4** | apple-icon 真文件；metadataBase；OG 绝对 URL |
| **M5** | 视觉回归截图：hub / desktop / login / why / canju 顶栏 + tab |

---

## 9. 快速对照：用户三类现象

| 用户说法 | 对应清单项 |
|----------|------------|
| 品牌官网没有采用品牌制订 SVG LOGO | Hub 内联 B + 无 icons；未用规范 Full A / brand 契约 |
| 部分页面没有标签页小 logo | Hub favicon 404；部分站仅 svg/无 ico；apple 死链风险 |
| desktop logo 比例不对、超出框体 | 营销顶栏 56px 槽 + 80–90px mark（S1–S3） |

---

**下一步**：确认字形 D1（Full vs Dual-arc）后按 M0–M3 一波修完并 redeploy。未确认前**不要**各仓各自改 path，否则漂移会加重。
