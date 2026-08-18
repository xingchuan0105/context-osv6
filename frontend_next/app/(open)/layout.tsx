import type { ReactNode } from "react";

import { SoftwareApplicationJsonLd } from "../../components/software-application-jsonld";

/** Public pages (agent-readable docs) — no auth shell. */
export default function OpenLayout({ children }: { children: ReactNode }) {
  return (
    <>
      <SoftwareApplicationJsonLd />
      {children}
    </>
  );
}
