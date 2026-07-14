import type { ReactNode } from "react";

/**
 * Marketing routes share a light discovery chrome via page wrappers.
 * Individual pages set `active` on MarketingShell where needed.
 */
export default function MarketingLayout({ children }: { children: ReactNode }) {
  return children;
}
