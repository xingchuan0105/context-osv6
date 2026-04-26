import type { ReactNode } from "react";

import { ProtectedRouteGate } from "../../components/auth-gates";
import { AdminShell } from "../../components/admin/admin-shell";

export default function AdminLayout({ children }: { children: ReactNode }) {
  return (
    <ProtectedRouteGate>
      <AdminShell>{children}</AdminShell>
    </ProtectedRouteGate>
  );
}
