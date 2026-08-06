"use client";

import Link from "next/link";

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
}: {
  /** Reserved for optional avatar badge; product keeps account text only. */
  avatarInitial: string;
  locale: DashboardLocale;
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
        <PlanEntry locale={locale} />
        <NotificationBell locale={locale} />
        <AccountMenu locale={locale} />
      </div>
    </header>
  );
}
