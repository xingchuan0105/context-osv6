"use client";

import Link from "next/link";

import { AppPrimaryNav } from "../../app-primary-nav";
import { ContextOsMark } from "../../context-os-mark";
import { AccountMenu } from "../../account-menu";
import { NotificationBell } from "../../notifications/notification-bell";
import { PlanEntry } from "../../plan-entry";
import { brandHomeHref } from "../../product-chrome-footer";
import { type DashboardLocale } from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";

export function DashboardHeader({
  avatarInitial: _avatarInitial,
  locale,
  onOpenGuide,
}: {
  /** Reserved for optional avatar badge; product keeps account text only. */
  avatarInitial: string;
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
          <Link className="dashboard-brand-title" href="/dashboard">
            Context-OS
          </Link>
          <div className="dashboard-brand-subtitle">{formatUiMessage(locale, "dashboardBrandSubtitle")}</div>
        </div>
      </div>
      <div className="dashboard-header-links">
        <AppPrimaryNav locale={locale} />
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
        <Link
          className="dashboard-header-client top-bar-capsule"
          data-testid="dashboard-client-entry"
          href="/desktop"
        >
          {formatUiMessage(locale, "productChrome.client")}
        </Link>
        <PlanEntry locale={locale} />
        <NotificationBell locale={locale} />
        <AccountMenu locale={locale} />
      </div>
    </header>
  );
}
