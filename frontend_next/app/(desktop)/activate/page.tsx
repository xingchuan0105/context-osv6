"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

import { APP_PATHS } from "@/lib/site-map";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

/**
 * ADR-0010 / v0.2.0: client is free — activation is not a product gate.
 * Legacy deep links and bookmarks land here; send users to download or app home.
 */
export default function ActivatePage() {
  const router = useRouter();
  const { locale } = useUiPreferences();

  useEffect(() => {
    router.replace(APP_PATHS.desktop);
  }, [router]);

  return (
    <main className="app-auth-shell">
      <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
          {formatUiMessage(locale, "desktop.activateRedirect")}
        </p>
      </section>
    </main>
  );
}
