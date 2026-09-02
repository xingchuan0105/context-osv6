"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
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
 * `locale` 覆盖 SSR 语言（/en/* 页传 "en"）；语言切换按 URL 跳转（/x ↔ /en/x）。
 */
export function MarketingChrome({
  active,
  locale: localeProp,
}: {
  active?: "desktop" | "pricing" | "legal" | "none";
  locale?: UiLocale;
}) {
  const { locale: uiLocale } = useUiPreferences();
  const locale = localeProp ?? uiLocale;
  const { isAuthenticated } = useAuth();
  const hub = brandHomeHref();
  const hubExternal = /^https?:\/\//i.test(hub);
  const pathname = usePathname();
  const isEnPath = pathname === "/en" || pathname.startsWith("/en/");
  const zhHref = isEnPath ? (pathname === "/en" ? "/" : pathname.slice(3)) : pathname;
  const enHref = isEnPath ? pathname : pathname === "/" ? "/en" : `/en${pathname}`;

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
            <Link
              href={zhHref}
              data-testid="mkt-lang-zh-CN"
              className={
                locale === "zh-CN"
                  ? `${styles.langButton} ${styles.langButtonActive}`
                  : styles.langButton
              }
            >
              中文
            </Link>
            <Link
              href={enHref}
              data-testid="mkt-lang-en"
              className={
                locale === "en"
                  ? `${styles.langButton} ${styles.langButtonActive}`
                  : styles.langButton
              }
            >
              EN
            </Link>
          </span>
          {isAuthenticated ? null : (
            <Link
              href={`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.chat)}`}
              className={`app-button-secondary ${styles.navButton}`}
            >
              {formatUiMessage(locale, "marketingChrome.login")}
            </Link>
          )}
          <Link
            href={`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.chat)}`}
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
  locale,
}: {
  children: ReactNode;
  active?: "desktop" | "pricing" | "legal" | "none";
  locale?: UiLocale;
}) {
  return (
    <div className={styles.shell}>
      <MarketingChrome active={active} locale={locale} />
      <div className={styles.shellContent}>{children}</div>
    </div>
  );
}
