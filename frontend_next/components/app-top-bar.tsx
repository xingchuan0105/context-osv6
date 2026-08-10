"use client";

import Link from "next/link";

import { ContextOsMark } from "./context-os-mark";
import { AccountMenu } from "./account-menu";
import { NotificationBell } from "./notifications/notification-bell";
import { ShareAccessMenu } from "./share-access-menu";
import { brandHomeHref } from "./product-chrome-footer";
import { appNavHref } from "../lib/navigation/nav-config";
import { type DashboardLocale } from "../lib/dashboard/model";
import { formatUiMessage } from "../lib/i18n/messages";

/**
 * Global App top bar (PRODUCT_IA §5): brand · 分享组(访问/API/升级) · 通知 ·
 * 账户. No 工作台|设置 wayfinding (brand returns to dashboard, settings lives
 * in the account menu); no 客户端/升级 capsules. Rendered by the dashboard and
 * every deep tool page (analytics / share center / api-access / usage / help)
 * — deep pages must never ship a bare back link as their only way out.
 */
export function AppTopBar({
  locale,
  onOpenGuide,
}: {
  locale: DashboardLocale;
  /** Opens onboarding product map modal (not primary nav). */
  onOpenGuide?: () => void;
}) {
  const brandHref = brandHomeHref();
  const brandIsExternal = /^https?:\/\//i.test(brandHref);

  return (
    <header className="dashboard-header">
      <div className="dashboard-brand">
        {brandIsExternal ? (
          <a
            className="dashboard-brand-link"
            href={brandHref}
            rel="noopener noreferrer"
            target="_blank"
            title={formatUiMessage(locale, "productChrome.brandHome")}
          >
            <ContextOsMark size={36} className="dashboard-brand-mark" />
          </a>
        ) : (
          <Link
            className="dashboard-brand-link"
            href={brandHref}
            title={formatUiMessage(locale, "productChrome.brandHome")}
          >
            <ContextOsMark size={36} className="dashboard-brand-mark" />
          </Link>
        )}
        <div>
          <Link className="dashboard-brand-title" href={appNavHref("dashboard")}>
            Context-OS
          </Link>
          <div className="dashboard-brand-subtitle">{formatUiMessage(locale, "dashboardBrandSubtitle")}</div>
        </div>
      </div>
      <div className="dashboard-header-links">
        <ShareAccessMenu locale={locale} />
        {onOpenGuide ? (
          <button
            type="button"
            className="dashboard-header-settings top-bar-capsule"
            data-testid="dashboard-guide-entry"
            title={formatUiMessage(locale, "productGuide.openHint")}
            onClick={onOpenGuide}
          >
            {formatUiMessage(locale, "productGuide.open")}
          </button>
        ) : null}
        <NotificationBell locale={locale} />
        <AccountMenu locale={locale} />
      </div>
    </header>
  );
}
