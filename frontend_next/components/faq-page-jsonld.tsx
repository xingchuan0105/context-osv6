/**
 * GEO：/help/faq 与 /en/help/faq 的 FAQPage 结构化数据。
 * 与页面可见的「H2 问题 + P 答案」同源（lib/content/faq.ts），避免标注与正文漂移。
 */
import { faqContent, type FaqLocale } from "@/lib/content/faq";

export function FaqPageJsonLd({ locale }: { locale: FaqLocale }) {
  const items = faqContent[locale].items;

  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "FAQPage",
    inLanguage: locale,
    mainEntity: items.map((item) => ({
      "@type": "Question",
      name: item.question,
      acceptedAnswer: {
        "@type": "Answer",
        text: item.answer,
      },
    })),
  };

  return (
    <script
      type="application/ld+json"
      dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
    />
  );
}
