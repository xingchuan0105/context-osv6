"use client";

import Link from "next/link";

import { formatUiMessage } from "../lib/i18n/messages";
import { useUiPreferences } from "../lib/ui-preferences";

/** Marketing / brand site; product app “home” remains `/dashboard`. */
export function brandHomeHref(): string {
  if (typeof process !== "undefined" && process.env.NEXT_PUBLIC_BRAND_HOME_URL?.trim()) {
    return process.env.NEXT_PUBLIC_BRAND_HOME_URL.trim();
  }
  // Public marketing site (see docs/engineering visual multi-site plan).
  return "https://www.contextlm.top";
}

/**
 * Persistent product chrome footer: brand, help/docs, legal, open-source.
 * Used on Dashboard / Settings (and other app shells) so legal & docs are always reachable.
 */
export function ProductChromeFooter({
  className,
  testId = "product-chrome-footer",
}: {
  className?: string;
  testId?: string;
}) {
  const { locale } = useUiPreferences();
  const year = new Date().getFullYear();
  const brandHref = brandHomeHref();
  const brandIsExternal = /^https?:\/\//i.test(brandHref);

  return (
    <footer
      className={className}
      data-testid={testId}
      style={{
        display: "grid",
        gap: "0.55rem",
        marginTop: "2rem",
        paddingTop: "1rem",
        borderTop: "1px solid hsl(var(--border) / 0.8)",
        color: "hsl(var(--muted-foreground))",
        fontSize: "0.85rem",
      }}
    >
      <nav
        aria-label={formatUiMessage(locale, "productChrome.footerNavLabel")}
        style={{ display: "flex", flexWrap: "wrap", gap: "0.35rem 0.75rem", alignItems: "center" }}
      >
        {brandIsExternal ? (
          <a className="app-link app-link-muted" href={brandHref} rel="noopener noreferrer" target="_blank">
            {formatUiMessage(locale, "productChrome.brandHome")}
          </a>
        ) : (
          <Link className="app-link app-link-muted" href={brandHref}>
            {formatUiMessage(locale, "productChrome.brandHome")}
          </Link>
        )}
        <span aria-hidden="true">·</span>
        <Link className="app-link app-link-muted" href="/dashboard">
          {formatUiMessage(locale, "productChrome.productHome")}
        </Link>
        <span aria-hidden="true">·</span>
        <Link className="app-link app-link-muted" href="/help">
          {formatUiMessage(locale, "productChrome.help")}
        </Link>
        <span aria-hidden="true">·</span>
        <Link className="app-link app-link-muted" href="/pricing">
          {formatUiMessage(locale, "productChrome.pricing")}
        </Link>
        <span aria-hidden="true">·</span>
        <Link className="app-link app-link-muted" href="/legal">
          {formatUiMessage(locale, "productChrome.legalCenter")}
        </Link>
        <span aria-hidden="true">·</span>
        <Link className="app-link app-link-muted" href="/legal/terms">
          {formatUiMessage(locale, "productChrome.terms")}
        </Link>
        <span aria-hidden="true">·</span>
        <Link className="app-link app-link-muted" href="/legal/privacy">
          {formatUiMessage(locale, "productChrome.privacy")}
        </Link>
        <span aria-hidden="true">·</span>
        <Link className="app-link app-link-muted" href="/legal/licenses">
          {formatUiMessage(locale, "productChrome.licenses")}
        </Link>
      </nav>
      <div>
        © {year} Context-OS
      </div>
    </footer>
  );
}
