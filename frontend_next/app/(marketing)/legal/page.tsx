import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import type { Metadata } from 'next';
import Link from 'next/link';

import LegalFooterLinks from '@/components/legal/LegalFooterLinks';

export const metadata: Metadata = {
  title: '法律中心',
  description: 'Context-OS 法律中心，查看用户服务协议、隐私政策和开源声明。',
};

// 从 MDX frontmatter 读取版本号作为卡片显示日期，
// 避免硬编码与 MDX 实际版本漂移。
const MDX_DIR = path.join(process.cwd(), 'content/legal/zh-CN');

function readMdxVersion(filename: string): string {
  const content = fs.readFileSync(path.join(MDX_DIR, filename), 'utf8');
  const { data } = matter(content);
  return typeof data.version === 'string' ? data.version : '';
}

export default function LegalCenter() {
  const termsVersion = readMdxVersion('terms.mdx');
  const privacyVersion = readMdxVersion('privacy.mdx');
  // 开源声明无独立 MDX；跟随最近一次法律文档升级
  const licensesVersion = termsVersion || privacyVersion;

  const cards = [
    {
      title: '用户服务协议',
      description: '使用Context-OS服务前请阅读本协议',
      href: '/legal/terms',
      lastUpdated: termsVersion,
    },
    {
      title: '隐私政策',
      description: '了解我们如何收集、使用和保护您的个人信息',
      href: '/legal/privacy',
      lastUpdated: privacyVersion,
    },
    {
      title: '开源声明',
      description: '查看我们使用的开源组件及其许可证',
      href: '/legal/licenses',
      lastUpdated: licensesVersion,
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
