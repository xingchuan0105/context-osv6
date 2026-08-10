"use client";

import Link from "next/link";

import { formatUiMessage } from "../../../../lib/i18n/messages";
import { useUiPreferences } from "../../../../lib/ui-preferences";
import { AppTopBar } from "../../../../components/app-top-bar";

export default function HelpWritePage() {
  const { locale } = useUiPreferences();

  return (
    <>
      <AppTopBar locale={locale} />
      <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "72rem" }}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <Link className="app-link app-link-muted" href="/help">
            {formatUiMessage(locale, "helpBackHelp")}
          </Link>
          <h1 className="app-page-title">{formatUiMessage(locale, "helpSectionWriteTitle")}</h1>
          <p className="app-page-subtitle">
            {formatUiMessage(locale, "helpWritePageSubtitle")}
          </p>
        </header>

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <ul
            style={{
              color: "hsl(var(--muted-foreground))",
              display: "grid",
              gap: "0.75rem",
              margin: 0,
              paddingLeft: "1.2rem",
            }}
          >
            <li>{formatUiMessage(locale, "helpItemWrite1")}</li>
            <li>{formatUiMessage(locale, "helpItemWrite2")}</li>
            <li>{formatUiMessage(locale, "helpItemWrite3")}</li>
          </ul>
          <div>
            <Link className="app-link" href="/docs/write-mode.md">
              {formatUiMessage(locale, "helpItemWriteDocs")}
            </Link>
          </div>
        </section>

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <h2 style={{ fontSize: "1.2rem", margin: 0 }}>
            {formatUiMessage(locale, "helpWriteUsageTitle")}
          </h2>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr>
                <th style={{ textAlign: "left", borderBottom: "1px solid hsl(var(--border))", padding: "0.5rem" }}>
                  {formatUiMessage(locale, "helpWriteMetricColumn")}
                </th>
                <th style={{ textAlign: "left", borderBottom: "1px solid hsl(var(--border))", padding: "0.5rem" }}>
                  {formatUiMessage(locale, "helpWriteRangeColumn")}
                </th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td style={{ padding: "0.5rem" }}>{formatUiMessage(locale, "helpWriteLlmCalls")}</td>
                <td style={{ padding: "0.5rem" }}>10–20 / {formatUiMessage(locale, "helpWritePerArticle")}</td>
              </tr>
              <tr>
                <td style={{ padding: "0.5rem" }}>{formatUiMessage(locale, "helpWriteTokenFull")}</td>
                <td style={{ padding: "0.5rem" }}>~100k–200k / {formatUiMessage(locale, "helpWritePerArticle")}</td>
              </tr>
              <tr>
                <td style={{ padding: "0.5rem" }}>{formatUiMessage(locale, "helpWriteWallClock")}</td>
                <td style={{ padding: "0.5rem" }}>2–5 min</td>
              </tr>
            </tbody>
          </table>
        </section>

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <h2 style={{ fontSize: "1.2rem", margin: 0 }}>
            {formatUiMessage(locale, "helpWriteDegradeTitle")}
          </h2>
          <p style={{ color: "hsl(var(--muted-foreground))", margin: 0 }}>
            {formatUiMessage(locale, "helpWriteDegradeBody")}
          </p>
        </section>
      </div>
      </main>
    </>
  );
}