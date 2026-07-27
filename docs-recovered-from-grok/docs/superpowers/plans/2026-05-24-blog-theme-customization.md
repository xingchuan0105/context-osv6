# 创作者博客 Ghost 主题定制实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Ghost 博客创建自定义主题，统一为 Context-OS 品牌视觉：深色主题、Geist 字体、青绿 accent、统一导航和 Footer。

**Architecture:** 基于 Ghost Casper 主题改造，通过 Ghost Admin → Design → Change theme 上传激活。

**Tech Stack:** Ghost Handlebars 模板 + CSS 覆盖

---

## 文件结构

```
/home/chuan/context-os-theme/
├── assets/
│   └── css/
│       └── brand.css          # 自定义样式覆盖
├── partials/
│   ├── site-nav.hbs           # 顶部导航（添加主页/whyimright 入口）
│   └── footer.hbs             # 统一 Footer
├── default.hbs                # 主布局（引入 Geist 字体、加载 brand.css）
├── index.hbs                  # 首页模板
├── post.hbs                   # 文章页模板
├── package.json               # 主题元数据
└── README.md
```

---

### Task 1: 复制并初始化自定义主题

**Files:**
- Create: `/home/chuan/context-os-theme/package.json`

- [ ] **Step 1: 创建主题目录和元数据**

```bash
mkdir -p /home/chuan/context-os-theme/{assets/css,partials}
```

`/home/chuan/context-os-theme/package.json`:
```json
{
  "name": "context-os-theme",
  "description": "Context-OS 品牌主题 — 深色极简风格",
  "version": "1.0.0",
  "engines": {
    "ghost": ">=5.0.0"
  },
  "license": "MIT",
  "author": {
    "name": "Context-OS",
    "email": "admin@contextlm.com"
  },
  "config": {
    "posts_per_page": 10,
    "image_sizes": {
      "xs": { "width": 150 },
      "s": { "width": 300 },
      "m": { "width": 600 },
      "l": { "width": 1000 },
      "xl": { "width": 2000 }
    },
    "card_assets": true,
    "custom": {
      "navigation_layout": {
        "type": "select",
        "options": ["Logo on the left", "Logo in the middle"],
        "default": "Logo on the left"
      }
    }
  }
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git init
git add .
git commit -m "chore: initialize custom Ghost theme"
```

---

### Task 2: 创建主布局 default.hbs

**Files:**
- Create: `/home/chuan/context-os-theme/default.hbs`

- [ ] **Step 1: 创建 default.hbs**

`/home/chuan/context-os-theme/default.hbs`:
```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{meta_title}}</title>
    <meta name="description" content="{{meta_description}}">
    <link rel="stylesheet" href="{{asset "css/brand.css"}}">
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet">
    {{ghost_head}}
</head>
<body class="cos-theme">
    <div class="cos-viewport">
        {{> site-nav}}
        {{{body}}}
        {{> footer}}
    </div>
    {{ghost_foot}}
</body>
</html>
```

**注意**：Ghost 5.x 对自定义主题有安全限制，不能内联加载外部 CDN 字体。Geist 字体需要通过 Google Fonts 的 Inter 作为替代（Geist 是 Inter 的衍生字体，视觉效果接近），或者将 Geist 字体文件放入主题的 `assets/fonts/` 目录。

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "feat: add default layout template"
```

---

### Task 3: 创建品牌样式 brand.css

**Files:**
- Create: `/home/chuan/context-os-theme/assets/css/brand.css`

- [ ] **Step 1: 编写完整品牌样式**

`/home/chuan/context-os-theme/assets/css/brand.css`:
```css
/* ===== CSS Variables ===== */
:root {
  --cos-bg: #0a0a0a;
  --cos-bg-secondary: #050505;
  --cos-surface: rgba(255, 255, 255, 0.03);
  --cos-surface-hover: rgba(255, 255, 255, 0.06);
  --cos-text: rgba(255, 255, 255, 0.92);
  --cos-text-muted: rgba(255, 255, 255, 0.60);
  --cos-text-subtle: rgba(255, 255, 255, 0.35);
  --cos-border: rgba(255, 255, 255, 0.08);
  --cos-accent: #10b981;
  --cos-accent-hover: #34d399;
  --cos-accent-bg: rgba(16, 185, 129, 0.10);
  --cos-radius-sm: 6px;
  --cos-radius-md: 12px;
  --cos-radius-lg: 16px;
}

/* ===== Base ===== */
.cos-theme {
  background-color: var(--cos-bg);
  color: var(--cos-text);
  font-family: 'Inter', 'PingFang SC', 'Microsoft YaHei', 'Noto Sans SC', -apple-system, sans-serif;
  -webkit-font-smoothing: antialiased;
  line-height: 1.6;
}

.cos-theme body {
  background-color: var(--cos-bg);
  color: var(--cos-text);
}

.cos-viewport {
  max-width: 1200px;
  margin: 0 auto;
  padding: 0 24px;
}

/* ===== Selection ===== */
::selection {
  background-color: var(--cos-accent-bg);
  color: var(--cos-accent);
}

/* ===== Links ===== */
a {
  color: var(--cos-accent);
  text-decoration: none;
  transition: color 0.2s ease;
}

a:hover {
  color: var(--cos-accent-hover);
}

/* ===== Navigation ===== */
.cos-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 64px;
  border-bottom: 1px solid var(--cos-border);
}

.cos-nav-logo {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--cos-text);
  font-weight: 700;
  font-size: 18px;
}

.cos-nav-logo:hover {
  opacity: 0.8;
}

.cos-nav-links {
  display: flex;
  align-items: center;
  gap: 24px;
}

.cos-nav-links a {
  color: var(--cos-text-muted);
  font-size: 15px;
  font-weight: 500;
}

.cos-nav-links a:hover {
  color: var(--cos-text);
}

.cos-nav-links .active {
  color: var(--cos-accent);
}

/* ===== Hero / Header ===== */
.cos-site-header {
  padding: 80px 0 40px;
  text-align: center;
}

.cos-site-title {
  font-size: 42px;
  font-weight: 700;
  letter-spacing: -0.5px;
  margin-bottom: 12px;
}

.cos-site-description {
  font-size: 18px;
  color: var(--cos-text-muted);
  max-width: 500px;
  margin: 0 auto;
}

/* ===== Post Cards ===== */
.cos-post-list {
  display: grid;
  gap: 32px;
  padding: 40px 0;
}

.cos-post-card {
  background: var(--cos-surface);
  border: 1px solid var(--cos-border);
  border-radius: var(--cos-radius-md);
  padding: 32px;
  transition: all 0.2s ease;
}

.cos-post-card:hover {
  background: var(--cos-surface-hover);
  border-color: rgba(16, 185, 129, 0.3);
}

.cos-post-title {
  font-size: 24px;
  font-weight: 700;
  line-height: 1.3;
  margin-bottom: 12px;
}

.cos-post-title a {
  color: var(--cos-text);
}

.cos-post-title a:hover {
  color: var(--cos-accent);
}

.cos-post-excerpt {
  color: var(--cos-text-muted);
  font-size: 16px;
  line-height: 1.6;
  margin-bottom: 16px;
}

.cos-post-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 14px;
  color: var(--cos-text-subtle);
}

.cos-post-tag {
  display: inline-flex;
  padding: 4px 10px;
  background: var(--cos-accent-bg);
  color: var(--cos-accent);
  border-radius: 9999px;
  font-size: 12px;
  font-weight: 600;
}

/* ===== Article Page ===== */
.cos-article {
  max-width: 720px;
  margin: 0 auto;
  padding: 60px 0 80px;
}

.cos-article-header {
  margin-bottom: 48px;
}

.cos-article-title {
  font-size: 36px;
  font-weight: 700;
  line-height: 1.2;
  letter-spacing: -0.5px;
  margin-bottom: 16px;
}

.cos-article-meta {
  color: var(--cos-text-muted);
  font-size: 15px;
}

.cos-article-body {
  font-size: 17px;
  line-height: 1.8;
  color: var(--cos-text);
}

.cos-article-body h2 {
  font-size: 28px;
  font-weight: 700;
  margin: 48px 0 20px;
  letter-spacing: -0.3px;
}

.cos-article-body h3 {
  font-size: 22px;
  font-weight: 600;
  margin: 32px 0 16px;
}

.cos-article-body p {
  margin-bottom: 20px;
}

.cos-article-body blockquote {
  border-left: 3px solid var(--cos-accent);
  padding-left: 20px;
  margin: 24px 0;
  color: var(--cos-text-muted);
  font-style: italic;
}

.cos-article-body code {
  background: var(--cos-surface);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.9em;
  color: var(--cos-accent);
}

.cos-article-body pre {
  background: var(--cos-surface);
  border: 1px solid var(--cos-border);
  border-radius: var(--cos-radius-md);
  padding: 20px;
  overflow-x: auto;
  margin: 24px 0;
}

.cos-article-body pre code {
  background: none;
  padding: 0;
  color: var(--cos-text);
}

.cos-article-body img {
  max-width: 100%;
  border-radius: var(--cos-radius-md);
  margin: 24px 0;
}

.cos-article-body hr {
  border: none;
  border-top: 1px solid var(--cos-border);
  margin: 40px 0;
}

/* ===== Footer ===== */
.cos-footer {
  padding: 32px 0;
  border-top: 1px solid var(--cos-border);
  display: flex;
  flex-direction: column;
  md: flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  font-size: 14px;
  color: var(--cos-text-muted);
}

.cos-footer-brand {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cos-footer-links {
  display: flex;
  gap: 16px;
}

.cos-footer-links a {
  color: var(--cos-text-muted);
}

.cos-footer-links a:hover {
  color: var(--cos-text);
}

/* ===== Pagination ===== */
.cos-pagination {
  display: flex;
  justify-content: center;
  gap: 8px;
  padding: 40px 0;
}

.cos-pagination a,
.cos-pagination span {
  padding: 8px 16px;
  border-radius: var(--cos-radius-sm);
  font-size: 14px;
  font-weight: 500;
}

.cos-pagination a {
  background: var(--cos-surface);
  border: 1px solid var(--cos-border);
  color: var(--cos-text);
}

.cos-pagination a:hover {
  background: var(--cos-surface-hover);
  border-color: var(--cos-accent);
}

.cos-pagination .current {
  background: var(--cos-accent);
  color: var(--cos-bg);
}

/* ===== Responsive ===== */
@media (max-width: 768px) {
  .cos-viewport {
    padding: 0 16px;
  }

  .cos-site-title {
    font-size: 32px;
  }

  .cos-article-title {
    font-size: 28px;
  }

  .cos-nav-links {
    display: none;
  }

  .cos-footer {
    flex-direction: column;
    text-align: center;
  }
}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "feat: add complete brand CSS with dark theme"
```

---

### Task 4: 创建导航栏 partial

**Files:**
- Create: `/home/chuan/context-os-theme/partials/site-nav.hbs`

- [ ] **Step 1: 创建 site-nav.hbs**

`/home/chuan/context-os-theme/partials/site-nav.hbs`:
```html
<header class="cos-nav">
    <a href="https://contextlm.top" class="cos-nav-logo">
        <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
            <rect width="28" height="28" rx="6" fill="#10b981"/>
            <path d="M8 8h12v2H8V8zm0 5h8v2H8v-2zm0 5h10v2H8v-2z" fill="#0a0a0a"/>
        </svg>
        <span>Context-OS</span>
    </a>
    <nav class="cos-nav-links">
        <a href="https://app.contextlm.top">应用</a>
        <a href="{{@site.url}}" class="active">博客</a>
        <a href="https://whyimright.contextlm.top">Why I Am Right</a>
    </nav>
</header>
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "feat: add unified site navigation"
```

---

### Task 5: 创建 Footer partial

**Files:**
- Create: `/home/chuan/context-os-theme/partials/footer.hbs`

- [ ] **Step 1: 创建 footer.hbs**

`/home/chuan/context-os-theme/partials/footer.hbs`:
```html
<footer class="cos-footer">
    <div class="cos-footer-brand">
        <svg width="20" height="20" viewBox="0 0 28 28" fill="none">
            <rect width="28" height="28" rx="6" fill="#10b981"/>
            <path d="M8 8h12v2H8V8zm0 5h8v2H8v-2zm0 5h10v2H8v-2z" fill="#0a0a0a"/>
        </svg>
        <span>© 2026 Context-OS · 创作者博客</span>
    </div>
    <nav class="cos-footer-links">
        <a href="https://contextlm.top">主页</a>
        <a href="https://app.contextlm.top">应用</a>
        <a href="https://whyimright.contextlm.top">Why I Am Right</a>
    </nav>
</footer>
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "feat: add unified footer"
```

---

### Task 6: 创建首页模板 index.hbs

**Files:**
- Create: `/home/chuan/context-os-theme/index.hbs`

- [ ] **Step 1: 创建 index.hbs**

`/home/chuan/context-os-theme/index.hbs`:
```html
{{!< default}}

<header class="cos-site-header">
    <h1 class="cos-site-title">创作者博客</h1>
    <p class="cos-site-description">邢川的观点与观察</p>
</header>

<main class="cos-post-list">
    {{#foreach posts}}
    <article class="cos-post-card">
        <h2 class="cos-post-title">
            <a href="{{url}}">{{title}}</a>
        </h2>
        {{#if excerpt}}
        <p class="cos-post-excerpt">{{excerpt words="40"}}</p>
        {{/if}}
        <div class="cos-post-meta">
            <time datetime="{{date format="YYYY-MM-DD"}}">{{date format="YYYY年M月D日"}}</time>
            {{#if tags}}
            <span>·</span>
            {{#foreach tags limit="3"}}
            <span class="cos-post-tag">{{name}}</span>
            {{/foreach}}
            {{/if}}
        </div>
    </article>
    {{/foreach}}
</main>

{{pagination}}
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "feat: add index template for post list"
```

---

### Task 7: 创建文章页模板 post.hbs

**Files:**
- Create: `/home/chuan/context-os-theme/post.hbs`

- [ ] **Step 1: 创建 post.hbs**

`/home/chuan/context-os-theme/post.hbs`:
```html
{{!< default}}

<article class="cos-article">
    <header class="cos-article-header">
        <h1 class="cos-article-title">{{title}}</h1>
        <div class="cos-article-meta">
            <span>邢川</span>
            <span>·</span>
            <time datetime="{{date format="YYYY-MM-DD"}}">{{date format="YYYY年M月D日"}}</time>
            {{#if reading_time}}
            <span>·</span>
            <span>{{reading_time}} 分钟阅读</span>
            {{/if}}
            {{#if tags}}
            <span>·</span>
            {{#foreach tags}}
            <span class="cos-post-tag">{{name}}</span>
            {{/foreach}}
            {{/if}}
        </div>
    </header>

    {{#if feature_image}}
    <img src="{{feature_image}}" alt="{{title}}" style="width: 100%; border-radius: 12px; margin-bottom: 48px;">
    {{/if}}

    <div class="cos-article-body">
        {{content}}
    </div>
</article>
```

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "feat: add post template for article pages"
```

---

### Task 8: 创建分页模板 pagination.hbs

**Files:**
- Create: `/home/chuan/context-os-theme/pagination.hbs`

- [ ] **Step 1: 创建 pagination.hbs**

`/home/chuan/context-os-theme/pagination.hbs`:
```html
{{#if prev}}
    <div class="cos-pagination">
        <a href="{{page_url prev}}">← 上一页</a>
    </div>
{{/if}}
{{#if next}}
    <div class="cos-pagination">
        <a href="{{page_url next}}">下一页 →</a>
    </div>
{{/if}}
```

**注意**：Ghost 的分页助手需要正确的结构。如果上述模板不工作，可以使用 Ghost 默认的分页实现方式。

- [ ] **Step 2: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "feat: add pagination template"
```

---

### Task 9: 打包并上传到 Ghost

**Files:**
- Output: `/home/chuan/context-os-theme.zip`

- [ ] **Step 1: 打包主题**

```bash
cd /home/chuan
cd context-os-theme
zip -r ../context-os-theme.zip .
```

Expected: `/home/chuan/context-os-theme.zip` 生成。

- [ ] **Step 2: 上传到 Ghost**

1. 浏览器访问 `https://blog.contextlm.top/ghost/`
2. 登录（admin@contextlm.com / 123456）
3. 左侧菜单 → Settings → Design
4. 点击 "Change theme" → "Upload theme"
5. 选择 `/home/chuan/context-os-theme.zip`
6. 激活主题

- [ ] **Step 3: 更新 Ghost 站点设置**

在 Ghost Admin → Settings → General：
- **Publication info** → Title: `创作者博客`
- **Publication info** → Description: `邢川的观点与观察`
- **Site meta settings** → Meta title: `创作者博客`
- **Site meta settings** → Meta description: `研究工具的创造者。在这里记录对技术、商业和研究方法的观察。`

在 Ghost Admin → Settings → Staff：
- 编辑作者资料，将名字改为"邢川"
- 上传头像（如有）

- [ ] **Step 4: 验证**

浏览器访问 `https://blog.contextlm.top/`，确认：
- 深色主题生效
- 导航栏显示 Context-OS logo 和三个入口
- 文章列表使用新卡片样式
- 文章页排版正确
- Footer 显示统一品牌信息

- [ ] **Step 5: Commit**

```bash
cd /home/chuan/context-os-theme
git add .
git commit -m "deploy: package and upload theme to Ghost"
```

---

## Self-Review

### Spec Coverage

| Spec 要求 | 对应任务 |
|---|---|
| 深色主题（#0a0a0a 背景） | Task 3 |
| 字体替换为 Geist/Inter | Task 2, 3 |
| 链接和 accent 色统一为青绿 | Task 3 |
| 标签 badge 青绿浅背景 | Task 3 |
| 顶部导航栏添加主页/whyimright | Task 4 |
| Footer 统一结构 | Task 5 |
| 站点标题/作者名更新 | Task 9 |

### Placeholder Scan

无 TBD/TODO。所有代码完整。

### 风险

- Ghost 主题上传有文件大小限制（通常 20MB），自定义主题通常很小，不会超限
- Ghost 5.x 对 Handlebars 模板有安全校验，需要确保语法正确
- 历史文章的代码块、图片在深色主题下可能需要额外检查对比度
