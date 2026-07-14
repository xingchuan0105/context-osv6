"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import { ContextOsMark } from "./context-os-mark";
import { brandHomeHref } from "./product-chrome-footer";
import { APP_PATHS } from "../lib/site-map";
import { formatUiMessage } from "../lib/i18n/messages";
import { useUiPreferences } from "../lib/ui-preferences";

/**
 * Light top bar for marketing paths (/desktop, /pricing, /legal).
 * Not used inside authenticated workspace chrome.
 */
export function MarketingChrome({ active }: { active?: "desktop" | "pricing" | "legal" | "none" }) {
  const { locale } = useUiPreferences();
  const hub = brandHomeHref();
  const hubExternal = /^https?:\/\//i.test(hub);

  const linkStyle = (isActive: boolean): React.CSSProperties => ({
    color: isActive ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
    fontWeight: isActive ? 600 : 500,
    fontSize: "0.9rem",
    textDecoration: "none",
  });

  return (
    <header
      data-testid="marketing-chrome"
      style={{
        position: "sticky",
        top: 0,
        zIndex: 40,
        height: "3.5rem",
        display: "flex",
        alignItems: "center",
        borderBottom: "1px solid hsl(var(--border))",
        background: "hsl(var(--background) / 0.92)",
        backdropFilter: "blur(10px)",
      }}
    >
      <div
        style={{
          width: "100%",
          maxWidth: "56rem",
          margin: "0 auto",
          padding: "0 1rem",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "1rem",
        }}
      >
        {hubExternal ? (
          <a
            href={hub}
            className="app-auth-brand-link"
            style={{ display: "inline-flex", alignItems: "center", gap: "0.5rem", textDecoration: "none" }}
            rel="noopener noreferrer"
          >
            <ContextOsMark className="app-auth-mark" />
            <span style={{ fontWeight: 600, color: "hsl(var(--foreground))" }}>Context-OS</span>
          </a>
        ) : (
          <Link
            href={hub}
            className="app-auth-brand-link"
            style={{ display: "inline-flex", alignItems: "center", gap: "0.5rem", textDecoration: "none" }}
          >
            <ContextOsMark className="app-auth-mark" />
            <span style={{ fontWeight: 600, color: "hsl(var(--foreground))" }}>Context-OS</span>
          </Link>
        )}

        <nav
          aria-label={formatUiMessage(locale, "marketingChrome.navLabel")}
          style={{ display: "flex", alignItems: "center", gap: "1rem", flexWrap: "wrap" }}
        >
          <Link href={APP_PATHS.pricing} style={linkStyle(active === "pricing")} data-testid="mkt-nav-pricing">
            {formatUiMessage(locale, "productChrome.pricing")}
          </Link>
          <Link href={APP_PATHS.desktop} style={linkStyle(active === "desktop")} data-testid="mkt-nav-desktop">
            {formatUiMessage(locale, "productChrome.desktop")}
          </Link>
          <Link href={APP_PATHS.legal} style={linkStyle(active === "legal")} data-testid="mkt-nav-legal">
            {formatUiMessage(locale, "productChrome.legalCenter")}
          </Link>
          <Link href={`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.dashboard)}`} className="app-button-secondary" style={{ fontSize: "0.85rem", padding: "0.35rem 0.75rem" }}>
            {formatUiMessage(locale, "marketingChrome.login")}
          </Link>
          <Link href={`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.dashboard)}`} className="app-button-primary" style={{ fontSize: "0.85rem", padding: "0.35rem 0.75rem" }} data-testid="mkt-nav-enter-app">
            {formatUiMessage(locale, "marketingChrome.enterApp")}
          </Link>
        </nav>
      </div>
    </header>
  );
}

export function MarketingShell({
  children,
  active = "none",
}: {
  children: ReactNode;
  active?: "desktop" | "pricing" | "legal" | "none";
}) {
  return (
    <div style={{ minHeight: "100vh", display: "flex", flexDirection: "column" }}>
      <MarketingChrome active={active} />
      <div style={{ flex: 1 }}>{children}</div>
    </div>
  );
}
