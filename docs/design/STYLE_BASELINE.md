# Context-OS 样式基准（Style Baseline）

**体系名**: Monochrome Ink  
**状态**: Canonical（样式源规范；实现以本文件为准）  
**日期**: 2026-07-10  
**结构参考**: `frontend_next`（字阶 / 间距 / 圆角 / 控件纪律）  
**色彩方向**: 黑白灰主导 + **单一标志色 Copper**（禁止品牌青 / 品牌绿）  
**关联计划**: [`docs/engineering/VISUAL_SYSTEM_AND_MULTI_SITE_UPGRADE_PLAN_2026-07-10.md`](../engineering/VISUAL_SYSTEM_AND_MULTI_SITE_UPGRADE_PLAN_2026-07-10.md)  
**废止色轴**: 旧 Precision Lab 青色主轴；公域 `#10b981` 翠绿主轴  

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

1. **中性优先** — 约 90% 的面积只用黑 / 白 / 灰。  
2. **标志色克制** — Copper 只用于：链接、焦点环、关键选中描边、少量 chip、品牌高光。**禁止**大面积铺色、禁止主按钮整块上色。  
3. **主行动 = 墨色** — Primary CTA 近黑（亮色模式）或近白（暗色模式），对标 Grok / OpenAI 对话产品，而不是彩色 CTA。  
4. **层级靠对比与间距** — 先字重 / 灰阶 / 边框，再用阴影；阴影宁少勿脏。  
5. **亮色默认（产品）** — App / Workspace 默认 light；dark 为完整第二套，不是简单反色。  
6. **公域可默认暗底** — Landing / Blog / Why 可用 dark 作为默认 `color-scheme`，但 **token 名与语义必须同一套**。  
7. **领域皮肤例外** — 象棋棋盘红黑 / 宣纸底可保留本地 `--board-*`；**导航壳必须服从本基准**。  
8. **中文优先可读** — Display 字体不假装覆盖汉字；中文标题用正文族 + 字阶。  

---

## 3. 品牌与壳（Cos Shell）

### 3.1 Logo

| 项 | 规范 |
|----|------|
| 唯一 mark | **ContextOsMark**（双弧「上下文」符号，见 `frontend_next/components/context-os-mark.tsx`） |
| 填色 | `currentColor` 或 `hsl(var(--foreground))`；暗底反白 |
| 禁止 | 绿块 + 三横线旧 mark；任意站点私自换标 |

### 3.2 产品家族导航

```text
[Mark] Context-OS
  应用    app.contextlm.top
  博客    blog.contextlm.top
  工具    whyimright.contextlm.top · canju.contextlm.top
```

| 表面 | 顶栏 |
|------|------|
| Marketing / Blog / Why / Canju chrome | 固定高 **3.5–4rem**（56–64px），底边 `1px solid hsl(var(--border))` |
| App 内 | 产品 chrome；设置或关于中保留家族链接即可 |

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
| 墨 / 纸 | `--foreground` / `--background` / `--cta-*` | 文字、页底、**主按钮** |
| 灰阶表面 | `--surface-*` / `--muted*` / `--border*` | 侧栏、卡片、分割 |
| **标志色 Copper** | `--accent*` / `--ring` / `--focus-ring` | 链接、焦点、选中点缀 |
| 状态 | `--success` / `--warning` / `--destructive` / `--info` | 仅状态；**低饱和，不抢品牌** |

### 4.2 中性 — Light

| Token | HSL 分量 | 约 hex | 用途 |
|-------|----------|--------|------|
| `--background` | `0 0% 100%` | `#FFFFFF` | 页背景 |
| `--foreground` | `0 0% 9%` | `#171717` | 主文字 |
| `--card` | `0 0% 100%` | `#FFFFFF` | 卡片面 |
| `--card-foreground` | `0 0% 9%` | `#171717` | 卡片字 |
| `--surface-elevated` | `0 0% 100%` | `#FFFFFF` | 浮层 / 面板 |
| `--surface-muted` | `0 0% 98%` | `#FAFAFA` | 侧栏、次表面 |
| `--surface-soft` | `0 0% 96%` | `#F5F5F5` | hover、输入底 |
| `--surface-sunken` | `0 0% 92%` | `#EBEBEB` | 极少用凹陷 |
| `--popover` | `0 0% 100%` | `#FFFFFF` | 弹出层 |
| `--popover-foreground` | `0 0% 9%` | `#171717` | |
| `--muted` | `0 0% 96%` | `#F5F5F5` | 弱底 |
| `--muted-foreground` | `0 0% 45%` | `#737373` | 次要文字 |
| `--subtle-foreground` | `0 0% 60%` | `#999999` | 占位、禁用辅文 |
| `--secondary` | `0 0% 96%` | `#F5F5F5` | 次按钮底 |
| `--secondary-foreground` | `0 0% 18%` | `#2E2E2E` | 次按钮字 |
| `--border` | `0 0% 90%` | `#E5E5E5` | 标准边 |
| `--border-strong` | `0 0% 80%` | `#CCCCCC` | hover 边 |
| `--border-whisper` | `0 0% 94%` | `#F0F0F0` | 极淡分割 |
| `--input` | `0 0% 90%` | `#E5E5E5` | 输入边 |
| `--input-background` | `0 0% 98%` | `#FAFAFA` | 输入底 |
| `--primary` | `0 0% 9%` | `#171717` | 墨色强调字 / 图标 |
| `--primary-foreground` | `0 0% 100%` | `#FFFFFF` | 墨底上的字 |

### 4.3 中性 — Dark

| Token | HSL 分量 | 约 hex |
|-------|----------|--------|
| `--background` | `0 0% 4%` | `#0A0A0A` |
| `--foreground` | `0 0% 93%` | `#EDEDED` |
| `--card` / `--surface-elevated` / `--popover` | `0 0% 8%` | `#141414` |
| `--surface-muted` | `0 0% 8%` | `#141414` |
| `--surface-soft` | `0 0% 12%` | `#1F1F1F` |
| `--surface-sunken` | `0 0% 4%` | `#0A0A0A` |
| `--muted` | `0 0% 12%` | `#1F1F1F` |
| `--muted-foreground` | `0 0% 55%` | `#8C8C8C` |
| `--subtle-foreground` | `0 0% 42%` | `#6B6B6B` |
| `--secondary` | `0 0% 12%` | `#1F1F1F` |
| `--secondary-foreground` | `0 0% 93%` | `#EDEDED` |
| `--border` | `0 0% 16%` | `#292929` |
| `--border-strong` | `0 0% 24%` | `#3D3D3D` |
| `--border-whisper` | `0 0% 12%` | `#1F1F1F` |
| `--input` | `0 0% 16%` | `#292929` |
| `--input-background` | `0 0% 8%` | `#141414` |
| `--primary` | `0 0% 93%` | `#EDEDED` |
| `--primary-foreground` | `0 0% 4%` | `#0A0A0A` |

### 4.4 标志色 Copper（全站唯一品牌色相）

| Token | Light HSL | 约 hex | Dark HSL | 用途 |
|-------|-----------|--------|----------|------|
| `--accent` | `28 55% 42%` | `#A66B30` | `32 45% 58%` | 链接、选中描边、小高光 |
| `--accent-soft` | `30 40% 96%` | `#F7F2EC` | `28 20% 14%` | 极淡选中底 |
| `--accent-glow` | 同 accent 通道 | — | 同 accent | 与 alpha 组合做光晕 |
| `--surface-accent-soft` | 同 `--accent-soft` | | | 别名，可映射 |
| `--ring` | = `--accent` | | | 焦点 |
| `--focus-ring` | = `--accent` | | | 焦点 |

**用法示例**

```css
color: hsl(var(--accent));
background: hsl(var(--accent-soft));
box-shadow: 0 0 0 3px hsl(var(--focus-ring) / 0.18);
```

**禁止**

- 主按钮 `background: hsl(var(--accent))` 作为默认样式  
- 大面积 hero 铜渐变、铜光扫描动画  
- 再引入第二品牌色相（青 / 绿 / 紫等）  

### 4.5 CTA（主行动）

| Token | Light | Dark |
|-------|-------|------|
| `--cta-background` | `0 0% 9%` | `0 0% 96%` |
| `--cta-background-hover` | `0 0% 18%` | `0 0% 86%` |
| `--cta-background-active` | `0 0% 14%` | `0 0% 80%` |
| `--cta-foreground` | `0 0% 100%` | `0 0% 9%` |

可选：hover 时 `box-shadow: var(--shadow-focus-ring)` 或 `border-color: hsl(var(--accent))`，**不要**把 CTA 背景改成 Copper。

### 4.6 语义色（状态 only）

刻意 **低饱和**，避免看起来像第二品牌色。

| Token | Light HSL | 说明 |
|-------|-----------|------|
| `--success` | `150 20% 32%` | 灰绿，非 `#10b981` |
| `--warning` | `36 70% 40%` | 琥珀 |
| `--warning-foreground` | `30 50% 22%` | |
| `--warning-surface` | `40 50% 96%` | |
| `--warning-border` | `36 40% 78%` | |
| `--destructive` | `0 65% 48%` | 错误 / 删除 |
| `--destructive-foreground` | `0 0% 100%` | |
| `--destructive-soft` | `0 60% 97%` | |
| `--destructive-border` | `0 40% 84%` | |
| `--info` | `0 0% 40%` | **中性灰信息**，不用天蓝抢色 |

Dark 模式：提高明度 8–15%，降低饱和，与表面对比 ≥ 可读即可。

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

允许在 `cchess` 内使用本地变量，**不得泄漏为全局品牌色**：

```text
--board-bg, --board-ink, --board-red, --board-line, …
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

**字重 token**: `--font-weight-medium` 500 · `--font-weight-semibold` 600 · `--font-weight-bold` 700  

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

**Dark**：提高不透明度（约 0.3–0.55），仍用 `hsl(0 0% 0% / …)`，**禁止**写死 `rgba(15, 23, 42, …)`。

### 8.2 焦点

- 可聚焦控件 `:focus-visible` → `outline: none` + `box-shadow: var(--shadow-focus-ring)` 或 `border-color: hsl(var(--focus-ring))`。  
- 勿去掉焦点且无替代样式。  

---

## 9. 控件契约

### 9.1 按钮三级

| 级别 | 类名约定（v6） | 视觉 |
|------|----------------|------|
| **Primary** | `.app-button-primary` | `cta-background` / `cta-foreground`；hover 用 `cta-background-hover`（墨色阶） |
| **Secondary** | `.app-button-secondary` | `secondary` 底 + `border` |
| **Ghost** | `.app-button-ghost` | 透明 / 淡边 + `muted-foreground` |
| **Danger** | 扩展 | `destructive` 底或描边，不用于主流程 |

**规则**

- 同一视图 **最多一个** 视觉主按钮。  
- 禁用：`opacity: 0.55` + `cursor: not-allowed`，勿仅靠变灰无禁用态。  
- 微交互：优先 **背景 / 边框**；控件 **避免** `translateY` 跳动（卡片 hover 可极轻）。  

### 9.2 输入

- 边：`border`；hover：`border-strong`；focus：`focus-ring` + `shadow-focus-ring`。  
- 背景：`input-background`。  
- 高度与 padding 对齐 `--font-size-control` 与 `space-3`–`space-4`。  

### 9.3 链接

- 默认：`color: hsl(var(--accent))`。  
- 次要链接：`muted-foreground`，hover 到 `foreground`。  
- 勿用下划线彩虹或青绿色。  

### 9.4 卡片 / 面板

- 边 `border` 或 `border-whisper` + 可选 `shadow-sm`。  
- 圆角 `--radius-card`。  
- 标题：`--font-size-section-title` + semibold。  

### 9.5 Tab / Segment

- 未选中：ghost / secondary。  
- 选中：**墨色底** 或 **accent-soft 底 + accent 字**，二选一全站统一；**禁止**再出现「默认黑、hover 变青」。  

### 9.6 空状态

结构固定为三件套：

1. 简标或线框图标（可用 mark 简化版，**单色**）  
2. 主句（`section-title` 或 `body-strong`）  
3. 次句（`muted-foreground`）+ **一个** Primary CTA（若有行动）  

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

| 类型 | 时长 | 说明 |
|------|------|------|
| 控件 hover / focus | 100–150ms | ease |
| 卡片入场 | 200–280ms | 可选 `translateY(8–12px)` |
| 模态 | 180–220ms | scale 0.96→1 + fade |
| 思考脉冲 | 循环 | 仅状态点，低对比 |

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
| 聊天 transcript | 内容柱 **≤ 48–54rem**，居中 |
| Dashboard 主列 | **max-width ≈ 80rem** |
| Auth 卡 | **max-width ≈ 28rem** |
| 顶栏左右 padding | `space-6`–`space-8` |

---

## 12. 实现映射（给工程）

### 12.1 源文件

| 角色 | 路径（目标态） |
|------|----------------|
| App 实现 | `frontend_next/app/design-tokens.css` |
| 全局基础 | `frontend_next/app/globals.css` |
| 单源（可选抽出） | `packages/cos-tokens` 或 `/home/chuan/cos-design-tokens` |
| Ghost | `context-os-theme/assets/css/brand.css`（由同源生成） |

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
// 禁止再写 '#10b981' / cyan hex
```

---

## 13. 禁止清单（Review 时直接打回）

| 禁止 | 原因 |
|------|------|
| 品牌青 / 青绿 / `#10b981` / `193 90% 35%` 作 accent | 已废止 |
| 主 CTA 默认彩色底 | 破坏 Monochrome Ink |
| 新 Logo 变体 | 品牌分裂 |
| 硬编码 `rgba(15, 23, 42, …)` 阴影 | 暗色失效 |
| 新增第三 UI 字体 | 噪音 |
| 按钮全局胶囊圆角 | 不精密 |
| Settings/Admin 新增大片 `style={{ color: '#…' }}` | 逃逸 token |
| success 做成高饱和品牌绿 | 与废止绿混淆 |
| 无 `:focus-visible` 的可点控件 | 无障碍底线 |

---

## 14. 与旧体系对照

| 旧 | 新（本基准） |
|----|----------------|
| Precision Lab 青 accent | Copper 点缀 |
| CTA hover → 青 | CTA 保持墨色阶 |
| 公域翠绿 `#10b981` | Copper 或中性 |
| 绿块三线 logo | ContextOsMark |
| Inter / Geist 混用 | IBM Plex + Space（拉丁）+ JetBrains |
| dashboard/workspace 独立色相 | 全局 alias |
| 高饱和 success/info 青蓝 | 低饱和 / 中性 info |

---

## 15. 变更流程

1. 先改 **本文件** 数值与语义。  
2. 再改 `design-tokens.css`（或 cos-tokens 单源）。  
3. 各站消费层跟进；禁止站点私自改 hex 而不回写基准。  
4. 重大色相变更需同步：App · Landing · Ghost · Why · Canju chrome。  

---

## 16. 速查卡片

```text
页底/主字     background / foreground
主按钮         cta-*（墨）
链接/焦点      accent / focus-ring（铜）
边/线          border / border-strong / border-whisper
次文           muted-foreground / subtle-foreground
卡片           card + radius-card + shadow-sm
圆角控件       radius-control 8px
正文字号       font-size-body 0.875rem（U13）
等宽           font-mono only
Logo           ContextOsMark only
```

---

**本文件是 Context-OS 视觉的法律文本；计划文档管「何时改」，本文件管「长什么样」。**
