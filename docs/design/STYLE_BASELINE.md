# Context-OS 样式基准（Style Baseline）

**体系名**: Cream × Void（暖米白纸 × 深空近黑）
**状态**: Canonical（样式源规范；实现以本文件为准）
**实现源**: `packages/cos-tokens/`（`sync.sh` 同步到 App / Landing / Why / Ghost / Canju）
**日期**: 2026-08-02（由 Slate × Indigo 全面修订为 Cream × Void）
**上游标准**: 浅色色彩 = [`design-md/cursor/DESIGN.md`](../../design-md/cursor/DESIGN.md)；深色色彩 + **全部非颜色项**（字体 / 字重 / 字距 / 圆角 / 阴影 / 间距 / 组件形状）= [`design-md/x.ai/DESIGN.md`](../../design-md/x.ai/DESIGN.md)
**色彩方向**: 浅色暖米纸（Cursor warm cream）+ 单一品牌色 **Cursor Orange**；深色近黑单色（xAI white-on-black）
**品牌色扩展决议（2026-08-02，用户拍板）**: Cursor 原标准中橙色仅用于 CTA / wordmark；本产品**允许扩展到链接 / 选中态 / 焦点环 / 交互反馈态**。深色主题遵守 xAI 单色纪律，不用橙色（链接 / 选中走白灰层级）
**变更记录**: 本文件 2026-08-02 版取代 2026-07-22 Slate × Indigo 版
**废止色轴**: 旧 Precision Lab 青色主轴；公域 `#10b981` 翠绿主轴；Monochrome Ink 铜色轴；**Slate × Indigo 冷石板 + 靛蓝轴（2026-08-02 废止）**

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

1. **浅色暖、深色黑** — 浅色是暖米纸（hue ~60 的 cream），不是冷白也不是纯白页底；深色是近黑 `#0a0a0a`，不是石板蓝黑。面积上中性仍占 ~85%。
2. **颜色给品牌，灰度给交互** — 浅色品牌电压是 Cursor Orange：CTA、wordmark 节点、链接、选中 / 勾选 / 开启、焦点环。深色遵守 xAI 单色：一切交互色 = 白 / 灰层级，不用橙。
3. **字重 400 走全场** — 层级靠字号阶梯 + 负字距 + 灰阶，不靠加粗。禁止 500/600/700（含 token 别名）。
4. **pill 是唯一交互形状** — 按钮一律胶囊；卡片 / 输入 = 8px；全系统只有 0 / 8px / pill 三档圆角。
5. **hairline-only** — 深度只靠 1px 边线 + 明度差。无 drop shadow（焦点环除外）。
6. **亮色默认（产品）** — App / Workspace 默认 light；dark 为完整第二套，不是简单反色。
7. **公域可默认暗底** — Landing / Blog / Why 可用 dark 作为默认 `color-scheme`，token 名与语义必须同一套。
8. **领域皮肤例外** — 象棋棋盘红黑 / 宣纸底可保留本地 `--board-*`（**禁止**占用品牌令牌名）；**导航壳必须服从本基准**。
9. **中文优先可读** — 中文标题用正文族 + 字阶做层级，不依赖字重。

---

## 3. 品牌与壳（Cos Shell）

### 3.1 Logo

| 项 | 规范 |
|----|------|
| 唯一 mark | **ContextOsMark Full**（双弧 + 中轴 + 节点圆点）。组件：`frontend_next/components/context-os-mark.tsx`；静态：`public/brand/context-os-mark.svg` 与 `app/icon.svg`（favicon） |
| 填色 | **plate = 墨**（React 组件 `hsl(var(--foreground))` 随主题：亮=暖墨、暗=纸白），**ink = 背景反色**（亮=米白、暗=近黑）。**右下节点 = `hsl(var(--accent))`**（亮=Cursor Orange；暗=白，随 accent token 主题化） |
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
| Marketing / Blog / Why / Canju chrome | 固定高 **4rem**（64px，对齐 Cursor top-nav），底边 `1px solid hsl(var(--border))`；**须含「桌面」** |
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
| 墨 / 纸 | `--foreground` / `--background` / `--cta-*` | 文字、页底、墨色高频按钮 |
| 暖灰阶表面 | `--surface-*` / `--muted*` / `--border*` | 侧栏、卡片、分割、输入底 |
| **品牌色（亮=橙 / 暗=白）** | `--accent*` / `--ring` / `--focus-ring` | CTA、链接、选中 / 勾选 / 开启、焦点、logo 节点 |
| 时间轴 pastel | `--timeline-*`（5 色） | **仅**产品内 agent / 工具调用时间轴 |
| 状态 | `--success` / `--warning` / `--destructive` / `--info` | 仅状态；不抢品牌 |

### 4.2 中性 — Light（Cursor 暖米纸）

| Token | Hex（标准值） | HSL 分量（约） | 用途 |
|-------|--------------|----------------|------|
| `--background` | `#f7f7f4` | `60 18% 96%` | 页背景（warm cream，**禁止纯白页底**） |
| `--foreground` | `#26251e` | `53 12% 13%` | 主文字（warm ink，禁止纯黑） |
| `--card` | `#ffffff` | `0 0% 100%` | 卡片面（纯白，浮于米白的微对比是签名） |
| `--card-foreground` | `#26251e` | `53 12% 13%` | 卡片字 |
| `--surface-elevated` | `#ffffff` | `0 0% 100%` | 浮层 / 面板 |
| `--surface-muted` | `#fafaf7` | `60 23% 97%` | canvas-soft；侧栏、IDE pane 底 |
| `--surface-soft` | `#efeee8` | `48 14% 92%` | hairline-soft；hover、弱底 |
| `--surface-sunken` | `#e6e5e0` | `50 11% 89%` | 凹陷、按下 |
| `--popover` | `#ffffff` | `0 0% 100%` | 弹出层 |
| `--popover-foreground` | `#26251e` | `53 12% 13%` | |
| `--muted` | `#efeee8` | `48 14% 92%` | 弱底 |
| `--muted-foreground` | `#807d72` | `47 6% 47%` | 次要文字（Cursor muted） |
| `--subtle-foreground` | `#a09c92` | `43 7% 60%` | 占位、禁用辅文（Cursor muted-soft） |
| `--secondary` | `#efeee8` | `48 14% 92%` | 次按钮底 |
| `--secondary-foreground` | `#26251e` | `53 12% 13%` | 次按钮字 |
| `--border` | `#e6e5e0` | `50 11% 89%` | 标准 hairline |
| `--border-strong` | `#cfcdc4` | `49 10% 79%` | hairline-strong；hover 边、面板外描 |
| `--border-whisper` | `#efeee8` | `48 14% 92%` | 极淡分割 |
| `--input` | `#e6e5e0` | `50 11% 89%` | 输入边 |
| `--input-background` | `#ffffff` | `0 0% 100%` | 输入底 |
| `--primary` | `#26251e` | `53 12% 13%` | 墨色强调（download 按钮等） |
| `--primary-foreground` | `#f7f7f4` | `60 18% 96%` | 墨底上的字 |
| `--body`（别名） | `#5a5852` | `45 5% 34%` | Cursor body 运行文（如需第三级文字） |

### 4.3 中性 — Dark（xAI 近黑）

| Token | Hex（标准值） | HSL 分量（约） |
|-------|--------------|----------------|
| `--background` | `#0a0a0a` | `0 0% 4%` |
| `--foreground` | `#ffffff` | `0 0% 100%` |
| `--card` / `--popover` | `#191919` | `0 0% 10%` |
| `--card-foreground` / `--popover-foreground` | `#ffffff` | `0 0% 100%` |
| `--surface-elevated` | `#191919` | `0 0% 10%` |
| `--surface-muted` | `#1a1c20` | `220 10% 11%` |
| `--surface-soft` | `#1a1c20` | `220 10% 11%` |
| `--surface-sunken` | `#060606` | `0 0% 2%` |
| `--muted` | `#1a1c20` | `220 10% 11%` |
| `--muted-foreground` | `#7d8187` | `216 4% 51%` |
| `--subtle-foreground` | `#7d8187` | `216 4% 51%` |
| `--secondary` | `#1a1c20` | `220 10% 11%` |
| `--secondary-foreground` | `#ffffff` | `0 0% 100%` |
| `--border` | `#212327` | `220 8% 14%` |
| `--border-strong` | `#363a3f` | `213 8% 23%` |
| `--border-whisper` | `#16181b` | `220 8% 9%` |
| `--input` | `#212327` | `220 8% 14%` |
| `--input-background` | `#1a1c20` | `220 10% 11%` |
| `--primary` | `#ffffff` | `0 0% 100%` |
| `--primary-foreground` | `#0a0a0a` | `0 0% 4%` |
| `--body`（别名） | `#dadbdf` | `228 7% 86%` |

### 4.4 品牌色（全站唯一品牌色相）

**Light = Cursor Orange；Dark = 白（xAI 单色纪律）。**

| Token | Light | Dark | 用途 |
|-------|-------|------|------|
| `--accent` | `#f54e00` ≈ `19 100% 48%` | `#ffffff` ≈ `0 0% 100%` | CTA 实底（亮）、勾选 / 开关 / 选中反馈、logo 节点 |
| `--accent-strong` | `#d04200` ≈ `19 100% 41%` | `#e6e6e6` ≈ `0 0% 90%` | accent hover / 按下 |
| `--accent-text` | `#d04200` ≈ `19 100% 41%` | `#ffffff` ≈ `0 0% 100%` | 链接 / 彩色小字（亮用 active 橙保证 AA；暗用白） |
| `--accent-soft` | `#f54e00` at 8–12% alpha | `#ffffff` at 8% alpha | 极淡品牌底（chip、浅徽章、选中底） |
| `--ring` / `--focus-ring` | = `--accent` | = `--accent` | 焦点 |

**用法示例**

```css
color: hsl(var(--accent-text));            /* 链接 */
background: hsl(var(--accent));            /* 浅色 CTA 实底 / 交互反馈态 */
box-shadow: 0 0 0 3px hsl(var(--focus-ring) / 0.18);
```

**橙色纪律（浅色）**

- 允许：CTA、wordmark / logo 节点、链接、选中 / 勾选 / 开启、焦点环、小徽章。（2026-08-02 扩展决议）
- 禁止：大面积橙色铺底、橙色渐变 hero、橙色装饰图形；橙色不是状态色（success / error 走 §4.6）。
- 深色不用橙：链接 = `--accent-text`（白）或 `--body` 灰层级；选中 = `--surface-soft` 底 + 白字 / 白描边。

**深色按钮纪律（xAI）**

- 默认按钮 = 白描边 outline pill：透明底 + `1px solid hsl(0 0% 100% / 0.25)` + 白字。
- 白色实底 pill 仅保留给最高优先级 CTA（每屏 ≤1），白底 + 近黑字。

### 4.5 时间轴 pastel（产品内 agent 签名，仅浅色语义）

| Token | Hex | 用途 |
|-------|-----|------|
| `--timeline-thinking` | `#dfa88f` | Thinking（peach） |
| `--timeline-grep` | `#9fc9a2` | Grepping（mint） |
| `--timeline-read` | `#9fbbe0` | Reading（pastel blue） |
| `--timeline-edit` | `#c0a8dd` | Editing（lavender） |
| `--timeline-done` | `#c08532` | Done（warm gold，字用白） |

- **仅**用于产品内 agent / 工具调用时间轴 pill（caption-mono 大写标签 + pill 形状）。
- **禁止**用作系统动作色、链接色、状态色。深色主题下同一组 token 保持 pastel（时间轴是浅色签名组件，深色下可整体降低饱和度，实现时校准对比度）。

### 4.6 CTA 档位

| 档位 | Token / 类 | 视觉（亮 / 暗） | 场景 |
|------|-----------|----------------|------|
| 主 CTA / 转化 | `--accent` 实底（亮=橙 / 暗=白实底 pill） | 橙底白字 / 白底近黑字，每屏 ≤1 | 升级订阅、pricing、主转化 |
| 高频行内动作 | `--cta-background`（亮 `#26251e` / 暗 `#ffffff`） | 墨实底 pill / 白实底 pill | 发送、登录、保存 |
| 创建 / 次级 | `--surface-soft` 底或描边 pill | 中性填充 / hairline 描边 | 新建工作区、新建会话 |
| Ghost | 透明 + hairline 描边 pill | 暗色下 = 白半透明描边（xAI 标准按钮） | 次动作 |

**规则**

- 同一视图 **最多一个** 视觉主按钮。
- 所有按钮 **pill**（`--radius-pill`），无例外档位。
- 禁用：`opacity: 0.55` + `cursor: not-allowed`。

### 4.7 语义色（状态 only）

| Token | Light | 说明 |
|-------|-------|------|
| `--success` | `#1f8a65` ≈ `161 64% 33%` | Cursor semantic-success |
| `--warning` | `38 90% 44%` | 琥珀 |
| `--warning-foreground` | `30 50% 22%` | |
| `--warning-surface` | `40 60% 94%` | |
| `--warning-border` | `38 60% 70%` | |
| `--destructive` | `#cf2d56` ≈ `345 64% 49%` | Cursor semantic-error |
| `--destructive-foreground` | `0 0% 100%` | |
| `--destructive-soft` | `345 60% 96%` | |
| `--destructive-border` | `345 50% 80%` | |
| `--info` | `205 85% 48%` | 天蓝（与品牌橙 hue 区隔 ≥30） |

Dark 模式：提高明度（见 tokens.css 暗色块），与表面对比 ≥ 可读即可。

### 4.8 场景别名（Dashboard / Workspace）

**原则**: 不再维护独立色相；映射到全局 token。

```text
--dashboard-shell              → --background
--dashboard-foreground         → --foreground
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

### 4.9 象棋领域色（仅棋盘局部）

允许在 `cchess` 内使用本地变量，**必须** `--board-*` 前缀，**禁止**占用品牌令牌名（`--accent` / `--card` / `--muted` 等）：

```text
--board-bg, --board-ink, --board-red, --board-line, --board-accent, …
```

Chrome（顶栏 / 页脚 / 全局按钮）必须用本基准 token。

---

## 5. 字体

### 5.1 家族（xAI 标准）

| 角色 | 字体 | CSS 变量 |
|------|------|----------|
| Display / 正文 / UI（同一家族） | **Inter**（universalSans 的开源替代）+ 中文系统栈 | `--font-heading` = `--font-body` |
| 等宽 / eyebrow / 技术标签 | **JetBrains Mono**（xAI 认可替代 Geist Mono） | `--font-mono` |

**中文栈（body 与标题共用）**

```text
"PingFang SC", "Hiragino Sans GB", "Noto Sans SC", "Microsoft YaHei", sans-serif
```

**规则**

- 全站只加载 **weight 400**。禁止 500/600/700，禁止 faux bold。
- 层级手段：字号阶梯、display 负字距、灰阶（foreground → body → muted-foreground）、mono eyebrow。
- 中文 UI 标题：用正文族 + 字阶；**不要**靠加粗出层级。
- 代码、FEN、API key、用量数字、eyebrow 标签：一律 `--font-mono`。
- 全站 **禁止** 第三套 UI 字体（Space Grotesk / IBM Plex Sans / Geist 等已废止）。

### 5.2 字阶

**Display 阶梯（xAI，marketing 与产品大标题）**——负字距是签名，不可省略：

| Token | 尺寸 | 行高 | 字距 | 用途 |
|-------|------|------|------|------|
| `--font-size-display-xl` | 96px | 1.0 | -2.4px | 最大 hero |
| `--font-size-display-lg` | 72px | 1.0 | -1.8px | 次 hero |
| `--font-size-display-md` | 48px | 1.0 | -1.2px | 区段标题 |
| `--font-size-display-sm` | 32px | 1.125 | -0.6px | 卡片簇标题 |
| `--font-size-display-xs` | 20px | 1.4 | 0 | 行内小标题 |

**产品密度字阶（沿用 U13 尺寸，字重全部 400）**：

| Token | 尺寸 | 行高 | 用途 |
|-------|------|------|------|
| `--font-size-overline` | 0.6875rem (11) | 1.4 | eyebrow；**配 `--font-mono` + 大写 + `0.1em` 正字距**（xAI caption-mono） |
| `--font-size-caption` | 0.75rem (12) | 1.45 | 图注、辅助 |
| `--font-size-meta` | 0.75rem (12) | 1.5 | 元信息、进度行 |
| `--font-size-label` | 0.75rem (12) | 1.5 | 表单 label |
| `--font-size-control` | 0.8125rem (13) | 1.48 | 按钮、输入、会话列表 |
| `--font-size-body` | 0.875rem (14) | 1.5 | 正文默认、助手回答 |
| `--font-size-body-lg` | 1rem (16) | 1.55 | 强调正文 / lead |
| `--font-size-title` | 1.5rem (24) | 1.18（字距 `-0.01em`） | 页题 H1（产品内） |

**字间距**

| Token | 值 |
|-------|-----|
| `--letter-spacing-display-xl` | `-2.4px`（约 `-0.025em` @96px） |
| `--letter-spacing-display-lg` | `-1.8px`（`-0.025em` @72px） |
| `--letter-spacing-display-md` | `-1.2px`（`-0.025em` @48px） |
| `--letter-spacing-display-sm` | `-0.6px`（约 `-0.019em` @32px） |
| `--letter-spacing-title` | `-0.01em` |
| `--letter-spacing-overline` | `0.1em`（mono eyebrow，xAI 1.4px@14px） |
| `--letter-spacing-normal` | `0` |

**字重 token**: 废止 `--font-weight-medium/semibold/bold`。保留单一 `--font-weight-regular: 400`（或不再设字重 token）。

> 已知债：中文阅读场景 `--font-size-body` 偏小（14px），计划正文升 15px（见 frontend-visual-debt.md）。

---

## 6. 间距（xAI scale，4px 基数）

| Token | 值 |
|-------|-----|
| `--space-xxs` | 0.125rem (2px) |
| `--space-xs` | 0.25rem (4px) |
| `--space-sm` | 0.5rem (8px) |
| `--space-md` | 0.75rem (12px) |
| `--space-lg` | 1rem (16px) |
| `--space-xl` | 1.5rem (24px) |
| `--space-2xl` | 2rem (32px) |
| `--space-3xl` | 3rem (48px) |
| `--space-4xl` | 4rem (64px) |

迁移期保留 `--space-1..8` 别名映射到上表；**禁止**新增 scale 外数值（20px / 96px 档取消，就近归并）。

**习惯**

- 表单字段垂直节奏：`space-lg`
- 卡片内边距：`space-xl`（24px）
- 区段（band）垂直：`space-4xl`（64px）
- 页边：水平 `space-xl`–`space-2xl`，移动端不少于 `space-lg`

---

## 7. 圆角（xAI 三档）

| Token | 值 | 用途 |
|-------|-----|------|
| `--radius-none` | `0` | 全出血 band |
| `--radius-sm` / `--radius-card` / `--radius-control` | `0.5rem` (8px) | 卡片、面板、输入、消息气泡 |
| `--radius-pill` | `999px` | **所有按钮**、badge、avatar、时间轴 pill |

**纪律**

- 全系统只有这三档。禁止组件硬编码其他值（20+ 种任意 rem 值是本次换标要清偿的债）。
- 按钮一律 pill——这是对旧基准「禁止全局胶囊」的正式反转（xAI：pill 是唯一交互形状）。
- 例外：象棋棋子 chip 沿用领域皮肤。

---

## 8. 阴影与焦点（hairline-only）

### 8.1 阴影

**原则：无 drop shadow。** 深度 = 1px hairline + 明度差（浅色：白卡浮于米白；深色：`#191919` 卡浮于 `#0a0a0a`）。

| Token | 值 | 说明 |
|-------|-----|------|
| `--shadow-focus-ring` | `0 0 0 3px hsl(var(--focus-ring) / 0.18)` | **唯一保留的阴影 token**，无障碍焦点 |
| `--shadow-sm/md/lg/xl/topbar/glow/popover` | **废止** | 全部删除；浮层用 hairline 描边代替 |

### 8.2 焦点

- 可聚焦控件 `:focus-visible` → `outline: none` + `box-shadow: var(--shadow-focus-ring)` 或 `border-color: hsl(var(--focus-ring))`。
- 勿去掉焦点且无替代样式。

---

## 9. 控件契约

### 9.1 按钮（见 §4.6 CTA 档位；全部 pill）

| 级别 | 类名约定 | 视觉 |
|------|----------------|------|
| **Primary CTA** | `.app-button-accent` | 亮：橙实底白字；暗：白实底近黑字；每屏 ≤1 |
| **Primary（高频中性）** | `.app-button-primary` | `cta-background` 实底 pill（亮=墨 / 暗=白） |
| **Secondary** | `.app-button-secondary` | 亮：`secondary` 底 + hairline；暗：透明底 + 白 25% 描边（xAI outline pill） |
| **Ghost** | `.app-button-ghost` | 透明 + `muted-foreground` |
| **Danger** | 扩展 | `destructive` 底或描边，不用于主流程 |

- 尺寸：`--font-size-control`（13px / 400）+ `padding: 8px 16px` 起步。
- 无阴影、无 glow。微交互只动背景 / 边框；控件 **避免** `translateY` 跳动。

### 9.2 输入

- 底：`input-background`；边：`border`；hover：`border-strong`；focus：`focus-ring`。
- 圆角 8px；padding `12px 16px`。
- 原生 checkbox / radio：`accent-color: hsl(var(--accent))`；自定义勾选 checked = `--accent` 底 + 反色勾。

### 9.3 链接

- 默认：`color: hsl(var(--accent-text))`（亮=橙 / 暗=白或 body 灰）。
- 次要链接：`muted-foreground`，hover 到 `foreground`。

### 9.4 卡片 / 面板

- 底：`card`；边：1px `border`（hairline）；圆角 8px；padding 24px。**无阴影**。
- 标题：用字阶（`--font-size-display-xs` 或 section 级字号），400。

### 9.5 Tab / Segment

- 未选中：ghost / secondary。
- 选中：浅色 = `--accent-soft` 淡橙底 或 `--surface-soft` + 橙小指示；深色 = `--surface-soft` 底 + 白字 / 白描边指示。

### 9.6 空状态

结构固定为三件套：

1. 简标或线框图标（可用 mark 简化版，**单色**）
2. 主句（字阶标题，400）
3. 次句（`muted-foreground`）+ **一个** CTA（若有行动）

禁止：仅一行灰字居中当作完成态。

### 9.7 加载

- 列表 / 卡片：与目标同圆角（8px）的 **skeleton**（`surface-soft` 脉冲）。
- 禁止布局塌缩：header 固定槽位预留高度。
- 长请求：文案 + 可选细进度，不用彩色转圈抢品牌色。

### 9.8 消息 / 聊天

- 气泡圆角 8px（`--radius-card`）。
- 用户消息：`surface-soft` 或弱墨底；助手：透明 / card。
- 工具调用时间轴：用 §4.5 pastel pill（mono 大写标签）。
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
| 内容容器 | **max-width ≈ 75rem（1200px）**（统一现行 1152 / 896） |
| 阅读 / 文章 | 正文柱 **≤ 42–45rem** |
| 聊天 transcript | 内容柱 **44rem**，居中（Workspace 中栏） |
| Workspace 三栏（≥1440px 视口） | 左栏默认 280（236–320）/ 右栏默认 336（280–360）/ 中栏 ≥55% |
| Dashboard 主列 | 容器内自适应 |
| Auth 卡 | **max-width ≈ 28rem** |
| 顶栏 | 高 64px；左右 padding `space-xl`–`space-2xl` |

---

## 12. 实现映射（给工程）

### 12.1 源文件

| 角色 | 路径 |
|------|----------------|
| **单源（canonical）** | `packages/cos-tokens/tokens.css`（改这里，跑 `sync.sh`；**禁止直接改拷贝**） |
| App 拷贝 | `frontend_next/app/design-tokens.css` |
| 全局基础 | `frontend_next/app/globals.css` |
| Ghost | `context-os-theme/assets/css/tokens.css`（由 sync 生成），主题样式在 `brand.css` |
| 品牌 mark | `packages/cos-tokens/mark.svg`（plate 暖墨 `#26251e` + 反色 ink + 橙节点 `#f54e00`） |

### 12.2 使用模板

```css
.element {
  background: hsl(var(--card));
  color: hsl(var(--foreground));
  border: 1px solid hsl(var(--border));   /* hairline-only，无阴影 */
  border-radius: var(--radius-card);       /* 8px */
  padding: var(--space-xl);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
}

.button {
  border-radius: var(--radius-pill);       /* 按钮一律 pill */
  font-weight: 400;                         /* 永远 400 */
}

.element:focus-visible {
  box-shadow: var(--shadow-focus-ring);    /* 唯一允许的 shadow */
}
```

### 12.3 Tailwind（Landing / Why）

`theme.extend.colors` 只映射本文件 token，例如：

```js
// 示意
accent: "hsl(var(--accent) / <alpha-value>)",
// 禁止硬编码 hex 品牌色
```

---

## 13. 禁止清单（Review 时直接打回）

| 禁止 | 原因 |
|------|------|
| **靛蓝 / 冷石板（hue 220–235 彩色 accent）任何残留** | 2026-08-02 废止，已换橙（亮）/ 白（暗） |
| 品牌青 / 青绿 / `#10b981` 作 accent | 已废止 |
| 铜色系（`#c49a5c` / `#a66b30` 等）任何残留 | 2026-07-22 废止 |
| **font-weight 500 / 600 / 700（含 faux bold、含 token 别名）** | xAI：400 走全场，层级靠字号 + 负字距 |
| **按钮非 pill 圆角** | xAI：pill 是唯一交互形状 |
| **8px / pill / 0 以外的圆角值** | 三档纪律 |
| **drop shadow（focus-ring 除外）** | hairline-only |
| 浅色纯白页底 / 纯黑文字 | 必须暖米 `#f7f7f4` + 暖墨 `#26251e` |
| 深色使用橙色或任何彩色 accent | xAI 单色纪律 |
| 深色实底按钮泛滥 | 白实底 pill 每屏 ≤1，其余白描边 outline |
| display 标题省略负字距 | 负字距是 xAI 签名 |
| eyebrow 用正文字体加粗 | 必须 mono + 大写 + 正字距 + 400 |
| 时间轴 pastel 用作系统动作色 / 状态色 | 仅限 agent 时间轴 |
| 新 Logo 变体 | 品牌分裂 |
| 组件内硬编码 hex / 阴影 rgba / 任意 rem 间距 | 逃逸 token |
| 新增第三 UI 字体 | 噪音 |
| success 做成 `#10b981` 翠绿 | 与废止绿混淆 |
| 无 `:focus-visible` 的可点控件 | 无障碍底线 |
| 本地 `:root` 覆盖同步令牌（各站） | 换肤被短路；要改就改基准 + tokens.css |
| 棋盘域变量占用品牌令牌名 | 必须用 `--board-*` 前缀 |

---

## 14. 与旧体系对照

| 旧（Slate × Indigo，已废止） | 新（Cream × Void） |
|----|----------------|
| 冷石板纸 `220 25% 98%` 页底 | 暖米 `#f7f7f4`（Cursor canvas） |
| 冷藏青墨 `225 25% 14%` | 暖墨 `#26251e`（Cursor ink） |
| 靛蓝 accent `235 65% 52%` | 亮：Cursor Orange `#f54e00`；暗：白（xAI 单色） |
| 石板蓝黑暗色 `225 20% 9%` | 近黑 `#0a0a0a` + 卡 `#191919`（xAI） |
| CTA 三档（创建灰 / 付费靛蓝 / 高频墨） | 主 CTA 橙（亮）/ 白实底（暗）；高频墨 pill；次级描边 pill |
| Space Grotesk + IBM Plex Sans | Inter（display + body 同族） |
| 字重 400/500/600 层级 | 全部 400；字号阶梯 + 负字距 |
| 阴影 8 token + 组件 104 处 | hairline-only，仅保留 focus-ring |
| 圆角 8/12/16/pill 四档 + 20+ 硬编码值 | 0 / 8px / pill 三档 |
| 按钮 8px 圆角 | 按钮一律 pill |
| 间距含 20px / 96px 档 | 2/4/8/12/16/24/32/48/64 |
| 容器 1152 / 896 混用 | 统一 ~1200px |
| eyebrow 正文字体 600 大写 | mono 400 大写 + 0.1em 字距 |
| —（无） | 新增 5 色 timeline pastel（agent 时间轴专用） |

---

## 15. 变更流程

1. 先改 **本文件** 数值与语义。
2. 再改 `packages/cos-tokens/tokens.css`（单源），跑 `sync.sh`。
3. 各站消费层跟进（含删除本地覆盖层）；禁止站点私自改 hex 而不回写基准。
4. 重大色相变更需同步：App · Landing · Ghost · Why · Canju chrome，并逐站人工核对渲染。

---

## 16. 速查卡片

```text
页底/主字（亮）  background #f7f7f4 / foreground #26251e（暖米 + 暖墨）
页底/主字（暗）  background #0a0a0a / foreground #ffffff（xAI 近黑）
卡片            card（亮 #fff / 暗 #191919）+ 1px border + 8px，无阴影
主 CTA          accent 实底 pill（亮=橙 #f54e00 / 暗=白），每屏 ≤1
高频按钮        cta-* 实底 pill（亮=墨 / 暗=白）
次级按钮        描边 pill（暗=白 25% 描边，xAI 标准）
链接/选中/焦点   accent-text / accent / focus-ring（亮=橙 / 暗=白）
时间轴          timeline-* 5 pastel pill（仅 agent 时间轴）
边/线           border / border-strong / border-whisper（hairline）
次文            muted-foreground / subtle-foreground
圆角            0 / 8px / 999px 三档；按钮一律 pill
字重            400 only；层级靠字号 + 负字距
字体            Inter + JetBrains Mono（eyebrow 大写 mono）
间距            2/4/8/12/16/24/32/48/64
容器            ~1200px；顶栏 64px
Logo            ContextOsMark（墨 plate + 反色 ink + accent 节点）
```

---

**本文件是 Context-OS 视觉的法律文本；计划文档管「何时改」，本文件管「长什么样」。**
