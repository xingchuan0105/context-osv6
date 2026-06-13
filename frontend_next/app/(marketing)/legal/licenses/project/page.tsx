import fs from 'fs';
import path from 'path';
import LegalLayout from '@/components/legal/LegalLayout';

export default async function ProjectLicense() {
  const licensePath = path.join(process.cwd(), 'public/legal/LICENSE');
  let licenseContent = '';

  try {
    licenseContent = fs.readFileSync(licensePath, 'utf8');
  } catch {
    licenseContent = 'MIT许可证文件正在加载中...';
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
