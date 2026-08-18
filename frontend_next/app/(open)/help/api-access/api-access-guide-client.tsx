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

/**
 * 公开版 API 接入说明（GEO/SEO 方案 A3）：未登录即可读，无 App top bar。
 * 密钥创建/撤销仍在登录后的工作区分享中心（/dashboard/:id/share#api）。
 */
export function ApiAccessGuideClient() {
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
              <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: "0.5rem 0 0" }}>
                {formatUiMessage(locale, "home.seoPublisher")}
              </p>
              <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "13px", margin: "0.25rem 0 0" }}>
                {formatUiMessage(locale, "home.seoUpdated", { date: "2026-08-12" })}
              </p>
              <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "13px", margin: "0.25rem 0 0" }}>
                {formatUiMessage(locale, "home.seoEvidence")}
              </p>
            </div>
            <div className="app-button-row">
              <Link className="app-button-secondary" href="/help">
                {formatUiMessage(locale, "helpApiAccessBackHelp")}
              </Link>
              <Link className="app-button-secondary" href="/help/api-access/agents">
                {formatUiMessage(locale, "helpItemApiAgentDocs")}
              </Link>
              <Link className="app-button-secondary" href="/help/faq">
                FAQ
              </Link>
              <Link className="app-button-secondary" href="/help/compare">
                选型对比
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
          <ol
            style={{
              color: "hsl(var(--muted-foreground))",
              display: "grid",
              gap: "0.5rem",
              margin: 0,
              paddingLeft: "1.2rem",
            }}
          >
            <li>{formatUiMessage(locale, "helpApiAccessAutomationStep1")}</li>
            <li>{formatUiMessage(locale, "helpApiAccessAutomationStep2")}</li>
            <li>{formatUiMessage(locale, "helpApiAccessAutomationStep3")}</li>
            <li>{formatUiMessage(locale, "helpApiAccessAutomationStep4")}</li>
          </ol>
          <div className="app-button-row" style={{ flexWrap: "wrap" }}>
            <Link className="app-link app-link-muted" href="/help/api-access/agents">
              {formatUiMessage(locale, "helpItemApiAgentDocs")}
            </Link>
          </div>
        </section>
      </div>
    </main>
  );
}
