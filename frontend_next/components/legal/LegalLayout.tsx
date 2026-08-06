"use client";

import Link from "next/link";

import type { TocEntry } from "@/lib/legal/render-markdown";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

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
  const { locale } = useUiPreferences();

  return (
    <div className="legal-layout">
      <header className="legal-header">
        <h1>{title}</h1>
        {lastUpdated ? (
          <p className="legal-updated">
            {formatUiMessage(locale, "legalLastUpdated", { date: lastUpdated })}
          </p>
        ) : null}
        {version ? (
          <p className="legal-version">
            {formatUiMessage(locale, "legalVersion", { version })}
          </p>
        ) : null}
      </header>
      <div className="legal-body">
        {toc && toc.length > 0 ? (
          <nav
            className="legal-toc"
            aria-label={formatUiMessage(locale, "legalTocAria")}
          >
            <p className="legal-toc-title">{formatUiMessage(locale, "legalTocTitle")}</p>
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
        ) : null}
        <div className="legal-content">{children}</div>
      </div>
      <footer className="legal-footer">
        <Link href="/legal">{formatUiMessage(locale, "legalBackToCenter")}</Link>
      </footer>
    </div>
  );
}
