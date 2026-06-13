import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import type { Metadata } from 'next';

import LegalDocRenderer from '@/components/legal/LegalDocRenderer';
import { renderLegalMarkdown } from '@/lib/legal/render-markdown';

export const metadata: Metadata = {
  title: '用户服务协议',
  description: 'Context-OS 用户服务协议，了解使用我们服务的条款与条件。',
};

export default async function TermsPage() {
  const termsPath = path.join(process.cwd(), 'content/legal/zh-CN/terms.mdx');
  const fileContent = fs.readFileSync(termsPath, 'utf8');
  const { content, data } = matter(fileContent);

  const { html, toc } = await renderLegalMarkdown(content);

  return (
    <LegalDocRenderer
      content={html}
      title={data.title}
      lastUpdated={data.version}
      version={data.version}
      toc={toc}
    />
  );
}
