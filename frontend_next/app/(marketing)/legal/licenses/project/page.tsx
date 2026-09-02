import type { Metadata } from 'next';

import { ProjectLicense } from '@/components/legal/project-license';

export const metadata: Metadata = {
  title: 'MIT 许可证',
  description: 'Context-OS 项目使用的 MIT 许可证全文。',
  alternates: {
    canonical: '/legal/licenses/project',
    languages: {
      "zh-CN": '/legal/licenses/project',
      en: '/en/legal/licenses/project',
      "x-default": '/legal/licenses/project',
    },
  },
};

export default function ProjectLicensePage() {
  return <ProjectLicense locale="zh-CN" />;
}
