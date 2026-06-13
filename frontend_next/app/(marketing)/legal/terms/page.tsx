import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import LegalDocRenderer from '@/components/legal/LegalDocRenderer';

export default async function TermsPage() {
  const termsPath = path.join(process.cwd(), 'content/legal/zh-CN/terms.mdx');
  const fileContent = fs.readFileSync(termsPath, 'utf8');
  const { content, data } = matter(fileContent);
  
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
