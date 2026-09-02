import LegalLayout from "./LegalLayout";

import type { TocEntry } from "@/lib/legal/render-markdown";
import type { UiLocale } from "@/lib/i18n/config";

interface LegalDocRendererProps {
  content: string;
  title: string;
  lastUpdated?: string;
  version?: string;
  toc?: TocEntry[];
  /** SSR 语言覆盖（/en/* 法律页传 "en"）。 */
  locale?: UiLocale;
}

export default function LegalDocRenderer({
  content,
  title,
  lastUpdated,
  version,
  toc,
  locale,
}: LegalDocRendererProps) {
  return (
    <LegalLayout
      title={title}
      lastUpdated={lastUpdated}
      version={version}
      toc={toc}
      locale={locale}
    >
      <div
        className="legal-document"
        dangerouslySetInnerHTML={{ __html: content }}
      />
    </LegalLayout>
  );
}
