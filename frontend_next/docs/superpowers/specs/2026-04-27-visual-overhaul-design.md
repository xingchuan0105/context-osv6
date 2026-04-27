# Context OS 前端视觉 overhaul 设计文档

**日期**: 2026-04-27
**方向**: 精密实验室 (Precision Lab)
**范围**: 全站统一改造（仪表盘、工作区聊天、认证页、设置页、管理后台）

---

## 1. 概述

### 1.1 设计方向

**精密实验室 (Precision Lab)** — 像使用一台精密的研究仪器。灵感来自 Claude、Perplexity 和科学仪器界面。整体气质是前沿科技感、AI-native、理性可信赖。

### 1.2 主题策略

- **亮色优先** — 默认体验为亮色模式，暗色模式作为优雅的可选项
- **全面统一** — 所有页面共享同一套视觉系统，不出现风格割裂

### 1.3 当前问题

- 使用 Inter 字体，过于通用，缺乏品牌辨识度
- 亮色模式纯灰阶，无品牌个性
- 无品牌强调色，CTA 按钮为纯黑
- 阴影过于 subtle，卡片显得扁平
- 圆角过度使用（999px 泛滥）
- 硬编码颜色（`#fafafa` 等）在暗色模式下失效
- 引用不存在的 CSS 变量（`--button`）

---

## 2. 设计原则

1. **精密感** — 每个像素都有目的，对齐如仪器面板
2. **层级清晰** — 通过色彩对比、阴影、间距建立明确的信息层次
3. **克制用色** — 以深靛蓝灰 + 青色为主轴，语义色仅用于状态提示
4. **动效有目的** — 每个动画传达状态变化，快速精准
5. **暗色平等** — 暗色模式不是"反色"，而是独立设计的"夜间实验室"

---

## 3. 色彩系统

### 3.1 亮色模式

| Token | 值 | 用途 |
|-------|-----|------|
| `--background` | `#ffffff` | 主页面背景 |
| `--surface-elevated` | `#ffffff` | 卡片、面板（靠边框区分） |
| `--surface-muted` | `#f8fafc` | 侧边栏、次表面 |
| `--surface-soft` | `#f1f5f9` | 悬停状态、输入框背景 |
| `--surface-sunken` | `#e2e8f0` | 极少使用 |
| `--foreground` | `#0f172a` | 主文字（深靛蓝灰，非纯黑） |
| `--muted-foreground` | `#64748b` | 次要文字 |
| `--subtle-foreground` | `#94a3b8` | 占位符、禁用 |
| `--accent` | `#0891b2` | 主交互色：链接、按钮、激活态 |
| `--accent-soft` | `#ecfeff` | 极淡青色背景：选中项、AI 消息底色 |
| `--accent-glow` | `rgba(8, 145, 178, 0.15)` | 发光效果 |
| `--success` | `#0d9488` | 成功、RAG 模式标识 |
| `--warning` | `#d97706` | 警告 |
| `--destructive` | `#e11d48` | 错误、删除 |
| `--info` | `#0284c7` | 信息提示 |
| `--border` | `#e2e8f0` | 标准边框 |
| `--border-strong` | `#cbd5e1` | 悬停边框 |
| `--border-whisper` | `#f1f5f9` | 极淡分割线 |

### 3.2 暗色模式

| Token | 值 | 说明 |
|-------|-----|------|
| `--background` | `#020617` | 极深靛蓝黑，非纯黑 |
| `--surface-elevated` | `#0f172a` | 卡片、面板 |
| `--surface-muted` | `#1e293b` | 侧边栏 |
| `--surface-soft` | `#334155` | 悬停状态 |
| `--foreground` | `#f1f5f9` | 冷白灰，非纯白（避免刺眼） |
| `--muted-foreground` | `#94a3b8` | 次要文字 |
| `--subtle-foreground` | `#64748b` | 占位符 |
| `--accent` | `#22d3ee` | 亮青色（暗色下需要更高亮度） |
| `--accent-soft` | `rgba(34, 211, 238, 0.1)` | 半透明青色背景 |
| `--accent-glow` | `rgba(34, 211, 238, 0.25)` | 更强的发光 |
| `--success` | `#2dd4bf` | 亮青绿 |
| `--warning` | `#fbbf24` | 亮琥珀 |
| `--destructive` | `#fb7185` | 亮玫瑰 |
| `--info` | `#38bdf8` | 亮天蓝 |
| `--border` | `#1e293b` | 深灰蓝 |
| `--border-strong` | `#334155` | 稍亮 |
| `--border-whisper` | `#0f172a` | 极暗 |

---

## 4. 字体系统

### 4.1 字体家族

| 用途 | 字体 | 加载方式 |
|------|------|----------|
| 标题 | Space Grotesk | Google Fonts CDN |
| 正文 | IBM Plex Sans | Google Fonts CDN |
| 等宽 | JetBrains Mono | Google Fonts CDN |

### 4.2 标题层级

| 层级 | 字体 | 大小 | 字重 | 字间距 |
|------|------|------|------|--------|
| 页面标题 (H1) | Space Grotesk | 28px | 600 | -0.02em |
| 区域标题 (H2) | Space Grotesk | 20px | 600 | -0.015em |
| 卡片标题 (H3) | Space Grotesk | 16px | 600 | -0.01em |
| 标签/上标 | IBM Plex Sans | 12px | 600 | 0.05em (uppercase) |

### 4.3 正文层级

| 层级 | 字体 | 大小 | 字重 | 行高 |
|------|------|------|------|------|
| 正文 | IBM Plex Sans | 15px | 400 | 1.65 |
| 正文强调 | IBM Plex Sans | 16px | 500 | 1.65 |
| 辅助文字 | IBM Plex Sans | 13px | 400 | 1.5 |
| 代码 | JetBrains Mono | 14px | 400 | 1.6 |
| 技术标签 | JetBrains Mono | 12px | 500 | 1.4 |

---

## 5. 间距与圆角

### 5.1 间距系统

保留现有 `--space-1` 到 `--space-6`，新增：

```css
--space-7: 2rem;      /* 32px */
--space-8: 3rem;      /* 48px */
--space-9: 4rem;      /* 64px */
--space-10: 6rem;     /* 96px */
```

### 5.2 圆角系统

| 元素 | 圆角 | 说明 |
|------|------|------|
| 卡片/面板 | 12px | 从当前 16px 收紧，更精密 |
| 按钮 | 8px | 从 999px 改为适度圆角 |
| 输入框 | 8px | 与按钮统一 |
| 消息气泡 | 16px | 比卡片更圆，形成层次对比 |
| 徽章/标签 | 999px | 保留胶囊形状 |
| 头像 | 999px | 圆形 |
| 搜索框 | 999px | 搜索框保留胶囊形作为视觉标识 |

---

## 6. 阴影系统

### 6.1 亮色模式

```css
--shadow-sm: 0 1px 2px rgba(15, 23, 42, 0.04);
--shadow-md: 0 4px 12px rgba(15, 23, 42, 0.06);
--shadow-lg: 0 8px 24px rgba(15, 23, 42, 0.08);
--shadow-xl: 0 16px 48px rgba(15, 23, 42, 0.12);
--shadow-glow: 0 0 20px rgba(8, 145, 178, 0.15);  /* 青色发光 */
```

### 6.2 暗色模式

```css
--shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.3);
--shadow-md: 0 4px 12px rgba(0, 0, 0, 0.4);
--shadow-lg: 0 8px 24px rgba(0, 0, 0, 0.5);
--shadow-xl: 0 16px 48px rgba(0, 0, 0, 0.6);
--shadow-glow: 0 0 20px rgba(34, 211, 238, 0.3);  /* 亮青色发光 */
```

---

## 7. 组件规范

### 7.1 按钮

**主按钮 (Primary)**
- 背景: `--foreground`，文字: 白色
- 圆角: 8px，阴影: `shadow-sm`
- Hover: 背景变 `--accent`，阴影升到 `shadow-md`，`translateY(-1px)`
- Active: `translateY(0)`，背景变 accent 暗 10%
- 禁用: opacity 0.5

**次按钮 (Secondary)**
- 背景: 白色，边框: 1px solid `--border`，文字: `--foreground`
- 圆角: 8px
- Hover: 背景变 `--surface-soft`，边框变 `--border-strong`

**幽灵按钮 (Ghost)**
- 背景: 透明，边框: 1px solid transparent，文字: `--muted-foreground`
- Hover: 背景变 `--surface-muted`，文字变 `--foreground`

**AI 操作按钮**
- 背景: `--accent`，文字: 白色，阴影: `shadow-glow`
- Hover: 背景亮 10%，发光增强
- 用于"发送"、"运行"等核心 AI 操作

### 7.2 卡片

**标准卡片**
- 背景: 白色，边框: 1px solid `--border`，圆角: 12px
- 阴影: `shadow-sm`
- Hover: 边框变 `--border-strong`，阴影升到 `shadow-md`，`translateY(-1px)`
- 过渡: 200ms ease

**仪表盘工作区卡片**
- 在标准卡片基础上，左侧增加 3px 彩色边框（根据 workspace tone）：

| Tone | 亮色模式 | 暗色模式 |
|------|----------|----------|
| rose | `#fda4af` | `#9f1239` |
| sage | `#86efac` | `#166534` |
| lavender | `#c4b5fd` | `#6d28d9` |
| amber | `#fcd34d` | `#92400e` |

- 暗色模式下使用降低亮度、提高饱和度的版本

**列表项卡片**
- 无边框，无阴影
- Hover: 背景 `--surface-muted`，圆角 8px
- 选中态: 背景 `--accent-soft`，左侧 3px `--accent` 边框

### 7.3 输入框

**标准输入框**
- 背景: `--surface-soft`，边框: 1px solid transparent
- 圆角: 8px，内边距: 12px 16px
- 文字: `--foreground` 15px，Placeholder: `--subtle-foreground`
- Hover: 边框变 `--border`
- **Focus**: 边框变 `--accent`，阴影 `0 0 0 3px var(--accent-glow)`
- 过渡: 150ms ease

### 7.4 消息气泡

**用户消息**
- 背景: `--foreground`，文字: `--background`
- 圆角: 16px（右上角 4px，形成方向感）
- 最大宽度: 70%，阴影: `shadow-sm`

**AI 普通消息**
- 背景: `--surface-elevated`，文字: `--foreground`
- 左侧边框: 3px solid `--border`
- 圆角: 16px（左上角 4px）

**AI RAG 消息**
- 背景: `--accent-soft`，左侧边框: 3px solid `--success`
- 顶部模式标识: "Knowledge Retrieval" 胶囊标签

**AI Search 消息**
- 背景: `--surface-elevated`，左侧边框: 3px solid `--info`
- 顶部模式标识: "Web Search" 胶囊标签

### 7.5 顶部栏

- 背景: `--background` + `backdrop-filter: blur(12px)`
- 底部边框: 1px solid `--border`
- 高度: 56px
- 品牌: "Context"（`--foreground`）+ "OS"（`--accent`），Space Grotesk 16px weight 600

### 7.6 标签 / 徽章

**模式标签（胶囊形）**
- General: 背景 `--surface-muted`，文字 `--muted-foreground`，边框 `--border`
- RAG: 背景 `rgba(13, 148, 136, 0.1)`，文字 `--success`，边框 `rgba(13, 148, 136, 0.2)`
- Search: 背景 `rgba(2, 132, 199, 0.1)`，文字 `--info`，边框 `rgba(2, 132, 199, 0.2)`

---

## 8. 页面设计

### 8.1 仪表盘 (Dashboard)

- 最大宽度: 1280px，居中
- 水平内边距: 32px（桌面），16px（移动端）
- 顶部: 品牌行（Logo + "Context OS"）+ 用户头像
- 标题区: "Workspaces"（28px）+ 副标题 + "New Workspace" AI 操作按钮
- 搜索栏: 居中，最大宽度 480px，胶囊形
- 视图切换: 网格/列表分段控件
- 卡片网格: 3 列，间距 20px

### 8.2 工作区聊天页 (Workspace)

**三栏布局**
```
历史边栏 (280px) | 聊天区域 (自适应) | 知识面板 (320px)
```

**历史边栏**
- 背景: `--surface-muted`
- 当前选中项: 背景 `--accent-soft`，左侧 3px `--accent` 边框
- 每项: 44px 高，圆角 8px

**聊天区域**
- 消息流: 消息间距 24px
- 底部输入区: 固定，渐变遮罩 + 输入框卡片（圆角 16px，阴影 `shadow-lg`）
- 发送按钮: 圆形 36px，青色背景

**知识面板**
- Sources 区域 + Notes 区域
- 状态圆点: 就绪（青绿）、处理中（琥珀脉冲）、错误（玫瑰）

### 8.3 认证页 (Login / Register)

- 全屏居中，背景: `--surface-muted`
- 居中卡片: 最大宽度 420px，圆角 16px，阴影 `shadow-lg`，内边距 40px
- Logo（64px）+ "Context" + "OS"（青色）
- 标题: Space Grotesk 24px
- 表单标签: 12px uppercase，weight 600
- 错误提示: 玫瑰色背景 8% + 玫瑰边框 + 玫瑰文字

### 8.4 设置页 (Settings)

- 左侧边栏: 200px，背景 `--surface-muted`
- 导航项: Profile / Workspace / API Keys / Billing / Danger Zone
- 当前项: 文字 `--accent`，左侧 3px `--accent` 边框
- 右侧内容: 卡片分组，标题 Space Grotesk 24px

### 8.5 管理后台 (Admin)

- 同设置页布局，左侧导航不同
- 数据表格: 表头 `--surface-muted`，行 hover `--surface-soft`
- 操作按钮: 图标按钮组

---

## 9. 暗色模式适配要点

1. **背景不是纯黑** — `#020617` 带有极淡的靛蓝调，比纯黑更精致
2. **文字不是纯白** — `#f1f5f9` 避免刺眼
3. **强调色更亮** — 青色从 `#0891b2` 提升到 `#22d3ee`，确保在暗背景上足够醒目
4. **发光效果增强** — `accent-glow` 不透明度从 0.15 提升到 0.25
5. **阴影改用黑色** — `rgba(0, 0, 0, ...)` 替代 `rgba(15, 23, 42, ...)`
6. **边框更暗但可见** — 使用 `#1e293b` 而非纯黑，保持层级感知
7. **代码块** — 使用暗色主题语法高亮（如 atomDark）

---

## 10. 动效与微交互

### 10.1 时间规范

| 场景 | 时长 | 缓动 |
|------|------|------|
| Hover / Focus | 150ms | ease |
| 按钮按压 | 100ms | ease-in |
| 状态切换 | 200ms | ease-out |
| 入场动画 | 250ms | ease-out |
| 页面过渡 | 300ms | ease-in-out |

### 10.2 入场动画

**仪表盘卡片**
```css
@keyframes cardEnter {
  from { opacity: 0; transform: translateY(12px); }
  to   { opacity: 1; transform: translateY(0); }
}
/* 交错延迟: calc(var(--index) * 50ms)，最多 400ms */
```

**聊天消息**
```css
@keyframes messageEnter {
  from { opacity: 0; transform: scale(0.96) translateY(8px); }
  to   { opacity: 1; transform: scale(1) translateY(0); }
}
```

**模态框**
- 卡片: `scale(0.96) → scale(1)` + fade，200ms ease-out
- 遮罩: fade + `backdrop-filter: blur(0) → blur(8px)`，200ms

### 10.3 微交互

- 按钮 Hover: `translateY(-1px)` + 阴影提升
- 按钮 Active: `translateY(0)` + `scale(0.98)`
- 卡片 Hover: `translateY(-2px)` + `shadow-sm → shadow-md`
- Focus ring: 从 0 扩展到 3px，使用 `--accent-glow`

### 10.4 AI 思考状态

```css
@keyframes thinkingPulse {
  0%, 100% { transform: scale(1); opacity: 0.6; }
  50%      { transform: scale(1.4); opacity: 1; }
}
```
- 3 个圆点，8px，青色
- 依次延迟: 0ms, 200ms, 400ms
- 单个动画: 800ms infinite

---

## 11. 实施注意事项

### 11.1 字体加载

在 `layout.tsx` 中通过 `next/font/google` 加载：

```tsx
import { Space_Grotesk, IBM_Plex_Sans, JetBrains_Mono } from 'next/font/google';

const spaceGrotesk = Space_Grotesk({ subsets: ['latin'], variable: '--font-heading' });
const ibmPlexSans = IBM_Plex_Sans({ weight: ['400', '500', '600'], subsets: ['latin'], variable: '--font-body' });
const jetbrainsMono = JetBrains_Mono({ subsets: ['latin'], variable: '--font-mono' });
```

在 `body` 标签上应用：

```tsx
<body className={`${spaceGrotesk.variable} ${ibmPlexSans.variable} ${jetbrainsMono.variable}`}>
```

在 CSS 中引用：

```css
body {
  font-family: var(--font-body), 'PingFang SC', 'Microsoft YaHei', sans-serif;
}

h1, h2, h3, h4, h5, h6 {
  font-family: var(--font-heading), sans-serif;
}

code, pre {
  font-family: var(--font-mono), monospace;
}
```

### 11.2 需要修复的 Bug

1. `globals.css:1079` — `background: #fafafa` 改为 `hsl(var(--surface-muted))`
2. `globals.css:888` — `rgba(255, 255, 255, 0.8)` 白色 inset glow 改为主题感知色
3. `globals.css:572` — 同上
4. `workspace-chat.module.css:1202-1232` — `--button` 和 `--button-hover` 未定义，需添加或替换
5. `workspace-shell.module.css:41` — 硬编码阴影颜色
6. `workspace-chat.module.css:230` — `SFMono-Regular` 重复

### 11.3 暗色模式切换

继续使用现有的 `data-theme="dark"` 属性切换机制，在 `design-tokens.css` 中通过 `:root[data-theme="dark"]` 覆盖变量。

### 11.4 逐步实施建议

1. **P0** — 修复硬编码颜色 bug，添加新色彩变量
2. **P1** — 替换字体，更新按钮/卡片/输入框样式
3. **P2** — 改造消息气泡和聊天界面
4. **P3** — 添加入场动画和微交互
5. **P4** — 精细化调整和暗色模式打磨
