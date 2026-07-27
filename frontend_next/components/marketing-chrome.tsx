"use client";

import Link from "next/link";
import type { ReactNode } from "react";

import { ContextOsMark } from "./context-os-mark";
import { brandHomeHref } from "./product-chrome-footer";
import styles from "./marketing-chrome.module.css";
import { useAuth } from "../lib/auth/context";
import { APP_PATHS } from "../lib/site-map";
import { formatUiMessage } from "../lib/i18n/messages";
import { useUiPreferences, type UiLocale } from "../lib/ui-preferences";

/**
 * Light top bar for marketing paths (/desktop, /pricing, /legal).
 * Brand lockup is always horizontal (mark + Context-OS).
 */
export function MarketingChrome({ active }: { active?: "desktop" | "pricing" | "legal" | "none" }) {
  const { locale, setLocale } = useUiPreferences();
  const { isAuthenticated } = useAuth();
  const hub = brandHomeHref();
  const hubExternal = /^https?:\/\//i.test(hub);

  const navLinkClass = (isActive: boolean) =>
    isActive ? `${styles.navLink} ${styles.navLinkActive}` : styles.navLink;

  const brandInner = (
    <>
      <ContextOsMark size={28} className="cos-mark--nav" />
      <span className="cos-brand-lockup__wordmark">Context-OS</span>
    </>
  );

  return (
    <header data-testid="marketing-chrome" className={styles.header}>
      <div className={styles.container}>
        {hubExternal ? (
          <a href={hub} className="cos-brand-lockup" rel="noopener noreferrer" data-testid="mkt-brand-lockup">
            {brandInner}
          </a>
        ) : (
          <Link href={hub} className="cos-brand-lockup" data-testid="mkt-brand-lockup">
            {brandInner}
          </Link>
        )}

        <nav
          aria-label={formatUiMessage(locale, "marketingChrome.navLabel")}
          className={styles.nav}
        >
          <Link href={APP_PATHS.pricing} className={navLinkClass(active === "pricing")} data-testid="mkt-nav-pricing">
            {formatUiMessage(locale, "productChrome.pricing")}
          </Link>
          <Link href={APP_PATHS.desktop} className={navLinkClass(active === "desktop")} data-testid="mkt-nav-desktop">
            {formatUiMessage(locale, "productChrome.client")}
          </Link>
          <Link href={APP_PATHS.legal} className={navLinkClass(active === "legal")} data-testid="mkt-nav-legal">
            {formatUiMessage(locale, "productChrome.legalCenter")}
          </Link>
          <span className={styles.langGroup} role="group" aria-label="Language">
            {(["zh-CN", "en"] as UiLocale[]).map((code) => (
              <button
                key={code}
                type="button"
                data-testid={`mkt-lang-${code}`}
                onClick={() => setLocale(code)}
                className={
                  locale === code
                    ? `${styles.langButton} ${styles.langButtonActive}`
                    : styles.langButton
                }
              >
                {code === "zh-CN" ? "中文" : "EN"}
              </button>
            ))}
          </span>
          {isAuthenticated ? null : (
            <Link
              href={`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.dashboard)}`}
              className={`app-button-secondary ${styles.navButton}`}
            >
              {formatUiMessage(locale, "marketingChrome.login")}
            </Link>
          )}
          <Link
            href={`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.dashboard)}`}
            className={`app-button-primary ${styles.navButton}`}
            data-testid="mkt-nav-enter-app"
          >
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
    <div className={styles.shell}>
      <MarketingChrome active={active} />
      <div className={styles.shellContent}>{children}</div>
    </div>
  );
}
