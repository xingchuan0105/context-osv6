"use client";

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";

import { UsageMeter } from "../../billing/UsageMeter";
import { ContextOsMark } from "../../context-os-mark";
import { useAuth } from "../../../lib/auth/context";
import { usageLimitToMeterProps } from "../../../lib/billing/usage-limit-adapter";
import { type DashboardLocale } from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";
import { getUsageLimit } from "../../../lib/settings/client";
import { useUiPreferences } from "../../../lib/ui-preferences";

function DashboardHeaderUsage() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const usageLimitQuery = useQuery({
    queryKey: ["dashboard", "usage-limit", token],
    enabled: Boolean(token),
    queryFn: () => getUsageLimit(token as string),
  });

  if (!token || usageLimitQuery.isLoading || !usageLimitQuery.data) {
    return null;
  }

  return (
    <div className="dashboard-header-usage">
      <UsageMeter
        {...usageLimitToMeterProps(usageLimitQuery.data, locale, { variant: "compact" })}
      />
    </div>
  );
}

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
      <DashboardHeaderUsage />
      <div className="dashboard-header-links">
        <Link
          aria-label={formatUiMessage(locale, "dashboardSettingsLink")}
          className="dashboard-header-settings"
          href="/settings?tab=appearance"
        >
          <svg aria-hidden="true" className="dashboard-header-icon" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" viewBox="0 0 24 24">
            <path d="M4 21V14M4 10V3M12 21V8M12 4V3M20 21v-9M20 8V3M1 14h6M9 8h6M17 18h6" />
          </svg>
          <span>{formatUiMessage(locale, "dashboardSettingsLink")}</span>
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