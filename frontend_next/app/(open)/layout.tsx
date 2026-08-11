import type { ReactNode } from "react";

/** Public pages (agent-readable docs) — no auth shell. */
export default function OpenLayout({ children }: { children: ReactNode }) {
  return children;
}
