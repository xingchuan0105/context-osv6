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
