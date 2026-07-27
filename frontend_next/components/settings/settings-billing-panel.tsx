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
import styles from "./settings-billing-panel.module.css";
import shared from "./settings-ui-shared.module.css";

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

export function BillingPanel({ hideManagePlan = false }: { hideManagePlan?: boolean } = {}) {
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
    <section className={shared.section}>
      <UsageLimitPanel />
      <section className={`app-inline-surface ${styles.planSection}`}>
        <div className={`app-inline-row ${styles.headerRow}`}>
          <div className={shared.headerText}>
            <h2 className={shared.flushTitle}>
              {formatUiMessage(locale, "settings.billing.sectionTitle")}
            </h2>
            <p className={shared.mutedText}>
              {formatUiMessage(locale, "settings.billing.sectionSubtitle")}
            </p>
          </div>
          {hideManagePlan ? null : (
            <Link
              className="app-button-primary app-button-accent"
              data-testid="settings-manage-subscription"
              href="/pricing"
            >
              {formatUiMessage(locale, "settings.billing.managePlanAction")}
            </Link>
          )}
        </div>
        {errorMessage ? <p className="app-notice-banner">{errorMessage}</p> : null}
        {billingQuery.isLoading ? (
          <p className={shared.mutedText}>
            {formatUiMessage(locale, "settings.billing.loading")}
          </p>
        ) : (
          <div
            className={`app-inline-surface ${styles.planCard}`}
            data-testid="plan-display"
          >
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>
                {formatUiMessage(locale, "settings.billing.currentPlanLabel")}
              </span>
              <strong>
                {currentPlanName ??
                  formatUiMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>{formatUiMessage(locale, "settings.billing.statusLabel")}</span>
              <strong>
                {billingQuery.data?.subscription
                  ? subscriptionStatusLabel(locale, billingQuery.data.subscription.status)
                  : formatUiMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
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
