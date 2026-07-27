# 法律合规页面实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 根据设计文档实现完整的法律与开源合规页面体系，满足商业化上线要求。

**Architecture:** 采用Next.js App Router路由组`(marketing)`，使用MDX渲染法律文档，构建时同步NOTICE文件，支持桌面静态导出。

**Tech Stack:** Next.js 16, React 19, TypeScript, MDX, next-intl, Playwright测试

---

## 文件结构

### 路由页面
- `frontend_next/app/(marketing)/legal/page.tsx` - 法律中心索引页
- `frontend_next/app/(marketing)/legal/terms/page.tsx` - 用户服务协议
- `frontend_next/app/(marketing)/legal/privacy/page.tsx` - 隐私政策
- `frontend_next/app/(marketing)/legal/licenses/page.tsx` - 开源摘要
- `frontend_next/app/(marketing)/legal/licenses/third-party/page.tsx` - 完整第三方声明
- `frontend_next/app/(marketing)/legal/licenses/project/page.tsx` - MIT许可证全文

### 组件
- `frontend_next/components/legal/LegalLayout.tsx` - 法律页面布局组件
- `frontend_next/components/legal/LegalDocRenderer.tsx` - MDX文档渲染组件
- `frontend_next/components/legal/LegalFooterLinks.tsx` - 页脚法律链接组件
- `frontend_next/components/legal/ConsentCheckbox.tsx` - 同意勾选框组件

### 内容文件
- `frontend_next/content/legal/zh-CN/terms.mdx` - 中文用户服务协议
- `frontend_next/content/legal/zh-CN/privacy.mdx` - 中文隐私政策

### 脚本和配置
- `frontend_next/scripts/sync-legal-assets.sh` - 同步法律资产脚本
- `frontend_next/public/legal/third-party-notices.md` - 第三方声明副本
- `frontend_next/public/legal/LICENSE` - MIT许可证副本

### 测试文件
- `frontend_next/tests/legal/` - 法律页面测试目录

---

## 任务分解

### Task 1: 项目设置和基础组件

**Covers:** P0-FE-1, P0-FE-2, P0-FE-3

**Files:**
- Create: `frontend_next/components/legal/LegalLayout.tsx`
- Create: `frontend_next/components/legal/LegalDocRenderer.tsx`
- Create: `frontend_next/components/legal/LegalFooterLinks.tsx`
- Create: `frontend_next/scripts/sync-legal-assets.sh`
- Modify: `frontend_next/package.json` (添加sync:legal脚本)

- [ ] **Step 1: 创建目录结构**

```bash
cd frontend_next
mkdir -p app/(marketing)/legal/terms
mkdir -p app/(marketing)/legal/privacy
mkdir -p app/(marketing)/legal/licenses/third-party
mkdir -p app/(marketing)/legal/licenses/project
mkdir -p components/legal
mkdir -p content/legal/zh-CN
mkdir -p public/legal
mkdir -p scripts
mkdir -p tests/legal
```

- [ ] **Step 2: 创建LegalLayout组件**

```tsx
// frontend_next/components/legal/LegalLayout.tsx
import Link from 'next/link';

interface LegalLayoutProps {
  children: React.ReactNode;
  title: string;
  lastUpdated?: string;
  version?: string;
}

export default function LegalLayout({ children, title, lastUpdated, version }: LegalLayoutProps) {
  return (
    <div className="legal-layout">
      <div className="legal-header">
        <h1>{title}</h1>
        {lastUpdated && <p className="legal-updated">最后更新: {lastUpdated}</p>}
        {version && <p className="legal-version">版本: {version}</p>}
      </div>
      <div className="legal-content">
        {children}
      </div>
      <div className="legal-footer">
        <Link href="/legal">返回法律中心</Link>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: 创建LegalDocRenderer组件**

```tsx
// frontend_next/components/legal/LegalDocRenderer.tsx
import LegalLayout from './LegalLayout';

interface LegalDocRendererProps {
  content: string;
  title: string;
  lastUpdated?: string;
  version?: string;
}

export default function LegalDocRenderer({ content, title, lastUpdated, version }: LegalDocRendererProps) {
  return (
    <LegalLayout title={title} lastUpdated={lastUpdated} version={version}>
      <div 
        className="legal-document"
        dangerouslySetInnerHTML={{ __html: content }} 
      />
    </LegalLayout>
  );
}
```

- [ ] **Step 4: 创建LegalFooterLinks组件**

```tsx
// frontend_next/components/legal/LegalFooterLinks.tsx
import Link from 'next/link';

export default function LegalFooterLinks() {
  const currentYear = new Date().getFullYear();
  
  return (
    <footer className="legal-footer-links">
      <div className="legal-footer-content">
        <Link href="/legal/terms">用户协议</Link>
        <span className="legal-footer-separator">·</span>
        <Link href="/legal/privacy">隐私政策</Link>
        <span className="legal-footer-separator">·</span>
        <Link href="/legal/licenses">开源声明</Link>
      </div>
      <div className="legal-footer-copyright">
        © {currentYear} Context-OS
      </div>
    </footer>
  );
}
```

- [ ] **Step 5: 创建同步脚本**

```bash
#!/bin/bash
# frontend_next/scripts/sync-legal-assets.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
PUBLIC_LEGAL="$PROJECT_ROOT/public/legal"

# 创建目录
mkdir -p "$PUBLIC_LEGAL"

# 同步THIRD_PARTY_NOTICES.md
if [ -f "$PROJECT_ROOT/../THIRD_PARTY_NOTICES.md" ]; then
  cp "$PROJECT_ROOT/../THIRD_PARTY_NOTICES.md" "$PUBLIC_LEGAL/third-party-notices.md"
  echo "✅ 同步 THIRD_PARTY_NOTICES.md"
else
  echo "⚠️  THIRD_PARTY_NOTICES.md 不存在"
fi

# 同步LICENSE
if [ -f "$PROJECT_ROOT/../LICENSE" ]; then
  cp "$PROJECT_ROOT/../LICENSE" "$PUBLIC_LEGAL/LICENSE"
  echo "✅ 同步 LICENSE"
else
  echo "⚠️  LICENSE 不存在"
fi

# 生成摘要JSON（可选）
# TODO: 扩展generate-third-party-notices.sh生成摘要

echo "✅ 法律资产同步完成"
```

- [ ] **Step 6: 更新package.json脚本**

```json
// 在scripts对象中添加
"sync:legal": "bash scripts/sync-legal-assets.sh",
"prebuild": "pnpm sync:legal"
```

- [ ] **Step 7: 提交初始设置**

```bash
git add frontend_next/components/legal/ frontend_next/scripts/sync-legal-assets.sh
git commit -m "feat(legal): add legal components and sync script"
```

### Task 2: 创建法律中心页面

**Covers:** P0-IA-1, P0-IA-4

**Files:**
- Create: `frontend_next/app/(marketing)/legal/page.tsx`

- [ ] **Step 1: 创建法律中心页面**

```tsx
// frontend_next/app/(marketing)/legal/page.tsx
import Link from 'next/link';
import LegalFooterLinks from '@/components/legal/LegalFooterLinks';

export default function LegalCenter() {
  const cards = [
    {
      title: '用户服务协议',
      description: '使用Context-OS服务前请阅读本协议',
      href: '/legal/terms',
      lastUpdated: '2026-06-13',
    },
    {
      title: '隐私政策',
      description: '了解我们如何收集、使用和保护您的个人信息',
      href: '/legal/privacy',
      lastUpdated: '2026-06-13',
    },
    {
      title: '开源声明',
      description: '查看我们使用的开源组件及其许可证',
      href: '/legal/licenses',
      lastUpdated: '2026-06-13',
    },
  ];

  return (
    <div className="legal-center">
      <div className="legal-center-header">
        <h1>法律中心</h1>
        <p>使用Context-OS前请阅读以下文档</p>
      </div>
      
      <div className="legal-cards">
        {cards.map((card) => (
          <Link key={card.href} href={card.href} className="legal-card">
            <h2>{card.title}</h2>
            <p>{card.description}</p>
            <span className="legal-card-updated">
              最后更新: {card.lastUpdated}
            </span>
          </Link>
        ))}
      </div>
      
      <div className="legal-contact">
        <p>如有法律问题，请联系: <a href="mailto:legal@context-os.com">legal@context-os.com</a></p>
      </div>
      
      <LegalFooterLinks />
    </div>
  );
}
```

- [ ] **Step 2: 添加法律中心页面样式**

```css
/* 在globals.css中添加 */
.legal-center {
  max-width: 48rem;
  margin: 0 auto;
  padding: 2rem;
}

.legal-center-header {
  text-align: center;
  margin-bottom: 3rem;
}

.legal-center-header h1 {
  font-size: 2rem;
  margin-bottom: 0.5rem;
}

.legal-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 2rem;
  margin-bottom: 3rem;
}

.legal-card {
  border: 1px solid #e2e8f0;
  border-radius: 0.5rem;
  padding: 1.5rem;
  text-decoration: none;
  color: inherit;
  transition: box-shadow 0.2s;
}

.legal-card:hover {
  box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
}

.legal-card h2 {
  font-size: 1.25rem;
  margin-bottom: 0.5rem;
}

.legal-card p {
  color: #64748b;
  margin-bottom: 1rem;
}

.legal-card-updated {
  font-size: 0.875rem;
  color: #94a3b8;
}

.legal-contact {
  text-align: center;
  margin-bottom: 2rem;
  color: #64748b;
}

.legal-footer-links {
  text-align: center;
  padding: 2rem 0;
  border-top: 1px solid #e2e8f0;
}

.legal-footer-content {
  margin-bottom: 0.5rem;
}

.legal-footer-separator {
  margin: 0 0.5rem;
  color: #94a3b8;
}

.legal-footer-copyright {
  font-size: 0.875rem;
  color: #94a3b8;
}
```

- [ ] **Step 3: 提交法律中心页面**

```bash
git add frontend_next/app/\(marketing\)/legal/page.tsx
git commit -m "feat(legal): add legal center page"
```

### Task 3: 创建条款页面

**Covers:** P0-IA-2, P0-CNT-1, P0-CNT-2, P0-FE-6

**Files:**
- Create: `frontend_next/content/legal/zh-CN/terms.mdx`
- Create: `frontend_next/app/(marketing)/legal/terms/page.tsx`

- [ ] **Step 1: 创建条款MDX内容**

```mdx
---
title: 用户服务协议
slug: terms
version: "2026-06-13"
locale: zh-CN
status: draft
---

# 用户服务协议

最后更新: 2026-06-13

## 1. 服务说明

Context-OS提供以下服务：
- RAG（检索增强生成）文档处理
- 文档上传与索引
- AI对话与推理
- 工作区与分享功能

## 2. 账号注册与安全

- 邮箱登录
- 密码重置
- 会话管理

## 3. 用户内容与授权

用户保留上传内容的权属。为提供服务，用户授权Context-OS处理上传内容。

## 4. AI生成内容

AI生成内容不保证准确，不构成专业建议，用户应自行核实。

## 5. 可接受使用

禁止以下行为：
- 违法使用
- 恶意行为
- 侵犯知识产权
- 滥用API

## 6. 付费与订阅

- Plus/Pro档位
- Creem/支付宝支付
- 自动续费与取消

## 7. 服务变更与终止

账号暂停/删除，服务中断等情况的处理。

## 8. 免责声明与责任限制

按律师模板填写。

## 9. 适用法律与争议解决

按律师模板填写。

## 10. 协议更新

版本号: 2026-06-13
继续使用视为接受更新
重大变更将通过邮件或站内通知
```

- [ ] **Step 2: 创建条款页面**

```tsx
// frontend_next/app/(marketing)/legal/terms/page.tsx
import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import LegalDocRenderer from '@/components/legal/LegalDocRenderer';

export default async function TermsPage() {
  const termsPath = path.join(process.cwd(), 'content/legal/zh-CN/terms.mdx');
  const fileContent = fs.readFileSync(termsPath, 'utf8');
  const { content, data } = matter(fileContent);
  
  // 简单的MDX到HTML转换（实际项目中应使用@next/mdx）
  const htmlContent = content
    .replace(/^### (.*$)/gim, '<h3>$1</h3>')
    .replace(/^## (.*$)/gim, '<h2>$1</h2>')
    .replace(/^# (.*$)/gim, '<h1>$1</h1>')
    .replace(/\*\*(.*)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.*)\*/g, '<em>$1</em>')
    .replace(/!\[(.*?)\]\((.*?)\)/g, '<img alt="$1" src="$2" />')
    .replace(/\[(.*?)\]\((.*?)\)/g, '<a href="$2">$1</a>')
    .replace(/\n/g, '<br />');
  
  return (
    <LegalDocRenderer
      content={htmlContent}
      title={data.title}
      lastUpdated={data.version}
      version={data.version}
    />
  );
}
```

- [ ] **Step 3: 安装gray-matter依赖**

```bash
cd frontend_next
pnpm add gray-matter
```

- [ ] **Step 4: 提交条款页面**

```bash
git add frontend_next/content/legal/zh-CN/terms.mdx frontend_next/app/\(marketing\)/legal/terms/page.tsx
git commit -m "feat(legal): add terms of service page"
```

### Task 4: 创建隐私政策页面

**Covers:** P0-IA-3, P0-CNT-3, P0-CNT-4

**Files:**
- Create: `frontend_next/content/legal/zh-CN/privacy.mdx`
- Create: `frontend_next/app/(marketing)/legal/privacy/page.tsx`

- [ ] **Step 1: 创建隐私政策MDX内容**

```mdx
---
title: 隐私政策
slug: privacy
version: "2026-06-13"
locale: zh-CN
status: draft
---

# 隐私政策

最后更新: 2026-06-13

## 1. 数据收集与使用

### 1.1 账号标识（邮箱等）
- 处理目的：认证、找回密码
- 存储/处理方：PostgreSQL
- 保留期：账号存续期间

### 1.2 上传文档与元数据
- 处理目的：RAG、索引
- 存储/处理方：对象存储（S3/MinIO）+ PostgreSQL
- 是否用于模型训练：**不会**

### 1.3 聊天与推理内容
- 处理目的：提供服务、记忆（若开启）
- 存储/处理方：PostgreSQL
- 保留、导出、删除方式：用户可随时删除

### 1.4 向量embedding
- 处理目的：检索
- 存储/处理方：Milvus
- 技术性描述：用于相似性搜索

### 1.5 使用与计费
- 处理目的：配额、账单
- 存储/处理方：PostgreSQL
- 与Creem/支付宝交互范围：支付处理

### 1.6 第三方AI/OCR
- 处理目的：推理、OCR
- 存储/处理方：DeepSeek、DashScope、Paddle API等
- 委托处理/跨境：数据可能传输至第三方服务器

### 1.7 日志与审计
- 处理目的：安全、运维
- 存储/处理方：服务器日志
- 保留周期：30天

## 2. 用户权利

根据适用法律，用户享有以下权利：
- 查阅个人信息
- 复制个人信息
- 删除个人信息
- 注销账号
- 撤回同意

行使权利方式：通过设置页面或联系 legal@context-os.com

## 3. 数据安全

我们采取适当的技术和组织措施保护您的个人信息。

## 4. 隐私政策更新

重大变更将通过邮件或站内通知。
```

- [ ] **Step 2: 创建隐私政策页面**

```tsx
// frontend_next/app/(marketing)/legal/privacy/page.tsx
import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import LegalDocRenderer from '@/components/legal/LegalDocRenderer';

export default async function PrivacyPage() {
  const privacyPath = path.join(process.cwd(), 'content/legal/zh-CN/privacy.mdx');
  const fileContent = fs.readFileSync(privacyPath, 'utf8');
  const { content, data } = matter(fileContent);
  
  // 简单的MDX到HTML转换
  const htmlContent = content
    .replace(/^### (.*$)/gim, '<h3>$1</h3>')
    .replace(/^## (.*$)/gim, '<h2>$1</h2>')
    .replace(/^# (.*$)/gim, '<h1>$1</h1>')
    .replace(/\*\*(.*)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.*)\*/g, '<em>$1</em>')
    .replace(/!\[(.*?)\]\((.*?)\)/g, '<img alt="$1" src="$2" />')
    .replace(/\[(.*?)\]\((.*?)\)/g, '<a href="$2">$1</a>')
    .replace(/\n/g, '<br />');
  
  return (
    <LegalDocRenderer
      content={htmlContent}
      title={data.title}
      lastUpdated={data.version}
      version={data.version}
    />
  );
}
```

- [ ] **Step 3: 提交隐私政策页面**

```bash
git add frontend_next/content/legal/zh-CN/privacy.mdx frontend_next/app/\(marketing\)/legal/privacy/page.tsx
git commit -m "feat(legal): add privacy policy page"
```

### Task 5: 创建开源摘要页面

**Covers:** P0-IA-4, P0-CNT-5, P0-CNT-6

**Files:**
- Create: `frontend_next/app/(marketing)/legal/licenses/page.tsx`

- [ ] **Step 1: 创建开源摘要页面**

```tsx
// frontend_next/app/(marketing)/legal/licenses/page.tsx
import Link from 'next/link';
import LegalLayout from '@/components/legal/LegalLayout';

export default function LicensesSummary() {
  const majorComponents = [
    { category: 'Web框架', components: 'Next.js, React', license: 'MIT' },
    { category: '后端运行时', components: 'Tokio, Axum', license: 'MIT' },
    { category: '向量数据库', components: 'Milvus', license: 'Apache-2.0' },
    { category: 'PDF解析', components: 'LiteParse / PDFium', license: 'Apache-2.0' },
    { category: 'AI推理', components: 'DeepSeek, DashScope', license: '商业API' },
  ];

  const weakCopyleft = [
    { component: 'dompurify', note: '选择Apache-2.0版本' },
    { component: 'cssparser', note: 'MPL，未修改则仅需NOTICE' },
  ];

  return (
    <LegalLayout title="开源软件说明" lastUpdated="2026-06-13">
      <div className="licenses-summary">
        <section className="licenses-overview">
          <h2>我们的产品</h2>
          <p>
            Context-OS服务端与Web客户端以自研为主；整体分发遵守MIT许可证。
          </p>
          <Link href="/legal/licenses/project" className="app-link">
            查看MIT许可证全文
          </Link>
        </section>

        <section className="licenses-major">
          <h2>主要开源组件</h2>
          <table className="licenses-table">
            <thead>
              <tr>
                <th>类别</th>
                <th>代表组件</th>
                <th>许可证</th>
              </tr>
            </thead>
            <tbody>
              {majorComponents.map((item, index) => (
                <tr key={index}>
                  <td>{item.category}</td>
                  <td>{item.components}</td>
                  <td><span className="license-badge">{item.license}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section className="licenses-copyleft">
          <h2>弱copyleft说明</h2>
          <ul>
            {weakCopyleft.map((item, index) => (
              <li key={index}>
                <strong>{item.component}</strong>: {item.note}
              </li>
            ))}
          </ul>
        </section>

        <section className="licenses-full">
          <h2>完整清单</h2>
          <div className="licenses-actions">
            <Link href="/legal/licenses/third-party" className="app-button-primary">
              查看完整第三方声明
            </Link>
            <a 
              href="/legal/third-party-notices.md" 
              download 
              className="app-button-secondary"
            >
              下载Markdown
            </a>
          </div>
        </section>

        <section className="licenses-desktop">
          <h2>桌面客户端</h2>
          <p>
            桌面客户端安装包内另附声明；可在About对话框中查看。
          </p>
        </section>
      </div>
    </LegalLayout>
  );
}
```

- [ ] **Step 2: 添加开源摘要页面样式**

```css
/* 在globals.css中添加 */
.licenses-summary section {
  margin-bottom: 2rem;
}

.licenses-table {
  width: 100%;
  border-collapse: collapse;
  margin: 1rem 0;
}

.licenses-table th,
.licenses-table td {
  padding: 0.75rem;
  border: 1px solid #e2e8f0;
  text-align: left;
}

.licenses-table th {
  background-color: #f8fafc;
  font-weight: 600;
}

.license-badge {
  background-color: #e2e8f0;
  padding: 0.25rem 0.5rem;
  border-radius: 0.25rem;
  font-size: 0.875rem;
}

.licenses-actions {
  display: flex;
  gap: 1rem;
  margin-top: 1rem;
}

.app-button-primary {
  background-color: #3b82f6;
  color: white;
  padding: 0.5rem 1rem;
  border-radius: 0.375rem;
  text-decoration: none;
  font-weight: 500;
}

.app-button-secondary {
  background-color: #f1f5f9;
  color: #475569;
  padding: 0.5rem 1rem;
  border-radius: 0.375rem;
  text-decoration: none;
  font-weight: 500;
}
```

- [ ] **Step 3: 提交开源摘要页面**

```bash
git add frontend_next/app/\(marketing\)/legal/licenses/page.tsx
git commit -m "feat(legal): add licenses summary page"
```

### Task 6: 创建第三方声明页面

**Covers:** P0-IA-5, P0-PIPE-2

**Files:**
- Create: `frontend_next/app/(marketing)/legal/licenses/third-party/page.tsx`

- [ ] **Step 1: 创建第三方声明页面**

```tsx
// frontend_next/app/(marketing)/legal/licenses/third-party/page.tsx
import fs from 'fs';
import path from 'path';
import Link from 'next/link';
import LegalLayout from '@/components/legal/LegalLayout';

export default async function ThirdPartyNotices() {
  const noticesPath = path.join(process.cwd(), 'public/legal/third-party-notices.md');
  let noticesContent = '';
  let totalPackages = 0;
  let generationDate = '';
  
  try {
    noticesContent = fs.readFileSync(noticesPath, 'utf8');
    // 统计包数量（简单统计）
    const crateMatches = noticesContent.match(/\bcrate\b/gi);
    const packageMatches = noticesContent.match(/\bpackage\b/gi);
    totalPackages = (crateMatches?.length || 0) + (packageMatches?.length || 0);
    generationDate = new Date().toISOString().split('T')[0];
  } catch (error) {
    noticesContent = '第三方声明文件正在生成中...';
  }
  
  // 简单的Markdown到HTML转换
  const htmlContent = noticesContent
    .replace(/^### (.*$)/gim, '<h3>$1</h3>')
    .replace(/^## (.*$)/gim, '<h2>$1</h2>')
    .replace(/^# (.*$)/gim, '<h1>$1</h1>')
    .replace(/\*\*(.*)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.*)\*/g, '<em>$1</em>')
    .replace(/!\[(.*?)\]\((.*?)\)/g, '<img alt="$1" src="$2" />')
    .replace(/\[(.*?)\]\((.*?)\)/g, '<a href="$2">$1</a>')
    .replace(/\n/g, '<br />');
  
  return (
    <LegalLayout title="完整第三方组件声明">
      <div className="third-party-notices">
        <div className="notices-header">
          <div className="notices-stats">
            <p>生成日期: {generationDate}</p>
            <p>组件总数: {totalPackages}+</p>
          </div>
          <div className="notices-actions">
            <a 
              href="/legal/third-party-notices.md" 
              download 
              className="app-button-secondary"
            >
              下载 .md
            </a>
            <a 
              href="/legal/third-party-notices.txt" 
              download 
              className="app-button-secondary"
            >
              下载 .txt
            </a>
          </div>
        </div>
        
        <div 
          className="notices-content"
          dangerouslySetInnerHTML={{ __html: htmlContent }} 
        />
        
        <div className="notices-footer">
          <Link href="/legal/licenses">返回开源摘要</Link>
        </div>
      </div>
    </LegalLayout>
  );
}
```

- [ ] **Step 2: 添加第三方声明页面样式**

```css
/* 在globals.css中添加 */
.third-party-notices {
  max-width: 48rem;
  margin: 0 auto;
}

.notices-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 2rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid #e2e8f0;
}

.notices-stats p {
  margin: 0.25rem 0;
  color: #64748b;
}

.notices-actions {
  display: flex;
  gap: 0.5rem;
}

.notices-content {
  line-height: 1.6;
}

.notices-content h1,
.notices-content h2,
.notices-content h3 {
  margin-top: 2rem;
  margin-bottom: 1rem;
}

.notices-content h1 {
  font-size: 1.5rem;
}

.notices-content h2 {
  font-size: 1.25rem;
}

.notices-content h3 {
  font-size: 1.1rem;
}

.notices-footer {
  margin-top: 3rem;
  padding-top: 1rem;
  border-top: 1px solid #e2e8f0;
}
```

- [ ] **Step 3: 运行同步脚本**

```bash
cd frontend_next
pnpm sync:legal
```

- [ ] **Step 4: 提交第三方声明页面**

```bash
git add frontend_next/app/\(marketing\)/legal/licenses/third-party/page.tsx
git commit -m "feat(legal): add third-party notices page"
```

### Task 7: 创建MIT许可证页面

**Covers:** P0-IA-6, P0-PIPE-1

**Files:**
- Create: `frontend_next/app/(marketing)/legal/licenses/project/page.tsx`

- [ ] **Step 1: 创建MIT许可证页面**

```tsx
// frontend_next/app/(marketing)/legal/licenses/project/page.tsx
import fs from 'fs';
import path from 'path';
import LegalLayout from '@/components/legal/LegalLayout';

export default async function ProjectLicense() {
  const licensePath = path.join(process.cwd(), 'public/legal/LICENSE');
  let licenseContent = '';
  
  try {
    licenseContent = fs.readFileSync(licensePath, 'utf8');
  } catch (error) {
    licenseContent = 'MIT许可证文件正在加载中...';
  }
  
  // 简单的Markdown到HTML转换
  const htmlContent = licenseContent
    .replace(/\n/g, '<br />');
  
  return (
    <LegalLayout title="MIT许可证">
      <div className="project-license">
        <div className="license-content">
          <pre className="license-text">{licenseContent}</pre>
        </div>
      </div>
    </LegalLayout>
  );
}
```

- [ ] **Step 2: 添加MIT许可证页面样式**

```css
/* 在globals.css中添加 */
.project-license {
  max-width: 48rem;
  margin: 0 auto;
}

.license-content {
  background-color: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 0.5rem;
  padding: 2rem;
}

.license-text {
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 0.875rem;
  line-height: 1.6;
  white-space: pre-wrap;
  word-wrap: break-word;
}
```

- [ ] **Step 3: 提交MIT许可证页面**

```bash
git add frontend_next/app/\(marketing\)/legal/licenses/project/page.tsx
git commit -m "feat(legal): add MIT license page"
```

### Task 8: 创建同意勾选框组件

**Covers:** P0-CON-1, P0-CON-2, P0-CON-3, P0-CON-4

**Files:**
- Create: `frontend_next/components/legal/ConsentCheckbox.tsx`

- [ ] **Step 1: 创建同意勾选框组件**

```tsx
// frontend_next/components/legal/ConsentCheckbox.tsx
'use client';

import { useState } from 'react';
import Link from 'next/link';

interface ConsentCheckboxProps {
  onConsentChange: (consented: boolean) => void;
  required?: boolean;
  termsVersion?: string;
  privacyVersion?: string;
}

export default function ConsentCheckbox({
  onConsentChange,
  required = true,
  termsVersion = '2026-06-13',
  privacyVersion = '2026-06-13',
}: ConsentCheckboxProps) {
  const [consented, setConsented] = useState(false);
  const [error, setError] = useState('');

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const isChecked = e.target.checked;
    setConsented(isChecked);
    setError('');
    onConsentChange(isChecked);
  };

  const handleSubmit = () => {
    if (required && !consented) {
      setError('请先阅读并同意用户协议与隐私政策');
      return false;
    }
    return true;
  };

  return (
    <div className="consent-checkbox">
      <label className="consent-label">
        <input
          type="checkbox"
          checked={consented}
          onChange={handleChange}
          required={required}
          className="consent-input"
        />
        <span className="consent-text">
          我已阅读并同意
          <Link href="/legal/terms" target="_blank" className="consent-link">
            《用户服务协议》
          </Link>
          与
          <Link href="/legal/privacy" target="_blank" className="consent-link">
            《隐私政策》
          </Link>
        </span>
      </label>
      {error && <p className="consent-error">{error}</p>}
      <input type="hidden" name="terms_version" value={termsVersion} />
      <input type="hidden" name="privacy_version" value={privacyVersion} />
      <input type="hidden" name="accepted_at" value={new Date().toISOString()} />
    </div>
  );
}
```

- [ ] **Step 2: 添加同意勾选框样式**

```css
/* 在globals.css中添加 */
.consent-checkbox {
  margin: 1rem 0;
}

.consent-label {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  cursor: pointer;
}

.consent-input {
  margin-top: 0.25rem;
}

.consent-text {
  font-size: 0.875rem;
  line-height: 1.5;
  color: #475569;
}

.consent-link {
  color: #3b82f6;
  text-decoration: underline;
}

.consent-link:hover {
  color: #2563eb;
}

.consent-error {
  margin-top: 0.5rem;
  color: #ef4444;
  font-size: 0.875rem;
}
```

- [ ] **Step 3: 提交同意勾选框组件**

```bash
git add frontend_next/components/legal/ConsentCheckbox.tsx
git commit -m "feat(legal): add consent checkbox component"
```

### Task 9: 集成页脚组件

**Covers:** P0-IA-7, P0-UX-1, P0-UX-2

**Files:**
- Modify: `frontend_next/app/page.tsx` (首页)
- Modify: `frontend_next/app/(marketing)/pricing/page.tsx` (定价页)
- Modify: `frontend_next/app/login/page.tsx` (登录页)
- Modify: `frontend_next/app/register/page.tsx` (注册页)

- [ ] **Step 1: 在首页添加页脚**

```tsx
// 在frontend_next/app/page.tsx的return语句中添加
import LegalFooterLinks from '@/components/legal/LegalFooterLinks';

// 在页面底部添加
<LegalFooterLinks />
```

- [ ] **Step 2: 在定价页添加页脚**

```tsx
// 在frontend_next/app/(marketing)/pricing/page.tsx的return语句中添加
import LegalFooterLinks from '@/components/legal/LegalFooterLinks';

// 在页面底部添加
<LegalFooterLinks />
```

- [ ] **Step 3: 在登录页添加页脚**

```tsx
// 在frontend_next/app/login/page.tsx的return语句中添加
import LegalFooterLinks from '@/components/legal/LegalFooterLinks';

// 在页面底部添加
<LegalFooterLinks />
```

- [ ] **Step 4: 在注册页添加同意勾选框**

```tsx
// 在frontend_next/app/register/page.tsx中添加
import ConsentCheckbox from '@/components/legal/ConsentCheckbox';

// 在表单中添加
<ConsentCheckbox 
  onConsentChange={(consented) => {
    // 处理同意状态变化
  }}
  termsVersion="2026-06-13"
  privacyVersion="2026-06-13"
/>
```

- [ ] **Step 5: 提交页脚集成**

```bash
git add frontend_next/app/page.tsx frontend_next/app/\(marketing\)/pricing/page.tsx frontend_next/app/login/page.tsx frontend_next/app/register/page.tsx
git commit -m "feat(legal): integrate legal footer and consent checkbox"
```

### Task 10: 测试和验证

**Covers:** P0-FE-5, P0-UX-3, P0-UX-4, P2-E2E-1

**Files:**
- Create: `frontend_next/tests/legal/legal-pages.spec.ts`
- Create: `frontend_next/tests/legal/consent-checkbox.spec.ts`

- [ ] **Step 1: 创建法律页面测试**

```typescript
// frontend_next/tests/legal/legal-pages.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Legal Pages', () => {
  test('Legal center page loads', async ({ page }) => {
    await page.goto('/legal');
    await expect(page.locator('h1')).toContainText('法律中心');
    await expect(page.locator('.legal-card')).toHaveCount(3);
  });

  test('Terms page loads', async ({ page }) => {
    await page.goto('/legal/terms');
    await expect(page.locator('h1')).toContainText('用户服务协议');
    await expect(page.locator('.legal-updated')).toBeVisible();
  });

  test('Privacy page loads', async ({ page }) => {
    await page.goto('/legal/privacy');
    await expect(page.locator('h1')).toContainText('隐私政策');
  });

  test('Licenses summary page loads', async ({ page }) => {
    await page.goto('/legal/licenses');
    await expect(page.locator('h1')).toContainText('开源软件说明');
    await expect(page.locator('.licenses-table')).toBeVisible();
  });

  test('Third-party notices page loads', async ({ page }) => {
    await page.goto('/legal/licenses/third-party');
    await expect(page.locator('h1')).toContainText('完整第三方组件声明');
  });

  test('MIT license page loads', async ({ page }) => {
    await page.goto('/legal/licenses/project');
    await expect(page.locator('h1')).toContainText('MIT许可证');
  });
});
```

- [ ] **Step 2: 创建同意勾选框测试**

```typescript
// frontend_next/tests/legal/consent-checkbox.spec.ts
import { test, expect } from '@playwright/test';

test.describe('Consent Checkbox', () => {
  test('Checkbox is required for registration', async ({ page }) => {
    await page.goto('/register');
    const checkbox = page.locator('.consent-input');
    await expect(checkbox).toBeVisible();
    await expect(checkbox).not.toBeChecked();
  });

  test('Error message appears when submitting without consent', async ({ page }) => {
    await page.goto('/register');
    await page.click('button[type="submit"]');
    await expect(page.locator('.consent-error')).toContainText('请先阅读并同意用户协议与隐私政策');
  });

  test('Links to terms and privacy open in new tab', async ({ page }) => {
    await page.goto('/register');
    const termsLink = page.locator('a[href="/legal/terms"]');
    const privacyLink = page.locator('a[href="/legal/privacy"]');
    
    await expect(termsLink).toBeVisible();
    await expect(privacyLink).toBeVisible();
  });
});
```

- [ ] **Step 3: 运行测试**

```bash
cd frontend_next
pnpm exec playwright test tests/legal/
```

- [ ] **Step 4: 运行类型检查**

```bash
cd frontend_next
pnpm typecheck
```

- [ ] **Step 5: 运行构建**

```bash
cd frontend_next
pnpm build
```

- [ ] **Step 6: 提交测试**

```bash
git add frontend_next/tests/legal/
git commit -m "test(legal): add legal pages and consent checkbox tests"
```

### Task 11: 文档和清理

**Covers:** P0-PIPE-3, P0-PIPE-4, P0-PIPE-5

**Files:**
- Create: `frontend_next/README-legal.md`
- Modify: `frontend_next/README.md` (添加法律页面部分)

- [ ] **Step 1: 创建法律页面文档**

```markdown
# 法律合规页面

## 概述

本文档描述Context-OS的法律合规页面实现，包括用户服务协议、隐私政策、开源声明等。

## 页面结构

- `/legal` - 法律中心索引页
- `/legal/terms` - 用户服务协议
- `/legal/privacy` - 隐私政策
- `/legal/licenses` - 开源摘要
- `/legal/licenses/third-party` - 完整第三方声明
- `/legal/licenses/project` - MIT许可证全文

## 开发指南

### 添加新的法律页面

1. 在`content/legal/zh-CN/`目录下创建MDX文件
2. 在`app/(marketing)/legal/`目录下创建页面组件
3. 使用`LegalDocRenderer`组件渲染MDX内容
4. 更新法律中心页面卡片

### 同步法律资产

运行以下命令同步THIRD_PARTY_NOTICES.md和LICENSE文件：

```bash
pnpm sync:legal
```

### 测试

运行法律页面测试：

```bash
pnpm exec playwright test tests/legal/
```

## 验收标准

根据设计文档§9，所有P0验收标准必须通过：

- P0-IA-*: 信息架构与页面
- P0-CNT-*: 页面内容
- P0-FE-*: 前端实现
- P0-UX-*: 视觉与可访问性
- P0-CON-*: 用户同意
- P0-PIPE-*: 仓库资产与CI
- P0-OPS-*: 生产runbook

## 部署

法律页面支持：
- Web SaaS部署
- 桌面静态导出
- 未来营销站部署
```

- [ ] **Step 2: 更新主README**

```markdown
// 在frontend_next/README.md中添加法律页面部分

## 法律合规页面

详见[README-legal.md](./README-legal.md)。
```

- [ ] **Step 3: 运行许可证检查**

```bash
cd ..
./scripts/check-licenses.sh
```

- [ ] **Step 4: 提交文档**

```bash
git add frontend_next/README-legal.md frontend_next/README.md
git commit -m "docs(legal): add legal pages documentation"
```

---

## 验收标准映射

### P0验收标准覆盖

| 验收标准ID | 任务覆盖 | 验证方式 |
|------------|----------|----------|
| P0-IA-1 | Task 2 | 未登录GET → 200 |
| P0-IA-2 | Task 3 | GET 200 |
| P0-IA-3 | Task 4 | GET 200 |
| P0-IA-4 | Task 5 | 产品走查 |
| P0-IA-5 | Task 6 | 见P0-PIPE-2 |
| P0-IA-6 | Task 7 | diff或人工 |
| P0-IA-7 | Task 9 | Playwright / 人工 |
| P0-CNT-1 | Task 3 | 法务checklist |
| P0-CNT-2 | Task 3 | frontmatter + 发布gate |
| P0-CNT-3 | Task 4 | 产品+法务签字 |
| P0-CNT-4 | Task 4 | 法务确认 |
| P0-CNT-5 | Task 5 | 产品走查 |
| P0-CNT-6 | Task 5 | 脚本或人工 |
| P0-FE-1 | Task 1 | 代码审查 |
| P0-FE-2 | Task 1 | 渲染检查 |
| P0-FE-3 | Task 1 | public/legal/有文件 |
| P0-FE-4 | Task 1 | workflow green |
| P0-FE-5 | Task 10 | 未登录访问 + 构建 |
| P0-FE-6 | Task 3 | locale走查 |
| P0-UX-1 | Task 9 | 视觉走查 |
| P0-UX-2 | Task 9 | 锚点跳转 |
| P0-UX-3 | Task 10 | 键盘/读屏抽检 |
| P0-UX-4 | Task 10 | 抽样检测 |
| P0-CON-1 | Task 8 | E2E |
| P0-CON-2 | Task 8 | E2E href |
| P0-CON-3 | Task 8 | DB/API |
| P0-CON-4 | Task 8 | 比对 |
| P0-PIPE-1 | Task 7 | 文件 |
| P0-PIPE-2 | Task 6 | sync + diff |
| P0-PIPE-3 | Task 11 | 本地 + CI |
| P0-PIPE-4 | Task 11 | GitHub Actions |
| P0-PIPE-5 | Task 11 | cargo license |
| P0-OPS-1 | 设计文档 | env清单 |
| P0-OPS-2 | 设计文档 | env清单 |
| P0-OPS-3 | 设计文档 | 文档可查 |

### P1验收标准覆盖

| 验收标准ID | 任务覆盖 | 验证方式 |
|------------|----------|----------|
| P1-PAY-1 | Task 9 | Billing E2E |
| P1-PAY-2 | 设计文档 | GET 200 |
| P1-PAY-3 | Task 4 | 法务 |
| P1-PAY-4 | Task 8 | API/DB |
| P1-APP-1 | 设计文档 | 登录E2E |
| P1-APP-2 | 设计文档 | 流程测试 |
| P1-APP-3 | 设计文档 | 产品走查 |
| P1-DESK-1 | 设计文档 | test -f |
| P1-DESK-2 | 设计文档 | 人工Tauri |
| P1-DESK-3 | 设计文档 | 打包清单 |
| P1-DESK-4 | 设计文档 | 人工 |
| P1-OPS-1 | 设计文档 | runbook |
| P1-OPS-2 | 设计文档 | 流程测试 |

---

## 自动化验收命令

```bash
# 1. 许可证检查
./scripts/check-licenses.sh

# 2. 生成第三方声明
./scripts/generate-third-party-notices.sh

# 3. 前端构建
cd frontend_next && pnpm build

# 4. 桌面构建
cd frontend_next && pnpm build:desktop && test -f out/legal/terms.html

# 5. 法律页面测试
cd frontend_next && pnpm exec playwright test tests/legal/

# 6. 类型检查
cd frontend_next && pnpm typecheck

# 7. 同步法律资产
cd frontend_next && pnpm sync:legal
```

---

## 执行说明

本计划包含11个任务，建议使用compose:subagent执行，每个任务由独立的subagent处理。

**执行顺序：**
1. Task 1: 项目设置和基础组件
2. Task 2-7: 法律页面创建（可并行）
3. Task 8: 同意勾选框组件
4. Task 9: 页脚组件集成
5. Task 10: 测试和验证
6. Task 11: 文档和清理

**关键里程碑：**
- M1: Task 1-7完成（技术骨架就绪）
- M2: Task 8-10完成（可商业化上线）
- M3: Task 11完成（完整设计落地）