import fs from "fs";
import path from "path";
import matter from "gray-matter";
import Link from "next/link";

import LegalFooterLinks from "@/components/legal/LegalFooterLinks";
import { MarketingShell } from "@/components/marketing-chrome";
import type { UiLocale } from "@/lib/i18n/config";

// 从 MDX frontmatter 读取版本号作为卡片显示日期，
// 避免硬编码与 MDX 实际版本漂移。en 版读 content/legal/en/。
const MDX_DIR_BY_LOCALE: Record<UiLocale, string> = {
  "zh-CN": path.join(process.cwd(), "content/legal/zh-CN"),
  en: path.join(process.cwd(), "content/legal/en"),
};

function readMdxVersion(locale: UiLocale, filename: string): string {
  const content = fs.readFileSync(path.join(MDX_DIR_BY_LOCALE[locale], filename), "utf8");
  const { data } = matter(content);
  return typeof data.version === "string" ? data.version : "";
}

type LegalCenterCopy = {
  title: string;
  lead: string;
  cards: { title: string; description: string; href: string }[];
  updatedLabel: string;
  contact: string;
};

const copy: Record<UiLocale, LegalCenterCopy> = {
  "zh-CN": {
    title: "法律中心",
    lead: "使用Context-OS前请阅读以下文档",
    cards: [
      {
        title: "用户服务协议",
        description: "使用Context-OS服务前请阅读本协议",
        href: "/legal/terms",
      },
      {
        title: "隐私政策",
        description: "了解我们如何收集、使用和保护您的个人信息",
        href: "/legal/privacy",
      },
      {
        title: "开源声明",
        description: "查看我们使用的开源组件及其许可证",
        href: "/legal/licenses",
      },
    ],
    updatedLabel: "最后更新",
    contact: "如有法律问题，请联系",
  },
  en: {
    title: "Legal center",
    lead: "Please read the following documents before using Context-OS",
    cards: [
      {
        title: "Terms of service",
        description: "Read these terms before using Context-OS",
        href: "/en/legal/terms",
      },
      {
        title: "Privacy policy",
        description: "How we collect, use, and protect your personal information",
        href: "/en/legal/privacy",
      },
      {
        title: "Open-source notices",
        description: "Open-source components we use and their licenses",
        href: "/en/legal/licenses",
      },
    ],
    updatedLabel: "Last updated",
    contact: "For legal questions, contact",
  },
};

export function LegalCenter({ locale }: { locale: UiLocale }) {
  const t = copy[locale];
  const termsVersion = readMdxVersion(locale, "terms.mdx");
  const privacyVersion = readMdxVersion(locale, "privacy.mdx");
  // 开源声明无独立 MDX；跟随最近一次法律文档升级
  const licensesVersion = termsVersion || privacyVersion;

  const cards = t.cards.map((card, index) => ({
    ...card,
    lastUpdated: [termsVersion, privacyVersion, licensesVersion][index],
  }));

  return (
    <MarketingShell active="legal" locale={locale}>
      <div className="legal-center">
        <div className="legal-center-header">
          <h1>{t.title}</h1>
          <p>{t.lead}</p>
        </div>

        <div className="legal-cards">
          {cards.map((card) => (
            <Link key={card.href} href={card.href} className="legal-card">
              <h2>{card.title}</h2>
              <p>{card.description}</p>
              <span className="legal-card-updated">
                {t.updatedLabel}: {card.lastUpdated}
              </span>
            </Link>
          ))}
        </div>

        <div className="legal-contact">
          <p>
            {t.contact}: <a href="mailto:legal@context-os.com">legal@context-os.com</a>
          </p>
        </div>

        <LegalFooterLinks locale={locale} />
      </div>
    </MarketingShell>
  );
}
