"use client";

import Link from "next/link";

import { ContextOsMark } from "../../context-os-mark";
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
            <ContextOsMark className="dashboard-brand-mark" />
          </a>
        ) : (
          <Link
            className="dashboard-brand-link"
            href={brandHref}
            title={formatUiMessage(locale, "productChrome.brandHome")}
          >
            <ContextOsMark className="dashboard-brand-mark" />
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
        <Link
          aria-label={formatUiMessage(locale, "dashboardAccountLink")}
          className="dashboard-header-settings"
          href="/settings?tab=profile"
        >
          <svg aria-hidden="true" className="dashboard-header-icon" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" viewBox="0 0 24 24">
            <path d="M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z" />
            <path d="M4 20c0-3.3 3.6-6 8-6s8 2.7 8 6" strokeLinecap="round" />
          </svg>
          <span>{formatUiMessage(locale, "dashboardAccountLink")}</span>
        </Link>
      </div>
    </header>
  );
}