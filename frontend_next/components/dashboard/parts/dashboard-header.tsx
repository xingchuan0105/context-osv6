"use client";

import Link from "next/link";

import { ContextOsMark } from "../../context-os-mark";
import { type DashboardLocale } from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";

export function DashboardHeader({
  avatarInitial,
  locale,
}: {
  avatarInitial: string;
  locale: DashboardLocale;
}) {
  return (
    <header className="dashboard-header">
      <div className="dashboard-brand">
        <ContextOsMark className="dashboard-brand-mark" />
        <div>
          <div className="dashboard-brand-title">Context OS</div>
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
        <Link
          aria-label={formatUiMessage(locale, "dashboardProfileLink")}
          className="dashboard-avatar-link"
          href="/settings?tab=profile"
        >
          {avatarInitial}
        </Link>
      </div>
    </header>
  );
}