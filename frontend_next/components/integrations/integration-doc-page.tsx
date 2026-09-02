import Link from "next/link";
import type { CSSProperties } from "react";

import {
  integrationDocs,
  integrationsShared,
  integrationsUpdated,
  type IntegrationSection,
} from "@/lib/content/integrations";

import { IntegrationsHeader, SharedCtaButtons } from "./shared";

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

function Section({ section }: { section: IntegrationSection }) {
  return (
    <section>
      <h2 style={h2}>{section.h2}</h2>
      {section.paragraphs?.map((text) => (
        <p key={text.slice(0, 24)} style={p}>
          {text}
        </p>
      ))}
      {section.bullets ? (
        <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
          {section.bullets.map((item) => (
            <li key={item.slice(0, 24)}>{item}</li>
          ))}
        </ul>
      ) : null}
      {section.table ? (
        <div style={{ overflowX: "auto", margin: "0.75rem 0" }}>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr>
                {section.table.headers.map((header) => (
                  <th key={header} style={th}>
                    {header}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {section.table.rows.map((row) => (
                <tr key={row[0]}>
                  {row.map((cell, i) => (
                    <td key={i} style={td}>
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {section.code ? (
        <div style={{ margin: "0.6rem 0" }}>
          <p style={muted}>{section.code.label}</p>
          <pre
            style={{
              background: "hsl(var(--muted))",
              fontSize: "0.85rem",
              lineHeight: 1.5,
              margin: "0.35rem 0 0.65rem",
              overflowX: "auto",
              padding: "0.75rem",
            }}
          >
            <code>{section.code.text}</code>
          </pre>
        </div>
      ) : null}
    </section>
  );
}

/** 集成承接页（zh 先行；en 待人工翻译后并入）。共享 zh/en 组件模式同 faq-page / compare-page。 */
export function IntegrationDocPage({ slug }: { slug: keyof typeof integrationDocs }) {
  const doc = integrationDocs[slug];
  const shared = integrationsShared;

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "52rem" }}>
        <IntegrationsHeader
          subtitle={doc.subtitle}
          h1={doc.h1}
          buttons={<SharedCtaButtons {...shared} />}
          updated={`${shared.updatedLabel}${integrationsUpdated}`}
          authorPrefix={shared.authorPrefix}
          authorName={shared.authorName}
        />

        <article
          className="app-surface-card"
          data-testid={`integrations-${doc.slug}-body`}
          style={{ margin: 0, padding: "1.25rem 1.35rem" }}
        >
          {doc.intro.map((text) => (
            <p key={text.slice(0, 24)} style={{ ...p, marginTop: 0 }}>
              {text}
            </p>
          ))}

          {doc.sections.map((section) => (
            <Section key={section.h2} section={section} />
          ))}

          <h2 style={h2}>{shared.evidenceTitle}</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            {shared.evidenceItems.map((item) => (
              <li key={item.href}>
                <Link href={item.href}>{item.label}</Link>
              </li>
            ))}
          </ul>
          <p style={muted}>
            <Link href="/integrations">{shared.backLabel}</Link>
          </p>
        </article>
      </div>
    </main>
  );
}
