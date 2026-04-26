"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import { formatUiMessage } from "../../../../lib/i18n/messages";
import { useUiPreferences } from "../../../../lib/ui-preferences";

function DocSection({
  title,
  items,
}: {
  title: string;
  items: ReactNode[];
}) {
  return (
    <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
      <h2 style={{ fontSize: "1.2rem", margin: 0 }}>{title}</h2>
      <ul
        style={{
          color: "hsl(var(--muted-foreground))",
          display: "grid",
          gap: "0.75rem",
          margin: 0,
          paddingLeft: "1.2rem",
        }}
      >
        {items.map((item, index) => (
          <li key={index}>{item}</li>
        ))}
      </ul>
    </section>
  );
}

export default function HelpApiAccessPage() {
  const { locale } = useUiPreferences();

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "72rem" }}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <div
            style={{
              alignItems: "start",
              display: "flex",
              flexWrap: "wrap",
              gap: "0.75rem",
              justifyContent: "space-between",
            }}
          >
            <div>
              <h1 className="app-page-title">{formatUiMessage(locale, "helpApiAccessTitle")}</h1>
              <p className="app-page-subtitle">{formatUiMessage(locale, "helpApiAccessSubtitle")}</p>
            </div>
            <div className="app-button-row">
              <Link className="app-button-secondary" href="/help">
                {formatUiMessage(locale, "helpApiAccessBackHelp")}
              </Link>
              <Link className="app-button-secondary" href="/docs/api-access-for-agents.md">
                {formatUiMessage(locale, "helpItemApiAgentDocs")}
              </Link>
            </div>
          </div>
        </header>

        <DocSection
          title={formatUiMessage(locale, "helpApiAccessOverviewTitle")}
          items={[
            formatUiMessage(locale, "helpItemApi1"),
            formatUiMessage(locale, "helpItemApi2"),
            formatUiMessage(locale, "helpItemApi3"),
          ]}
        />

        <section className="app-surface-card" style={{ display: "grid", gap: "0.75rem" }}>
          <h2 style={{ fontSize: "1.2rem", margin: 0 }}>{formatUiMessage(locale, "helpApiAccessAutomationTitle")}</h2>
          <p style={{ color: "hsl(var(--muted-foreground))", margin: 0 }}>
            {formatUiMessage(locale, "helpApiAccessAutomationBody")}
          </p>
          <div>
            <Link className="app-link app-link-muted" href="/docs/api-access-for-agents.md">
              {formatUiMessage(locale, "helpItemApiAgentDocs")}
            </Link>
          </div>
        </section>
      </div>
    </main>
  );
}
