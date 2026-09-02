import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import type { Metadata } from 'next';

import LegalDocRenderer from '@/components/legal/LegalDocRenderer';
import { renderLegalMarkdown } from '@/lib/legal/render-markdown';

export const metadata: Metadata = {
  title: 'Terms of Service',
  description:
    'Context-OS Terms of Service: the terms and conditions that apply when you use our service.',
  alternates: {
    canonical: '/en/legal/terms',
    languages: {
      'zh-CN': '/legal/terms',
      en: '/en/legal/terms',
      'x-default': '/legal/terms',
    },
  },
};

export default async function EnTermsPage() {
  const termsPath = path.join(process.cwd(), 'content/legal/en/terms.mdx');
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
      locale="en"
    />
  );
}