import Link from "next/link";

import type { TocEntry } from "@/lib/legal/render-markdown";

interface LegalLayoutProps {
  children: React.ReactNode;
  title: string;
  lastUpdated?: string;
  version?: string;
  toc?: TocEntry[];
}

export default function LegalLayout({
  children,
  title,
  lastUpdated,
  version,
  toc,
}: LegalLayoutProps) {
  return (
    <div className="legal-layout">
      <header className="legal-header">
        <h1>{title}</h1>
        {lastUpdated && (
          <p className="legal-updated">最后更新: {lastUpdated}</p>
        )}
        {version && <p className="legal-version">版本: {version}</p>}
      </header>
      <div className="legal-body">
        {toc && toc.length > 0 && (
          <nav className="legal-toc" aria-label="文档目录">
            <p className="legal-toc-title">目录</p>
            <ul className="legal-toc-list">
              {toc.map((entry) => (
                <li
                  key={entry.id}
                  className={`legal-toc-item legal-toc-depth-${entry.depth}`}
                >
                  <a href={`#${entry.id}`}>{entry.text}</a>
                </li>
              ))}
            </ul>
          </nav>
        )}
        <div className="legal-content">{children}</div>
      </div>
      <footer className="legal-footer">
        <Link href="/legal">返回法律中心</Link>
      </footer>
    </div>
  );
}
