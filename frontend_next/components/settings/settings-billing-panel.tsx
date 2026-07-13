"use client";

import Link from "next/link";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  getSubscription,
  listPlans,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  formatDate,
  settingsKeys,
  subscriptionStatusLabel,
} from "./settings-shared";
import { UsageLimitPanel } from "./settings-usage-limit-panel";

export function BillingPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const [actionError, setActionError] = useState("");
  /** In-app plan actions (no external Stripe portal — Creem/Alipay only). */
  const [showPlanPicker, setShowPlanPicker] = useState(false);

  const billingQuery = useQuery({
    queryKey: settingsKeys.billing(token),
    enabled: Boolean(token),
    queryFn: async () => {
      // Product metering truth is UsageLimitPanel (5h/7d usage_units). Do not load
      // legacy monthly token/document counters from /billing/usage (confusing dual numbers).
      const [subscriptionResult, plansResult] = await Promise.allSettled([
        getSubscription(token as string),
        listPlans(token as string),
      ]);

      const failedItems: string[] = [];

      if (subscriptionResult.status === "rejected") {
        failedItems.push(
          formatUiMessage(locale, "settings.billing.failedItem.subscription"),
        );
      }

      if (plansResult.status === "rejected") {
        failedItems.push(formatUiMessage(locale, "settings.billing.failedItem.plans"));
      }

      return {
        subscription:
          subscriptionResult.status === "fulfilled" ? subscriptionResult.value : null,
        plans: plansResult.status === "fulfilled" ? plansResult.value.plans : [],
        partialError:
          failedItems.length > 0
            ? formatUiMessage(locale, "settings.billing.failedData", {
                items: failedItems.join(", "),
              })
            : "",
      };
    },
  });
  const currentPlan = billingQuery.data?.subscription
    ? billingQuery.data.plans.find(
        (plan) => plan.id === billingQuery.data?.subscription?.plan_id,
      ) ?? null
    : null;

  function handleManagePlan() {
    // Stripe Customer Portal removed with Stripe payment stack (2026-07-13).
    // Product path: expand in-app plan list + link to /pricing (Creem/Alipay checkout).
    setActionError("");
    setShowPlanPicker(true);
  }

  const errorMessage =
    actionError ||
    (billingQuery.error
      ? describeAuthError(
          formatUiMessage(locale, "settings.loadError"),
          billingQuery.error,
        )
      : billingQuery.data?.partialError ?? "");

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
          <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem" }}>
            <button
              className="app-button-primary"
              data-testid="settings-manage-subscription"
              disabled={!token}
              type="button"
              onClick={() => handleManagePlan()}
            >
              {formatUiMessage(locale, "settings.billing.managePlanAction")}
            </button>
            <Link
              className="app-button-secondary"
              data-testid="settings-change-plan"
              href="/pricing"
            >
              {formatUiMessage(locale, "settings.billing.changePlanAction")}
            </Link>
          </div>
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
                {currentPlan?.name ??
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

      {showPlanPicker || (billingQuery.data?.plans?.length ?? 0) > 0 ? (
        <section
          className="app-inline-surface"
          data-testid="settings-plan-picker"
          style={{ display: "grid", gap: "0.8rem" }}
        >
          <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
            <div style={{ display: "grid", gap: "0.25rem" }}>
              <h3 style={{ margin: 0 }}>
                {formatUiMessage(locale, "settings.billing.availablePlansTitle")}
              </h3>
              <p style={{ margin: 0, color: "hsl(var(--muted-foreground))", fontSize: "0.9rem" }}>
                {formatUiMessage(locale, "settings.billing.planPickerHint")}
              </p>
            </div>
            <Link className="app-link" href="/pricing">
              {formatUiMessage(locale, "settings.billing.openPricingPage")}
            </Link>
          </div>
          {(billingQuery.data?.plans ?? []).length === 0 ? (
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatUiMessage(locale, "settings.billing.noPlans")}
            </p>
          ) : (
            <ul style={{ display: "grid", gap: "0.55rem", listStyle: "none", margin: 0, padding: 0 }}>
              {(billingQuery.data?.plans ?? []).map((plan) => {
                const isCurrent = plan.id === billingQuery.data?.subscription?.plan_id;
                return (
                  <li
                    key={plan.id}
                    className="app-inline-surface"
                    style={{
                      display: "flex",
                      flexWrap: "wrap",
                      gap: "0.75rem",
                      alignItems: "center",
                      justifyContent: "space-between",
                    }}
                  >
                    <div style={{ display: "grid", gap: "0.15rem" }}>
                      <strong>
                        {plan.name}
                        {isCurrent
                          ? ` · ${formatUiMessage(locale, "settings.billing.currentPlanBadge")}`
                          : ""}
                      </strong>
                      {plan.features.length > 0 ? (
                        <span style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.9rem" }}>
                          {plan.features.slice(0, 3).join(" · ")}
                        </span>
                      ) : null}
                    </div>
                    {!isCurrent ? (
                      <Link
                        className="app-button-secondary"
                        href={`/pricing?plan=${encodeURIComponent(plan.id)}`}
                      >
                        {formatUiMessage(locale, "settings.billing.changePlanAction")}
                      </Link>
                    ) : null}
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      ) : null}

    </section>
  );
}

