"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState, type CSSProperties, type FormEvent } from "react";
import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import {
  useForm,
  type FieldValues,
  type Path,
  type UseFormSetError,
} from "react-hook-form";
import { z } from "zod";

import { AppPageFrame } from "../page-frame";
import { changePassword } from "../../lib/auth/client";
import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import {
  createPortalSession,
  createCheckoutSession,
  defaultNotificationPreferences,
  getSubscription,
  getUsage,
  getUsageLimit,
  getUserPreferences,
  listNotifications,
  listPlans,
  markNotificationRead,
  updateProfile,
  updateUserPreferences,
  type NotificationPreferences,
  type NotificationRow,
  type PlanRow,
  type UserPreferences,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SETTINGS_TABS, type SettingsTab } from "./settings-tabs";

type ProfileFormValues = {
  fullName: string;
};

type NotificationFormValues = {
  email_enabled: boolean;
  product_enabled: boolean;
  security_enabled: boolean;
  weekly_digest_enabled: boolean;
  quiet_hours_start: string;
  quiet_hours_end: string;
};

const TIME_24H_PATTERN = /^([01]\d|2[0-3]):[0-5]\d$/;

const settingsKeys = {
  billing: (token: string | null) => ["settings", "billing", token] as const,
  notifications: (token: string | null) => ["settings", "notifications", token] as const,
  preferences: (token: string | null) => ["settings", "preferences", token] as const,
  usageLimit: (token: string | null) => ["settings", "usage-limit", token] as const,
};

function applyZodErrors<TFieldValues extends FieldValues>(
  error: z.ZodError<TFieldValues>,
  setError: UseFormSetError<TFieldValues>,
) {
  for (const issue of error.issues) {
    const field = issue.path[0];

    if (typeof field === "string") {
      setError(field as Path<TFieldValues>, {
        type: "manual",
        message: issue.message,
      });
    }
  }
}

function bannerStyle(tone: "success" | "error" | "info"): CSSProperties {
  if (tone === "success") {
    return {
      border: "1px solid rgba(25, 135, 84, 0.24)",
      background: "rgba(25, 135, 84, 0.08)",
      color: "hsl(var(--success))",
    };
  }

  if (tone === "info") {
    return {
      border: "1px solid rgba(32, 124, 229, 0.18)",
      background: "rgba(32, 124, 229, 0.08)",
      color: "hsl(var(--info))",
    };
  }

  return {};
}

function panelChoiceStyle(selected: boolean): CSSProperties {
  return {
    display: "grid",
    gap: "0.45rem",
    width: "100%",
    padding: "1rem",
    borderRadius: "1rem",
    border: `1px solid ${selected ? "hsl(var(--primary))" : "hsl(var(--border))"}`,
    background: selected ? "hsl(var(--surface-muted))" : "hsl(var(--card))",
    color: "inherit",
    textAlign: "left",
  };
}

function progressTrackStyle(): CSSProperties {
  return {
    width: "100%",
    height: "0.5rem",
    borderRadius: "999px",
    background: "hsl(var(--muted))",
    overflow: "hidden",
  };
}

function progressBarStyle(percent: number): CSSProperties {
  return {
    width: `${Math.max(0, Math.min(100, percent))}%`,
    height: "100%",
    borderRadius: "999px",
    background:
      percent >= 90
        ? "hsl(var(--destructive))"
        : percent >= 70
          ? "hsl(var(--warning))"
          : "hsl(var(--success))",
  };
}

function formatDate(value: string | null, locale: "zh-CN" | "en", fallback: string) {
  if (!value?.trim()) {
    return fallback;
  }

  const timestamp = Date.parse(value);

  if (Number.isNaN(timestamp)) {
    return value;
  }

  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(new Date(timestamp));
}

function formatDateTime(value: string, locale: "zh-CN" | "en") {
  const timestamp = Date.parse(value);

  if (Number.isNaN(timestamp)) {
    return value;
  }

  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

function formatCompactNumber(value: number) {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }

  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}K`;
  }

  return value.toString();
}

function formatPrice(cents: number) {
  return `$${(cents / 100).toFixed(2)}`;
}

function metricLabel(locale: "zh-CN" | "en", metric: string) {
  const keyMap: Record<string, Parameters<typeof formatSettingsShareMessage>[1]> = {
    embedding_tokens: "settings.metric.embedding_tokens",
    llm_input_tokens: "settings.metric.llm_input_tokens",
    llm_output_tokens: "settings.metric.llm_output_tokens",
    pages_processed: "settings.metric.pages_processed",
    storage_bytes: "settings.metric.storage_bytes",
  };

  const key = keyMap[metric.trim()];
  return key ? formatSettingsShareMessage(locale, key) : metric;
}

function featureLabel(locale: "zh-CN" | "en", feature: string) {
  const [metric, value] = feature.split(":");

  if (!metric || !value) {
    return feature;
  }

  const normalizedValue =
    value.trim().toLowerCase() === "unlimited"
      ? formatSettingsShareMessage(locale, "commonUnlimited")
      : value.trim();

  return `${metricLabel(locale, metric)}: ${normalizedValue}`;
}

function notificationTypeLabel(locale: "zh-CN" | "en", eventType: string) {
  const keyMap: Record<string, Parameters<typeof formatSettingsShareMessage>[1]> = {
    product_update: "settings.notifications.event.product_update",
    security_alert: "settings.notifications.event.security_alert",
    weekly_digest: "settings.notifications.event.weekly_digest",
  };

  const key = keyMap[eventType];
  return key ? formatSettingsShareMessage(locale, key) : eventType;
}

function subscriptionStatusLabel(locale: "zh-CN" | "en", status: string) {
  const keyMap: Record<string, Parameters<typeof formatSettingsShareMessage>[1]> = {
    active: "settings.billing.status.active",
    past_due: "settings.billing.status.past_due",
    canceled: "settings.billing.status.canceled",
  };

  const key = keyMap[status];
  return key ? formatSettingsShareMessage(locale, key) : status;
}

function notificationFormDefaults(
  preferences: NotificationPreferences = defaultNotificationPreferences(),
): NotificationFormValues {
  return {
    email_enabled: preferences.email_enabled,
    product_enabled: preferences.product_enabled,
    security_enabled: preferences.security_enabled,
    weekly_digest_enabled: preferences.weekly_digest_enabled,
    quiet_hours_start: preferences.quiet_hours_start ?? "",
    quiet_hours_end: preferences.quiet_hours_end ?? "",
  };
}

function SettingsTabBar({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();
  const tabKeyMap: Record<SettingsTab, Parameters<typeof formatSettingsShareMessage>[1]> = {
    billing: "settings.tabs.billing",
    profile: "settings.tabs.profile",
    appearance: "settings.tabs.appearance",
    security: "settings.tabs.security",
    notifications: "settings.tabs.notifications",
  };

  return (
    <nav
      aria-label={formatSettingsShareMessage(locale, "settings.tabsLabel")}
      className="app-tab-bar"
    >
      {SETTINGS_TABS.map((tab) => (
        <Link
          aria-current={tab === activeTab ? "page" : undefined}
          className={`app-tab-button${tab === activeTab ? " app-tab-button-active" : ""}`}
          href={`/settings?tab=${tab}`}
          key={tab}
        >
          {formatSettingsShareMessage(locale, tabKeyMap[tab])}
        </Link>
      ))}
    </nav>
  );
}

function UsageLimitPanel() {
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
      ? formatSettingsShareMessage(locale, "settings.usage.quotaScopePlanDefault", {
          planId: usageLimitQuery.data.scope.plan_default.plan_id,
        })
      : formatSettingsShareMessage(locale, "settings.usage.quotaScopeUserOverride")
    : "";
  const usageError = usageLimitQuery.error
    ? describeAuthError(
        formatSettingsShareMessage(locale, "settings.loadError"),
        usageLimitQuery.error,
      )
    : "";

  return (
    <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "grid", gap: "0.35rem" }}>
        <h2 style={{ margin: 0 }}>
          {formatSettingsShareMessage(locale, "settings.usage.sectionTitle")}
        </h2>
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
          {formatSettingsShareMessage(locale, "settings.usage.sectionSubtitle")}
        </p>
      </div>
      {usageLimitQuery.isLoading ? (
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
          {formatSettingsShareMessage(locale, "settings.usage.loading")}
        </p>
      ) : usageError ? (
        <p className="app-notice-banner">{usageError}</p>
      ) : !usageLimitQuery.data ? (
        <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
          {formatSettingsShareMessage(locale, "settings.usage.empty")}
        </p>
      ) : (
        <>
          <div className="app-inline-surface" style={{ display: "grid", gap: "0.5rem" }}>
            <div className="app-inline-row">
              <span>{formatSettingsShareMessage(locale, "settings.usage.scopeLabel")}</span>
              <strong>{scopeLabel}</strong>
            </div>
            <div className="app-inline-row">
              <span>{formatSettingsShareMessage(locale, "settings.usage.policyLabel")}</span>
              <strong>
                {usageLimitQuery.data.policy.enabled
                  ? formatSettingsShareMessage(locale, "settings.usage.policyEnabled")
                  : formatSettingsShareMessage(locale, "settings.usage.policyDisabled")}
              </strong>
            </div>
            {usageLimitQuery.data.has_estimated_usage ? (
              <p className="app-notice-banner" style={bannerStyle("info")}>
                {formatSettingsShareMessage(locale, "settings.usage.estimated")}
              </p>
            ) : null}
          </div>
          {[
            {
              label: formatSettingsShareMessage(locale, "settings.usage.window5h"),
              window: usageLimitQuery.data.windows.rolling_5h,
            },
            {
              label: formatSettingsShareMessage(locale, "settings.usage.window7d"),
              window: usageLimitQuery.data.windows.rolling_7d,
            },
          ].map(({ label, window }) => (
            <div className="app-inline-surface" key={label} style={{ display: "grid", gap: "0.6rem" }}>
              <div className="app-inline-row" style={{ marginBottom: 0 }}>
                <span>{label}</span>
                <strong>
                  {formatCompactNumber(window.used_units)} / {formatCompactNumber(window.limit_units)}
                </strong>
              </div>
              <div style={progressTrackStyle()}>
                <div style={progressBarStyle(window.percent_used)} />
              </div>
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  gap: "1rem",
                  flexWrap: "wrap",
                  color: "hsl(var(--muted-foreground))",
                }}
              >
                <span>
                  {formatSettingsShareMessage(locale, "settings.usage.remaining")}{" "}
                  {formatCompactNumber(window.remaining_units)}
                </span>
                {window.next_relief_at ? (
                  <span>
                    {formatSettingsShareMessage(locale, "settings.usage.nextRelief")}{" "}
                    {formatDate(
                      window.next_relief_at,
                      locale,
                      formatSettingsShareMessage(locale, "settings.usage.notSet"),
                    )}
                  </span>
                ) : null}
                {window.blocked ? (
                  <span>{formatSettingsShareMessage(locale, "settings.usage.blocked")}</span>
                ) : null}
              </div>
            </div>
          ))}
          {breakdown.length > 0 ? (
            <div className="app-inline-surface" style={{ display: "grid", gap: "0.5rem" }}>
              <strong>{formatSettingsShareMessage(locale, "settings.usage.breakdownTitle")}</strong>
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

const TRANSLATIONS: Record<string, {
  selectProvider: string;
  payWithStripe: string;
  payWithCreem: string;
  payWithAlipay: string;
  subscribeBtn: string;
  subscribing: string;
  scanToPay: string;
  alipayWait: string;
  cancelPay: string;
}> = {
  "zh-CN": {
    selectProvider: "选择支付方式",
    payWithStripe: "Stripe (信用卡)",
    payWithCreem: "Creem (国际支付)",
    payWithAlipay: "支付宝 (Alipay)",
    subscribeBtn: "订阅",
    subscribing: "正在处理...",
    scanToPay: "请使用支付宝扫码支付",
    alipayWait: "等待支付中，请在手机上完成付款...",
    cancelPay: "取消支付",
  },
  en: {
    selectProvider: "Select Payment Method",
    payWithStripe: "Stripe (Credit Card)",
    payWithCreem: "Creem (Global Pay)",
    payWithAlipay: "Alipay",
    subscribeBtn: "Subscribe",
    subscribing: "Processing...",
    scanToPay: "Please scan with Alipay to pay",
    alipayWait: "Waiting for payment, please complete on your phone...",
    cancelPay: "Cancel",
  }
};

function BillingPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const [actionError, setActionError] = useState("");
  const [selectedProviders, setSelectedProviders] = useState<Record<string, "stripe" | "creem" | "alipay">>({});
  const [alipayQr, setAlipayQr] = useState<string | null>(null);
  const [alipayOrderId, setAlipayOrderId] = useState<string | null>(null);
  const [pendingPlanId, setPendingPlanId] = useState<string | null>(null);
  const [checkoutPendingPlan, setCheckoutPendingPlan] = useState<string | null>(null);
  
  const queryClient = useQueryClient();
  const t = TRANSLATIONS[locale === "zh-CN" ? "zh-CN" : "en"];

  useEffect(() => {
    if (!alipayQr || !alipayOrderId || !pendingPlanId || !token) {
      return;
    }

    let active = true;
    let timer: NodeJS.Timeout;

    async function poll() {
      try {
        const sub = await getSubscription(token as string);
        if (sub && sub.plan_id === pendingPlanId && sub.status === "active") {
          setAlipayQr(null);
          setAlipayOrderId(null);
          setPendingPlanId(null);
          await queryClient.invalidateQueries({ queryKey: settingsKeys.billing(token) });
          return;
        }
      } catch (err) {
        console.error("Failed to poll subscription status", err);
      }

      if (active) {
        timer = setTimeout(poll, 3000);
      }
    }

    timer = setTimeout(poll, 3000);

    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [alipayQr, alipayOrderId, pendingPlanId, token, queryClient]);

  async function handleCheckout(planId: string) {
    if (!token) return;
    setActionError("");
    setCheckoutPendingPlan(planId);
    
    const provider = selectedProviders[planId] || "stripe";
    
    try {
      const response = await createCheckoutSession(token, {
        plan_id: planId,
        provider: provider
      });
      
      if (provider === "alipay") {
        if (response.qr_code) {
          setAlipayQr(response.qr_code);
          setAlipayOrderId(response.order_id ?? null);
          setPendingPlanId(planId);
        } else {
          throw new Error("No QR code returned from Alipay checkout");
        }
      } else {
        if (response.url) {
          window.location.assign(response.url);
        } else {
          throw new Error("No URL returned from checkout session");
        }
      }
    } catch (error) {
      setActionError(
        describeAuthError(
          formatSettingsShareMessage(locale, "settings.saveError"),
          error,
        ),
      );
    } finally {
      setCheckoutPendingPlan(null);
    }
  }
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
          formatSettingsShareMessage(locale, "settings.billing.failedItem.subscription"),
        );
      }

      if (usageResult.status === "rejected") {
        failedItems.push(formatSettingsShareMessage(locale, "settings.billing.failedItem.usage"));
      }

      if (plansResult.status === "rejected") {
        failedItems.push(formatSettingsShareMessage(locale, "settings.billing.failedItem.plans"));
      }

      return {
        subscription:
          subscriptionResult.status === "fulfilled" ? subscriptionResult.value : null,
        usage: usageResult.status === "fulfilled" ? usageResult.value : null,
        plans: plansResult.status === "fulfilled" ? plansResult.value.plans : [],
        partialError:
          failedItems.length > 0
            ? formatSettingsShareMessage(locale, "settings.billing.failedData", {
                items: failedItems.join(", "),
              })
            : "",
      };
    },
  });
  const portalMutation = useMutation({
    mutationFn: async () => {
      if (!token) {
        throw new Error(formatSettingsShareMessage(locale, "settings.profile.notAuthenticated"));
      }

      const response = await createPortalSession(token);

      if (!response.url.trim()) {
        throw new Error(formatSettingsShareMessage(locale, "settings.billing.portalEmpty"));
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
      setActionError(
        describeAuthError(
          formatSettingsShareMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  const errorMessage =
    actionError ||
    (billingQuery.error
      ? describeAuthError(
          formatSettingsShareMessage(locale, "settings.loadError"),
          billingQuery.error,
        )
      : billingQuery.data?.partialError ?? "");

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      {alipayQr ? (
        <section className="app-inline-surface" style={{ display: "grid", gap: "1rem", border: "2px solid hsl(var(--ring))", padding: "1.5rem", justifyItems: "center", textAlign: "center" }}>
          <h3 style={{ margin: 0, color: "hsl(var(--foreground))" }}>{t.scanToPay}</h3>
          <div style={{ background: "#fff", padding: "1rem", borderRadius: "8px", boxShadow: "0 4px 12px rgba(0,0,0,0.1)" }}>
            <img
              src={`https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=${encodeURIComponent(alipayQr)}`}
              alt="Alipay QR Code"
              style={{ display: "block", width: "200px", height: "200px" }}
            />
          </div>
          <p style={{ margin: 0, fontSize: "0.9rem", color: "hsl(var(--muted-foreground))" }}>
            {t.alipayWait}
          </p>
          <button
            className="app-button-secondary"
            onClick={() => {
              setAlipayQr(null);
              setAlipayOrderId(null);
              setPendingPlanId(null);
            }}
          >
            {t.cancelPay}
          </button>
        </section>
      ) : null}
      <section className="app-inline-surface" style={{ display: "grid", gap: "0.75rem" }}>
        <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
          <div style={{ display: "grid", gap: "0.35rem" }}>
            <h2 style={{ margin: 0 }}>
              {formatSettingsShareMessage(locale, "settings.billing.sectionTitle")}
            </h2>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatSettingsShareMessage(locale, "settings.billing.sectionSubtitle")}
            </p>
          </div>
          <button
            className="app-button-primary"
            disabled={portalMutation.isPending || !token}
            type="button"
            onClick={() => void handleManagePlan()}
          >
            {portalMutation.isPending
              ? formatSettingsShareMessage(locale, "settings.billing.loadingPortal")
              : formatSettingsShareMessage(locale, "settings.billing.managePlanAction")}
          </button>
        </div>
        {errorMessage ? <p className="app-notice-banner">{errorMessage}</p> : null}
        {billingQuery.isLoading ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.billing.loading")}
          </p>
        ) : (
          <div className="app-inline-surface" style={{ display: "grid", gap: "0.5rem" }}>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>
                {formatSettingsShareMessage(locale, "settings.billing.currentPlanLabel")}
              </span>
              <strong>
                {currentPlan?.name ??
                  formatSettingsShareMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{formatSettingsShareMessage(locale, "settings.billing.statusLabel")}</span>
              <strong>
                {billingQuery.data?.subscription
                  ? subscriptionStatusLabel(locale, billingQuery.data.subscription.status)
                  : formatSettingsShareMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{formatSettingsShareMessage(locale, "settings.billing.renewsOnLabel")}</span>
              <strong>
                {formatDate(
                  billingQuery.data?.subscription?.current_period_end ?? null,
                  locale,
                  formatSettingsShareMessage(locale, "settings.usage.notSet"),
                )}
              </strong>
            </div>
          </div>
        )}
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <h3 style={{ margin: 0 }}>
          {formatSettingsShareMessage(locale, "settings.billing.usageTitle")}
        </h3>
        {!billingQuery.data?.usage ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {billingQuery.isLoading
              ? formatSettingsShareMessage(locale, "settings.billing.loadingUsage")
              : formatSettingsShareMessage(locale, "settings.billing.noUsageData")}
          </p>
        ) : (
          <>
            {[
              {
                label: formatSettingsShareMessage(locale, "settings.billing.tokensLabel"),
                used: billingQuery.data.usage.used_tokens,
                limit: billingQuery.data.usage.limit_tokens,
              },
              {
                label: formatSettingsShareMessage(locale, "settings.billing.documentsLabel"),
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
                        : formatSettingsShareMessage(locale, "settings.usage.notSet")}
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
        <h3 style={{ margin: 0 }}>
          {formatSettingsShareMessage(locale, "settings.billing.availablePlansTitle")}
        </h3>
        {billingQuery.data && billingQuery.data.plans.length === 0 ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {billingQuery.isLoading
              ? formatSettingsShareMessage(locale, "settings.billing.loadingPlans")
              : formatSettingsShareMessage(locale, "settings.billing.noPlans")}
          </p>
        ) : (
          <div
            style={{
              display: "grid",
              gap: "0.8rem",
              gridTemplateColumns: "repeat(auto-fit, minmax(15rem, 1fr))",
            }}
          >
            {(billingQuery.data?.plans ?? []).map((plan: PlanRow) => {
              const isCurrentPlan = plan.id === currentPlan?.id;

              return (
                <section
                  className="app-inline-surface"
                  key={plan.id}
                  style={{ display: "grid", gap: "0.8rem", height: "100%" }}
                >
                  <div style={{ display: "grid", gap: "0.25rem" }}>
                    <div className="app-inline-row" style={{ marginBottom: 0 }}>
                      <h4 style={{ margin: 0 }}>{plan.name}</h4>
                      {isCurrentPlan ? (
                        <span className="app-status-badge">
                          {formatSettingsShareMessage(locale, "settings.billing.currentPlanLabel")}
                        </span>
                      ) : null}
                    </div>
                    <strong style={{ fontSize: "1.3rem" }}>{formatPrice(plan.price)}</strong>
                  </div>
                  <div style={{ display: "grid", gap: "0.45rem" }}>
                    {plan.features.map((feature) => (
                      <div key={feature} style={{ color: "hsl(var(--muted-foreground))" }}>
                        {featureLabel(locale, feature)}
                      </div>
                    ))}
                  </div>
                  {plan.id !== "free" && !isCurrentPlan ? (
                    <div style={{ display: "grid", gap: "0.6rem", marginTop: "auto", paddingTop: "0.8rem", borderTop: "1px solid hsl(var(--border))" }}>
                      <div style={{ fontSize: "0.85rem", fontWeight: "600", color: "hsl(var(--muted-foreground))" }}>
                        {t.selectProvider}
                      </div>
                      <div style={{ display: "grid", gap: "0.45rem" }}>
                        {([
                          ["stripe", t.payWithStripe],
                          ["creem", t.payWithCreem],
                          ["alipay", t.payWithAlipay]
                        ] as const).map(([prov, label]) => {
                          const currentProvider = selectedProviders[plan.id] || "stripe";
                          return (
                            <label
                              key={prov}
                              style={{
                                display: "flex",
                                alignItems: "center",
                                gap: "0.5rem",
                                fontSize: "0.85rem",
                                cursor: "pointer",
                                padding: "0.3rem 0.5rem",
                                borderRadius: "var(--radius)",
                                background: currentProvider === prov ? "hsl(var(--accent))" : "transparent",
                                transition: "background 0.2s"
                              }}
                            >
                              <input
                                type="radio"
                                name={`provider-${plan.id}`}
                                checked={currentProvider === prov}
                                onChange={() => setSelectedProviders(prev => ({ ...prev, [plan.id]: prov }))}
                              />
                              <span>{label}</span>
                            </label>
                          );
                        })}
                      </div>
                      <button
                        className="app-button-primary"
                        disabled={checkoutPendingPlan === plan.id}
                        onClick={() => void handleCheckout(plan.id)}
                        style={{ width: "100%", marginTop: "0.4rem" }}
                      >
                        {checkoutPendingPlan === plan.id ? t.subscribing : t.subscribeBtn}
                      </button>
                    </div>
                  ) : null}
                </section>
              );
            })}
          </div>
        )}
      </section>
    </section>
  );
}

function ProfilePanel() {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const profileForm = useForm<ProfileFormValues>({
    defaultValues: {
      fullName: auth.user?.full_name ?? "",
    },
  });
  const [banner, setBanner] = useState("");
  const [actionError, setActionError] = useState("");
  const profileMutation = useMutation({
    mutationFn: async (fullName: string | null) => {
      if (!auth.token) {
        throw new Error(formatSettingsShareMessage(locale, "settings.profile.notAuthenticated"));
      }

      const response = await updateProfile(auth.token, fullName);

      if (!response.success || !response.data) {
        throw new Error(
          response.error ?? formatSettingsShareMessage(locale, "settings.saveError"),
        );
      }

      return response.data.user;
    },
    onSuccess: (user) => {
      auth.updateUser(user);
      setBanner(formatSettingsShareMessage(locale, "settings.saveSuccess"));
    },
  });

  useEffect(() => {
    profileForm.reset({
      fullName: auth.user?.full_name ?? "",
    });
  }, [auth.user?.full_name, profileForm]);

  const profileSchema = z.object({
    fullName: z.string().trim().max(120, {
      message: formatSettingsShareMessage(locale, "settings.profile.nameTooLong"),
    }),
  });

  async function handleSubmit(values: ProfileFormValues) {
    setBanner("");
    setActionError("");
    profileForm.clearErrors();

    const parsed = profileSchema.safeParse(values);

    if (!parsed.success) {
      applyZodErrors(parsed.error, profileForm.setError);
      return;
    }

    try {
      await profileMutation.mutateAsync(parsed.data.fullName || null);
    } catch (error) {
      setActionError(
        describeAuthError(
          formatSettingsShareMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <UsageLimitPanel />
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.profile.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.profile.sectionSubtitle")}
          </p>
        </div>
        <form
          noValidate
          style={{ display: "grid", gap: "1rem" }}
          onSubmit={profileForm.handleSubmit(handleSubmit)}
        >
          <div>
            <label className="app-form-label" htmlFor="settings-profile-email">
              {formatSettingsShareMessage(locale, "settings.profile.emailLabel")}
            </label>
            <input
              className="app-input"
              id="settings-profile-email"
              readOnly
              style={{ color: "hsl(var(--muted-foreground))" }}
              type="email"
              value={auth.user?.email ?? ""}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="settings-profile-name">
              {formatSettingsShareMessage(locale, "settings.profile.nameLabel")}
            </label>
            <input
              className="app-input"
              id="settings-profile-name"
              placeholder={formatSettingsShareMessage(locale, "settings.profile.namePlaceholder")}
              type="text"
              {...profileForm.register("fullName")}
            />
            {profileForm.formState.errors.fullName?.message ? (
              <p className="app-form-footnote" style={{ color: "hsl(var(--destructive))" }}>
                {profileForm.formState.errors.fullName.message}
              </p>
            ) : null}
          </div>
          {banner ? (
            <p className="app-notice-banner" style={bannerStyle("success")}>
              {banner}
            </p>
          ) : null}
          {actionError ? <p className="app-notice-banner">{actionError}</p> : null}
          <div className="app-button-row">
            <button
              className="app-button-primary"
              disabled={profileMutation.isPending}
              type="submit"
            >
              {profileMutation.isPending
                ? formatSettingsShareMessage(locale, "shareCenter.saving")
                : formatSettingsShareMessage(locale, "settings.profile.saveAction")}
            </button>
          </div>
        </form>
      </section>
    </section>
  );
}

function AppearancePanel() {
  const { locale, setLocale, setTheme, theme } = useUiPreferences();

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.appearance.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.appearance.sectionSubtitle")}
          </p>
        </div>
        <div
          style={{
            display: "grid",
            gap: "0.75rem",
            gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))",
          }}
        >
          {([
            [
              "system",
              formatSettingsShareMessage(locale, "settings.appearance.theme.system"),
              formatSettingsShareMessage(locale, "settings.appearance.themeDescription.system"),
            ],
            [
              "light",
              formatSettingsShareMessage(locale, "settings.appearance.theme.light"),
              formatSettingsShareMessage(locale, "settings.appearance.themeDescription.light"),
            ],
            [
              "dark",
              formatSettingsShareMessage(locale, "settings.appearance.theme.dark"),
              formatSettingsShareMessage(locale, "settings.appearance.themeDescription.dark"),
            ],
          ] as const).map(([value, title, description]) => (
            <button
              key={value}
              style={panelChoiceStyle(theme === value)}
              type="button"
              onClick={() => setTheme(value)}
            >
              <strong>{title}</strong>
              <span style={{ color: "hsl(var(--muted-foreground))" }}>{description}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.appearance.localeLabel")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.appearance.sectionSubtitle")}
          </p>
        </div>
        <div
          style={{
            display: "grid",
            gap: "0.75rem",
            gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))",
          }}
        >
          {([
            [
              "zh-CN",
              formatSettingsShareMessage(locale, "workspaceLanguageChinese"),
              formatSettingsShareMessage(locale, "settings.appearance.localeDescription.zh-CN"),
            ],
            [
              "en",
              formatSettingsShareMessage(locale, "workspaceLanguageEnglish"),
              formatSettingsShareMessage(locale, "settings.appearance.localeDescription.en"),
            ],
          ] as const).map(([value, title, description]) => (
            <button
              key={value}
              style={panelChoiceStyle(locale === value)}
              type="button"
              onClick={() => setLocale(value)}
            >
              <strong>{title}</strong>
              <span style={{ color: "hsl(var(--muted-foreground))" }}>{description}</span>
            </button>
          ))}
        </div>
        <div className="app-inline-surface" style={{ display: "grid", gap: "0.45rem" }}>
          <div className="app-inline-row" style={{ marginBottom: 0 }}>
            <span>{formatSettingsShareMessage(locale, "settings.appearance.currentTheme")}</span>
            <strong>
              {{
                system: formatSettingsShareMessage(locale, "settings.appearance.theme.system"),
                light: formatSettingsShareMessage(locale, "settings.appearance.theme.light"),
                dark: formatSettingsShareMessage(locale, "settings.appearance.theme.dark"),
              }[theme]}
            </strong>
          </div>
          <div className="app-inline-row" style={{ marginBottom: 0 }}>
            <span>
              {formatSettingsShareMessage(locale, "settings.appearance.currentLanguage")}
            </span>
            <strong>
              {locale === "zh-CN"
                ? formatSettingsShareMessage(locale, "workspaceLanguageChinese")
                : formatSettingsShareMessage(locale, "workspaceLanguageEnglish")}
            </strong>
          </div>
        </div>
      </section>
    </section>
  );
}

function NotificationsPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const queryClient = useQueryClient();
  const notificationsForm = useForm<NotificationFormValues>({
    defaultValues: notificationFormDefaults(),
  });
  const [banner, setBanner] = useState("");
  const [actionError, setActionError] = useState("");
  const preferencesQuery = useQuery({
    queryKey: settingsKeys.preferences(token),
    enabled: Boolean(token),
    queryFn: () => getUserPreferences(token as string),
  });
  const notificationsQuery = useQuery({
    queryKey: settingsKeys.notifications(token),
    enabled: Boolean(token),
    queryFn: () => listNotifications(token as string),
  });
  const saveMutation = useMutation({
    mutationFn: async (preferences: UserPreferences) =>
      updateUserPreferences(token as string, preferences),
    onSuccess: async (updatedPreferences) => {
      queryClient.setQueryData(settingsKeys.preferences(token), updatedPreferences);
      await queryClient.invalidateQueries({ queryKey: settingsKeys.preferences(token) });
      setBanner(formatSettingsShareMessage(locale, "settings.saveSuccess"));
    },
  });
  const markReadMutation = useMutation({
    mutationFn: async (notificationId: string) => {
      await markNotificationRead(token as string, notificationId);
      return notificationId;
    },
    onSuccess: (notificationId) => {
      queryClient.setQueryData(
        settingsKeys.notifications(token),
        (current: { notifications: NotificationRow[] } | undefined) =>
          current
            ? {
                notifications: current.notifications.map((notification) =>
                  notification.id === notificationId
                    ? { ...notification, read_at: new Date().toISOString() }
                    : notification,
                ),
              }
            : current,
      );
    },
  });

  useEffect(() => {
    notificationsForm.reset(
      notificationFormDefaults(preferencesQuery.data?.notifications),
    );
  }, [notificationsForm, preferencesQuery.data]);

  const quietHoursSchema = z
    .string()
    .trim()
    .refine((value) => value.length === 0 || TIME_24H_PATTERN.test(value), {
      message: formatSettingsShareMessage(locale, "settings.notifications.invalidTime"),
    });
  const notificationSchema = z.object({
    email_enabled: z.boolean(),
    product_enabled: z.boolean(),
    security_enabled: z.boolean(),
    weekly_digest_enabled: z.boolean(),
    quiet_hours_start: quietHoursSchema,
    quiet_hours_end: quietHoursSchema,
  });

  async function handleSave(values: NotificationFormValues) {
    setBanner("");
    setActionError("");
    notificationsForm.clearErrors();

    const parsed = notificationSchema.safeParse(values);

    if (!parsed.success) {
      applyZodErrors(parsed.error, notificationsForm.setError);
      return;
    }

    if (!token) {
      setActionError(formatSettingsShareMessage(locale, "settings.profile.notAuthenticated"));
      return;
    }

    try {
      const basePreferences =
        preferencesQuery.data ?? (await getUserPreferences(token));

      await saveMutation.mutateAsync({
        ...basePreferences,
        notifications: {
          email_enabled: parsed.data.email_enabled,
          product_enabled: parsed.data.product_enabled,
          security_enabled: parsed.data.security_enabled,
          weekly_digest_enabled: parsed.data.weekly_digest_enabled,
          quiet_hours_start: parsed.data.quiet_hours_start || null,
          quiet_hours_end: parsed.data.quiet_hours_end || null,
        },
      });
    } catch (error) {
      setActionError(
        describeAuthError(
          formatSettingsShareMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  async function handleMarkRead(notificationId: string) {
    setActionError("");

    try {
      await markReadMutation.mutateAsync(notificationId);
    } catch (error) {
      setActionError(
        describeAuthError(
          formatSettingsShareMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  const loadError =
    (preferencesQuery.error &&
      describeAuthError(
        formatSettingsShareMessage(locale, "settings.loadError"),
        preferencesQuery.error,
      )) ||
    (notificationsQuery.error &&
      describeAuthError(
        formatSettingsShareMessage(locale, "settings.loadError"),
        notificationsQuery.error,
      )) ||
    "";
  const notifications = notificationsQuery.data?.notifications ?? [];

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
          <div style={{ display: "grid", gap: "0.35rem" }}>
            <h2 style={{ margin: 0 }}>
              {formatSettingsShareMessage(locale, "settings.notifications.sectionTitle")}
            </h2>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatSettingsShareMessage(locale, "settings.notifications.sectionSubtitle")}
            </p>
          </div>
          <button
            className="app-button-secondary"
            disabled={saveMutation.isPending || !token}
            type="submit"
            form="settings-notifications-form"
          >
            {saveMutation.isPending
              ? formatSettingsShareMessage(locale, "shareCenter.saving")
              : formatSettingsShareMessage(locale, "settings.notifications.saveAction")}
          </button>
        </div>
        {banner ? (
          <p className="app-notice-banner" style={bannerStyle("success")}>
            {banner}
          </p>
        ) : null}
        {actionError || loadError ? (
          <p className="app-notice-banner">{actionError || loadError}</p>
        ) : null}
        <form
          id="settings-notifications-form"
          noValidate
          style={{ display: "grid", gap: "1rem" }}
          onSubmit={notificationsForm.handleSubmit(handleSave)}
        >
          <div
            style={{
              display: "grid",
              gap: "0.75rem",
              gridTemplateColumns: "repeat(auto-fit, minmax(16rem, 1fr))",
            }}
          >
            {([
              ["email_enabled", formatSettingsShareMessage(locale, "settings.notifications.emailUpdatesLabel")],
              ["product_enabled", formatSettingsShareMessage(locale, "settings.notifications.productUpdatesLabel")],
              ["security_enabled", formatSettingsShareMessage(locale, "settings.notifications.securityAlertsLabel")],
              ["weekly_digest_enabled", formatSettingsShareMessage(locale, "settings.notifications.weeklyDigestLabel")],
            ] as const).map(([key, title]) => (
              <label
                className="app-inline-surface"
                key={key}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: "1rem",
                  cursor: "pointer",
                }}
              >
                <span>{title}</span>
                <input type="checkbox" {...notificationsForm.register(key)} />
              </label>
            ))}
          </div>
          <div
            style={{
              display: "grid",
              gap: "0.75rem",
              gridTemplateColumns: "repeat(auto-fit, minmax(16rem, 1fr))",
            }}
          >
            <div>
              <label className="app-form-label" htmlFor="settings-quiet-hours-start">
                {formatSettingsShareMessage(locale, "settings.notifications.quietHoursStartLabel")}
              </label>
              <input
                className="app-input"
                id="settings-quiet-hours-start"
                placeholder={formatSettingsShareMessage(
                  locale,
                  "settings.notifications.quietHoursPlaceholderStart",
                )}
                type="text"
                {...notificationsForm.register("quiet_hours_start")}
              />
              {notificationsForm.formState.errors.quiet_hours_start?.message ? (
                <p className="app-form-footnote" style={{ color: "hsl(var(--destructive))" }}>
                  {notificationsForm.formState.errors.quiet_hours_start.message}
                </p>
              ) : null}
            </div>
            <div>
              <label className="app-form-label" htmlFor="settings-quiet-hours-end">
                {formatSettingsShareMessage(locale, "settings.notifications.quietHoursEndLabel")}
              </label>
              <input
                className="app-input"
                id="settings-quiet-hours-end"
                placeholder={formatSettingsShareMessage(
                  locale,
                  "settings.notifications.quietHoursPlaceholderEnd",
                )}
                type="text"
                {...notificationsForm.register("quiet_hours_end")}
              />
              {notificationsForm.formState.errors.quiet_hours_end?.message ? (
                <p className="app-form-footnote" style={{ color: "hsl(var(--destructive))" }}>
                  {notificationsForm.formState.errors.quiet_hours_end.message}
                </p>
              ) : null}
            </div>
          </div>
        </form>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <h3 style={{ margin: 0 }}>
          {formatSettingsShareMessage(locale, "settings.notifications.historyTitle")}
        </h3>
        {notificationsQuery.isLoading ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.notifications.loading")}
          </p>
        ) : notifications.length === 0 ? (
          <div style={{ display: "grid", gap: "0.3rem" }}>
            <strong>
              {formatSettingsShareMessage(locale, "settings.notifications.emptyTitle")}
            </strong>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatSettingsShareMessage(locale, "settings.notifications.emptyBody")}
            </p>
          </div>
        ) : (
          <div style={{ display: "grid", gap: "0.75rem" }}>
            {notifications.map((notification) => (
              <article
                className="app-inline-surface"
                key={notification.id}
                style={{
                  display: "grid",
                  gap: "0.6rem",
                  borderColor: notification.read_at ? "hsl(var(--border))" : "hsl(var(--primary))",
                }}
              >
                <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
                  <div style={{ display: "grid", gap: "0.25rem" }}>
                    <strong>{notification.title}</strong>
                    <span style={{ color: "hsl(var(--muted-foreground))" }}>
                      {notificationTypeLabel(locale, notification.event_type)} ·{" "}
                      {formatDateTime(notification.created_at, locale)}
                    </span>
                  </div>
                  <button
                    className="app-button-ghost"
                    disabled={
                      Boolean(notification.read_at) ||
                      (markReadMutation.isPending && markReadMutation.variables === notification.id)
                    }
                    type="button"
                    onClick={() => void handleMarkRead(notification.id)}
                  >
                    {notification.read_at
                      ? formatSettingsShareMessage(locale, "settings.notifications.read")
                      : markReadMutation.isPending &&
                          markReadMutation.variables === notification.id
                        ? formatSettingsShareMessage(locale, "settings.notifications.processing")
                        : formatSettingsShareMessage(locale, "settings.notifications.markRead")}
                  </button>
                </div>
                <p style={{ margin: 0 }}>{notification.body}</p>
              </article>
            ))}
          </div>
        )}
      </section>
    </section>
  );
}

function SecurityPanel() {
  const router = useRouter();
  const { clearAuth, logout, passwordResetEnabled, token, user } = useAuth();
  const { locale, theme } = useUiPreferences();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!token) {
      setError(formatSettingsShareMessage(locale, "settings.security.notAuthenticated"));
      return;
    }

    if (!currentPassword.trim() || !newPassword.trim()) {
      setError(formatSettingsShareMessage(locale, "settings.security.missingPassword"));
      return;
    }

    setLoading(true);
    setError("");

    try {
      await changePassword(token, {
        old_password: currentPassword,
        new_password: newPassword,
      });
      clearAuth();
      router.replace("/login");
    } catch (submitError) {
      setError(
        describeAuthError(
          formatSettingsShareMessage(locale, "settings.security.failed"),
          submitError,
        ),
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleLogout() {
    await logout();
    router.replace("/login");
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.security.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.security.sectionSubtitle")}
          </p>
        </div>
        <form style={{ display: "grid", gap: "1rem" }} onSubmit={handleSubmit}>
          <div>
            <label className="app-form-label" htmlFor="settings-current-password">
              {formatSettingsShareMessage(locale, "settings.security.currentPasswordLabel")}
            </label>
            <input
              autoComplete="current-password"
              className="app-input"
              id="settings-current-password"
              type="password"
              value={currentPassword}
              onChange={(event) => setCurrentPassword(event.target.value)}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="settings-new-password">
              {formatSettingsShareMessage(locale, "settings.security.newPasswordLabel")}
            </label>
            <input
              autoComplete="new-password"
              className="app-input"
              id="settings-new-password"
              type="password"
              value={newPassword}
              onChange={(event) => setNewPassword(event.target.value)}
            />
          </div>
          {error ? <p className="app-notice-banner">{error}</p> : null}
          <div className="app-button-row">
            <button className="app-button-primary" disabled={loading} type="submit">
              {loading
                ? formatSettingsShareMessage(locale, "settings.security.updating")
                : formatSettingsShareMessage(locale, "settings.security.changePasswordAction")}
            </button>
            {passwordResetEnabled ? (
              <Link className="app-button-secondary" href="/reset-password">
                {formatSettingsShareMessage(locale, "settings.security.resetPasswordAction")}
              </Link>
            ) : null}
            <button className="app-button-ghost" type="button" onClick={() => void handleLogout()}>
              {formatSettingsShareMessage(locale, "workspaceLogout")}
            </button>
          </div>
        </form>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <h3 style={{ margin: 0 }}>
          {formatSettingsShareMessage(locale, "settings.security.currentSessionTitle")}
        </h3>
        <div className="app-inline-row" style={{ marginBottom: 0 }}>
          <span>{formatSettingsShareMessage(locale, "settings.security.signedInAs")}</span>
          <strong>
            {user?.email ??
              formatSettingsShareMessage(locale, "settings.security.unknownAccount")}
          </strong>
        </div>
        <div className="app-inline-row" style={{ marginBottom: 0 }}>
          <span>{formatSettingsShareMessage(locale, "settings.appearance.currentLanguage")}</span>
          <strong>
            {locale === "zh-CN"
              ? formatSettingsShareMessage(locale, "workspaceLanguageChinese")
              : formatSettingsShareMessage(locale, "workspaceLanguageEnglish")}
          </strong>
        </div>
        <div className="app-inline-row" style={{ marginBottom: 0 }}>
          <span>{formatSettingsShareMessage(locale, "settings.appearance.currentTheme")}</span>
          <strong>
            {{
              system: formatSettingsShareMessage(locale, "settings.appearance.theme.system"),
              light: formatSettingsShareMessage(locale, "settings.appearance.theme.light"),
              dark: formatSettingsShareMessage(locale, "settings.appearance.theme.dark"),
            }[theme]}
          </strong>
        </div>
      </section>
    </section>
  );
}

function SettingsPanel({ activeTab }: { activeTab: SettingsTab }) {
  switch (activeTab) {
    case "billing":
      return <BillingPanel />;
    case "profile":
      return <ProfilePanel />;
    case "appearance":
      return <AppearancePanel />;
    case "security":
      return <SecurityPanel />;
    case "notifications":
      return <NotificationsPanel />;
  }
}

export function SettingsSurface({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();

  return (
    <AppPageFrame
      title={formatSettingsShareMessage(locale, "settings.pageTitle")}
      subtitle={formatSettingsShareMessage(locale, "settings.pageSubtitle")}
    >
      <div className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
        <SettingsTabBar activeTab={activeTab} />
        <SettingsPanel activeTab={activeTab} />
      </div>
    </AppPageFrame>
  );
}
