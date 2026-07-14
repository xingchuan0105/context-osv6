import type { ReactNode } from "react";

import { AppShellGate } from "@/components/desktop/AppShellGate";

export default function AppLayout({ children }: { children: ReactNode }) {
  return <AppShellGate>{children}</AppShellGate>;
}
