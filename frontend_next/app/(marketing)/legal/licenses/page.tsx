import type { Metadata } from 'next';

import { LicensesSummary } from '@/components/legal/licenses-summary';

export const metadata: Metadata = {
  title: '开源软件说明',
  description: 'Context-OS 使用的开源组件及其许可证摘要。',
  alternates: {
    canonical: '/legal/licenses',
    languages: {
      "zh-CN": '/legal/licenses',
      en: '/en/legal/licenses',
      "x-default": '/legal/licenses',
    },
  },
};

export default function LicensesSummaryPage() {
  return <LicensesSummary locale="zh-CN" />;
}
