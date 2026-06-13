import LegalLayout from "./LegalLayout";

import type { TocEntry } from "@/lib/legal/render-markdown";

interface LegalDocRendererProps {
  content: string;
  title: string;
  lastUpdated?: string;
  version?: string;
  toc?: TocEntry[];
}

export default function LegalDocRenderer({
  content,
  title,
  lastUpdated,
  version,
  toc,
}: LegalDocRendererProps) {
  return (
    <LegalLayout
      title={title}
      lastUpdated={lastUpdated}
      version={version}
      toc={toc}
    >
      <div
        className="legal-document"
        dangerouslySetInnerHTML={{ __html: content }}
      />
    </LegalLayout>
  );
}
