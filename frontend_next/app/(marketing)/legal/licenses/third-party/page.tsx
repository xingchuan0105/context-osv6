import type { Metadata } from 'next';

import { ThirdPartyNotices } from '@/components/legal/third-party-notices';

export const metadata: Metadata = {
  title: '完整第三方组件声明',
  description: 'Context-OS 使用的所有第三方开源组件及其许可证完整列表。',
  alternates: {
    canonical: '/legal/licenses/third-party',
    languages: {
      "zh-CN": '/legal/licenses/third-party',
      en: '/en/legal/licenses/third-party',
      "x-default": '/legal/licenses/third-party',
    },
  },
};

export default function ThirdPartyNoticesPage() {
  return <ThirdPartyNotices locale="zh-CN" />;
}
