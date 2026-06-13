import LegalLayout from "./LegalLayout";

interface LegalDocRendererProps {
  content: string;
  title: string;
  lastUpdated?: string;
  version?: string;
}

export default function LegalDocRenderer({
  content,
  title,
  lastUpdated,
  version,
}: LegalDocRendererProps) {
  return (
    <LegalLayout title={title} lastUpdated={lastUpdated} version={version}>
      <div
        className="legal-document"
        dangerouslySetInnerHTML={{ __html: content }}
      />
    </LegalLayout>
  );
}
