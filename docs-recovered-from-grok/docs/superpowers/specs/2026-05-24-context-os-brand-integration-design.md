# Context-OS 品牌整合设计文档

> 日期：2026-05-24
> 状态：已批准，待实施

## 1. 项目目标

将 VPS 上三个独立项目（contextos、创作者博客、whyimright）整合到统一的品牌体系下，以 `contextlm.top` 为品牌主页入口，统一视觉风格，建立清晰的品牌层级关系。

## 2. 品牌架构

### 2.1 品牌层级

```
Context-OS（主品牌）
│
├── contextlm.top              ← 品牌主页（新 Next.js 项目）
│
├── app.contextlm.top          ← contextos 应用
│   └── 定位：主产品，智能文档研究助手（SaaS 工作台）
│
├── blog.contextlm.top         ← 创作者博客
│   ├── 名称：创作者博客
│   ├── 作者：邢川
│   └── 定位：个人视角、引流、观点分享
│
└── whyimright.contextlm.top   ← whyimright
    └── 定位：娱乐功能，Context-OS 的趣味副产品
```

### 2.2 命名关系

- **Context-OS**：唯一主品牌名，出现在所有产品的 logo 旁或 footer
- **创作者博客**：子品牌，强调个人创作者视角
- **whyimright**：保留原有名称，作为趣味副产品的独立品牌

### 2.3 域名调整

| 域名 | 当前指向 | 调整后指向 |
|---|---|---|
| `contextlm.top` | contextos v3 应用 | **品牌主页**（新 Next.js） |
| `app.contextlm.top` | 无 | **contextos 应用**（从 contextlm.top 迁移） |
| `blog.contextlm.top` | Ghost 博客 | **创作者博客**（主题改造） |
| `whyimright.contextlm.top` | whyimright | **whyimright**（视觉对齐） |

## 3. 视觉系统

### 3.1 设计方向

**「理性与趣味的交集」** — 以深色主题为底，专业骨架 + 活力细节。三个产品在统一视觉下保留各自个性。

### 3.2 色彩系统

**深色主题（主页、whyimright）：**

| 角色 | 值 | 用途 |
|---|---|---|
| 背景底色 | `#0a0a0a` | 页面背景 |
| 卡片背景 | `rgba(255,255,255,0.03)` | 卡片、区块 |
| 卡片 hover | `rgba(255,255,255,0.06)` | 交互反馈 |
| 主文字 | `rgba(255,255,255,0.92)` | 标题、正文 |
| 次级文字 | `rgba(255,255,255,0.60)` | 描述、元数据 |
| 禁用文字 | `rgba(255,255,255,0.35)` | 占位符 |
| 边框 | `rgba(255,255,255,0.08)` | 分割线、卡片边框 |
| Accent | `#10b981` | CTA、链接、标签、活跃状态 |
| Accent hover | `#34d399` | 按钮 hover |
| Accent 浅背景 | `rgba(16,185,129,0.10)` | badge 背景 |

**浅色主题（博客）：**

| 角色 | 值 | 用途 |
|---|---|---|
| 背景 | `#fafafa` | 页面背景 |
| 文字 | `#171717` | 主文字 |
| 边框 | `rgba(0,0,0,0.08)` | 分割线 |
| Accent | `#10b981` | 与深色主题一致 |

### 3.3 字体

- **全站统一**：Geist
- **中文回退**：`PingFang SC, Microsoft YaHei, Noto Sans SC`
- **Weight 体系**：
  - `400` — 正文阅读
  - `500` — 导航、按钮
  - `600` — 强调标签、小标题
  - `700` — 大标题、display

### 3.4 交互质感

- **过渡**：`transition: all 0.2s ease`
- **Hover 反馈**：背景亮度提升，无边影变化
- **Focus 状态**：`2px solid #10b981` outline，offset 2px
- **圆角**：按钮 6px，卡片 12px，大区块 16px

## 4. 主页布局

### 4.1 页面结构（从上到下）

#### 1. 固定导航栏（64px，背景 `#0a0a0a` + 底部边框）
- **左侧**：Context-OS logo（图标 + "Context-OS" 文字）
- **右侧**：文字链接「应用 · 博客 · Why I Am Right」+ accent 色按钮「进入应用 →」
- 滚动时保持固定，加 `backdrop-blur` 效果

#### 2. 品牌入口区（首屏，约 70vh）
- 大标题 "Context-OS"（Geist 700, 56px, 字距 -1.5px）
- 副标题 "由邢川创建的研究工具集"（次级色，20px）
- 三个入口卡片（横向排列）：
  - **Context-OS**："智能文档研究助手" → 进入应用
  - **创作者博客**："邢川的观点与观察" → 阅读文章
  - **Why I Am Right**："你的偏见，值得被证实" → 开始抬杠

#### 3. 产品功能亮点
- 标题 "Context-OS 能做什么"（28px, 600）
- 2x2 功能卡片网格：
  - 上传与解析：自动解析 PDF、Office、网页、图片
  - 精准检索：基于 RAG 生成低幻觉、带精准引用的回答
  - 引用溯源：每个观点都标注出处，点击跳转原文
  - 多模态支持：文本、图表、扫描页统一检索

#### 4. 最新文章预览
- 标题 "创作者博客最新文章"（28px, 600）
- 3 篇文章卡片（从 Ghost Content API `/ghost/api/content/posts/?limit=3` 拉取）
- 每篇：标题 + 摘要（2 行）+ 日期 + 标签 badge
- "查看全部文章 →" 链接

#### 5. 创作者介绍
- 左侧：圆形头像（120px）
- 右侧：
  - "邢川"（24px/700）
  - 介绍（2-3 句）："研究工具的创造者。相信好的工具应该让人更专注于思考本身，而不是被信息淹没。在这里记录我对技术、商业和研究方法的观察。"
  - 小字："主理 Context-OS、创作者博客与 Why I Am Right"

#### 6. 社交链接
- 标题 "关注更新"（20px/600）
- 平台图标行：Twitter/X · GitHub · 邮箱
- 可选：邮件订阅输入框

#### 7. Footer
- 左侧：Context-OS 小 logo + "© 2026 Context-OS"
- 右侧：应用 | 博客 | Why I Am Right · 隐私政策 | 服务条款
- 背景 `#050505`

## 5. 产品视觉对齐策略

### 5.1 Context-OS 应用（app.contextlm.top）

**范围：完全独立**。应用内部 UI 视觉暂时不动，仅做域名迁移。

### 5.2 创作者博客（blog.contextlm.top）

**范围：Ghost 主题深度定制**

- 创建自定义 Ghost 主题（基于 Casper 改造）
- 深色主题（`#0a0a0a` 背景）
- 字体替换为 Geist
- 链接和 accent 色统一为青绿 `#10b981`
- 标签 badge 使用青绿浅背景样式
- 顶部导航栏添加主页和 whyimright 入口
- Footer 替换为统一结构
- 站点标题改为"创作者博客"，作者改为"邢川"

### 5.3 Why I Am Right（whyimright.contextlm.top）

**范围：视觉层统一，功能不变**

- accent 色统一为 `#10b981`
- 深色背景统一为 `#0a0a0a`
- 字体统一为 Geist（如当前不同）
- 顶部添加统一导航栏
- Footer 更新为"Context-OS 旗下"
- Favicon 统一
- 浏览器标题格式：`页面名 | Why I Am Right · Context-OS`

## 6. 技术架构

### 6.1 主页项目结构（Next.js 15）

```
context-os-landing/
├── app/
│   ├── layout.tsx              # 根布局，加载 Geist 字体
│   ├── page.tsx                # 主页
│   ├── globals.css             # Tailwind + CSS 变量
│   └── sections/
│       ├── Navbar.tsx
│       ├── HeroEntries.tsx
│       ├── Features.tsx
│       ├── LatestPosts.tsx
│       ├── Creator.tsx
│       ├── Social.tsx
│       └── Footer.tsx
├── components/ui/
│   ├── Button.tsx
│   ├── Card.tsx
│   └── Badge.tsx
├── public/
│   └── logo.svg
├── next.config.js              # output: 'export'
├── tailwind.config.ts
└── package.json
```

### 6.2 构建与部署

- **输出模式**：`output: 'export'`（静态导出）
- **部署路径**：VPS `/var/www/context-os-landing/`
- **Nginx**：`contextlm.top` 指向静态文件目录
- **更新流程**：本地 build → rsync → 完成（无需重启服务）

### 6.3 Nginx 配置变更

**新配置：contextlm.top（主页）**
```nginx
server {
    listen 443 ssl http2;
    server_name contextlm.top www.contextlm.top;
    root /var/www/context-os-landing;
    index index.html;
    location / {
        try_files $uri $uri.html $uri/ =404;
    }
}
```

**修改：app.contextlm.top（contextos 迁移）**
```nginx
server {
    listen 443 ssl http2;
    server_name app.contextlm.top;
    location / {
        proxy_pass http://127.0.0.1:3003;
    }
    location /api/ {
        proxy_pass http://127.0.0.1:3002;
    }
}
```

**SSL**：`app.contextlm.top` 需新申请 Let's Encrypt 证书。

### 6.4 实施顺序

| 顺序 | 任务 | 预估工作量 |
|---|---|---|
| 1 | 搭建主页（Next.js 项目 + 构建 + Nginx + SSL） | 中等 |
| 2 | contextos 迁移到 app.contextlm.top | 小 |
| 3 | whyimright 视觉对齐（配色 + 导航 + Footer） | 小 |
| 4 | 博客主题定制（Ghost 自定义主题 + 信息更新） | 中等 |

## 7. 风险与注意事项

1. **contextos 域名迁移**：现有用户可能收藏了 `contextlm.top`，迁移后需要确保 301 重定向或提前通知
2. **Ghost API 跨域**：主页调用 `blog.contextlm.top/ghost/api/content/posts` 时需注意 CORS 设置
3. **SSL 证书**：`app.contextlm.top` 需要新证书，申请期间可能有短暂不可用
4. **深色主题可读性**：博客从浅色改为深色后，需检查所有历史文章的图片/代码块对比度
