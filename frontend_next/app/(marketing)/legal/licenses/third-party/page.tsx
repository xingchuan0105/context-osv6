import fs from 'fs';
import path from 'path';
import type { Metadata } from 'next';
import Link from 'next/link';

import LegalLayout from '@/components/legal/LegalLayout';
import { renderLegalMarkdown } from '@/lib/legal/render-markdown';

export const metadata: Metadata = {
  title: '完整第三方组件声明',
  description: 'Context-OS 使用的所有第三方开源组件及其许可证完整列表。',
};

export default async function ThirdPartyNotices() {
  const noticesPath = path.join(process.cwd(), 'public/legal/third-party-notices.md');
  let noticesContent = '';
  let totalPackages = 0;
  let generationDate = '';

  try {
    noticesContent = fs.readFileSync(noticesPath, 'utf8');
    // 统计 ### 级别标题作为组件条目数
    const componentEntries = noticesContent.match(/^### /gm);
    totalPackages = componentEntries?.length || 0;
    // 尝试从生成日期注释中提取
    const dateMatch = noticesContent.match(
      /[Gg]enerated:\s*(\d{4}-\d{2}-\d{2})/,
    );
    generationDate = dateMatch?.[1] || new Date().toISOString().split('T')[0];
  } catch {
    noticesContent = '第三方声明文件正在生成中，请运行 `pnpm sync:legal` 生成。';
    generationDate = new Date().toISOString().split('T')[0];
  }

  const { html } = await renderLegalMarkdown(noticesContent);

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
          </div>
        </div>

        <div
          className="notices-content"
          dangerouslySetInnerHTML={{ __html: html }}
        />

        <div className="notices-footer">
          <Link href="/legal/licenses">返回开源摘要</Link>
        </div>
      </div>
    </LegalLayout>
  );
}
