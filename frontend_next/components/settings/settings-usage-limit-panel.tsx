"use client";

import { useQuery } from "@tanstack/react-query";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { getUsageLimit } from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { UsageMeter } from "../billing/UsageMeter";
import { usageLimitToMeterProps } from "../../lib/billing/usage-limit-adapter";
import { formatCompactNumber, metricLabel, settingsKeys, bannerStyle } from "./settings-shared";

export function UsageLimitPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const usageLimitQuery = useQuery({
    queryKey: settingsKeys.usageLimit(token),
    enabled: Boolean(token),
    queryFn: () => getUsageLimit(token as string),
  });

  const breakdown = usageLimitQuery.data
    ? Object.entries(usageLimitQuery.data.breakdown).sort(([left], [right]) =>
        left.localeCompare(right),
      )
    : [];
  const scopeLabel = usageLimitQuery.data
    ? "plan_default" in usageLimitQuery.data.scope
      ? formatUiMessage(locale, "settings.usage.quotaScopePlanDefault", {
          planId: usageLimitQuery.data.scope.plan_default.plan_id,
        })
      : formatUiMessage(locale, "settings.usage.quotaScopeUserOverride")
    : "";
  const usageError = usageLimitQuery.error
    ? describeAuthError(
        formatUiMessage(locale, "settings.loadError"),
        usageLimitQuery.error,
      )
    : "";

  return (
    <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0 }}>
          {formatUiMessage(locale, "settings.usage.sectionTitle")}
        </h2>
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
          {formatUiMessage(locale, "settings.usage.sectionSubtitle")}
        </p>
      </div>
      {usageLimitQuery.isLoading ? (
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
          {formatUiMessage(locale, "settings.usage.loading")}
        </p>
      ) : usageError ? (
        <p className="app-notice-banner">{usageError}</p>
      ) : !usageLimitQuery.data ? (
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
          {formatUiMessage(locale, "settings.usage.empty")}
        </p>
      ) : (
        <>
          <div className="app-inline-surface" style={{ display: "grid", gap: "0.5rem" }}>
            <div className="app-inline-row">
              <span>{formatUiMessage(locale, "settings.usage.scopeLabel")}</span>
              <strong>{scopeLabel}</strong>
            </div>
            <div className="app-inline-row">
              <span>{formatUiMessage(locale, "settings.usage.policyLabel")}</span>
              <strong>
                {usageLimitQuery.data.policy.enabled
                  ? formatUiMessage(locale, "settings.usage.policyEnabled")
                  : formatUiMessage(locale, "settings.usage.policyDisabled")}
              </strong>
            </div>
            {usageLimitQuery.data.has_estimated_usage ? (
              <p className="app-notice-banner" style={bannerStyle("info")}>
                {formatUiMessage(locale, "settings.usage.estimated")}
              </p>
            ) : null}
          </div>
          <UsageMeter {...usageLimitToMeterProps(usageLimitQuery.data, locale)} />
          {breakdown.length > 0 ? (
            <div className="app-inline-surface" style={{ display: "grid", gap: "0.5rem" }}>
              <strong>{formatUiMessage(locale, "settings.usage.breakdownTitle")}</strong>
              {breakdown.map(([metric, value]) => (
                <div className="app-inline-row" key={metric} style={{ marginBottom: 0 }}>
                  <span>{metricLabel(locale, metric)}</span>
                  <strong>{formatCompactNumber(value)}</strong>
                </div>
              ))}
            </div>
          ) : null}
        </>
      )}
    </section>
  );
}

