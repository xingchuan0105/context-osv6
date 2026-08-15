"use client";

import { useEffect, useState, type ReactNode } from "react";

import { ProtectedRouteGate } from "@/components/auth-gates";
import { CommandPaletteHost } from "@/components/command-palette/command-palette";
import { ClientLicenseGate } from "@/components/desktop/ClientLicenseGate";
import { ClientLocalSessionBootstrap } from "@/components/desktop/ClientLocalSessionBootstrap";
import { CloudLoginGate } from "@/components/desktop/CloudLoginGate";
import { isTauri } from "@/lib/runtime/tauri-ipc";

/**
 * Web SaaS: cloud session required.
 * Desktop client: license → cloud login (official models, 走余额) → local
 * B2C session against the on-machine API — never redirect to cloud /login.
 * Cloud login sits BEFORE the stack bootstrap: it needs no local stack, and
 * the bootstrap then comes up with relay credentials already in client.env.
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
    return (
      <ClientLicenseGate>
        <CloudLoginGate>
          <ClientLocalSessionBootstrap>
            <CommandPaletteHost />
            {children}
          </ClientLocalSessionBootstrap>
        </CloudLoginGate>
      </ClientLicenseGate>
    );
  }

  return <ProtectedRouteGate>{children}</ProtectedRouteGate>;
}
