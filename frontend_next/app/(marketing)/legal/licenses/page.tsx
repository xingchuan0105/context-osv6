import type { Metadata } from 'next';
import Link from 'next/link';

import LegalLayout from '@/components/legal/LegalLayout';

export const metadata: Metadata = {
  title: '开源软件说明',
  description: 'Context-OS 使用的开源组件及其许可证摘要。',
};

export default function LicensesSummary() {
  const majorComponents = [
    { category: 'Web框架', components: 'Next.js, React', license: 'MIT' },
    { category: '后端运行时', components: 'Tokio, Axum', license: 'MIT / Apache-2.0' },
    {
      category: '向量检索',
      components: 'Milvus · pgvector',
      license: 'Apache-2.0 / PostgreSQL',
    },
    {
      category: '文档解析',
      components: 'markitdown · firecrawl-anydoc',
      license: 'MIT',
    },
    { category: '客户端壳', components: 'Tauri 2', license: 'MIT / Apache-2.0' },
    { category: 'AI推理', components: 'DeepSeek, DashScope 等', license: '商业API' },
  ];

  const weakCopyleft = [
    { component: 'dompurify', note: '选择Apache-2.0版本' },
    { component: 'cssparser', note: 'MPL，未修改则仅需NOTICE' },
    {
      component: 'MinIO / Redis 7.4+（服务端）',
      note: '见第三方声明商业清单：优先云 S3/OSS；Redis 用 Valkey 或 ≤7.2',
    },
  ];

  return (
    <LegalLayout title="开源软件说明" lastUpdated="2026-08-05">
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
          <h2>客户端</h2>
          <p>
            客户端壳层使用 Tauri 2（MIT / Apache-2.0）。完整安装包可捆绑便携
            PostgreSQL、pgvector 与 Redis Windows 端口（BSD-3-Clause 历史端口，
            非 SSPL），声明见安装目录 <code>runtime/THIRD_PARTY.txt</code>，
            以及完整第三方声明中的 Desktop 章节。About 对话框亦可查看摘要。
          </p>
        </section>
      </div>
    </LegalLayout>
  );
}
