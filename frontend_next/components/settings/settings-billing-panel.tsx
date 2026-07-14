"use client";

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { getSubscription } from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  formatDate,
  settingsKeys,
  subscriptionStatusLabel,
} from "./settings-shared";
import { UsageLimitPanel } from "./settings-usage-limit-panel";

function planLabel(planId: string | null | undefined): string | null {
  if (!planId) {
    return null;
  }
  const known: Record<string, string> = {
    free: "Free",
    plus: "Plus",
    pro: "Pro",
  };
  return known[planId.toLowerCase()] ?? planId;
}

export function BillingPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();

  const billingQuery = useQuery({
    queryKey: settingsKeys.billing(token),
    enabled: Boolean(token),
    queryFn: async () => {
      // Product metering truth is UsageLimitPanel (5h/7d). Plan catalog lives on /pricing only.
      try {
        const subscription = await getSubscription(token as string);
        return { subscription, partialError: "" };
      } catch (error) {
        return {
          subscription: null,
          partialError: formatUiMessage(locale, "settings.billing.failedData", {
            items: formatUiMessage(locale, "settings.billing.failedItem.subscription"),
          }),
        };
      }
    },
  });

  const errorMessage = billingQuery.error
    ? describeAuthError(
        formatUiMessage(locale, "settings.loadError"),
        billingQuery.error,
      )
    : (billingQuery.data?.partialError ?? "");

  const currentPlanName = planLabel(billingQuery.data?.subscription?.plan_id);

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <UsageLimitPanel />
      <section className="app-inline-surface" style={{ display: "grid", gap: "0.75rem" }}>
        <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
          <div style={{ display: "grid", gap: "0.35rem" }}>
            <h2 style={{ margin: 0 }}>
              {formatUiMessage(locale, "settings.billing.sectionTitle")}
            </h2>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatUiMessage(locale, "settings.billing.sectionSubtitle")}
            </p>
          </div>
          <Link
            className="app-button-primary"
            data-testid="settings-manage-subscription"
            href="/pricing"
          >
            {formatUiMessage(locale, "settings.billing.managePlanAction")}
          </Link>
        </div>
        {errorMessage ? <p className="app-notice-banner">{errorMessage}</p> : null}
        {billingQuery.isLoading ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "settings.billing.loading")}
          </p>
        ) : (
          <div
            className="app-inline-surface"
            data-testid="plan-display"
            style={{ display: "grid", gap: "0.5rem" }}
          >
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>
                {formatUiMessage(locale, "settings.billing.currentPlanLabel")}
              </span>
              <strong>
                {currentPlanName ??
                  formatUiMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{formatUiMessage(locale, "settings.billing.statusLabel")}</span>
              <strong>
                {billingQuery.data?.subscription
                  ? subscriptionStatusLabel(locale, billingQuery.data.subscription.status)
                  : formatUiMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{formatUiMessage(locale, "settings.billing.renewsOnLabel")}</span>
              <strong>
                {formatDate(
                  billingQuery.data?.subscription?.current_period_end ?? null,
                  locale,
                  formatUiMessage(locale, "settings.usage.notSet"),
                )}
              </strong>
            </div>
          </div>
        )}
      </section>
    </section>
  );
}
