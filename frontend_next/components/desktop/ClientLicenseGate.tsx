"use client";

import { useEffect, useState, type ReactNode } from "react";

import { isTauri } from "@/lib/runtime/tauri-ipc";
import { getLicenseStatus, type LicenseStatusKind } from "@/lib/desktop/tauri-license";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

/**
 * ADR-0010: desktop client is free — activation is not a product gate.
 * License status may still be queried for legacy UI badges, but we never
 * redirect unactivated users to /activate.
 */
export function ClientLicenseGate({ children }: { children: ReactNode }) {
  const { locale } = useUiPreferences();
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (!isTauri()) {
      setReady(true);
      return;
    }
    // Optional status fetch for side effects only — never block navigation.
    let cancelled = false;
    void getLicenseStatus()
      .catch(() => undefined)
      .finally(() => {
        if (!cancelled) setReady(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (isTauri() && !ready) {
    return (
      <main className="app-auth-shell">
        <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "desktop.startingClient")}
          </p>
        </section>
      </main>
    );
  }

  return <>{children}</>;
}

/** @deprecated ADR-0010 free client — always true. */
export function licenseAllowsWorkspace(_kind: LicenseStatusKind): boolean {
  return true;
}
