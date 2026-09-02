import type { ReactNode } from "react";

import { OrganizationJsonLd } from "../../components/organization-jsonld";
import { SoftwareApplicationJsonLd } from "../../components/software-application-jsonld";

/** Public pages (agent-readable docs) — no auth shell. */
export default function OpenLayout({ children }: { children: ReactNode }) {
  return (
    <>
      <OrganizationJsonLd />
      <SoftwareApplicationJsonLd />
      {children}
    </>
  );
}
