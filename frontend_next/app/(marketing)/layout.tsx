import type { ReactNode } from "react";

import { OrganizationJsonLd } from "../../components/organization-jsonld";
import { SoftwareApplicationJsonLd } from "../../components/software-application-jsonld";

/**
 * Marketing routes share a light discovery chrome via page wrappers.
 * Individual pages set `active` on MarketingShell where needed.
 */
export default function MarketingLayout({ children }: { children: ReactNode }) {
  return (
    <>
      <OrganizationJsonLd />
      <SoftwareApplicationJsonLd />
      {children}
    </>
  );
}
