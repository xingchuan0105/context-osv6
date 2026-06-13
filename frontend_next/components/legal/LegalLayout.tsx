import Link from "next/link";

interface LegalLayoutProps {
  children: React.ReactNode;
  title: string;
  lastUpdated?: string;
  version?: string;
}

export default function LegalLayout({
  children,
  title,
  lastUpdated,
  version,
}: LegalLayoutProps) {
  return (
    <div className="legal-layout">
      <div className="legal-header">
        <h1>{title}</h1>
        {lastUpdated && (
          <p className="legal-updated">最后更新: {lastUpdated}</p>
        )}
        {version && <p className="legal-version">版本: {version}</p>}
      </div>
      <div className="legal-content">{children}</div>
      <div className="legal-footer">
        <Link href="/legal">返回法律中心</Link>
      </div>
    </div>
  );
}
