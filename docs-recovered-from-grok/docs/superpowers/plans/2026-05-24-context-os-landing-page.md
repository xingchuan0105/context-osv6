# Context-OS 品牌主页搭建与部署实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 创建一个基于 Next.js 15 的静态品牌主页，部署到 VPS 的 `/var/www/context-os-landing/`，并配置 Nginx + SSL。

**Architecture:** 使用 Next.js `output: 'export'` 模式生成纯静态 HTML/CSS/JS，Nginx 直接 serving，无需 Node.js 运行时。

**Tech Stack:** Next.js 15 + React 19 + TypeScript + Tailwind CSS + Geist 字体

---

## 文件结构

```
/home/chuan/context-os-landing/
├── app/
│   ├── layout.tsx              # 根布局，加载 Geist 字体
│   ├── page.tsx                # 主页，组合所有 section
│   ├── globals.css             # Tailwind directives + CSS 变量
│   └── sections/
│       ├── Navbar.tsx          # 固定导航栏
│       ├── HeroEntries.tsx     # 品牌入口区（三个卡片）
│       ├── Features.tsx        # 产品功能亮点 2x2 网格
│       ├── LatestPosts.tsx     # 博客文章预览（调用 Ghost API）
│       ├── Creator.tsx         # 创作者介绍
│       ├── Social.tsx          # 社交链接
│       └── Footer.tsx          # Footer
├── components/ui/
│   ├── Button.tsx              # 可复用按钮组件
│   ├── Card.tsx                # 可复用卡片组件
│   └── Badge.tsx               # 标签 badge 组件
├── lib/
│   └── ghost.ts                # Ghost Content API 客户端
├── public/
│   └── logo.svg                # Context-OS logo（SVG）
├── next.config.js              # Static export config
├── tailwind.config.ts          # Tailwind + 自定义颜色/字体
├── tsconfig.json
└── package.json
```

---

### Task 1: 初始化 Next.js 15 项目

**Files:**
- Create: `/home/chuan/context-os-landing/package.json`
- Create: `/home/chuan/context-os-landing/tsconfig.json`
- Create: `/home/chuan/context-os-landing/next.config.js`
- Create: `/home/chuan/context-os-landing/tailwind.config.ts`
- Create: `/home/chuan/context-os-landing/postcss.config.mjs`

- [ ] **Step 1: 创建项目目录并初始化 package.json**

```bash
mkdir -p /home/chuan/context-os-landing
```

`/home/chuan/context-os-landing/package.json`:
```json
{
  "name": "context-os-landing",
  "version": "1.0.0",
  "private": true,
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "start": "next start",
    "lint": "next lint"
  },
  "dependencies": {
    "next": "15.2.0",
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "geist": "^1.3.0"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "autoprefixer": "^10.4.20",
    "postcss": "^8.5.0",
    "tailwindcss": "^3.4.17",
    "typescript": "^5.7.0"
  }
}
```

- [ ] **Step 2: 创建 tsconfig.json**

`/home/chuan/context-os-landing/tsconfig.json`:
```json
{
  "compilerOptions": {
    "lib": ["dom", "dom.iterable", "esnext"],
    "allowJs": true,
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true,
    "esModuleInterop": true,
    "module": "esnext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "preserve",
    "incremental": true,
    "plugins": [{ "name": "next" }],
    "paths": {
      "@/*": ["./*"]
    }
  },
  "include": ["next-env.d.ts", "**/*.ts", "**/*.tsx", ".next/types/**/*.ts"],
  "exclude": ["node_modules"]
}
```

- [ ] **Step 3: 创建 next.config.js（静态导出模式）**

`/home/chuan/context-os-landing/next.config.js`:
```js
/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'export',
  distDir: 'dist',
  images: {
    unoptimized: true
  }
};

module.exports = nextConfig;
```

- [ ] **Step 4: 创建 tailwind.config.ts**

`/home/chuan/context-os-landing/tailwind.config.ts`:
```typescript
import type { Config } from 'tailwindcss';

const config: Config = {
  content: [
    './app/**/*.{js,ts,jsx,tsx,mdx}',
    './components/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        background: '#0a0a0a',
        foreground: 'rgba(255,255,255,0.92)',
        muted: 'rgba(255,255,255,0.60)',
        subtle: 'rgba(255,255,255,0.35)',
        border: 'rgba(255,255,255,0.08)',
        accent: '#10b981',
        'accent-hover': '#34d399',
        'accent-bg': 'rgba(16,185,129,0.10)',
        surface: 'rgba(255,255,255,0.03)',
        'surface-hover': 'rgba(255,255,255,0.06)',
        footer: '#050505',
      },
      fontFamily: {
        sans: ['Geist', 'PingFang SC', 'Microsoft YaHei', 'Noto Sans SC', 'sans-serif'],
      },
      borderRadius: {
        button: '6px',
        card: '12px',
        section: '16px',
      },
    },
  },
  plugins: [],
};

export default config;
```

- [ ] **Step 5: 创建 postcss.config.mjs**

`/home/chuan/context-os-landing/postcss.config.mjs`:
```javascript
/** @type {import('postcss-load-config').Config} */
const config = {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};

export default config;
```

- [ ] **Step 6: 安装依赖**

```bash
cd /home/chuan/context-os-landing
npm install
```

Expected: `node_modules/` 目录创建完成，无报错。

- [ ] **Step 7: Commit**

```bash
cd /home/chuan/context-os-landing
git init
git add .
git commit -m "chore: initialize Next.js 15 project with Tailwind"
```

---

### Task 2: 创建全局样式和根布局

**Files:**
- Create: `/home/chuan/context-os-landing/app/globals.css`
- Create: `/home/chuan/context-os-landing/app/layout.tsx`

- [ ] **Step 1: 创建 globals.css**

`/home/chuan/context-os-landing/app/globals.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

:root {
  --background: #0a0a0a;
  --foreground: rgba(255, 255, 255, 0.92);
  --muted: rgba(255, 255, 255, 0.60);
  --subtle: rgba(255, 255, 255, 0.35);
  --border: rgba(255, 255, 255, 0.08);
  --accent: #10b981;
  --accent-hover: #34d399;
  --accent-bg: rgba(16, 185, 129, 0.10);
  --surface: rgba(255, 255, 255, 0.03);
  --surface-hover: rgba(255, 255, 255, 0.06);
  --footer: #050505;
}

body {
  background-color: var(--background);
  color: var(--foreground);
  font-family: 'Geist', 'PingFang SC', 'Microsoft YaHei', 'Noto Sans SC', sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

::selection {
  background-color: var(--accent-bg);
  color: var(--accent);
}
```

- [ ] **Step 2: 创建根布局 layout.tsx**

`/home/chuan/context-os-landing/app/layout.tsx`:
```tsx
import type { Metadata } from 'next';
import { GeistSans } from 'geist/font/sans';
import './globals.css';

export const metadata: Metadata = {
  title: 'Context-OS — 研究工具集',
  description: '由邢川创建的智能文档研究助手、创作者博客与趣味工具。',
  keywords: ['Context-OS', 'RAG', '文档研究', '知识管理'],
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="zh-CN" className={GeistSans.variable}>
      <body className={`${GeistSans.className} min-h-screen`}>
        {children}
      </body>
    </html>
  );
}
```

- [ ] **Step 3: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add global styles and root layout with Geist font"
```

---

### Task 3: 创建可复用 UI 组件

**Files:**
- Create: `/home/chuan/context-os-landing/components/ui/Button.tsx`
- Create: `/home/chuan/context-os-landing/components/ui/Card.tsx`
- Create: `/home/chuan/context-os-landing/components/ui/Badge.tsx`

- [ ] **Step 1: 创建 Button 组件**

`/home/chuan/context-os-landing/components/ui/Button.tsx`:
```tsx
import React from 'react';

interface ButtonProps {
  children: React.ReactNode;
  href?: string;
  variant?: 'primary' | 'outline' | 'ghost';
  size?: 'sm' | 'md';
  className?: string;
  onClick?: () => void;
}

export function Button({
  children,
  href,
  variant = 'primary',
  size = 'md',
  className = '',
  onClick,
}: ButtonProps) {
  const baseClasses = 'inline-flex items-center justify-center font-semibold transition-all duration-200 rounded-button focus:outline-none focus:ring-2 focus:ring-accent focus:ring-offset-2 focus:ring-offset-background';

  const variantClasses = {
    primary: 'bg-accent text-background hover:bg-accent-hover',
    outline: 'border border-border text-foreground hover:bg-surface-hover hover:border-accent',
    ghost: 'text-foreground hover:bg-surface-hover',
  };

  const sizeClasses = {
    sm: 'px-3 py-1.5 text-sm',
    md: 'px-4 py-2 text-[15px]',
  };

  const classes = `${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`;

  if (href) {
    return (
      <a href={href} className={classes}>
        {children}
      </a>
    );
  }

  return (
    <button onClick={onClick} className={classes}>
      {children}
    </button>
  );
}
```

- [ ] **Step 2: 创建 Card 组件**

`/home/chuan/context-os-landing/components/ui/Card.tsx`:
```tsx
import React from 'react';

interface CardProps {
  children: React.ReactNode;
  className?: string;
  hover?: boolean;
}

export function Card({ children, className = '', hover = true }: CardProps) {
  return (
    <div
      className={`bg-surface border border-border rounded-card p-6 ${
        hover ? 'transition-all duration-200 hover:bg-surface-hover hover:border-accent/30' : ''
      } ${className}`}
    >
      {children}
    </div>
  );
}
```

- [ ] **Step 3: 创建 Badge 组件**

`/home/chuan/context-os-landing/components/ui/Badge.tsx`:
```tsx
import React from 'react';

interface BadgeProps {
  children: React.ReactNode;
  className?: string;
}

export function Badge({ children, className = '' }: BadgeProps) {
  return (
    <span
      className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-accent-bg text-accent ${className}`}
    >
      {children}
    </span>
  );
}
```

- [ ] **Step 4: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add reusable UI components (Button, Card, Badge)"
```

---

### Task 4: 创建 Ghost API 客户端

**Files:**
- Create: `/home/chuan/context-os-landing/lib/ghost.ts`

- [ ] **Step 1: 创建 Ghost Content API 客户端**

`/home/chuan/context-os-landing/lib/ghost.ts`:
```typescript
export interface GhostPost {
  id: string;
  title: string;
  slug: string;
  excerpt: string;
  published_at: string;
  tags: { name: string }[];
  feature_image: string | null;
}

export interface GhostPostsResponse {
  posts: GhostPost[];
}

const GHOST_URL = 'https://blog.contextlm.top';
const GHOST_KEY = 'YOUR_GHOST_CONTENT_API_KEY'; // 需要从 Ghost Admin → Integrations 获取

export async function fetchLatestPosts(limit: number = 3): Promise<GhostPost[]> {
  try {
    const url = new URL(`${GHOST_URL}/ghost/api/content/posts/`);
    url.searchParams.set('key', GHOST_KEY);
    url.searchParams.set('limit', String(limit));
    url.searchParams.set('fields', 'id,title,slug,excerpt,published_at,tags,feature_image');
    url.searchParams.set('include', 'tags');

    const response = await fetch(url.toString(), { next: { revalidate: 3600 } });

    if (!response.ok) {
      throw new Error(`Ghost API error: ${response.status}`);
    }

    const data: GhostPostsResponse = await response.json();
    return data.posts || [];
  } catch (error) {
    console.error('Failed to fetch Ghost posts:', error);
    return [];
  }
}

export function formatDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  });
}
```

**注意**：需要在 Ghost Admin → Settings → Integrations 中创建一个 Custom Integration，获取 Content API Key，然后替换 `GHOST_KEY`。

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add Ghost Content API client for latest posts"
```

---

### Task 5: 创建 Navbar 组件

**Files:**
- Create: `/home/chuan/context-os-landing/app/sections/Navbar.tsx`

- [ ] **Step 1: 创建 Navbar**

`/home/chuan/context-os-landing/app/sections/Navbar.tsx`:
```tsx
'use client';

import React, { useState, useEffect } from 'react';
import { Button } from '@/components/ui/Button';

export function Navbar() {
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setScrolled(window.scrollY > 20);
    };
    window.addEventListener('scroll', handleScroll);
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 h-16 flex items-center border-b transition-all duration-200 ${
        scrolled
          ? 'bg-background/80 backdrop-blur-md border-border'
          : 'bg-background border-transparent'
      }`}
    >
      <div className="w-full max-w-6xl mx-auto px-6 flex items-center justify-between">
        {/* Logo */}
        <a href="/" className="flex items-center gap-2 text-foreground hover:opacity-80 transition-opacity">
          <svg width="28" height="28" viewBox="0 0 28 28" fill="none" xmlns="http://www.w3.org/2000/svg">
            <rect width="28" height="28" rx="6" fill="#10b981" />
            <path d="M8 8h12v2H8V8zm0 5h8v2H8v-2zm0 5h10v2H8v-2z" fill="#0a0a0a" />
          </svg>
          <span className="text-lg font-bold tracking-tight">Context-OS</span>
        </a>

        {/* Nav Links */}
        <nav className="hidden md:flex items-center gap-6">
          <a href="https://app.contextlm.top" className="text-[15px] font-medium text-muted hover:text-foreground transition-colors">
            应用
          </a>
          <a href="https://blog.contextlm.top" className="text-[15px] font-medium text-muted hover:text-foreground transition-colors">
            博客
          </a>
          <a href="https://whyimright.contextlm.top" className="text-[15px] font-medium text-muted hover:text-foreground transition-colors">
            Why I Am Right
          </a>
          <div className="w-px h-4 bg-border" />
          <Button href="https://app.contextlm.top" size="sm">
            进入应用 →
          </Button>
        </nav>
      </div>
    </header>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add Navbar with scroll-aware backdrop blur"
```

---

### Task 6: 创建 HeroEntries 组件

**Files:**
- Create: `/home/chuan/context-os-landing/app/sections/HeroEntries.tsx`

- [ ] **Step 1: 创建 HeroEntries**

`/home/chuan/context-os-landing/app/sections/HeroEntries.tsx`:
```tsx
import React from 'react';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';

const entries = [
  {
    title: 'Context-OS',
    subtitle: '智能文档研究助手',
    href: 'https://app.contextlm.top',
    cta: '进入应用 →',
    icon: (
      <svg className="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
      </svg>
    ),
  },
  {
    title: '创作者博客',
    subtitle: '邢川的观点与观察',
    href: 'https://blog.contextlm.top',
    cta: '阅读文章 →',
    icon: (
      <svg className="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125M18 14v4.75A2.25 2.25 0 0115.75 21H5.25A2.25 2.25 0 013 18.75V8.25A2.25 2.25 0 015.25 6H10" />
      </svg>
    ),
  },
  {
    title: 'Why I Am Right',
    subtitle: '你的偏见，值得被证实',
    href: 'https://whyimright.contextlm.top',
    cta: '开始抬杠 →',
    icon: (
      <svg className="w-8 h-8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 00-2.455 2.456zM16.894 20.567L16.5 21.75l-.394-1.183a2.25 2.25 0 00-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 001.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 001.423 1.423l1.183.394-1.183.394a2.25 2.25 0 00-1.423 1.423z" />
      </svg>
    ),
  },
];

export function HeroEntries() {
  return (
    <section className="pt-32 pb-20 px-6">
      <div className="max-w-6xl mx-auto text-center">
        <h1 className="text-5xl md:text-6xl font-bold tracking-tight text-foreground">
          Context-OS
        </h1>
        <p className="mt-4 text-xl text-muted">
          由邢川创建的研究工具集
        </p>

        <div className="mt-16 grid grid-cols-1 md:grid-cols-3 gap-6">
          {entries.map((entry) => (
            <Card key={entry.title} className="text-left">
              <div className="text-accent mb-4">{entry.icon}</div>
              <h3 className="text-xl font-bold text-foreground">{entry.title}</h3>
              <p className="mt-2 text-[15px] text-muted">{entry.subtitle}</p>
              <div className="mt-6">
                <Button href={entry.href} variant="outline" size="sm">
                  {entry.cta}
                </Button>
              </div>
            </Card>
          ))}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add HeroEntries section with three product cards"
```

---

### Task 7: 创建 Features 组件

**Files:**
- Create: `/home/chuan/context-os-landing/app/sections/Features.tsx`

- [ ] **Step 1: 创建 Features**

`/home/chuan/context-os-landing/app/sections/Features.tsx`:
```tsx
import React from 'react';
import { Card } from '@/components/ui/Card';

const features = [
  {
    title: '上传与解析',
    description: '自动解析 PDF、Office、网页、图片为可检索单元',
    icon: (
      <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M9 8.25H7.5a2.25 2.25 0 00-2.25 2.25v9a2.25 2.25 0 002.25 2.25h9a2.25 2.25 0 002.25-2.25v-9a2.25 2.25 0 00-2.25-2.25H15m0-3l-3-3m0 0l-3 3m3-3V15" />
      </svg>
    ),
  },
  {
    title: '精准检索',
    description: '基于 RAG 生成低幻觉、带精准引用的回答',
    icon: (
      <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
      </svg>
    ),
  },
  {
    title: '引用溯源',
    description: '每个观点都标注出处，点击跳转原文',
    icon: (
      <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M13.19 8.688a4.5 4.5 0 011.242 7.244l-4.5 4.5a4.5 4.5 0 01-6.364-6.364l1.757-1.757m13.35-.622l1.757-1.757a4.5 4.5 0 00-6.364-6.364l-4.5 4.5a4.5 4.5 0 001.242 7.244" />
      </svg>
    ),
  },
  {
    title: '多模态支持',
    description: '文本、图表、扫描页统一检索',
    icon: (
      <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M2.25 15.75l5.159-5.159a2.25 2.25 0 013.182 0l5.159 5.159m-1.5-1.5l1.409-1.409a2.25 2.25 0 013.182 0l2.909 2.909m-18 3.75h16.5a1.5 1.5 0 001.5-1.5V6a1.5 1.5 0 00-1.5-1.5H3.75A1.5 1.5 0 002.25 6v12a1.5 1.5 0 001.5 1.5zm10.5-11.25h.008v.008h-.008V8.25zm.375 0a.375.375 0 11-.75 0 .375.375 0 01.75 0z" />
      </svg>
    ),
  },
];

export function Features() {
  return (
    <section className="py-20 px-6">
      <div className="max-w-6xl mx-auto">
        <h2 className="text-[28px] font-semibold text-foreground text-center mb-12">
          Context-OS 能做什么
        </h2>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {features.map((feature) => (
            <Card key={feature.title} className="text-left">
              <div className="text-accent mb-4">{feature.icon}</div>
              <h3 className="text-lg font-semibold text-foreground">{feature.title}</h3>
              <p className="mt-2 text-[15px] text-muted leading-relaxed">{feature.description}</p>
            </Card>
          ))}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add Features section with 2x2 grid"
```

---

### Task 8: 创建 LatestPosts 组件

**Files:**
- Create: `/home/chuan/context-os-landing/app/sections/LatestPosts.tsx`

- [ ] **Step 1: 创建 LatestPosts（服务端组件，调用 Ghost API）**

`/home/chuan/context-os-landing/app/sections/LatestPosts.tsx`:
```tsx
import React from 'react';
import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { fetchLatestPosts, formatDate } from '@/lib/ghost';

export async function LatestPosts() {
  const posts = await fetchLatestPosts(3);

  return (
    <section className="py-20 px-6">
      <div className="max-w-6xl mx-auto">
        <h2 className="text-[28px] font-semibold text-foreground text-center mb-12">
          创作者博客最新文章
        </h2>

        {posts.length === 0 ? (
          <p className="text-center text-muted">暂无文章</p>
        ) : (
          <>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              {posts.map((post) => (
                <a
                  key={post.id}
                  href={`https://blog.contextlm.top/${post.slug}`}
                  className="block group"
                >
                  <Card className="h-full">
                    <h3 className="text-lg font-semibold text-foreground group-hover:text-accent transition-colors line-clamp-2">
                      {post.title}
                    </h3>
                    <p className="mt-3 text-[15px] text-muted line-clamp-2 leading-relaxed">
                      {post.excerpt || '暂无摘要'}
                    </p>
                    <div className="mt-4 flex items-center gap-2 flex-wrap">
                      <span className="text-sm text-subtle">{formatDate(post.published_at)}</span>
                      {post.tags?.slice(0, 2).map((tag) => (
                        <Badge key={tag.name}>{tag.name}</Badge>
                      ))}
                    </div>
                  </Card>
                </a>
              ))}
            </div>

            <div className="mt-10 text-center">
              <a
                href="https://blog.contextlm.top"
                className="text-[15px] font-medium text-accent hover:text-accent-hover transition-colors"
              >
                查看全部文章 →
              </a>
            </div>
          </>
        )}
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add LatestPosts section fetching from Ghost API"
```

---

### Task 9: 创建 Creator、Social、Footer 组件

**Files:**
- Create: `/home/chuan/context-os-landing/app/sections/Creator.tsx`
- Create: `/home/chuan/context-os-landing/app/sections/Social.tsx`
- Create: `/home/chuan/context-os-landing/app/sections/Footer.tsx`

- [ ] **Step 1: 创建 Creator**

`/home/chuan/context-os-landing/app/sections/Creator.tsx`:
```tsx
import React from 'react';

export function Creator() {
  return (
    <section className="py-20 px-6">
      <div className="max-w-3xl mx-auto">
        <div className="flex flex-col md:flex-row items-center gap-8">
          {/* Avatar placeholder */}
          <div className="w-[120px] h-[120px] rounded-full bg-surface border border-border flex items-center justify-center flex-shrink-0">
            <svg className="w-12 h-12 text-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path strokeLinecap="round" strokeLinejoin="round" d="M15.75 6a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0zM4.501 20.118a7.5 7.5 0 0114.998 0A17.933 17.933 0 0112 21.75c-2.676 0-5.216-.584-7.499-1.632z" />
            </svg>
          </div>

          <div className="text-center md:text-left">
            <h3 className="text-2xl font-bold text-foreground">邢川</h3>
            <p className="mt-3 text-base text-muted leading-relaxed">
              研究工具的创造者。相信好的工具应该让人更专注于思考本身，而不是被信息淹没。在这里记录我对技术、商业和研究方法的观察。
            </p>
            <p className="mt-3 text-sm text-subtle">
              主理 Context-OS、创作者博客与 Why I Am Right
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 2: 创建 Social**

`/home/chuan/context-os-landing/app/sections/Social.tsx`:
```tsx
import React from 'react';

const socials = [
  {
    name: 'Twitter / X',
    href: 'https://twitter.com',
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
        <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
      </svg>
    ),
  },
  {
    name: 'GitHub',
    href: 'https://github.com',
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
        <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
      </svg>
    ),
  },
  {
    name: '邮箱',
    href: 'mailto:admin@contextlm.com',
    icon: (
      <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path strokeLinecap="round" strokeLinejoin="round" d="M21.75 6.75v10.5a2.25 2.25 0 01-2.25 2.25h-15a2.25 2.25 0 01-2.25-2.25V6.75m19.5 0A2.25 2.25 0 0019.5 4.5h-15a2.25 2.25 0 00-2.25 2.25m19.5 0v.243a2.25 2.25 0 01-1.07 1.916l-7.5 4.615a2.25 2.25 0 01-2.36 0L3.32 8.91a2.25 2.25 0 01-1.07-1.916V6.75" />
      </svg>
    ),
  },
];

export function Social() {
  return (
    <section className="py-16 px-6">
      <div className="max-w-3xl mx-auto text-center">
        <h3 className="text-xl font-semibold text-foreground">关注更新</h3>
        <div className="mt-6 flex items-center justify-center gap-4">
          {socials.map((social) => (
            <a
              key={social.name}
              href={social.href}
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-2 px-4 py-2 rounded-button bg-surface border border-border text-muted hover:text-foreground hover:border-accent/30 transition-all duration-200"
            >
              {social.icon}
              <span className="text-sm font-medium">{social.name}</span>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 3: 创建 Footer**

`/home/chuan/context-os-landing/app/sections/Footer.tsx`:
```tsx
import React from 'react';

export function Footer() {
  return (
    <footer className="py-10 px-6 bg-footer border-t border-border">
      <div className="max-w-6xl mx-auto flex flex-col md:flex-row items-center justify-between gap-4">
        <div className="flex items-center gap-2 text-muted">
          <svg width="20" height="20" viewBox="0 0 28 28" fill="none">
            <rect width="28" height="28" rx="6" fill="#10b981" />
            <path d="M8 8h12v2H8V8zm0 5h8v2H8v-2zm0 5h10v2H8v-2z" fill="#0a0a0a" />
          </svg>
          <span className="text-sm">© 2026 Context-OS</span>
        </div>

        <nav className="flex items-center gap-4 text-sm text-muted">
          <a href="https://app.contextlm.top" className="hover:text-foreground transition-colors">应用</a>
          <span className="text-border">·</span>
          <a href="https://blog.contextlm.top" className="hover:text-foreground transition-colors">博客</a>
          <span className="text-border">·</span>
          <a href="https://whyimright.contextlm.top" className="hover:text-foreground transition-colors">Why I Am Right</a>
        </nav>
      </div>
    </footer>
  );
}
```

- [ ] **Step 4: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: add Creator, Social, and Footer sections"
```

---

### Task 10: 组装主页 page.tsx

**Files:**
- Create: `/home/chuan/context-os-landing/app/page.tsx`

- [ ] **Step 1: 创建 page.tsx 组合所有 section**

`/home/chuan/context-os-landing/app/page.tsx`:
```tsx
import { Navbar } from './sections/Navbar';
import { HeroEntries } from './sections/HeroEntries';
import { Features } from './sections/Features';
import { LatestPosts } from './sections/LatestPosts';
import { Creator } from './sections/Creator';
import { Social } from './sections/Social';
import { Footer } from './sections/Footer';

export default function Home() {
  return (
    <main className="min-h-screen">
      <Navbar />
      <HeroEntries />
      <Features />
      <LatestPosts />
      <Creator />
      <Social />
      <Footer />
    </main>
  );
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "feat: assemble homepage with all sections"
```

---

### Task 11: 本地构建验证

**Files:**
- Modify: `/home/chuan/context-os-landing/lib/ghost.ts`（替换 GHOST_KEY）

- [ ] **Step 1: 先设置一个临时的 Ghost API Key 用于测试**

如果还没有 Ghost Content API Key，可以先用一个空字符串让构建通过（组件会显示"暂无文章"）。

`/home/chuan/context-os-landing/lib/ghost.ts` 第 8 行：
```typescript
const GHOST_KEY = process.env.GHOST_CONTENT_API_KEY || '';
```

- [ ] **Step 2: 运行构建**

```bash
cd /home/chuan/context-os-landing
npm run build
```

Expected:
- `dist/` 目录生成
- 包含 `index.html` 和所有静态资源
- 无报错（如果 Ghost API Key 为空，LatestPosts 会显示"暂无文章"，这是预期行为）

- [ ] **Step 3: 本地预览（可选）**

```bash
cd /home/chuan/context-os-landing
cd dist && python3 -m http.server 3456
```

在浏览器打开 `http://localhost:3456`，验证页面渲染正常。

- [ ] **Step 4: Commit**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "chore: verify local build"
```

---

### Task 12: 部署到 VPS

**Files:**
- VPS: `/var/www/context-os-landing/` (新目录)
- VPS: `/etc/nginx/conf.d/context-os-landing.conf` (新配置)

- [ ] **Step 1: 上传构建产物到 VPS**

```bash
cd /home/chuan/context-os-landing
rsync -avz --delete dist/ root@43.161.220.253:/var/www/context-os-landing/
```

Expected: 所有文件成功上传到 VPS 的 `/var/www/context-os-landing/`。

- [ ] **Step 2: 创建 Nginx 配置文件**

在 VPS 上创建 `/etc/nginx/conf.d/context-os-landing.conf`：

```nginx
server {
    listen 443 ssl http2;
    server_name contextlm.top www.contextlm.top;

    ssl_certificate /etc/letsencrypt/live/blog.contextlm.top/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/blog.contextlm.top/privkey.pem;
    include /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam /etc/letsencrypt/ssl-dhparams.pem;

    root /var/www/context-os-landing;
    index index.html;

    location / {
        try_files $uri $uri.html $uri/ =404;
    }
}
```

**注意**：这里复用了 `blog.contextlm.top` 的 SSL 证书，因为 `contextlm.top` 和 `blog.contextlm.top` 可能在同一张通配符证书下。如果证书不包含 `contextlm.top`，需要单独申请。

- [ ] **Step 3: 检查 Nginx 配置并重载**

```bash
ssh root@43.161.220.253 "nginx -t && systemctl reload nginx"
```

Expected: `nginx: configuration file /etc/nginx/nginx.conf test is successful`

- [ ] **Step 4: 验证部署**

浏览器访问 `https://contextlm.top`，确认页面正常显示。

- [ ] **Step 5: Commit 部署配置**

```bash
cd /home/chuan/context-os-landing
git add .
git commit -m "deploy: deploy landing page to VPS"
```

---

## Self-Review

### Spec Coverage

| Spec 要求 | 对应任务 |
|---|---|
| Next.js 15 项目结构 | Task 1 |
| Geist 字体 | Task 2 |
| Tailwind 自定义颜色/字体 | Task 1 |
| 深色主题 CSS 变量 | Task 2 |
| 固定导航栏 + blur | Task 5 |
| 品牌入口区 + 三个卡片 | Task 6 |
| 产品功能亮点 2x2 | Task 7 |
| Ghost API 文章预览 | Task 4, 8 |
| 创作者介绍 | Task 9 |
| 社交链接 | Task 9 |
| Footer | Task 9 |
| 静态导出 + Nginx 部署 | Task 1, 12 |

无遗漏。

### Placeholder Scan

- 无 TBD/TODO
- Ghost API Key 使用环境变量或空字符串降级，无阻塞
- 所有代码完整

### Type Consistency

- `GhostPost`, `GhostPostsResponse` 接口在 `lib/ghost.ts` 中定义，`LatestPosts.tsx` 中正确消费
- Card, Button, Badge 组件的 props 类型在全站一致

---

## 执行选项

Plan complete and saved to `docs/superpowers/plans/2026-05-24-context-os-landing-page.md`.

**Next:** 继续编写 Plan B（whyimright 视觉对齐）和 Plan C（博客主题定制），或直接进入实施。
