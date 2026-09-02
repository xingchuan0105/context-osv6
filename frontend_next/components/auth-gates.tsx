"use client";

import { useEffect, type ReactNode } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";

import { useAuth } from "../lib/auth/context";
import { formatUiMessage } from "../lib/i18n/messages";
import { useUiPreferences } from "../lib/ui-preferences";
import { CommandPaletteHost } from "./command-palette/command-palette";
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
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const { initialized, isAuthenticated } = useAuth();
  const { locale } = useUiPreferences();
  const query = searchParams.toString();
  const currentPath = `${pathname}${query ? `?${query}` : ""}`;
  const loginHref = `/login?next=${encodeURIComponent(currentPath)}`;

  useEffect(() => {
    if (initialized && !isAuthenticated) {
      router.replace(loginHref);
    }
  }, [initialized, isAuthenticated, loginHref, router]);

  if (!initialized) {
    return <FullscreenMessage message={formatUiMessage(locale, "gateCheckingSession")} />;
  }

  if (!isAuthenticated) {
    return <FullscreenMessage message={formatUiMessage(locale, "gateRedirectingLogin")} />;
  }

  return (
    <LegalReacceptanceGate>
      <CommandPaletteHost />
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
      router.replace("/chat");
    }
  }, [initialized, isAuthenticated, router]);

  if (isAuthenticated) {
    return <FullscreenMessage message={formatUiMessage(locale, "gateRedirectingChat")} />;
  }

  return <>{children}</>;
}
