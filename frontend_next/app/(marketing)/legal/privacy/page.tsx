import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import type { Metadata } from 'next';

import LegalDocRenderer from '@/components/legal/LegalDocRenderer';
import { renderLegalMarkdown } from '@/lib/legal/render-markdown';

export const metadata: Metadata = {
  title: '隐私政策',
  description: 'Context-OS 隐私政策，了解我们如何收集、使用和保护您的个人信息。',
};

export default async function PrivacyPage() {
  const privacyPath = path.join(process.cwd(), 'content/legal/zh-CN/privacy.mdx');
  const fileContent = fs.readFileSync(privacyPath, 'utf8');
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
