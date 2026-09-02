import Link from "next/link";

import { getHubOrigin } from "@/lib/site-map";

/** /integrations 族共用页眉（作者 / 更新日 / CTA），口径与 faq-page / compare-page 一致。 */
export function IntegrationsHeader({
  subtitle,
  h1,
  buttons,
  updated,
  authorPrefix,
  authorName,
}: {
  subtitle: string;
  h1: string;
  buttons: React.ReactNode;
  updated: string;
  authorPrefix: string;
  authorName: string;
}) {
  return (
    <header style={{ display: "grid", gap: "0.5rem" }}>
      <p className="app-page-subtitle" style={{ margin: 0 }}>
        {subtitle}
      </p>
      <h1 className="app-page-title" style={{ margin: 0 }}>
        {h1}
      </h1>
      <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: 0 }}>
        {authorPrefix}
        <a
          href={`${getHubOrigin()}/#studio`}
          style={{ color: "inherit", textDecoration: "underline" }}
        >
          {authorName}
        </a>
      </p>
      <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "13px", margin: 0 }}>
        {updated}
      </p>
      <div className="app-button-row" style={{ flexWrap: "wrap" }}>
        {buttons}
      </div>
    </header>
  );
}

/** CTA 三链是 E0.2 canonical 约束：agents 文档 / desktop / pricing，不出现第二 checkout / 下载路径。 */
export function SharedCtaButtons({
  buttonAgents,
  buttonDesktop,
  buttonPricing,
}: {
  buttonAgents: string;
  buttonDesktop: string;
  buttonPricing: string;
}) {
  return (
    <>
      <Link className="app-button-secondary" href="/help/api-access/agents">
        {buttonAgents}
      </Link>
      <Link className="app-button-secondary" href="/desktop">
        {buttonDesktop}
      </Link>
      <Link className="app-button-secondary" href="/pricing">
        {buttonPricing}
      </Link>
    </>
  );
}
