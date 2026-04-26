"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { useUiPreferences } from "../../../lib/ui-preferences";

function HelpSection({
  title,
  items,
}: {
  title: string;
  items: ReactNode[];
}) {
  return (
    <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
      <h2 style={{ fontSize: "1.2rem", margin: 0 }}>{title}</h2>
      <ul style={{ color: "hsl(var(--muted-foreground))", display: "grid", gap: "0.75rem", margin: 0, paddingLeft: "1.2rem" }}>
        {items.map((item, index) => (
          <li key={index}>{item}</li>
        ))}
      </ul>
    </section>
  );
}

export default function HelpPage() {
  const { locale } = useUiPreferences();

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "72rem" }}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <div style={{ alignItems: "start", display: "flex", flexWrap: "wrap", gap: "0.75rem", justifyContent: "space-between" }}>
            <div>
              <h1 className="app-page-title">{formatUiMessage(locale, "helpTitle")}</h1>
              <p className="app-page-subtitle">
                {formatUiMessage(locale, "helpSubtitle")}
              </p>
            </div>
            <div className="app-button-row">
              <Link className="app-button-secondary" href="/dashboard">
                {formatUiMessage(locale, "helpBackDashboard")}
              </Link>
              <Link className="app-button-secondary" href="/settings?tab=profile">
                {formatUiMessage(locale, "helpAccountSettings")}
              </Link>
            </div>
          </div>
        </header>

        <HelpSection
          title={formatUiMessage(locale, "helpSectionAccountTitle")}
          items={[
            formatUiMessage(locale, "helpItemAccount1"),
            formatUiMessage(locale, "helpItemAccount2"),
            formatUiMessage(locale, "helpItemAccount3"),
          ]}
        />

        <HelpSection
          title={formatUiMessage(locale, "helpSectionWorkspaceTitle")}
          items={[
            formatUiMessage(locale, "helpItemWorkspace1"),
            formatUiMessage(locale, "helpItemWorkspace2"),
            formatUiMessage(locale, "helpItemWorkspace3"),
          ]}
        />

        <HelpSection
          title={formatUiMessage(locale, "helpSectionDocsTitle")}
          items={[
            formatUiMessage(locale, "helpItemDocs1"),
            formatUiMessage(locale, "helpItemDocs2"),
            formatUiMessage(locale, "helpItemDocs3"),
          ]}
        />

        <HelpSection
          title={formatUiMessage(locale, "helpSectionCollabTitle")}
          items={[
            formatUiMessage(locale, "helpItemCollab1"),
            formatUiMessage(locale, "helpItemCollab2"),
            formatUiMessage(locale, "helpItemCollab3"),
          ]}
        />

        <HelpSection
          title={formatUiMessage(locale, "helpSectionApiTitle")}
          items={[
            formatUiMessage(locale, "helpItemApi1"),
            formatUiMessage(locale, "helpItemApi2"),
            formatUiMessage(locale, "helpItemApi3"),
            <Link className="app-link app-link-muted" href="/help/api-access">
              {formatUiMessage(locale, "helpItemApiHumanDocs")}
            </Link>,
            <Link className="app-link app-link-muted" href="/docs/api-access-for-agents.md">
              {formatUiMessage(locale, "helpItemApiAgentDocs")}
            </Link>,
          ]}
        />

        <HelpSection
          title={formatUiMessage(locale, "helpSectionTroubleshootingTitle")}
          items={[
            formatUiMessage(locale, "helpItemTroubleshooting1"),
            formatUiMessage(locale, "helpItemTroubleshooting2"),
            formatUiMessage(locale, "helpItemTroubleshooting3"),
          ]}
        />
      </div>
    </main>
  );
}
