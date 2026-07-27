# Why I Am Right 视觉对齐实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 whyimright 的视觉风格对齐到 Context-OS 品牌体系：统一 accent 色、字体、导航栏和 Footer。

**Architecture:** 在现有 whyiamright 前端项目中修改配色变量、添加统一导航栏组件、更新 Footer，保持所有功能不变。

**Tech Stack:** Next.js 15 + Tailwind CSS（与主页一致）

---

## 文件结构

现有项目位置：`/home/chuan/whyiamright/frontend/`

需要修改的文件：
- `/home/chuan/whyiamright/frontend/tailwind.config.ts` — 更新颜色变量
- `/home/chuan/whyiamright/frontend/app/layout.tsx` — 确认 Geist 字体加载
- `/home/chuan/whyiamright/frontend/app/page.tsx` 或相关页面 — 添加统一导航栏
- `/home/chuan/whyiamright/frontend/components/` — 新增或修改导航栏、Footer 组件
- `/home/chuan/whyiamright/frontend/public/favicon.ico` — 替换

---

### Task 1: 检查当前 whyimright 的技术栈

**Files:**
- Read: `/home/chuan/whyiamright/frontend/package.json`
- Read: `/home/chuan/whyiamright/frontend/tailwind.config.ts`
- Read: `/home/chuan/whyiamright/frontend/app/layout.tsx`

- [ ] **Step 1: 检查 package.json 确认依赖**

```bash
cat /home/chuan/whyiamright/frontend/package.json | grep -E '"next"|"tailwindcss"|"geist"'
```

Expected: 确认 next、tailwindcss、geist 的版本。

- [ ] **Step 2: 检查 tailwind.config.ts 当前颜色配置**

```bash
cat /home/chuan/whyiamright/frontend/tailwind.config.ts
```

记录当前的 color/theme 配置，后续需要对比修改。

- [ ] **Step 3: 检查 layout.tsx 当前字体配置**

```bash
cat /home/chuan/whyiamright/frontend/app/layout.tsx
```

确认是否已使用 Geist 字体。

---

### Task 2: 更新 Tailwind 颜色变量

**Files:**
- Modify: `/home/chuan/whyiamright/frontend/tailwind.config.ts`

- [ ] **Step 1: 更新 tailwind.config.ts 添加 Context-OS 品牌色**

在现有配置的基础上，添加/更新以下颜色（不删除原有颜色，避免破坏现有 UI）：

```typescript
// 在 theme.extend.colors 中添加：
'cos-accent': '#10b981',
'cos-accent-hover': '#34d399',
'cos-accent-bg': 'rgba(16,185,129,0.10)',
'cos-background': '#0a0a0a',
'cos-surface': 'rgba(255,255,255,0.03)',
'cos-surface-hover': 'rgba(255,255,255,0.06)',
'cos-border': 'rgba(255,255,255,0.08)',
'cos-text': 'rgba(255,255,255,0.92)',
'cos-muted': 'rgba(255,255,255,0.60)',
```

- [ ] **Step 2: 更新全局样式或 CSS 变量**

如果项目有全局 CSS 文件，添加 CSS 变量：

```css
:root {
  --cos-accent: #10b981;
  --cos-accent-hover: #34d399;
  --cos-background: #0a0a0a;
}
```

- [ ] **Step 3: Commit**

```bash
cd /home/chuan/whyiamright
git add .
git commit -m "style: add Context-OS brand colors to Tailwind config"
```

---

### Task 3: 确认 Geist 字体统一

**Files:**
- Modify: `/home/chuan/whyiamright/frontend/app/layout.tsx`

- [ ] **Step 1: 检查并统一字体**

如果当前 layout.tsx 未使用 Geist，修改为：

```tsx
import { GeistSans } from 'geist/font/sans';

// 在 html 标签上：
<html lang="zh-CN" className={GeistSans.variable}>
  <body className={GeistSans.className}>
```

如果已经使用了 Geist，确认配置和主页一致即可。

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/whyiamright
git add .
git commit -m "style: unify font to Geist"
```

---

### Task 4: 创建统一导航栏组件

**Files:**
- Create: `/home/chuan/whyiamright/frontend/components/UnifiedNavbar.tsx`

- [ ] **Step 1: 创建 UnifiedNavbar 组件**

```tsx
'use client';

import React from 'react';
import Link from 'next/link';

export function UnifiedNavbar() {
  return (
    <header className="fixed top-0 left-0 right-0 z-50 h-14 bg-[#0a0a0a]/90 backdrop-blur-md border-b border-white/[0.08]">
      <div className="w-full max-w-6xl mx-auto px-4 h-full flex items-center justify-between">
        {/* Logo */}
        <a
          href="https://contextlm.top"
          className="flex items-center gap-2 text-white/[0.92] hover:opacity-80 transition-opacity"
        >
          <svg width="24" height="24" viewBox="0 0 28 28" fill="none">
            <rect width="28" height="28" rx="6" fill="#10b981" />
            <path d="M8 8h12v2H8V8zm0 5h8v2H8v-2zm0 5h10v2H8v-2z" fill="#0a0a0a" />
          </svg>
          <span className="text-base font-bold">Context-OS</span>
        </a>

        {/* Nav */}
        <nav className="flex items-center gap-4 text-sm">
          <a
            href="https://app.contextlm.top"
            className="text-white/60 hover:text-white/90 transition-colors"
          >
            应用
          </a>
          <a
            href="https://blog.contextlm.top"
            className="text-white/60 hover:text-white/90 transition-colors"
          >
            博客
          </a>
          <span className="text-white/20">|</span>
          <span className="text-[#10b981] font-medium">Why I Am Right</span>
        </nav>
      </div>
    </header>
  );
}
```

- [ ] **Step 2: 将导航栏添加到主布局**

修改 `/home/chuan/whyiamright/frontend/app/layout.tsx` 或 `/home/chuan/whyiamright/frontend/app/page.tsx`，在 body 顶部添加 `<UnifiedNavbar />`。

同时确保页面内容有 `pt-14`（导航栏高度）的 padding，避免内容被遮挡。

- [ ] **Step 3: Commit**

```bash
cd /home/chuan/whyiamright
git add .
git commit -m "feat: add unified navbar linking to brand homepage"
```

---

### Task 5: 更新 Footer

**Files:**
- Modify: `/home/chuan/whyiamright/frontend/components/Footer.tsx`（或创建）

- [ ] **Step 1: 更新 Footer 为统一品牌信息**

```tsx
import React from 'react';

export function Footer() {
  return (
    <footer className="py-6 px-4 bg-[#050505] border-t border-white/[0.08]">
      <div className="max-w-6xl mx-auto flex flex-col md:flex-row items-center justify-between gap-3 text-sm text-white/60">
        <div className="flex items-center gap-2">
          <svg width="16" height="16" viewBox="0 0 28 28" fill="none">
            <rect width="28" height="28" rx="6" fill="#10b981" />
            <path d="M8 8h12v2H8V8zm0 5h8v2H8v-2zm0 5h10v2H8v-2z" fill="#0a0a0a" />
          </svg>
          <span>Why I Am Right · Context-OS 旗下</span>
        </div>
        <nav className="flex items-center gap-3">
          <a href="https://contextlm.top" className="hover:text-white/90 transition-colors">主页</a>
          <a href="https://app.contextlm.top" className="hover:text-white/90 transition-colors">应用</a>
          <a href="https://blog.contextlm.top" className="hover:text-white/90 transition-colors">博客</a>
        </nav>
      </div>
    </footer>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/whyiamright
git add .
git commit -m "style: update footer with Context-OS branding"
```

---

### Task 6: 更新页面标题和 favicon

**Files:**
- Modify: `/home/chuan/whyiamright/frontend/app/layout.tsx`（metadata）
- Replace: `/home/chuan/whyiamright/frontend/public/favicon.ico`

- [ ] **Step 1: 更新页面 metadata**

```tsx
export const metadata = {
  title: 'Why I Am Right · Context-OS',
  description: '你的偏见，值得被证实。Context-OS 旗下趣味工具。',
};
```

- [ ] **Step 2: 替换 favicon**

将主页项目的 logo.svg 转换为 favicon.ico（可以使用在线转换工具或 imagemagick），替换到 whyimright 的 public 目录。

或者，如果暂时没有 favicon，可以先创建一个简单的 SVG favicon：

`/home/chuan/whyiamright/frontend/app/icon.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 28 28">
  <rect width="28" height="28" rx="6" fill="#10b981"/>
  <path d="M8 8h12v2H8V8zm0 5h8v2H8v-2zm0 5h10v2H8v-2z" fill="#0a0a0a"/>
</svg>
```

- [ ] **Step 3: Commit**

```bash
cd /home/chuan/whyiamright
git add .
git commit -m "chore: update page title and favicon"
```

---

### Task 7: 局部调整 accent 色使用

**Files:**
- 搜索并修改 whyimright 中使用原有 accent/主题色的组件

- [ ] **Step 1: 搜索现有 accent 色使用**

```bash
cd /home/chuan/whyiamright/frontend
grep -rn "green-\|blue-\|indigo-\|primary" --include="*.tsx" --include="*.ts" --include="*.css" app/ components/
```

- [ ] **Step 2: 将关键 accent 替换为品牌青绿**

对于按钮、链接、高亮等关键交互元素，将原有 accent 色替换为 `#10b981` 或 `cos-accent` Tailwind class。

**注意**：只修改品牌层面的 accent，不要修改功能性的颜色（如错误红色、警告黄色等）。

- [ ] **Step 3: Commit**

```bash
cd /home/chuan/whyiamright
git add .
git commit -m "style: replace accent colors with Context-OS brand green"
```

---

### Task 8: 构建并部署

**Files:**
- Docker: whyimright 的 docker-compose.yml

- [ ] **Step 1: 本地构建验证**

```bash
cd /home/chuan/whyiamright/frontend
npm run build
```

Expected: 构建成功，无错误。

- [ ] **Step 2: 重新部署 Docker 容器**

```bash
cd /home/chuan/whyiamright
docker compose up -d --build
```

- [ ] **Step 3: 验证**

浏览器访问 `https://whyimright.contextlm.top`，确认：
- 顶部出现统一导航栏
- Footer 显示 "Context-OS 旗下"
- 页面标题正确
- accent 色已更新为青绿

---

## Self-Review

### Spec Coverage

| Spec 要求 | 对应任务 |
|---|---|
| accent 色统一为 `#10b981` | Task 2, 7 |
| 深色背景统一为 `#0a0a0a` | Task 2 |
| 字体统一为 Geist | Task 3 |
| 顶部统一导航栏 | Task 4 |
| Footer 更新 | Task 5 |
| Favicon 统一 | Task 6 |
| 浏览器标题格式 | Task 6 |

### Placeholder Scan

无 TBD/TODO，所有代码完整。

### 风险

- whyimright 原有 UI 可能有大量自定义颜色，只应修改品牌层面的 accent，避免破坏功能色
- 导航栏会增加页面顶部高度，需确认所有页面有正确的 padding
