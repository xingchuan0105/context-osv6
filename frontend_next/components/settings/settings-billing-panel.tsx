"use client";

import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import ConsentCheckbox from "../legal/ConsentCheckbox";
import { recordPaymentLegalAcceptance } from "../../lib/legal/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  createCheckoutSession,
  createPortalSession,
  getSubscription,
  getUsage,
  listPlans,
  type PlanRow,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  featureLabel,
  formatCompactNumber,
  formatDate,
  formatPrice,
  progressBarStyle,
  progressTrackStyle,
  settingsKeys,
  subscriptionStatusLabel,
} from "./settings-shared";
import { UsageLimitPanel } from "./settings-usage-limit-panel";

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

export function BillingPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const [actionError, setActionError] = useState("");
  const [selectedProviders, setSelectedProviders] = useState<Record<string, "stripe" | "creem" | "alipay">>({});
  const [alipayQr, setAlipayQr] = useState<string | null>(null);
  const [alipayOrderId, setAlipayOrderId] = useState<string | null>(null);
  const [pendingPlanId, setPendingPlanId] = useState<string | null>(null);
  const [checkoutPendingPlan, setCheckoutPendingPlan] = useState<string | null>(null);
  const [paymentConsented, setPaymentConsented] = useState(false);
  
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
      await recordPaymentLegalAcceptance(token, paymentConsented);
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
          formatUiMessage(locale, "settings.saveError"),
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
      setActionError(
        describeAuthError(
          formatUiMessage(locale, "settings.saveError"),
          error,
        ),
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
              {formatUiMessage(locale, "settings.billing.sectionTitle")}
            </h2>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatUiMessage(locale, "settings.billing.sectionSubtitle")}
            </p>
          </div>
          <button
            className="app-button-primary"
            disabled={portalMutation.isPending || !token}
            type="button"
            onClick={() => void handleManagePlan()}
          >
            {portalMutation.isPending
              ? formatUiMessage(locale, "settings.billing.loadingPortal")
              : formatUiMessage(locale, "settings.billing.managePlanAction")}
          </button>
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
        <h3 style={{ margin: 0 }}>
          {formatUiMessage(locale, "settings.billing.availablePlansTitle")}
        </h3>
        <ConsentCheckbox onConsentChange={setPaymentConsented} />
        {billingQuery.data && billingQuery.data.plans.length === 0 ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {billingQuery.isLoading
              ? formatUiMessage(locale, "settings.billing.loadingPlans")
              : formatUiMessage(locale, "settings.billing.noPlans")}
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
                          {formatUiMessage(locale, "settings.billing.currentPlanLabel")}
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

