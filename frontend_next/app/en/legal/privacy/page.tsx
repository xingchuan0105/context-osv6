import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import type { Metadata } from 'next';

import LegalDocRenderer from '@/components/legal/LegalDocRenderer';
import { renderLegalMarkdown } from '@/lib/legal/render-markdown';

export const metadata: Metadata = {
  title: 'Privacy Policy',
  description:
    'Context-OS Privacy Policy: how we collect, use, and protect your personal information.',
  alternates: {
    canonical: '/en/legal/privacy',
    languages: {
      'zh-CN': '/legal/privacy',
      en: '/en/legal/privacy',
      'x-default': '/legal/privacy',
    },
  },
};

export default async function EnPrivacyPage() {
  const privacyPath = path.join(process.cwd(), 'content/legal/en/privacy.mdx');
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
      locale="en"
    />
  );
}