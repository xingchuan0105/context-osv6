import type { CSSProperties } from "react";
import Link from "next/link";

import { compareContent, type CompareLocale } from "@/lib/content/compare";
import { getHubOrigin } from "@/lib/site-map";

const UPDATED = "2026-08-29";

const h2: CSSProperties = {
  fontSize: "1.2rem",
  margin: "1.15rem 0 0.45rem",
  fontWeight: 400,
};

const p: CSSProperties = {
  margin: "0.35rem 0 0.65rem",
  lineHeight: 1.6,
  color: "hsl(var(--foreground))",
};

const muted: CSSProperties = {
  color: "hsl(var(--muted-foreground))",
  fontSize: "13px",
  margin: 0,
};

const th: CSSProperties = {
  textAlign: "left",
  borderBottom: "1px solid hsl(var(--border))",
  padding: "0.45rem 0.55rem",
  fontWeight: 400,
  verticalAlign: "top",
};

const td: CSSProperties = {
  borderBottom: "1px solid hsl(var(--border))",
  padding: "0.45rem 0.55rem",
  verticalAlign: "top",
  fontSize: "0.92rem",
  lineHeight: 1.5,
};

/** Neutral comparison page — SSR for crawlers / GEO (Phase C). Shared by zh (/help/compare) and en (/en/help/compare). */
export function ComparePage({ locale }: { locale: CompareLocale }) {
  const c = compareContent[locale];

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "52rem" }}>
        <header style={{ display: "grid", gap: "0.5rem" }}>
          <p className="app-page-subtitle" style={{ margin: 0 }}>
            {c.subtitle}
          </p>
          <h1 className="app-page-title" style={{ margin: 0 }}>
            {c.h1}
          </h1>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: 0 }}>
            {c.authorPrefix}
            <a
              href={`${getHubOrigin()}/#studio`}
              style={{ color: "inherit", textDecoration: "underline" }}
            >
              {c.authorName}
            </a>
          </p>
          <p style={muted}>
            {c.updatedLabel}
            {UPDATED}
          </p>
          <p style={muted}>
            {c.evidenceText}
            <Link href="/pricing">{c.linkPricing}</Link> ·{" "}
            <Link href="/help/api-access/agents">{c.linkAgents}</Link> ·{" "}
            <Link href="/help/faq">{c.linkFaq}</Link>。
          </p>
          <div className="app-button-row" style={{ flexWrap: "wrap" }}>
            <Link className="app-button-secondary" href="/help/faq">
              {c.buttonFaq}
            </Link>
            <Link className="app-button-secondary" href="/help/api-access/agents">
              {c.buttonAgents}
            </Link>
            <Link className="app-button-secondary" href="/pricing">
              {c.buttonPricing}
            </Link>
            <Link className="app-button-secondary" href="/desktop">
              {c.buttonDesktop}
            </Link>
          </div>
        </header>

        <article
          className="app-surface-card"
          data-testid="help-compare-body"
          style={{ margin: 0, padding: "1.25rem 1.35rem" }}
        >
          <h2 style={h2}>{c.positioningTitle}</h2>
          <p style={p}>
            {c.positioning.map((item, i) => (
              <span key={item.label}>
                {i > 0 ? "  " : ""}
                <strong>{item.label}</strong>
                {locale === "zh-CN" ? "：" : ": "}
                {item.text}
              </span>
            ))}
          </p>

          <h2 style={h2}>{c.tableTitle}</h2>
          <div style={{ overflowX: "auto", margin: "0.75rem 0" }}>
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead>
                <tr>
                  <th style={th}>{c.tableHeaders.dim}</th>
                  <th style={th}>{c.tableHeaders.cos}</th>
                  <th style={th}>{c.tableHeaders.notes}</th>
                  <th style={th}>{c.tableHeaders.rag}</th>
                </tr>
              </thead>
              <tbody>
                {c.rows.map((row) => (
                  <tr key={row.dim}>
                    <td style={td}>{row.dim}</td>
                    <td style={td}>{row.cos}</td>
                    <td style={td}>{row.notes}</td>
                    <td style={td}>{row.rag}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <h2 style={h2}>{c.fitsTitle}</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            {c.fits.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>

          <h2 style={h2}>{c.otherTitle}</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            {c.other.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>

          <h2 style={h2}>{c.noClaimTitle}</h2>
          <p style={p}>{c.noClaim}</p>

          <h2 style={h2}>{c.nextTitle}</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            <li>
              {c.nextFaq}
              <Link href="/help/faq">FAQ</Link>
            </li>
            <li>
              {c.nextAgents}
              <Link href="/help/api-access/agents">Agent API</Link> ·{" "}
              <Link href="/help/api-access">{c.nextApiAccess}</Link>
            </li>
            <li>
              {c.nextPricing}
              <Link href="/pricing">{c.linkPricing}</Link>
            </li>
            <li>
              {c.nextIntegrations}
              <Link href="/integrations">{c.nextIntegrationsLabel}</Link>
            </li>
            <li>
              {c.nextEnter}
              <Link href="/">{c.nextEnterLabel}</Link> · <Link href="/register">{c.nextRegister}</Link>
            </li>
          </ul>
        </article>
      </div>
    </main>
  );
}
