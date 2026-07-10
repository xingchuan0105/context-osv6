"use client";

import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  createPortalSession,
  getSubscription,
  getUsage,
  listPlans,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  formatCompactNumber,
  formatDate,
  progressBarStyle,
  progressTrackStyle,
  settingsKeys,
  subscriptionStatusLabel,
} from "./settings-shared";
import { UsageLimitPanel } from "./settings-usage-limit-panel";

export function BillingPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const [actionError, setActionError] = useState("");

  const billingQuery = useQuery({
    queryKey: settingsKeys.billing(token),
    enabled: Boolean(token),
    queryFn: async () => {
      const [subscriptionResult, usageResult, plansResult] = await Promise.allSettled([
        getSubscription(token as string),
        getUsage(token as string),
        listPlans(token as string),
      ]);

      const failedItems: string[] = [];

      if (subscriptionResult.status === "rejected") {
        failedItems.push(
          formatUiMessage(locale, "settings.billing.failedItem.subscription"),
        );
      }

      if (usageResult.status === "rejected") {
        failedItems.push(formatUiMessage(locale, "settings.billing.failedItem.usage"));
      }

      if (plansResult.status === "rejected") {
        failedItems.push(formatUiMessage(locale, "settings.billing.failedItem.plans"));
      }

      return {
        subscription:
          subscriptionResult.status === "fulfilled" ? subscriptionResult.value : null,
        usage: usageResult.status === "fulfilled" ? usageResult.value : null,
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
  const portalMutation = useMutation({
    mutationFn: async () => {
      if (!token) {
        throw new Error(formatUiMessage(locale, "settings.profile.notAuthenticated"));
      }

      const response = await createPortalSession(token);

      if (!response.url.trim()) {
        throw new Error(formatUiMessage(locale, "settings.billing.portalEmpty"));
      }

      return response;
    },
    onSuccess: (response) => {
      window.location.assign(response.url);
    },
  });

  const currentPlan = billingQuery.data?.subscription
    ? billingQuery.data.plans.find(
        (plan) => plan.id === billingQuery.data?.subscription?.plan_id,
      ) ?? null
    : null;

  async function handleManagePlan() {
    setActionError("");

    try {
      await portalMutation.mutateAsync();
    } catch (error) {
      const raw =
        error instanceof Error
          ? error.message
          : describeAuthError(formatUiMessage(locale, "settings.saveError"), error);
      const lower = raw.toLowerCase();
      const portalUnavailable =
        lower.includes("portal is unavailable") ||
        lower.includes("self-service billing") ||
        lower.includes("creem") ||
        lower.includes("not available");
      setActionError(
        portalUnavailable
          ? formatUiMessage(locale, "settings.billing.portalUnavailable")
          : raw,
      );
    }
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
            <a className="app-button-primary" href="/upgrade" style={{ textDecoration: "none" }}>
              {formatUiMessage(locale, "settings.billing.changePlanAction")}
            </a>
            <button
              className="app-button-secondary"
              disabled={portalMutation.isPending || !token}
              type="button"
              onClick={() => void handleManagePlan()}
            >
              {portalMutation.isPending
                ? formatUiMessage(locale, "settings.billing.loadingPortal")
                : formatUiMessage(locale, "settings.billing.managePlanAction")}
            </button>
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

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <h3 style={{ margin: 0 }}>
          {formatUiMessage(locale, "settings.billing.usageTitle")}
        </h3>
        {!billingQuery.data?.usage ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {billingQuery.isLoading
              ? formatUiMessage(locale, "settings.billing.loadingUsage")
              : formatUiMessage(locale, "settings.billing.noUsageData")}
          </p>
        ) : (
          <>
            {[
              {
                label: formatUiMessage(locale, "settings.billing.tokensLabel"),
                used: billingQuery.data.usage.used_tokens,
                limit: billingQuery.data.usage.limit_tokens,
              },
              {
                label: formatUiMessage(locale, "settings.billing.documentsLabel"),
                used: billingQuery.data.usage.used_documents,
                limit: billingQuery.data.usage.limit_documents,
              },
            ].map(({ label, used, limit }) => {
              const percent =
                typeof limit === "number" && limit > 0 ? Math.min(100, (used / limit) * 100) : 0;

              return (
                <div key={label} style={{ display: "grid", gap: "0.45rem" }}>
                  <div className="app-inline-row" style={{ marginBottom: 0 }}>
                    <span>{label}</span>
                    <strong>
                      {formatCompactNumber(used)}
                      {" / "}
                      {limit > 0
                        ? formatCompactNumber(limit)
                        : formatUiMessage(locale, "settings.usage.notSet")}
                    </strong>
                  </div>
                  <div style={progressTrackStyle()}>
                    <div style={progressBarStyle(percent)} />
                  </div>
                </div>
              );
            })}
          </>
        )}
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
          <div style={{ display: "grid", gap: "0.35rem" }}>
            <h3 style={{ margin: 0 }}>
              {formatUiMessage(locale, "settings.billing.availablePlansTitle")}
            </h3>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))", fontSize: "0.9rem" }}>
              {locale === "zh-CN"
                ? "升级与更换方案在独立页面完成，避免与账单状态混在同一折叠区。"
                : "Upgrade and plan changes open on a dedicated page."}
            </p>
          </div>
          <a className="app-button-primary" href="/upgrade" style={{ textDecoration: "none" }}>
            {formatUiMessage(locale, "settings.billing.changePlanAction")}
          </a>
        </div>
        {billingQuery.data?.plans && billingQuery.data.plans.length > 0 ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))", fontSize: "0.9rem" }}>
            {locale === "zh-CN"
              ? `当前可选 ${billingQuery.data.plans.length} 个方案（Free / 付费等），点击「更换方案」查看详情。`
              : `${billingQuery.data.plans.length} plans available — open Change plan for details.`}
          </p>
        ) : null}
      </section>
    </section>
  );
}

