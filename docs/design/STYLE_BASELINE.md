# Context-OS 样式基准（Style Baseline）

**体系名**: Slate × Indigo（石板 × 靛蓝）  
**状态**: Canonical（样式源规范；实现以本文件为准）  
**实现源**: `packages/cos-tokens/`（`sync.sh` 同步到 App / Landing / Why / Ghost / Canju）  
**日期**: 2026-07-10（2026-07-22 由 Monochrome Ink 全面修订为 Slate × Indigo）  
**结构参考**: `frontend_next`（字阶 / 间距 / 圆角 / 控件纪律）  
**色彩方向**: 冷石板调中性色 + **单一标志色 Indigo**（禁止品牌青 / 品牌绿 / 铜色系回潮）  
**变更记录**: [`docs/design/UI_REVIEW_AND_VIBRANT_COLOR_PROPOSAL_2026-07-21.md`](./UI_REVIEW_AND_VIBRANT_COLOR_PROPOSAL_2026-07-21.md)（评审、四案比选、分层决策全过程）  
**废止色轴**: 旧 Precision Lab 青色主轴；公域 `#10b981` 翠绿主轴；**Monochrome Ink 铜色轴（2026-07-22 废止）**

---

## 1. 本文件做什么

| 是 | 不是 |
|----|------|
| 全站 / 多产品 **唯一视觉源规范** | 升级排期（见 engineering 计划） |
| Token 命名、语义、推荐值 | 业务 API / workspace 运维细节 |
| 组件用法契约（按钮 / 输入 / 壳） | 逐文件改动清单 |

**消费方**: `frontend_next` · `context-os-landing` · `context-os-theme`（Ghost）· `whyiamright` · `cchess`（仅 chrome）  

**实现约定**: CSS 变量存 **HSL 分量**（无 `hsl()` 包裹），使用时写 `hsl(var(--token))` 或 `hsl(var(--token) / 0.15)`。与现 v6 `design-tokens.css` 格式一致。

---

## 2. 设计原则

1. **中性带色调** — 中性色不是纯灰：亮色是冷石板纸（hue 220），暗色是石板黑（hue 225）。面积上中性仍占 ~85%。  
2. **颜色给品牌，灰度给交互** — Indigo 用于：logo、链接、焦点环、**交互反馈态**（选中 / 勾选 / 开启 / 激活）、状态点、小徽章、低频付费 CTA 实底。**导航区与高频动作按钮一律中性**（对齐 Gemini / Kimi / Grok：创建按钮 = 中性灰填充，选中态 = 浅灰底）。  
3. **按钮三档** — 创建 = 中性灰填充；付费 / 转化 = 靛蓝实底（`--accent-cta`，每屏 ≤1 个）；高频行内动作（发送、登录、保存）= 墨色实底。次级 = 描边 / ghost。  
4. **层级靠对比与间距** — 先字重 / 灰阶 / 边框，再用阴影；阴影宁少勿脏。  
5. **亮色默认（产品）** — App / Workspace 默认 light；dark 为完整第二套，不是简单反色。  
6. **公域可默认暗底** — Landing / Blog / Why 可用 dark 作为默认 `color-scheme`，但 **token 名与语义必须同一套**。  
7. **领域皮肤例外** — 象棋棋盘红黑 / 宣纸底可保留本地 `--board-*`（**禁止**占用品牌令牌名，如 `--accent`）；**导航壳必须服从本基准**。  
8. **中文优先可读** — Display 字体不假装覆盖汉字；中文标题用正文族 + 字阶。

---

## 3. 品牌与壳（Cos Shell）

### 3.1 Logo

| 项 | 规范 |
|----|------|
| 唯一 mark | **ContextOsMark Full**（双弧 + 中轴 + 节点圆点）。组件：`frontend_next/components/context-os-mark.tsx`；静态：`public/brand/context-os-mark.svg` 与 `app/icon.svg`（favicon） |
| 填色 | **plate = 墨**（React 组件 `hsl(var(--foreground))` 随主题：亮=石板墨、暗=纸白；静态 SVG 硬编码 `#1b1f2d`），**ink = 背景反色**（亮=白、暗=墨）。**右下节点 = `hsl(var(--accent))` 靛蓝**（静态 `#4f7cf3`）——墨族主体 + 蓝族血缘点，与 CTA 三档体系（创建灰 / 付费蓝 / 高频墨）角色一致 |
| 尺寸 | 由 `size` / CSS 槽控制（nav ~28px、shell ~32px、auth ~56px）；**禁止**组件硬编码 90px |
| 锁头布局 | 营销/家族顶栏用 **`.cos-brand-lockup` 横排**（mark + 「Context-OS」）；**禁止**营销壳复用 `.app-auth-brand-link` 的 column |
| 对外命名 | 用户面 **Context-OS**；安装版 **Context-OS 客户端** / EN **Context-OS Client**；导航短标中文 **客户端**、英文 **Client**（勿用「桌面」作导航主文案） |
| 多语言 | App 营销顶栏提供 中文/EN；Hub/Why/Canju 顶栏提供语言切换；禁止用户面 **AVRag** |
| 禁止 | Dual-arc 简化标；绿块 + 三横线旧 mark；任意站点私自换 path；导航写「桌面」 |

### 3.2 产品家族导航

```text
[Mark] Context-OS
  应用    app.contextlm.top/login?next=/dashboard   （云端 SaaS）
  桌面    app.contextlm.top/desktop                 （Windows 客户端 — 交付形态，L1 必显）
  博客    blog.contextlm.top
  工具    whyimright.contextlm.top · canju.contextlm.top · elo.contextlm.top
```

| 表面 | 顶栏 |
|------|------|
| Marketing / Blog / Why / Canju chrome | 固定高 **3.5–4rem**（56–64px），底边 `1px solid hsl(var(--border))`；**须含「桌面」** |
| App 营销 path（`/desktop` `/pricing` `/legal`） | 轻顶栏：Hub · 定价 · 桌面 · 登录 / 进入应用 |
| App 内工作区 | 产品 chrome；**页脚/帮助**链到桌面，**勿**把营销项塞进工作顶栏 |

链接契约：`frontend_next/lib/site-map.ts`。发现矩阵见 [`MULTI_SITE_IA_INTEGRATION_PLAN_2026-07-14.md`](../engineering/MULTI_SITE_IA_INTEGRATION_PLAN_2026-07-14.md)。

### 3.3 默认主题

| 表面 | 默认 |
|------|------|
| App / Dashboard / Workspace / Settings / Admin | **light** |
| Landing / Blog / Why | **dark** 推荐 |
| 用户可切换 | `html[data-theme="light"|"dark"]`；并尊重 `prefers-color-scheme` |

---

## 4. 色彩

### 4.1 语义分工

| 角色 | Token 族 | 用途 |
|------|----------|------|
| 墨 / 纸 | `--foreground` / `--background` / `--cta-*` | 文字、页底、高频中性按钮 |
| 石板灰阶表面 | `--surface-*` / `--muted*` / `--border*` | 侧栏、卡片、分割、创建按钮填充 |
| **标志色 Indigo** | `--accent*` / `--ring` / `--focus-ring` | logo、链接、选中 / 勾选 / 开启、付费 CTA |
| 状态 | `--success` / `--warning` / `--destructive` / `--info` | 仅状态；不抢品牌 |

### 4.2 中性 — Light（冷石板纸）

| Token | HSL 分量 | 用途 |
|-------|----------|------|
| `--background` | `220 25% 98%` | 页背景 |
| `--foreground` | `225 25% 14%` | 主文字 |
| `--card` | `0 0% 100%` | 卡片面（纯白，浮于石板底） |
| `--card-foreground` | `225 25% 14%` | 卡片字 |
| `--surface-elevated` | `0 0% 100%` | 浮层 / 面板 |
| `--surface-muted` | `220 20% 95%` | 侧栏、次表面 |
| `--surface-soft` | `220 16% 92%` | 创建按钮填充、hover、输入底 |
| `--surface-sunken` | `220 18% 89%` | 创建按钮 hover、凹陷 |
| `--popover` | `0 0% 100%` | 弹出层 |
| `--popover-foreground` | `225 25% 14%` | |
| `--muted` | `220 16% 92%` | 弱底 |
| `--muted-foreground` | `222 12% 38%` | 次要文字 |
| `--subtle-foreground` | `222 10% 46%` | 占位、禁用辅文 |
| `--secondary` | `220 16% 92%` | 次按钮底 |
| `--secondary-foreground` | `225 25% 18%` | 次按钮字 |
| `--border` | `220 14% 85%` | 标准边 |
| `--border-strong` | `220 12% 74%` | hover 边、强描边 |
| `--border-whisper` | `220 16% 91%` | 极淡分割 |
| `--input` | `220 14% 85%` | 输入边 |
| `--input-background` | `220 20% 97%` | 输入底 |
| `--primary` | `225 25% 16%` | 墨色强调 |
| `--primary-foreground` | `0 0% 100%` | 墨底上的字 |

### 4.3 中性 — Dark（石板黑）

| Token | HSL 分量 |
|-------|----------|
| `--background` | `225 20% 9%` |
| `--foreground` | `220 20% 92%` |
| `--card` / `--popover` | `225 16% 12%` |
| `--surface-elevated` | `225 16% 13%` |
| `--surface-muted` | `225 16% 12%` |
| `--surface-soft` | `225 14% 16%` |
| `--surface-sunken` | `225 18% 7%` |
| `--muted` | `225 14% 16%` |
| `--muted-foreground` | `218 12% 68%` |
| `--subtle-foreground` | `218 10% 56%` |
| `--secondary` | `225 14% 16%` |
| `--secondary-foreground` | `220 20% 92%` |
| `--border` | `220 10% 21%` |
| `--border-strong` | `220 10% 30%` |
| `--border-whisper` | `220 10% 16%` |
| `--input` | `220 10% 21%` |
| `--input-background` | `225 16% 12%` |
| `--primary` | `220 22% 92%` |
| `--primary-foreground` | `225 20% 9%` |

### 4.4 标志色 Indigo（全站唯一品牌色相）

| Token | Light HSL | Dark HSL | 用途 |
|-------|-----------|----------|------|
| `--accent` | `235 65% 52%` | `230 85% 68%` | 交互反馈实底（勾选 / 开关 / 选中项）、徽章、选中描边 |
| `--accent-strong` | `235 65% 45%` | `230 85% 75%` | accent hover / 按下 |
| `--accent-text` | `235 60% 46%` | `230 85% 70%` | 链接 / 彩色小字（AA 达标） |
| `--accent-soft` | `232 60% 95%` | `232 30% 18%` | 极淡品牌底（chip、浅徽章） |
| `--accent-glow` | 同 accent | 同 accent | 与 alpha 组合做光晕 |
| `--surface-accent-soft` | 同 `--accent-soft` | | 别名 |
| `--ring` / `--focus-ring` | = `--accent` | | 焦点 |

**用法示例**

```css
color: hsl(var(--accent-text));
background: hsl(var(--accent));            /* 仅交互反馈态与付费 CTA */
box-shadow: 0 0 0 3px hsl(var(--focus-ring) / 0.18);
```

**禁止**

- 导航区 / 高频按钮用 `hsl(var(--accent))` 实底（付费 CTA 除外）  
- 大面积 hero 靛蓝渐变、彩色扫描动画  
- 再引入第二品牌色相（青 / 绿 / 铜等）  

### 4.5 CTA 三档

| 档位 | Token / 类 | 视觉 | 场景 |
|------|-----------|------|------|
| 付费 / 转化 | `--accent-cta`（亮 `235 55% 45%` / 暗 `235 70% 62%`）、`.app-button-accent` | 靛蓝实底 + 白字，每屏 ≤1 | 升级订阅、管理订阅、pricing 升级 |
| 创建 | `.app-button-create`（`--surface-soft` 底 + `--foreground` 字，hover `--surface-sunken`）、`.app-button-create-soft`（描边 ghost） | 中性灰填充 / 描边 | 新建工作区、新建会话、添加内容源 |
| 高频行内动作 | `--cta-background`（亮 `225 25% 16%` / 暗 `220 22% 92%`） | 墨色实底 | 发送、登录、保存 |

**规则**

- 同一视图 **最多一个** 视觉主按钮（三档中按其场景取最高者）。  
- 禁用：`opacity: 0.55` + `cursor: not-allowed`。  

### 4.6 语义色（状态 only）

| Token | Light HSL | 说明 |
|-------|-----------|------|
| `--success` | `155 55% 36%` | 明确的绿，非 `#10b981` |
| `--warning` | `38 90% 44%` | 琥珀 |
| `--warning-foreground` | `30 50% 22%` | |
| `--warning-surface` | `40 60% 94%` | |
| `--warning-border` | `38 60% 70%` | |
| `--destructive` | `4 70% 52%` | 错误 / 删除 |
| `--destructive-foreground` | `0 0% 100%` | |
| `--destructive-soft` | `4 65% 96%` | |
| `--destructive-border` | `4 50% 80%` | |
| `--info` | `205 85% 48%` | 天蓝（与品牌靛蓝 hue 区隔 ≥30） |

Dark 模式：提高明度（见 tokens.css 暗色块），与表面对比 ≥ 可读即可。

### 4.7 场景别名（Dashboard / Workspace）

**原则**: 不再维护独立色相；映射到全局 token。

```text
--dashboard-shell              → --background
--dashboard-foreground           → --foreground
--dashboard-surface            → --surface-elevated / --card
--dashboard-surface-muted      → --surface-muted
--dashboard-border             → --border
--dashboard-primary            → --cta-background 或 --primary
--dashboard-muted-foreground   → --muted-foreground
…（其余同理）

--workspace-shell              → --background
--workspace-panel              → --surface-elevated
--workspace-rail               → --surface-muted
--workspace-border             → --border
--workspace-primary            → --primary
--workspace-muted-foreground   → --muted-foreground
```

实现阶段允许暂时保留变量名做 alias，**禁止** alias 指向另一套色相。

### 4.8 象棋领域色（仅棋盘局部）

允许在 `cchess` 内使用本地变量，**必须** `--board-*` 前缀，**禁止**占用品牌令牌名（`--accent` / `--card` / `--muted` 等）：

```text
--board-bg, --board-ink, --board-red, --board-line, --board-accent, …
```

Chrome（顶栏 / 页脚 / 全局按钮）必须用本基准 token。

---

## 5. 字体

### 5.1 家族

| 角色 | 字体 | CSS 变量 |
|------|------|----------|
| 正文 / UI | IBM Plex Sans + 中文系统栈 | `--font-body` |
| 标题（拉丁 / 数字 / 品牌英文） | Space Grotesk | `--font-heading` |
| 等宽 | JetBrains Mono | `--font-mono` |

**中文栈（body 与标题共用）**

```text
"PingFang SC", "Hiragino Sans GB", "Noto Sans SC", "Microsoft YaHei", sans-serif
```

**规则**

- 中文 UI 标题：**不要**依赖 Space Grotesk 出形；用 `--font-body` + 字阶 / `font-weight: 600`。  
- 品牌英文 wordmark「Context-OS」可用 `--font-heading`。  
- 全站 **禁止** 第三套 UI 字体（Geist / Inter 等）；Ghost 主题不得再引 Google Inter。  
- 代码、FEN、API key、用量数字：一律 `--font-mono`。  

### 5.2 字阶

**产品壳默认（U13，2026-07-10）**：相对旧 v6 字阶 **整体下一档**，优先服务 Chat / Workspace 密度。实现源：`frontend_next/app/design-tokens.css`。Marketing 页题可用更大本地 clamp，**不要**把 marketing 尺写回产品壳 token。

| Token | 尺寸 | 行高 token | 字重 | 用途 |
|-------|------|------------|------|------|
| `--font-size-overline` | 0.6875rem (11) | `--line-height-overline` 1.4 | 600–700 | 上标、eyebrow；可 `letter-spacing: 0.05em`，**少用** |
| `--font-size-caption` | 0.6875rem (11) | 1.45 | 400 | 图注、辅助 |
| `--font-size-caption-strong` | 0.75rem (12) | 1.45 | 500 | 强调图注 / 弱 chip |
| `--font-size-meta` | 0.75rem (12) | 1.5 | 400 | 元信息、进度行、会话 meta |
| `--font-size-label` | 0.75rem (12) | 1.5 | 600 | 表单 label |
| `--font-size-control` | 0.8125rem (13) | 1.48 | 500 | 按钮、输入、模式、会话列表标题 |
| `--font-size-body` | 0.875rem (14) | 1.65 | 400 | 正文默认、助手回答 |
| `--font-size-body-strong` | 0.9375rem (15) | 1.65 | 500 | 强调正文 |
| `--font-size-section-title` | 0.9375rem (15) | 1.35 | 600 | 卡片 / 区标题 / 侧栏区标题 |
| `--font-size-brand` | 1rem (16) | 1.1 | 600 | 品牌字 |
| `--font-size-shell-title` | 1.0625rem (17) | 1.25 | 600 | 壳层标题 |
| `--font-size-title-sm` | 1.125rem (18) | 1.2 | 600 | 小页题 |
| `--font-size-title` | 1.5rem (24) | 1.18 | 600 | 页题 H1（产品内；Marketing 可更大） |

**字间距**

| Token | 值 |
|-------|-----|
| `--letter-spacing-title` | `-0.02em` |
| `--letter-spacing-tight` | `-0.01em` |
| `--letter-spacing-overline` | `0.05em` |
| `--letter-spacing-normal` | `0` |

**字重 token**: `--font-weight-medium` 500 · `--font-weight-semibold` 600 · `--font-weight-bold` 600（webfont 只加载 400/500/600，禁止写 700 产生 faux bold）

> 已知债：中文阅读场景 `--font-size-body` 偏小（14px）、caption 11px 对中文过小，计划正文升 15px、caption ≥12px（见 frontend-visual-debt.md）。

---

## 6. 间距

| Token | 值 |
|-------|-----|
| `--space-1` | 0.25rem (4px) |
| `--space-2` | 0.5rem (8px) |
| `--space-3` | 0.75rem (12px) |
| `--space-4` | 1rem (16px) |
| `--space-5` | 1.25rem (20px) |
| `--space-6` | 1.5rem (24px) |
| `--space-7` | 2rem (32px) |
| `--space-8` | 3rem (48px) |
| `--space-9` | 4rem (64px) |
| `--space-10` | 6rem (96px) |

**习惯**

- 表单字段垂直节奏：`space-4`  
- 卡片内边距：`space-4`–`space-6`  
- 页边：水平 `space-6`–`space-8`，移动端不少于 `space-4`  

---

## 7. 圆角

| Token | 值 | 用途 |
|-------|-----|------|
| `--radius-control` / `--radius-button` | `0.5rem` (8px) | 按钮、输入、小控件 |
| `--radius-card` / `--radius` | `0.75rem` (12px) | 卡片、面板 |
| `--radius-message` | `1rem` (16px) | 消息气泡 |
| `--radius-badge` / `--radius-pill` | `999px` | **仅** badge、avatar、搜索胶囊 |

禁止：全局按钮 `border-radius: 999px`（象棋棋子 chip 除外）。

---

## 8. 阴影与焦点

### 8.1 阴影（随主题变）

**Light**（墨色低透明）

| Token | 值 |
|-------|-----|
| `--shadow-sm` | `0 1px 2px hsl(0 0% 0% / 0.04)` |
| `--shadow-md` | `0 4px 12px hsl(0 0% 0% / 0.06)` |
| `--shadow-lg` | `0 8px 24px hsl(0 0% 0% / 0.08)` |
| `--shadow-xl` | `0 16px 48px hsl(0 0% 0% / 0.12)` |
| `--shadow-topbar` | `0 1px 3px hsl(0 0% 0% / 0.05)` |
| `--shadow-glow` | `0 0 20px hsl(var(--accent) / 0.12)` | 极少用 |
| `--shadow-focus-ring` | `0 0 0 3px hsl(var(--focus-ring) / 0.18)` | |
| `--shadow-popover` | `0 12px 32px rgba(15, 23, 42, 0.12)` | 浮层专用 |

**Dark**：提高不透明度（约 0.3–0.55），仍用 `hsl(0 0% 0% / …)`。

### 8.2 焦点

- 可聚焦控件 `:focus-visible` → `outline: none` + `box-shadow: var(--shadow-focus-ring)` 或 `border-color: hsl(var(--focus-ring))`。  
- 勿去掉焦点且无替代样式。  

---

## 9. 控件契约

### 9.1 按钮（见 §4.5 CTA 三档）

| 级别 | 类名约定 | 视觉 |
|------|----------------|------|
| **Paid Primary** | `.app-button-accent` | `--accent-cta` 实底；仅付费/转化场景，每屏 ≤1 |
| **Create** | `.app-button-create` / `.app-button-create-soft` | 中性灰填充 / 描边 ghost |
| **Primary（高频中性）** | `.app-button-primary` | `cta-background` / `cta-foreground` 墨色 |
| **Secondary** | `.app-button-secondary` | `secondary` 底 + `border` |
| **Ghost** | `.app-button-ghost` | 透明 / 淡边 + `muted-foreground` |
| **Danger** | 扩展 | `destructive` 底或描边，不用于主流程 |

微交互：优先 **背景 / 边框**；控件 **避免** `translateY` 跳动（卡片 hover 可极轻）。

### 9.2 输入

- 边：`border`；hover：`border-strong`；focus：`focus-ring` + `shadow-focus-ring`。  
- 背景：`input-background`。  
- 高度与 padding 对齐 `--font-size-control` 与 `space-3`–`space-4`。  
- 原生 checkbox / radio：`accent-color: hsl(var(--accent))`；自定义勾选控件 checked 态 = `--accent` 底 + `--primary-foreground` 勾。  

### 9.3 链接

- 默认：`color: hsl(var(--accent-text))`。  
- 次要链接：`muted-foreground`，hover 到 `foreground`。  
- 勿用下划线彩虹或青绿色。  

### 9.4 卡片 / 面板

- 边 `border` 或 `border-whisper` + 可选 `shadow-sm`。  
- 圆角 `--radius-card`。  
- 标题：`--font-size-section-title` + semibold。  

### 9.5 Tab / Segment

- 未选中：ghost / secondary。  
- 选中：**浅灰底（`--surface-soft`）+ `--foreground` 字**（对齐 Gemini / Kimi / Grok）；品牌色只出现在选中态的描边或小图标上，不用品牌色铺底。  

### 9.6 空状态

结构固定为三件套：

1. 简标或线框图标（可用 mark 简化版，**单色**）  
2. 主句（`section-title` 或 `body-strong`）  
3. 次句（`muted-foreground`）+ **一个** CTA（若有行动）  

禁止：仅一行灰字居中当作完成态。

### 9.7 加载

- 列表 / 卡片：与目标同圆角的 **skeleton**（`surface-soft` 脉冲）。  
- 禁止布局塌缩：header 固定槽位预留高度。  
- 长请求：文案 + 可选细进度，不用彩色转圈抢品牌色。  

### 9.8 消息 / 聊天

- 气泡圆角 `--radius-message`。  
- 用户消息：`surface-soft` 或弱墨底；助手：透明 / card。  
- 引用 chip：`accent-soft` + 小字 mono 可选。  

---

## 10. 动效

| Token | 值 | 用途 |
|------|------|------|
| `--duration-fast` | 120ms | 控件 hover / focus |
| `--duration-base` | 160ms | 常规过渡 |
| `--duration-slow` | 250ms | 卡片入场、模态 |
| `--ease-standard` | `cubic-bezier(0.2, 0, 0, 1)` | 全站统一缓动 |

**强制**

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## 11. 布局参考

| 场景 | 建议 |
|------|------|
| 阅读 / 文章 | 正文柱 **≤ 42–45rem** |
| 聊天 transcript | 内容柱 **44rem**，居中（Workspace 中栏） |
| Workspace 三栏（≥1440px 视口） | 左栏默认 280（236–320）/ 右栏默认 336（280–360）/ 中栏 ≥55% |
| Dashboard 主列 | **max-width ≈ 80rem** |
| Auth 卡 | **max-width ≈ 28rem** |
| 顶栏左右 padding | `space-6`–`space-8` |

---

## 12. 实现映射（给工程）

### 12.1 源文件

| 角色 | 路径 |
|------|----------------|
| **单源（canonical）** | `packages/cos-tokens/tokens.css`（改这里，跑 `sync.sh`；**禁止直接改拷贝**） |
| App 拷贝 | `frontend_next/app/design-tokens.css` |
| 全局基础 | `frontend_next/app/globals.css` |
| Ghost | `context-os-theme/assets/css/tokens.css`（由 sync 生成），主题样式在 `brand.css` |
| 品牌 mark | `packages/cos-tokens/mark.svg`（plate `#1b1f2d` + 白 ink + 靛蓝节点 `#4f7cf3`） |

### 12.2 使用模板

```css
.element {
  background: hsl(var(--background));
  color: hsl(var(--foreground));
  border: 1px solid hsl(var(--border));
  border-radius: var(--radius-card);
  box-shadow: var(--shadow-sm);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.element:focus-visible {
  box-shadow: var(--shadow-focus-ring);
}
```

### 12.3 Tailwind（Landing / Why）

`theme.extend.colors` 只映射本文件 token，例如：

```js
// 示意
accent: "hsl(var(--accent) / <alpha-value>)",
// 禁止硬编码 hex 品牌色（' #c49a5c ' / '#10b981' 等）
```

---

## 13. 禁止清单（Review 时直接打回）

| 禁止 | 原因 |
|------|------|
| 品牌青 / 青绿 / `#10b981` 作 accent | 已废止 |
| **铜色系（`#c49a5c` / `#a66b30` / `28 55%` / `22 85%` 等）任何残留** | 2026-07-22 废止，已全面换靛蓝 |
| 导航区 / 高频按钮品牌色实底（付费 CTA 除外） | 颜色给品牌，灰度给交互 |
| 新 Logo 变体 | 品牌分裂 |
| 硬编码 `rgba(15, 23, 42, …)` 以外的硬编码阴影；组件内硬编码品牌 hex | 逃逸 token |
| 新增第三 UI 字体 | 噪音 |
| 按钮全局胶囊圆角 | 不精密 |
| Settings/Admin 新增大片 `style={{ color: '#…' }}` | 逃逸 token |
| success 做成 `#10b981` 翠绿 | 与废止绿混淆 |
| 无 `:focus-visible` 的可点控件 | 无障碍底线 |
| 本地 `:root` 覆盖同步令牌（各站） | 换肤被短路；要改就改基准 + tokens.css |
| 棋盘域变量占用品牌令牌名 | 必须用 `--board-*` 前缀 |

---

## 14. 与旧体系对照

| 旧 | 新（本基准） |
|----|----------------|
| Precision Lab 青 accent | Indigo 靛蓝 |
| Monochrome Ink 纯灰 + Copper 点缀（2026-07-10 ~ 07-22） | Slate 石板调中性 + Indigo |
| CTA 一律墨色 / 曾短暂全铜实底 | 三档：创建灰 / 付费靛蓝 / 高频墨 |
| 公域翠绿 `#10b981` | `--success` 绿（`155 55% 36%`） |
| 绿块三线 logo / 黑底 mark | 靛蓝底 ContextOsMark |
| Inter / Geist 混用 | IBM Plex + Space（拉丁）+ JetBrains |
| dashboard/workspace 独立色相 | 全局 alias |
| info = 中性灰 | `--info` 天蓝（`205 85% 48%`） |

---

## 15. 变更流程

1. 先改 **本文件** 数值与语义。  
2. 再改 `packages/cos-tokens/tokens.css`（单源），跑 `sync.sh`。  
3. 各站消费层跟进（含删除本地覆盖层）；禁止站点私自改 hex 而不回写基准。  
4. 重大色相变更需同步：App · Landing · Ghost · Why · Canju chrome，并逐站人工核对渲染。  

---

## 16. 速查卡片

```text
页底/主字       background / foreground（石板调）
创建按钮        app-button-create（灰填充）/ app-button-create-soft（描边）
付费按钮        app-button-accent / --accent-cta（靛蓝实底，每屏 ≤1）
高频中性按钮     cta-*（墨）
链接/彩色小字    accent-text
勾选/开关/选中   accent 实底（交互反馈态）
Tab 选中        surface-soft 浅灰底
边/线           border / border-strong / border-whisper
次文            muted-foreground / subtle-foreground
卡片            card + radius-card + shadow-sm
圆角控件        radius-control 8px
正文字号        font-size-body 0.875rem（U13）
等宽            font-mono only
Logo            ContextOsMark（墨 plate + 反色 ink + 靛蓝节点）
```

---

**本文件是 Context-OS 视觉的法律文本；计划文档管「何时改」，本文件管「长什么样」。**
