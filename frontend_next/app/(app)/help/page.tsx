import type { Metadata } from "next";

import HelpCenterClient from "./help-center-client";

/**
 * 应用内帮助中心 = app 壳页（cookie 双语、登录后主题），不是公开帮助索引；
 * 公开帮助发现走 /help/faq、/help/compare、/help/api-access（Phase E E1.2-B）。
 * noindex 而非 robots Disallow：/help 前缀会误杀 /help/faq 等公开帮助页。
 */
export const metadata: Metadata = {
  robots: { index: false, follow: true },
};

export default function HelpPage() {
  return <HelpCenterClient />;
}
