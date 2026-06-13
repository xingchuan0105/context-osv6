"use client";

import { useEffect, type ReactNode } from "react";
import { useRouter } from "next/navigation";

import { useAuth } from "../lib/auth/context";
import { formatUiMessage } from "../lib/i18n/messages";
import { useUiPreferences } from "../lib/ui-preferences";
import { LegalReacceptanceGate } from "./legal/LegalReacceptanceGate";

function FullscreenMessage({ message }: { message: string }) {
  return (
    <main className="app-auth-shell">
      <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>{message}</p>
      </section>
    </main>
  );
}

export function ProtectedRouteGate({ children }: { children: ReactNode }) {
  const router = useRouter();
  const { initialized, isAuthenticated } = useAuth();
  const { locale } = useUiPreferences();

  useEffect(() => {
    if (initialized && !isAuthenticated) {
      router.replace("/login");
    }
  }, [initialized, isAuthenticated, router]);

  if (!initialized) {
    return <FullscreenMessage message={formatUiMessage(locale, "gateCheckingSession")} />;
  }

  if (!isAuthenticated) {
    return <FullscreenMessage message={formatUiMessage(locale, "gateRedirectingLogin")} />;
  }

  return (
    <LegalReacceptanceGate>
      {children}
    </LegalReacceptanceGate>
  );
}

export function GuestOnlyGate({ children }: { children: ReactNode }) {
  const router = useRouter();
  const { initialized, isAuthenticated } = useAuth();
  const { locale } = useUiPreferences();

  useEffect(() => {
    if (initialized && isAuthenticated) {
      router.replace("/dashboard");
    }
  }, [initialized, isAuthenticated, router]);

  if (isAuthenticated) {
    return <FullscreenMessage message={formatUiMessage(locale, "gateRedirectingDashboard")} />;
  }

  return <>{children}</>;
}
