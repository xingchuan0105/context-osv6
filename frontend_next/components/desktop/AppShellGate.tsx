"use client";

import { useEffect, useState, type ReactNode } from "react";

import { ProtectedRouteGate } from "@/components/auth-gates";
import { ClientLicenseGate } from "@/components/desktop/ClientLicenseGate";
import { isTauri } from "@/lib/runtime/tauri-ipc";

/**
 * Web SaaS: cloud session required.
 * Desktop client: license only — never redirect to /login.
 */
export function AppShellGate({ children }: { children: ReactNode }) {
  const [mode, setMode] = useState<"unknown" | "web" | "desktop">("unknown");

  useEffect(() => {
    setMode(isTauri() ? "desktop" : "web");
  }, []);

  if (mode === "unknown") {
    return (
      <main className="app-auth-shell">
        <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>加载中…</p>
        </section>
      </main>
    );
  }

  if (mode === "desktop") {
    return <ClientLicenseGate>{children}</ClientLicenseGate>;
  }

  return <ProtectedRouteGate>{children}</ProtectedRouteGate>;
}
