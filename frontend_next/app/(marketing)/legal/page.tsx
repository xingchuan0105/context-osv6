import type { Metadata } from 'next';

import { LegalCenter } from '@/components/legal/legal-center';

export const metadata: Metadata = {
  title: '法律中心',
  description: 'Context-OS 法律中心，查看用户服务协议、隐私政策和开源声明。',
  alternates: {
    canonical: '/legal',
    languages: {
      "zh-CN": '/legal',
      en: '/en/legal',
      "x-default": '/legal',
    },
  },
};

export default function LegalCenterPage() {
  return <LegalCenter locale="zh-CN" />;
}
