"use client";

import { useEffect, useState, type ReactNode } from "react";
import { useRouter, usePathname } from "next/navigation";

import { isTauri } from "@/lib/runtime/tauri-ipc";
import {
  getLicenseStatus,
  type LicenseStatus,
  type LicenseStatusKind,
} from "@/lib/desktop/tauri-license";

const OPEN_KINDS: LicenseStatusKind[] = ["trial", "active", "offline_grace"];

function allowsWorkspace(kind: LicenseStatusKind): boolean {
  return OPEN_KINDS.includes(kind);
}

/**
 * Desktop client gate: license-only, never cloud login.
 * Unactivated / expired / revoked → welcome (/activate).
 * Trial / active → allow app routes.
 */
export function ClientLicenseGate({ children }: { children: ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const [ready, setReady] = useState(false);
  const [status, setStatus] = useState<LicenseStatus | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setReady(true);
      return;
    }

    let cancelled = false;
    void getLicenseStatus()
      .then((s) => {
        if (!cancelled) {
          setStatus(s);
          setReady(true);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setStatus({
            kind: "unactivated",
            dev_mode: false,
          });
          setReady(true);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [pathname]);

  useEffect(() => {
    if (!isTauri() || !ready || !status) return;

    const onWelcome =
      pathname === "/activate" ||
      pathname === "/setup" ||
      pathname?.startsWith("/desktop");

    if (!allowsWorkspace(status.kind) && !onWelcome) {
      router.replace("/activate");
      return;
    }

    // Do not auto-bounce licensed users off /activate or /setup — they may open settings/welcome intentionally.
  }, [ready, status, pathname, router]);

  if (isTauri() && !ready) {
    return (
      <main className="app-auth-shell">
        <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>正在检查客户端许可…</p>
        </section>
      </main>
    );
  }

  return <>{children}</>;
}

export function licenseAllowsWorkspace(kind: LicenseStatusKind): boolean {
  return allowsWorkspace(kind);
}
