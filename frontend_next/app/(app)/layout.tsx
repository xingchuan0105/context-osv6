import type { ReactNode } from "react";

import { ProtectedRouteGate } from "../../components/auth-gates";

export default function AppLayout({ children }: { children: ReactNode }) {
  return <ProtectedRouteGate>{children}</ProtectedRouteGate>;
}
