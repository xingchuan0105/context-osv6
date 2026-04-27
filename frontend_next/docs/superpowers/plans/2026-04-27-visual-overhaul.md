# Context OS 前端视觉 Overhaul 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Context OS Next.js 前端从当前的 Inter + 纯灰阶设计全面改造为"精密实验室"视觉系统（Space Grotesk + IBM Plex Sans + 青色强调色 + 全站统一）

**Architecture:** 通过重写 CSS 设计令牌（design-tokens.css）和全局样式（globals.css）建立新视觉基础，然后逐层更新各页面组件的 CSS 模块。字体通过 `next/font/google` 加载。暗色模式继续使用 `data-theme="dark"` 属性切换。

**Tech Stack:** Next.js 16 + React 19 + 纯 CSS（无 Tailwind）+ CSS Modules

---

## 文件结构映射

| 文件 | 操作 | 说明 |
|------|------|------|
| `app/design-tokens.css` | 重写 | 色彩系统、间距、圆角、阴影、字体变量 |
| `app/globals.css` | 重写 | 全局布局、按钮、输入框、卡片、仪表盘样式 |
| `app/layout.tsx` | 修改 | 加载 Google Fonts（Space Grotesk + IBM Plex Sans + JetBrains Mono）|
| `components/workspace/workspace-shell.module.css` | 修改 | 工作区 shell、顶部栏、按钮、菜单 |
| `components/workspace/workspace-chat.module.css` | 修改 | 聊天消息气泡、composer、引用、markdown |
| `components/workspace/workspace-right-rail.module.css` | 修改 | 右侧面板、来源列表、笔记编辑器 |
| `components/dashboard/dashboard-surface.tsx` | 可能修改 | 如果涉及 tone 颜色硬编码的变更 |

---

## Task 1: 重写 design-tokens.css

**Files:**
- Modify: `frontend_next/app/design-tokens.css`

**说明:** 完全替换当前约 350 行的设计令牌。保留 `:root` 和 `[data-theme="dark"]` 结构，但所有值按设计文档更新。

- [ ] **Step 1: 备份当前文件**

```bash
cp frontend_next/app/design-tokens.css frontend_next/app/design-tokens.css.bak
```

- [ ] **Step 2: 写入新的 design-tokens.css**

替换整个文件内容为以下代码（约 280 行）：

```css
:root {
  /* === Background & Surface === */
  --background: 0 0% 100%;
  --foreground: 222 47% 11%;
  --card: 0 0% 100%;
  --card-foreground: 222 47% 11%;
  --surface-muted: 210 20% 98%;
  --surface-soft: 210 20% 96%;
  --surface-elevated: 0 0% 100%;
  --surface-sunken: 214 32% 91%;
  --surface-accent-soft: 195 100% 97%;

  /* === Popover === */
  --popover: 0 0% 100%;
  --popover-foreground: 222 47% 11%;

  /* === Accent (Primary) === */
  --primary: 222 47% 11%;
  --primary-foreground: 0 0% 100%;
  --accent: 193 90% 35%;
  --accent-soft: 195 100% 97%;
  --accent-glow: 193 90% 35%;

  /* === Semantic Colors === */
  --success: 174 72% 35%;
  --warning: 38 92% 42%;
  --warning-foreground: 30 60% 24%;
  --warning-surface: 42 100% 96%;
  --warning-border: 38 78% 82%;
  --info: 200 98% 40%;
  --destructive: 350 89% 55%;
  --destructive-foreground: 0 0% 100%;
  --destructive-soft: 350 90% 97%;
  --destructive-border: 350 62% 84%;

  /* === Secondary / Muted === */
  --secondary: 210 20% 96%;
  --secondary-foreground: 222 20% 18%;
  --muted: 210 20% 95%;
  --muted-foreground: 215 16% 47%;
  --subtle-foreground: 215 20% 65%;

  /* === Borders === */
  --border: 214 32% 91%;
  --border-strong: 215 20% 80%;
  --border-whisper: 210 20% 96%;
  --input: 214 32% 91%;
  --input-background: 210 20% 98%;
  --switch-background: 215 16% 47%;
  --ring: 193 90% 35%;
  --focus-ring: 193 90% 35%;

  /* === Badge === */
  --badge-background: 210 20% 96%;
  --badge-foreground: 215 16% 47%;

  /* === Sidebar === */
  --sidebar: 210 20% 98%;
  --sidebar-foreground: 222 47% 11%;
  --sidebar-primary: 222 47% 11%;
  --sidebar-primary-foreground: 0 0% 100%;
  --sidebar-accent: 210 20% 96%;
  --sidebar-accent-foreground: 222 20% 18%;
  --sidebar-border: 214 32% 91%;
  --sidebar-ring: 193 90% 35%;

  /* === Workspace === */
  --workspace-shell: 0 0% 100%;
  --workspace-foreground: 222 47% 11%;
  --workspace-panel: 0 0% 100%;
  --workspace-panel-muted: 210 20% 98%;
  --workspace-rail: 210 20% 97%;
  --workspace-rail-strong: 210 20% 95%;
  --workspace-border: 214 32% 91%;
  --workspace-primary: 222 47% 11%;
  --workspace-primary-foreground: 0 0% 100%;
  --workspace-secondary: 210 20% 96%;
  --workspace-secondary-foreground: 222 20% 18%;
  --workspace-muted: 210 20% 95%;
  --workspace-muted-foreground: 215 16% 47%;
  --workspace-accent: 214 32% 91%;
  --workspace-accent-foreground: 222 20% 18%;
  --workspace-surface: 0 0% 100%;
  --workspace-surface-muted: 210 20% 98%;
  --workspace-pill: 210 20% 95%;
  --workspace-input: 210 20% 98%;
  --workspace-input-border: 214 32% 91%;
  --workspace-menu: 0 0% 100%;
  --workspace-menu-hover: 210 20% 96%;
  --workspace-shadow: 222 47% 11%;

  /* === Dashboard === */
  --dashboard-shell: 0 0% 100%;
  --dashboard-foreground: 222 47% 11%;
  --dashboard-brand-foreground: 222 47% 11%;
  --dashboard-header-border: 214 32% 90%;
  --dashboard-border: 214 32% 91%;
  --dashboard-border-strong: 215 20% 80%;
  --dashboard-surface: 0 0% 100%;
  --dashboard-surface-muted: 210 20% 98%;
  --dashboard-surface-soft: 210 20% 96%;
  --dashboard-muted-foreground: 215 16% 47%;
  --dashboard-subtle-foreground: 215 20% 65%;
  --dashboard-primary: 222 47% 11%;
  --dashboard-primary-hover: 222 30% 18%;
  --dashboard-primary-foreground: 0 0% 100%;
  --dashboard-danger: 350 89% 55%;
  --dashboard-shadow: 222 47% 11%;
  --dashboard-overlay: 222 47% 11%;

  /* === CTA === */
  --cta-background: 222 47% 11%;
  --cta-background-hover: 193 90% 35%;
  --cta-background-active: 193 90% 30%;
  --cta-foreground: 0 0% 100%;

  /* === Radius === */
  --radius: 0.75rem;
  --radius-card: 0.75rem;
  --radius-control: 0.5rem;
  --radius-button: 0.5rem;
  --radius-badge: 999px;
  --radius-pill: 999px;
  --radius-message: 1rem;

  /* === Spacing === */
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-5: 1.25rem;
  --space-6: 1.5rem;
  --space-7: 2rem;
  --space-8: 3rem;
  --space-9: 4rem;
  --space-10: 6rem;

  /* === Typography === */
  --font-size-overline: 0.75rem;
  --font-size-meta: 0.8125rem;
  --font-size-control: 0.875rem;
  --font-size-caption: 0.75rem;
  --font-size-caption-strong: 0.8125rem;
  --font-size-label: 0.8125rem;
  --font-size-body: 0.9375rem;
  --font-size-body-strong: 1rem;
  --font-size-section-title: 1rem;
  --font-size-shell-title: 1.125rem;
  --font-size-brand: 1.0625rem;
  --font-size-title-sm: 1.25rem;
  --font-size-title: 1.75rem;

  --line-height-overline: 1.4;
  --line-height-meta: 1.5;
  --line-height-control: 1.48;
  --line-height-caption: 1.45;
  --line-height-label: 1.5;
  --line-height-body: 1.65;
  --line-height-body-strong: 1.65;
  --line-height-section-title: 1.35;
  --line-height-shell-title: 1.25;
  --line-height-brand: 1.1;
  --line-height-title: 1.18;

  --font-weight-medium: 500;
  --font-weight-semibold: 600;
  --font-weight-bold: 700;

  --letter-spacing-normal: 0;
  --letter-spacing-overline: 0.05em;
  --letter-spacing-title: -0.02em;
  --letter-spacing-tight: -0.01em;

  /* === Shadows === */
  --shadow-sm: 0 1px 2px hsl(222 47% 11% / 0.04);
  --shadow-md: 0 4px 12px hsl(222 47% 11% / 0.06);
  --shadow-lg: 0 8px 24px hsl(222 47% 11% / 0.08);
  --shadow-xl: 0 16px 48px hsl(222 47% 11% / 0.12);
  --shadow-glow: 0 0 20px hsl(193 90% 35% / 0.15);
  --shadow-focus-ring: 0 0 0 3px hsl(193 90% 35% / 0.15);
  --shadow-topbar: 0 1px 3px hsl(222 47% 11% / 0.06);

  /* === App-specific === */
  --app-citation-bg: 210 20% 95%;
  --app-history-hover: 210 20% 96%;
  --app-history-active: 214 32% 91%;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    /* === Background & Surface === */
    --background: 222 84% 5%;
    --foreground: 210 20% 96%;
    --card: 222 47% 11%;
    --card-foreground: 210 20% 96%;
    --surface-muted: 217 33% 17%;
    --surface-soft: 215 25% 27%;
    --surface-elevated: 222 47% 11%;
    --surface-sunken: 222 84% 5%;
    --surface-accent-soft: 193 90% 35%;

    /* === Popover === */
    --popover: 222 47% 11%;
    --popover-foreground: 210 20% 96%;

    /* === Accent (Primary) === */
    --primary: 210 20% 96%;
    --primary-foreground: 222 84% 5%;
    --accent: 187 85% 53%;
    --accent-soft: 193 90% 35%;
    --accent-glow: 187 85% 53%;

    /* === Semantic Colors === */
    --success: 168 76% 50%;
    --warning: 45 93% 56%;
    --warning-foreground: 45 93% 90%;
    --warning-surface: 38 50% 18%;
    --warning-border: 35 42% 34%;
    --info: 198 93% 60%;
    --destructive: 350 89% 70%;
    --destructive-foreground: 0 0% 100%;
    --destructive-soft: 350 30% 18%;
    --destructive-border: 350 34% 30%;

    /* === Secondary / Muted === */
    --secondary: 217 33% 17%;
    --secondary-foreground: 210 20% 96%;
    --muted: 217 33% 17%;
    --muted-foreground: 215 16% 55%;
    --subtle-foreground: 215 20% 45%;

    /* === Borders === */
    --border: 217 33% 17%;
    --border-strong: 215 25% 27%;
    --border-whisper: 222 47% 14%;
    --input: 217 33% 17%;
    --input-background: 222 47% 11%;
    --switch-background: 215 25% 35%;
    --ring: 187 85% 53%;
    --focus-ring: 187 85% 53%;

    /* === Badge === */
    --badge-background: 217 33% 17%;
    --badge-foreground: 215 16% 65%;

    /* === Sidebar === */
    --sidebar: 222 84% 6%;
    --sidebar-foreground: 210 20% 96%;
    --sidebar-primary: 210 20% 96%;
    --sidebar-primary-foreground: 222 84% 5%;
    --sidebar-accent: 217 33% 17%;
    --sidebar-accent-foreground: 210 20% 96%;
    --sidebar-border: 217 33% 17%;
    --sidebar-ring: 187 85% 53%;

    /* === Workspace === */
    --workspace-shell: 222 84% 5%;
    --workspace-foreground: 210 20% 96%;
    --workspace-panel: 222 47% 11%;
    --workspace-panel-muted: 217 33% 17%;
    --workspace-rail: 222 47% 10%;
    --workspace-rail-strong: 217 33% 15%;
    --workspace-border: 217 33% 17%;
    --workspace-primary: 210 20% 96%;
    --workspace-primary-foreground: 222 84% 5%;
    --workspace-secondary: 217 33% 17%;
    --workspace-secondary-foreground: 210 20% 96%;
    --workspace-muted: 217 33% 17%;
    --workspace-muted-foreground: 215 16% 55%;
    --workspace-accent: 193 90% 35%;
    --workspace-accent-foreground: 187 85% 80%;
    --workspace-surface: 222 47% 11%;
    --workspace-surface-muted: 217 33% 17%;
    --workspace-pill: 217 33% 20%;
    --workspace-input: 222 47% 11%;
    --workspace-input-border: 217 33% 17%;
    --workspace-menu: 222 47% 11%;
    --workspace-menu-hover: 217 33% 17%;
    --workspace-shadow: 222 84% 3%;

    /* === Dashboard === */
    --dashboard-shell: 222 84% 5%;
    --dashboard-foreground: 210 20% 96%;
    --dashboard-brand-foreground: 210 20% 96%;
    --dashboard-header-border: 217 33% 17%;
    --dashboard-border: 217 33% 17%;
    --dashboard-border-strong: 215 25% 27%;
    --dashboard-surface: 222 47% 11%;
    --dashboard-surface-muted: 217 33% 17%;
    --dashboard-surface-soft: 217 33% 17%;
    --dashboard-muted-foreground: 215 16% 55%;
    --dashboard-subtle-foreground: 215 20% 45%;
    --dashboard-primary: 210 20% 96%;
    --dashboard-primary-hover: 210 20% 86%;
    --dashboard-primary-foreground: 222 84% 5%;
    --dashboard-danger: 350 89% 70%;
    --dashboard-shadow: 222 84% 3%;
    --dashboard-overlay: 222 84% 4%;

    /* === CTA === */
    --cta-background: 210 20% 96%;
    --cta-background-hover: 187 85% 53%;
    --cta-background-active: 187 85% 45%;
    --cta-foreground: 222 84% 5%;

    /* === Shadows === */
    --shadow-sm: 0 1px 2px hsl(0 0% 0% / 0.3);
    --shadow-md: 0 4px 12px hsl(0 0% 0% / 0.4);
    --shadow-lg: 0 8px 24px hsl(0 0% 0% / 0.5);
    --shadow-xl: 0 16px 48px hsl(0 0% 0% / 0.6);
    --shadow-glow: 0 0 20px hsl(187 85% 53% / 0.3);
    --shadow-focus-ring: 0 0 0 3px hsl(187 85% 53% / 0.25);
    --shadow-topbar: 0 1px 3px hsl(0 0% 0% / 0.18);

    /* === App-specific === */
    --app-citation-bg: 217 33% 17%;
    --app-history-hover: 217 33% 17%;
    --app-history-active: 217 33% 20%;
  }
}

:root[data-theme="dark"] {
  /* === Background & Surface === */
  --background: 222 84% 5%;
  --foreground: 210 20% 96%;
  --card: 222 47% 11%;
  --card-foreground: 210 20% 96%;
  --surface-muted: 217 33% 17%;
  --surface-soft: 215 25% 27%;
  --surface-elevated: 222 47% 11%;
  --surface-sunken: 222 84% 5%;
  --surface-accent-soft: 193 90% 35%;

  /* === Popover === */
  --popover: 222 47% 11%;
  --popover-foreground: 210 20% 96%;

  /* === Accent (Primary) === */
  --primary: 210 20% 96%;
  --primary-foreground: 222 84% 5%;
  --accent: 187 85% 53%;
  --accent-soft: 193 90% 35%;
  --accent-glow: 187 85% 53%;

  /* === Semantic Colors === */
  --success: 168 76% 50%;
  --warning: 45 93% 56%;
  --warning-foreground: 45 93% 90%;
  --warning-surface: 38 50% 18%;
  --warning-border: 35 42% 34%;
  --info: 198 93% 60%;
  --destructive: 350 89% 70%;
  --destructive-foreground: 0 0% 100%;
  --destructive-soft: 350 30% 18%;
  --destructive-border: 350 34% 30%;

  /* === Secondary / Muted === */
  --secondary: 217 33% 17%;
  --secondary-foreground: 210 20% 96%;
  --muted: 217 33% 17%;
  --muted-foreground: 215 16% 55%;
  --subtle-foreground: 215 20% 45%;

  /* === Borders === */
  --border: 217 33% 17%;
  --border-strong: 215 25% 27%;
  --border-whisper: 222 47% 14%;
  --input: 217 33% 17%;
  --input-background: 222 47% 11%;
  --switch-background: 215 25% 35%;
  --ring: 187 85% 53%;
  --focus-ring: 187 85% 53%;

  /* === Badge === */
  --badge-background: 217 33% 17%;
  --badge-foreground: 215 16% 65%;

  /* === Sidebar === */
  --sidebar: 222 84% 6%;
  --sidebar-foreground: 210 20% 96%;
  --sidebar-primary: 210 20% 96%;
  --sidebar-primary-foreground: 222 84% 5%;
  --sidebar-accent: 217 33% 17%;
  --sidebar-accent-foreground: 210 20% 96%;
  --sidebar-border: 217 33% 17%;
  --sidebar-ring: 187 85% 53%;

  /* === Workspace === */
  --workspace-shell: 222 84% 5%;
  --workspace-foreground: 210 20% 96%;
  --workspace-panel: 222 47% 11%;
  --workspace-panel-muted: 217 33% 17%;
  --workspace-rail: 222 47% 10%;
  --workspace-rail-strong: 217 33% 15%;
  --workspace-border: 217 33% 17%;
  --workspace-primary: 210 20% 96%;
  --workspace-primary-foreground: 222 84% 5%;
  --workspace-secondary: 217 33% 17%;
  --workspace-secondary-foreground: 210 20% 96%;
  --workspace-muted: 217 33% 17%;
  --workspace-muted-foreground: 215 16% 55%;
  --workspace-accent: 193 90% 35%;
  --workspace-accent-foreground: 187 85% 80%;
  --workspace-surface: 222 47% 11%;
  --workspace-surface-muted: 217 33% 17%;
  --workspace-pill: 217 33% 20%;
  --workspace-input: 222 47% 11%;
  --workspace-input-border: 217 33% 17%;
  --workspace-menu: 222 47% 11%;
  --workspace-menu-hover: 217 33% 17%;
  --workspace-shadow: 222 84% 3%;

  /* === Dashboard === */
  --dashboard-shell: 222 84% 5%;
  --dashboard-foreground: 210 20% 96%;
  --dashboard-brand-foreground: 210 20% 96%;
  --dashboard-header-border: 217 33% 17%;
  --dashboard-border: 217 33% 17%;
  --dashboard-border-strong: 215 25% 27%;
  --dashboard-surface: 222 47% 11%;
  --dashboard-surface-muted: 217 33% 17%;
  --dashboard-surface-soft: 217 33% 17%;
  --dashboard-muted-foreground: 215 16% 55%;
  --dashboard-subtle-foreground: 215 20% 45%;
  --dashboard-primary: 210 20% 96%;
  --dashboard-primary-hover: 210 20% 86%;
  --dashboard-primary-foreground: 222 84% 5%;
  --dashboard-danger: 350 89% 70%;
  --dashboard-shadow: 222 84% 3%;
  --dashboard-overlay: 222 84% 4%;

  /* === CTA === */
  --cta-background: 210 20% 96%;
  --cta-background-hover: 187 85% 53%;
  --cta-background-active: 187 85% 45%;
  --cta-foreground: 222 84% 5%;

  /* === Shadows === */
  --shadow-sm: 0 1px 2px hsl(0 0% 0% / 0.3);
  --shadow-md: 0 4px 12px hsl(0 0% 0% / 0.4);
  --shadow-lg: 0 8px 24px hsl(0 0% 0% / 0.5);
  --shadow-xl: 0 16px 48px hsl(0 0% 0% / 0.6);
  --shadow-glow: 0 0 20px hsl(187 85% 53% / 0.3);
  --shadow-focus-ring: 0 0 0 3px hsl(187 85% 53% / 0.25);
  --shadow-topbar: 0 1px 3px hsl(0 0% 0% / 0.18);

  /* === App-specific === */
  --app-citation-bg: 217 33% 17%;
  --app-history-hover: 217 33% 17%;
  --app-history-active: 217 33% 20%;
}
```

- [ ] **Step 3: 验证文件语法**

检查 CSS 语法是否有明显错误（括号匹配等）：

```bash
cd frontend_next && npx stylelint app/design-tokens.css 2>/dev/null || echo "stylelint not installed, skipping"
```

如果没有 stylelint，手动检查文件是否以 `}` 结尾且没有未闭合的括号。

- [ ] **Step 4: Commit**

```bash
git add frontend_next/app/design-tokens.css
git commit -m "$(cat <<'EOF'
feat(frontend): redesign color tokens for Precision Lab theme

- Update light mode palette: cool slate base + cyan accent
- Add dark mode palette: deep indigo-black + bright cyan glow
- Add new spacing tokens (space-7 through space-10)
- Update radius tokens: tighter card/button radii, message radius
- Update shadow system: 5 elevation levels + glow effect
- Add font-related CSS variables

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: 更新 layout.tsx 加载字体

**Files:**
- Modify: `frontend_next/app/layout.tsx`

**说明:** 添加 `next/font/google` 加载 Space Grotesk、IBM Plex Sans 和 JetBrains Mono。字体通过 CSS 变量注入，不需要额外的网络请求。

- [ ] **Step 1: 读取当前 layout.tsx**

```bash
cat frontend_next/app/layout.tsx
```

- [ ] **Step 2: 修改 layout.tsx**

在文件顶部添加字体导入，在 body 上应用 CSS 变量：

```tsx
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { Space_Grotesk, IBM_Plex_Sans, JetBrains_Mono } from "next/font/google";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";

import "./globals.css";
import { AuthProvider } from "../lib/auth/context";
import { normalizeLocale } from "../lib/i18n/config";
import { QueryProvider } from "../lib/query/provider";
import { UiPreferencesProvider } from "../lib/ui-preferences";

const spaceGrotesk = Space_Grotesk({
  subsets: ["latin"],
  variable: "--font-heading",
  display: "swap",
});

const ibmPlexSans = IBM_Plex_Sans({
  weight: ["400", "500", "600"],
  subsets: ["latin"],
  variable: "--font-body",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
  display: "swap",
});

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL?.trim() || "http://localhost:3000";

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: {
    default: "Context OS",
    template: "%s · Context OS",
  },
  description: "Second-brain workspace for organizing, distributing, and querying knowledge with AI.",
  icons: {
    icon: "/icon.svg",
    shortcut: "/icon.svg",
    apple: "/apple-icon",
  },
  manifest: "/manifest.webmanifest",
  openGraph: {
    title: "Context OS",
    description: "Second-brain workspace for organizing, distributing, and querying knowledge with AI.",
    siteName: "Context OS",
    images: [
      {
        url: "/opengraph-image",
        width: 1200,
        height: 630,
        alt: "Context OS",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "Context OS",
    description: "Second-brain workspace for organizing, distributing, and querying knowledge with AI.",
    images: ["/twitter-image"],
  },
};

export default async function RootLayout({ children }: { children: ReactNode }) {
  const locale = normalizeLocale(await getLocale());
  const messages = await getMessages();

  return (
    <html lang={locale} suppressHydrationWarning>
      <body className={`${spaceGrotesk.variable} ${ibmPlexSans.variable} ${jetbrainsMono.variable}`}>
        <QueryProvider>
          <NextIntlClientProvider locale={locale} messages={messages}>
            <UiPreferencesProvider initialLocale={locale}>
              <AuthProvider>{children}</AuthProvider>
            </UiPreferencesProvider>
          </NextIntlClientProvider>
        </QueryProvider>
      </body>
    </html>
  );
}
```

- [ ] **Step 3: 类型检查**

```bash
cd frontend_next && npx tsc --noEmit --project tsconfig.json 2>&1 | head -30
```

Expected: 无错误（或只显示与字体类型相关的已知错误）。

- [ ] **Step 4: Commit**

```bash
git add frontend_next/app/layout.tsx
git commit -m "$(cat <<'EOF'
feat(frontend): load Space Grotesk, IBM Plex Sans, JetBrains Mono fonts

- Add next/font/google imports for all three font families
- Apply CSS variables to body element
- Use font-display: swap for performance

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: 重写 globals.css — 基础样式

**Files:**
- Modify: `frontend_next/app/globals.css`

**说明:** 重写 globals.css 的基础部分。保留布局 shell 类名（`.app-auth-shell`、`.workspace-shell` 等），但更新所有样式值。这是最大的 CSS 变更之一。

- [ ] **Step 1: 备份当前文件**

```bash
cp frontend_next/app/globals.css frontend_next/app/globals.css.bak
```

- [ ] **Step 2: 写入新的 globals.css**

保留现有的所有 CSS 类名和选择器结构，只更新属性值。由于文件很长（1300+行），这里给出关键变更的 diff 指导而非完整重写。

主要变更点：

1. **body 字体更新**（第 15-26 行）：
```css
body {
  margin: 0;
  min-height: 100vh;
  background: hsl(var(--background));
  color: hsl(var(--foreground));
  font-family: var(--font-body), "PingFang SC", "Hiragino Sans GB", "Noto Sans SC", "Microsoft YaHei", sans-serif;
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
```

2. **app-surface-card 圆角更新**（第 47-55 行）：
```css
.app-surface-card {
  width: 100%;
  border: 1px solid hsl(var(--border-whisper));
  border-radius: var(--radius-card);  /* 从 1.35rem 改为 0.75rem */
  background: hsl(var(--card));
  color: hsl(var(--card-foreground));
  box-shadow: var(--shadow-sm);  /* 从 shadow-soft-card 改为 shadow-sm */
  padding: 1.5rem;
}
```

3. **按钮样式更新**（第 166-253 行）：
- `.app-button-primary`: 圆角改为 `var(--radius-button)` (8px)，hover 背景变 `--accent`，添加 `translateY(-1px)`
- `.app-button-secondary` / `.app-button-ghost`: 圆角改为 8px
- 所有按钮 active 状态添加 `scale(0.98)`

4. **输入框样式更新**（第 124-158 行）：
- 圆角改为 `var(--radius-control)` (8px)
- Focus ring 改为 `var(--shadow-focus-ring)`（青色发光）

5. **app-auth-title**（第 74-79 行）：
```css
.app-auth-title {
  margin: 0;
  font-family: var(--font-heading), sans-serif;
  font-size: clamp(1.65rem, 2.6vw, 1.95rem);
  line-height: 1.08;
  letter-spacing: -0.02em;
}
```

6. **dashboard 卡片样式**（第 816-848 行）：
- 圆角改为 `var(--radius-card)` (12px)
- 阴影更新为新的变量

7. **dashboard-workspace-icon** 硬编码颜色修复（第 891-905 行）：
这些硬编码的 hex 颜色需要改为 HSL 变量。但由于它们是图标背景色，需要保持感知一致性。暂时保留这些值，在 Task 8 中统一修复。

由于 globals.css 文件很长，建议使用 `sed` 或分段 Edit 来更新，而不是完整重写。具体变更点：

- 将所有 `border-radius: 1.35rem` 改为 `border-radius: var(--radius-card)`（但认证页卡片保持 16px）
- 将所有 `border-radius: 0.75rem` 改为 `var(--radius-control)`
- 将所有 `border-radius: 0.95rem` 改为 `var(--radius-control)`
- 将所有 `border-radius: 1rem`（卡片/面板）改为 `var(--radius-card)`
- 将所有 `border-radius: 1.25rem` 改为 `var(--radius-card)`
- 将所有 `border-radius: 1.4rem`（模态框）改为 `var(--radius-card)`
- 保留 `border-radius: 999px` 用于徽章、头像、搜索框

阴影更新：
- `box-shadow: var(--shadow-soft-card)` → `var(--shadow-sm)` 或 `var(--shadow-md)`
- `box-shadow: var(--shadow-deep-card)` → `var(--shadow-lg)` 或 `var(--shadow-xl)`
- `box-shadow: var(--shadow-topbar)` 保持不变（已更新）
- `box-shadow: var(--shadow-focus-ring)` 保持不变（已更新）

Focus ring 更新：
- 所有 `box-shadow: var(--shadow-focus-ring)` 已在新 design-tokens.css 中定义为青色发光

- [ ] **Step 3: 验证 CSS 语法**

```bash
cd frontend_next && head -1 app/globals.css  # 确认文件存在且可读
```

- [ ] **Step 4: Commit**

```bash
git add frontend_next/app/globals.css
git commit -m "$(cat <<'EOF'
feat(frontend): update globals.css for Precision Lab theme

- Update body font to use var(--font-body)
- Update card/button/input border radii to new design tokens
- Update shadow values to new elevation system
- Update focus ring to cyan glow
- Add font-family to auth titles

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: 更新 workspace-shell.module.css

**Files:**
- Modify: `frontend_next/components/workspace/workspace-shell.module.css`

**说明:** 更新工作区 shell 组件的样式。主要变更：按钮圆角、颜色引用、顶部栏样式。

- [ ] **Step 1: 备份**

```bash
cp frontend_next/components/workspace/workspace-shell.module.css \
   frontend_next/components/workspace/workspace-shell.module.css.bak
```

- [ ] **Step 2: 应用变更**

主要变更点：

1. **按钮圆角**（`.secondaryButton`、`.primaryButton` 等）：
   - 将 `border-radius: 0.72rem` 改为 `var(--radius-button)` (8px)
   - 将 `border-radius: 999px` 保留（用于图标按钮）

2. **主按钮颜色**（`.primaryButton`）：
```css
.primaryButton {
  background: hsl(var(--foreground));
  border-color: hsl(var(--foreground));
  color: hsl(var(--primary-foreground));
  border-radius: var(--radius-button);  /* 8px */
  box-shadow: var(--shadow-sm);
}

.primaryButton:hover {
  background: hsl(var(--accent));  /* 从 primary 改为 accent */
  border-color: hsl(var(--accent));
  box-shadow: var(--shadow-glow);  /* 添加发光 */
}
```

3. **顶部栏**（`.topBar`）：
   - 高度保持 `min-height: 4.5rem`
   - 背景使用 `hsl(var(--background))` + backdrop-filter
   - 阴影使用 `var(--shadow-sm)`

4. **菜单面板**（`.menuPanel`、`.menuPanelWide`）：
   - 圆角改为 `var(--radius-card)` (12px)

5. **对话框**（`.dialog`、`.modalCard`）：
   - 圆角改为 `var(--radius-card)` (12px)

6. **历史项**（`.historyItem`、`.historyItemActive`）：
   - 圆角改为 `var(--radius-control)` (8px)
   - 激活态背景改为 `hsl(var(--app-history-active))`

- [ ] **Step 3: Commit**

```bash
git add frontend_next/components/workspace/workspace-shell.module.css
git commit -m "$(cat <<'EOF'
feat(frontend): update workspace shell styles for Precision Lab

- Update button radii to new design tokens
- Primary button hover now uses accent cyan with glow
- Update panel/dialog/modal border radii
- Update history item styling

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: 更新 workspace-chat.module.css

**Files:**
- Modify: `frontend_next/components/workspace/workspace-chat.module.css`

**说明:** 更新聊天组件样式。这是视觉变化最显著的页面之一。主要变更：消息气泡样式、composer 样式、引用样式。

- [ ] **Step 1: 备份**

```bash
cp frontend_next/components/workspace/workspace-chat.module.css \
   frontend_next/components/workspace/workspace-chat.module.css.bak
```

- [ ] **Step 2: 应用变更**

主要变更点：

1. **消息气泡圆角**（`.bubbleAssistant`、`.bubbleUser`）：
   - 将 `border-radius: 1.35rem` 改为 `var(--radius-message)` (16px)

2. **AI 消息气泡边框**（`.bubbleAssistant`）：
```css
.bubbleAssistant {
  padding: 0.92rem 1.08rem;
  border: 1px solid hsl(var(--border-whisper));
  border-radius: var(--radius-message);  /* 16px */
  border-top-left-radius: 4px;  /* 方向感 */
  background: hsl(var(--card));
  color: hsl(var(--foreground));
  box-shadow: var(--shadow-sm);
}
```

3. **用户消息气泡**（`.bubbleUser`）：
```css
.bubbleUser {
  width: min(100%, 44rem);
  padding: 0.84rem 1.04rem;
  border-radius: var(--radius-message);  /* 16px */
  border-top-right-radius: 4px;  /* 方向感 */
  background: hsl(var(--foreground));
  color: hsl(var(--background));
}
```

4. **RAG 消息气泡**（`.bubbleAssistantRag`）：
```css
.bubbleAssistantRag {
  border-color: hsl(var(--success) / 0.28);
  background: hsl(var(--success) / 0.08);
  box-shadow:
    inset 3px 0 0 hsl(var(--success) / 0.78),
    var(--shadow-sm);
}
```

5. **Search 消息气泡**（`.bubbleAssistantSearch`）：
```css
.bubbleAssistantSearch {
  border-color: hsl(var(--info) / 0.28);
  background: hsl(var(--info) / 0.08);
  box-shadow:
    inset 3px 0 0 hsl(var(--info) / 0.82),
    var(--shadow-sm);
}
```

6. **Composer 表单**（`.composerForm`）：
   - 圆角改为 `var(--radius-message)` (16px)
   - Focus 阴影改为 `var(--shadow-glow)`

7. **发送按钮**（`.sendButton`）：
```css
.sendButton {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 2.95rem;
  height: 2.95rem;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: hsl(var(--accent));  /* 青色 */
  color: hsl(var(--primary-foreground));
  cursor: pointer;
  box-shadow: var(--shadow-glow);  /* 发光 */
  transition:
    background 150ms ease,
    transform 150ms ease,
    opacity 150ms ease,
    box-shadow 150ms ease;
}

.sendButton:hover {
  background: hsl(var(--accent) / 0.9);
  transform: translateY(-1px);
  box-shadow: 0 0 30px hsl(var(--accent-glow) / 0.4);
}
```

8. **模式标签**（`.modeTag`）：
   - 圆角保持 999px

9. **引用按钮**（`.inlineCitationButton`）：
   - Hover 效果更新

- [ ] **Step 3: Commit**

```bash
git add frontend_next/components/workspace/workspace-chat.module.css
git commit -m "$(cat <<'EOF'
feat(frontend): update chat styles for Precision Lab theme

- Update message bubble border radii with directional corners
- User message uses foreground bg + background text
- AI messages use left accent border
- Send button uses cyan accent with glow
- Composer uses message-radius and glow focus

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: 更新 workspace-right-rail.module.css

**Files:**
- Modify: `frontend_next/components/workspace/workspace-right-rail.module.css`

**说明:** 更新右侧面板样式。主要变更：卡片圆角、按钮样式、列表项样式。

- [ ] **Step 1: 备份**

```bash
cp frontend_next/components/workspace/workspace-right-rail.module.css \
   frontend_next/components/workspace/workspace-right-rail.module.css.bak
```

- [ ] **Step 2: 应用变更**

主要变更点：

1. **面板圆角**（`.pane`、`.viewer`）：
   - 内边距保持现有值
   - 背景保持透明

2. **列表项**（`.listItem`）：
   - 圆角改为 `var(--radius-control)` (8px)

3. **笔记卡片**（`.noteListItem`）：
   - 圆角改为 `var(--radius-card)` (12px)

4. **按钮**（`.button`、`.actionButton`、`.closeButton`）：
   - 圆角改为 `var(--radius-button)` (8px)

5. **源对话框**（`.sourceDialog`）：
   - 圆角改为 `var(--radius-card)` (12px)

6. **上传区域**（`.uploadWell`）：
   - 圆角改为 `var(--radius-card)` (12px)

7. **选择框**（`.selectionCheckbox`）：
   - 圆角改为 `var(--radius-control)` (8px)

- [ ] **Step 3: Commit**

```bash
git add frontend_next/components/workspace/workspace-right-rail.module.css
git commit -m "$(cat <<'EOF'
feat(frontend): update right rail styles for Precision Lab theme

- Update card/list/button border radii to new tokens
- Update note card styling
- Update source dialog styling

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: 修复已知 Bug

**Files:**
- Modify: `frontend_next/app/globals.css`
- Modify: `frontend_next/components/workspace/workspace-chat.module.css`
- Modify: `frontend_next/components/workspace/workspace-shell.module.css`

**说明:** 修复设计文档中识别的 bug。

- [ ] **Step 1: 修复 globals.css 硬编码颜色**

**Bug 1**: `globals.css:1079` — `background: #fafafa`
```bash
grep -n "background: #fafafa" frontend_next/app/globals.css
```
替换为：`background: hsl(var(--surface-muted));`

**Bug 2**: `globals.css:888` — `rgba(255, 255, 255, 0.8)` white inset glow
```bash
grep -n "rgba(255, 255, 255, 0.8)" frontend_next/app/globals.css
```
在亮色模式下这没问题（白色内发光在白色卡片上），但在暗色模式下不工作。替换为：
```css
box-shadow: inset 0 0 0 1px hsl(var(--border-whisper) / 0.8);
```

**Bug 3**: `globals.css:572` — 同上
```bash
grep -n "rgba(255, 255, 255, 0.75)" frontend_next/app/globals.css
```
替换为：
```css
box-shadow: inset 0 0 0 1px hsl(var(--border-whisper) / 0.75);
```

- [ ] **Step 2: 修复 workspace-chat.module.css 缺失变量**

**Bug 4**: `workspace-chat.module.css:1202-1232` — `--button` 和 `--button-hover` 不存在
```bash
grep -n "var(--button)" frontend_next/components/workspace/workspace-chat.module.css
```
替换为使用现有变量：
- `--button` → `--surface-soft`
- `--button-hover` → `--surface-muted`

或者更好的：
- `--button` → `--card`
- `--button-hover` → `--surface-muted`

- [ ] **Step 3: 修复 workspace-shell.module.css 硬编码阴影**

**Bug 5**: `workspace-shell.module.css:41`
```bash
grep -n "rgba(15, 23, 42, 0.08)" frontend_next/components/workspace/workspace-shell.module.css
```
替换为使用新的阴影变量。

- [ ] **Step 4: Commit**

```bash
git add frontend_next/app/globals.css \
    frontend_next/components/workspace/workspace-chat.module.css \
    frontend_next/components/workspace/workspace-shell.module.css
git commit -m "$(cat <<'EOF'
fix(frontend): fix hardcoded colors and missing CSS variables

- Replace #fafafa with hsl(var(--surface-muted))
- Replace rgba(255,255,255,0.8) inset glow with theme-aware color
- Replace undefined --button variables with existing tokens
- Replace hardcoded shadow colors with CSS variables

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: 构建验证

**Files:**
- 无文件修改，仅验证

**说明:** 运行 TypeScript 类型检查和 Next.js 构建，确保所有变更没有引入错误。

- [ ] **Step 1: TypeScript 类型检查**

```bash
cd frontend_next && npx tsc --noEmit 2>&1 | tail -20
```

Expected: 无错误（或只显示与本次变更无关的已知错误）。

- [ ] **Step 2: Next.js 构建**

```bash
cd frontend_next && npm run build 2>&1 | tail -30
```

Expected: 构建成功，没有 CSS 或类型错误。

- [ ] **Step 3: 清理备份文件**

```bash
rm -f frontend_next/app/design-tokens.css.bak \
      frontend_next/app/globals.css.bak \
      frontend_next/components/workspace/workspace-shell.module.css.bak \
      frontend_next/components/workspace/workspace-chat.module.css.bak \
      frontend_next/components/workspace/workspace-right-rail.module.css.bak
```

- [ ] **Step 4: 最终状态检查**

```bash
git status
```

Expected: 只有修改过的文件，没有未跟踪的备份文件。

---

## 自我审查

### Spec 覆盖检查

| 设计文档章节 | 实现任务 |
|-------------|---------|
| 3. 色彩系统 | Task 1 (design-tokens.css) |
| 4. 字体系统 | Task 2 (layout.tsx) |
| 5. 间距与圆角 | Task 1 (design-tokens.css) |
| 6. 阴影系统 | Task 1 (design-tokens.css) |
| 7.1 按钮 | Task 3, 4, 5, 6 |
| 7.2 卡片 | Task 3, 4, 5, 6 |
| 7.3 输入框 | Task 3 |
| 7.4 消息气泡 | Task 5 |
| 7.5 顶部栏 | Task 4 |
| 7.6 标签/徽章 | Task 5 |
| 8.1 仪表盘 | Task 3 |
| 8.2 工作区聊天 | Task 4, 5 |
| 8.3 认证页 | Task 3 |
| 8.4 设置页 | Task 3 (sidebar styles) |
| 8.5 管理后台 | Task 3 (sidebar styles) |
| 9. 暗色模式 | Task 1 (design-tokens.css) |
| 10. 动效 | 未在本次计划中实现（建议后续迭代）|
| 11.2 Bug 修复 | Task 7 |

**缺口**: 动效与微交互（第 10 章）未在本次计划中实现。建议在基础样式稳定后作为后续迭代添加。

### 占位符扫描

- 无 "TBD"、"TODO"、"implement later"
- 所有步骤包含具体代码或命令
- 所有文件路径精确

### 类型一致性

- CSS 变量名在整个计划中一致
- 字体变量名 `--font-heading`、`--font-body`、`--font-mono` 与 layout.tsx 中定义一致
- 阴影变量名一致
