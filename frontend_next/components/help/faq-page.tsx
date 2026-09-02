import type { CSSProperties } from "react";
import Link from "next/link";

import { FaqPageJsonLd } from "@/components/faq-page-jsonld";
import { faqContent, type FaqLocale } from "@/lib/content/faq";
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

/** Public product FAQ — SSR for crawlers / GEO (Phase C). Shared by zh (/help/faq) and en (/en/help/faq). */
export function FaqPage({ locale }: { locale: FaqLocale }) {
  const c = faqContent[locale];

  return (
    <main className="app-page-shell">
      <FaqPageJsonLd locale={locale} />
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "48rem" }}>
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
            {c.evidencePrefix}
            <Link href="/pricing">{c.linkPricing}</Link> ·{" "}
            <Link href="/help/api-access">{c.linkApiAccess}</Link> ·{" "}
            <Link href="/help/api-access/agents">{c.linkAgents}</Link>
            {c.evidenceSuffix}
          </p>
          <div className="app-button-row" style={{ flexWrap: "wrap" }}>
            <Link className="app-button-secondary" href="/help/compare">
              {c.buttonCompare}
            </Link>
            <Link className="app-button-secondary" href="/help/api-access">
              {c.buttonApiAccess}
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
          data-testid="help-faq-body"
          style={{ margin: 0, padding: "1.25rem 1.35rem" }}
        >
          {c.items.map((item) => (
            <section key={item.question}>
              <h2 style={h2}>{item.question}</h2>
              <p style={p}>{item.answer}</p>
            </section>
          ))}

          <h2 style={h2}>{c.evidenceTitle}</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            <li>
              <Link href="/pricing">{c.evidencePricing}</Link>
            </li>
            <li>
              <Link href="/help/api-access">{c.evidenceApiAccess}</Link>
            </li>
            <li>
              <Link href="/help/api-access/agents">{c.evidenceAgents}</Link>
            </li>
            <li>
              <Link href="/help/compare">{c.evidenceCompare}</Link>
            </li>
            <li>
              <Link href="/integrations">{c.evidenceIntegrations}</Link>
            </li>
          </ul>
        </article>
      </div>
    </main>
  );
}
