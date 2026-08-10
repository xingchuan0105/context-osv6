"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";

import { MarketingShell } from "@/components/marketing-chrome";
import { APP_PATHS } from "@/lib/site-map";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

/**
 * ADR-0010: desktop buyout retired. Legacy /desktop/buy → free download + cloud pricing.
 */
export default function DesktopBuyPage() {
  const router = useRouter();
  const { locale } = useUiPreferences();

  useEffect(() => {
    router.replace(APP_PATHS.desktop);
  }, [router]);

  return (
    <MarketingShell active="desktop">
      <main className="app-page-shell" style={{ background: "hsl(var(--surface-muted))" }}>
        <div className="app-page-center" style={{ maxWidth: "32rem", padding: "2rem 1rem" }}>
          <h1 className="app-page-title">{formatUiMessage(locale, "desktop.buyTitle")}</h1>
          <p className="app-page-subtitle">{formatUiMessage(locale, "desktop.buyFreeBanner")}</p>
          <div className="app-button-row" style={{ marginTop: "1.25rem" }}>
            <Link href={APP_PATHS.desktop} className="app-button-primary">
              {formatUiMessage(locale, "desktop.buyFreeCta")}
            </Link>
            <Link href={APP_PATHS.pricing} className="app-button-secondary">
              {formatUiMessage(locale, "desktop.buyPricingCta")}
            </Link>
          </div>
        </div>
      </main>
    </MarketingShell>
  );
}
