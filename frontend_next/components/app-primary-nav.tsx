"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { formatUiMessage } from "../lib/i18n/messages";
import type { UiLocale } from "../lib/i18n/config";

type AppPrimaryNavProps = {
  locale: UiLocale;
  className?: string;
  /** Optional outer test id (e.g. settings header legacy id). */
  "data-testid"?: string;
};

/**
 * Minimal cross-surface wayfinding: 工作台 | 设置 (PRODUCT_IA P1-5 light).
 * Not an encyclopedia sidebar — only two product homes.
 */
export function AppPrimaryNav({
  locale,
  className,
  "data-testid": testId = "app-primary-nav",
}: AppPrimaryNavProps) {
  const pathname = usePathname() ?? "";
  const onSettings = pathname.startsWith("/settings");
  // Workbench home: list, analytics, and workspace surfaces under /dashboard/*
  const onDashboard = !onSettings && (pathname === "/dashboard" || pathname.startsWith("/dashboard/"));

  return (
    <nav
      className={className}
      aria-label={formatUiMessage(locale, "appPrimaryNav.label")}
      data-testid={testId}
    >
      <Link
        className={onDashboard && !onSettings ? "app-primary-nav-link is-active" : "app-primary-nav-link"}
        data-testid="app-nav-dashboard"
        href="/dashboard"
      >
        {formatUiMessage(locale, "productChrome.productHome")}
      </Link>
      <Link
        className={onSettings ? "app-primary-nav-link is-active" : "app-primary-nav-link"}
        data-testid="app-nav-settings"
        href="/settings"
      >
        {formatUiMessage(locale, "appPrimaryNav.settings")}
      </Link>
    </nav>
  );
}
