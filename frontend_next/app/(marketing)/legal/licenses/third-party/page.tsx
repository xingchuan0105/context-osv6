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
    const crateMatches = noticesContent.match(/\bcrate\b/gi);
    const packageMatches = noticesContent.match(/\bpackage\b/gi);
    totalPackages = (crateMatches?.length || 0) + (packageMatches?.length || 0);
    generationDate = new Date().toISOString().split('T')[0];
  } catch {
    noticesContent = '第三方声明文件正在生成中...';
  }

  const htmlContent = noticesContent
    .replace(/^### (.*$)/gim, '<h3>$1</h3>')
    .replace(/^## (.*$)/gim, '<h2>$1</h2>')
    .replace(/^# (.*$)/gim, '<h1>$1</h1>')
    .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.*?)\*/g, '<em>$1</em>')
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
