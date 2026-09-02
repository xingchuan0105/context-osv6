import Link from "next/link";
import type { CSSProperties } from "react";

import { integrationDocs, integrationsShared, integrationsUpdated } from "@/lib/content/integrations";

import { IntegrationsHeader, SharedCtaButtons } from "./shared";

const cardStyle: CSSProperties = {
  display: "grid",
  gap: "0.4rem",
  padding: "1rem 1.1rem",
};

/** /integrations 索引：三张承接卡 + 互链（help 族入口，非主 nav）。 */
export function IntegrationIndexPage() {
  const shared = integrationsShared;

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "52rem" }}>
        <IntegrationsHeader
          subtitle={shared.indexSubtitle}
          h1={shared.indexH1}
          buttons={<SharedCtaButtons {...shared} />}
          updated={`${shared.updatedLabel}${integrationsUpdated}`}
          authorPrefix={shared.authorPrefix}
          authorName={shared.authorName}
        />

        <div style={{ display: "grid", gap: "0.75rem" }}>
          {Object.values(integrationDocs).map((doc) => (
            <Link
              key={doc.slug}
              className="app-surface-card"
              href={`/integrations/${doc.slug}`}
              data-testid={`integrations-card-${doc.slug}`}
              style={cardStyle}
            >
              <strong>{doc.cardTitle}</strong>
              <span style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.92rem", lineHeight: 1.5 }}>
                {doc.cardDesc}
              </span>
            </Link>
          ))}
        </div>

        <article
          className="app-surface-card"
          data-testid="integrations-index-body"
          style={{ margin: 0, padding: "1.25rem 1.35rem" }}
        >
          {shared.indexIntro.map((text) => (
            <p key={text.slice(0, 24)} style={{ margin: "0.35rem 0 0.65rem", lineHeight: 1.6 }}>
              {text}
            </p>
          ))}
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "13px", margin: 0 }}>
            {shared.indexEvidenceLine}
          </p>

          <h2 style={{ fontSize: "1.2rem", fontWeight: 400, margin: "1.15rem 0 0.45rem" }}>
            {shared.evidenceTitle}
          </h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            {shared.evidenceItems.map((item) => (
              <li key={item.href}>
                <Link href={item.href}>{item.label}</Link>
              </li>
            ))}
          </ul>
        </article>
      </div>
    </main>
  );
}
