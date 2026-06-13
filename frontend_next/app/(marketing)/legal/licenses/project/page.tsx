import fs from 'fs';
import path from 'path';
import type { Metadata } from 'next';

import LegalLayout from '@/components/legal/LegalLayout';

export const metadata: Metadata = {
  title: 'MIT 许可证',
  description: 'Context-OS 项目使用的 MIT 许可证全文。',
};

export default async function ProjectLicense() {
  const licensePath = path.join(process.cwd(), 'public/legal/LICENSE');
  let licenseContent = '';

  try {
    licenseContent = fs.readFileSync(licensePath, 'utf8');
  } catch {
    // 回退到根目录 LICENSE
    try {
      const rootLicense = path.join(process.cwd(), '../../LICENSE');
      licenseContent = fs.readFileSync(rootLicense, 'utf8');
    } catch {
      licenseContent = 'MIT许可证文件正在加载中...';
    }
  }

  return (
    <LegalLayout title="MIT许可证">
      <div className="project-license">
        <div className="license-content">
          <pre className="license-text">{licenseContent}</pre>
        </div>
      </div>
    </LegalLayout>
  );
}
