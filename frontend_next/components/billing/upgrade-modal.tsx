"use client";

import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

import { AppModal } from "../ui/app-modal";
import ConsentCheckbox from "../legal/ConsentCheckbox";
import { AlipayQrDialog } from "./AlipayQrDialog";
import { PricingCards } from "./PricingCards";
import { useAuth } from "../../lib/auth/context";
import { describeAuthError } from "../../lib/auth/errors";
import type { BillingPlan } from "../../lib/billing/api";
import { billingApi } from "../../lib/billing/api";
import { MARKETING_BILLING_PLANS, plansForInterval } from "../../lib/billing/publicPlans";
import { billingProviderForLocale, planPriceLabel } from "../../lib/billing/provider";
import { formatUiMessage } from "../../lib/i18n/messages";
import { recordPaymentLegalAcceptance } from "../../lib/legal/client";
import { createCheckoutSession } from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";

type AlipaySession = {
  qrCode: string;
  orderId: string;
  planId: string;
};

type UpgradeModalProps = {
  open: boolean;
  onClose: () => void;
};

type BillingInterval = "month" | "year";
type Step = "tiers" | "checkout";

/**
 * Upgrade flow: Free / Plus / Pro (3 tiers) → then interval + pay.
 * Avoids dumping monthly + yearly SKUs as five equal cards.
 */
export function UpgradeModal({ open, onClose }: UpgradeModalProps) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [allPlans, setAllPlans] = useState<BillingPlan[]>(MARKETING_BILLING_PLANS);
  const [step, setStep] = useState<Step>("tiers");
  const [selectedTier, setSelectedTier] = useState<"plus" | "pro" | null>(null);
  const [interval, setInterval] = useState<BillingInterval>("month");
  const [paymentConsented, setPaymentConsented] = useState(false);
  const [checkoutError, setCheckoutError] = useState("");
  const [checkoutNotice, setCheckoutNotice] = useState("");
  const [alipaySession, setAlipaySession] = useState<AlipaySession | null>(null);

  useEffect(() => {
    if (!open) {
      setStep("tiers");
      setSelectedTier(null);
      setInterval("month");
      setCheckoutError("");
      setCheckoutNotice("");
      return;
    }
    void billingApi
      .getPlans()
      .then((response) => {
        const remote = response.plans ?? [];
        const ids = new Set(remote.map((p) => p.plan_id));
        const extras = MARKETING_BILLING_PLANS.filter((p) => !ids.has(p.plan_id));
        setAllPlans(extras.length ? [...remote, ...extras] : remote);
      })
      .catch(() => setAllPlans(MARKETING_BILLING_PLANS));
  }, [open]);

  const tierPlans = useMemo(() => plansForInterval(allPlans, "month"), [allPlans]);
  const checkoutPlans = useMemo(() => {
    if (!selectedTier) {
      return [];
    }
    return plansForInterval(allPlans, interval).filter((p) => {
      const base = p.plan_id.replace(/_annual$/, "");
      return base === selectedTier;
    });
  }, [allPlans, interval, selectedTier]);

  async function handleCheckout(planId: string) {
    if (planId === "free" || !auth.token) {
      return;
    }

    setCheckoutError("");
    setCheckoutNotice("");
    try {
      await recordPaymentLegalAcceptance(auth.token, paymentConsented);
      const checkout = await createCheckoutSession(auth.token, {
        plan_id: planId,
        provider: billingProviderForLocale(locale),
      });
      if (checkout.qr_code && checkout.order_id) {
        setAlipaySession({ qrCode: checkout.qr_code, orderId: checkout.order_id, planId });
      } else if (checkout.url) {
        router.push(checkout.url);
      }
    } catch (error) {
      setCheckoutError(
        describeAuthError(
          formatUiMessage(locale, "authErrorConsentRequired"),
          error,
          locale,
        ),
      );
    }
  }

  function handleTierSelect(planId: string) {
    const base = planId.replace(/_annual$/, "");
    if (base === "free") {
      onClose();
      return;
    }
    if (base === "plus" || base === "pro") {
      setSelectedTier(base);
      setStep("checkout");
    }
  }

  function handleAlipayPaid() {
    setAlipaySession(null);
    setCheckoutNotice(formatUiMessage(locale, "alipayQrPaid"));
    void billingApi
      .getPlans()
      .then((response) => setAllPlans(response.plans))
      .catch(() => {});
  }

  const alipayPlan = alipaySession
    ? allPlans.find((plan) => plan.plan_id === alipaySession.planId)
    : undefined;

  return (
    <>
      <AppModal
        open={open}
        size="lg"
        title={formatUiMessage(locale, "upgradeModal.title")}
        closeLabel={formatUiMessage(locale, "appModal.close")}
        fullPageHref="/pricing"
        fullPageLabel={formatUiMessage(locale, "upgradeModal.openFullPage")}
        testId="upgrade-modal"
        onClose={onClose}
      >
        <div style={{ display: "grid", gap: "1rem" }}>
          <p className="app-page-subtitle" style={{ margin: 0 }}>
            {step === "tiers"
              ? formatUiMessage(locale, "upgradeModal.subtitle")
              : locale === "zh-CN"
                ? `已选 ${selectedTier === "pro" ? "Pro" : "Plus"} · 选择计费周期后继续支付`
                : `Selected ${selectedTier === "pro" ? "Pro" : "Plus"} · pick billing cycle, then pay`}
          </p>

          {step === "tiers" ? (
            <PricingCards
              compact
              highlightTier="plus"
              locale={locale}
              plans={tierPlans}
              onSelect={handleTierSelect}
            />
          ) : (
            <>
              <div
                role="group"
                aria-label={locale === "zh-CN" ? "计费周期" : "Billing interval"}
                style={{
                  display: "flex",
                  gap: "0.5rem",
                  flexWrap: "wrap",
                  alignItems: "center",
                }}
              >
                <button
                  type="button"
                  className="app-button-secondary"
                  data-testid="upgrade-interval-month"
                  data-active={interval === "month" ? "true" : "false"}
                  style={
                    interval === "month"
                      ? { background: "hsl(var(--cta-background))", color: "hsl(var(--cta-foreground))" }
                      : undefined
                  }
                  onClick={() => setInterval("month")}
                >
                  {formatUiMessage(locale, "pricingMonthly")}
                </button>
                <button
                  type="button"
                  className="app-button-secondary"
                  data-testid="upgrade-interval-year"
                  data-active={interval === "year" ? "true" : "false"}
                  style={
                    interval === "year"
                      ? { background: "hsl(var(--cta-background))", color: "hsl(var(--cta-foreground))" }
                      : undefined
                  }
                  onClick={() => setInterval("year")}
                >
                  {formatUiMessage(locale, "pricingYearly")}
                </button>
                <button
                  type="button"
                  className="app-link"
                  style={{ marginLeft: "auto" }}
                  onClick={() => {
                    setStep("tiers");
                    setSelectedTier(null);
                  }}
                >
                  {locale === "zh-CN" ? "← 返回档位" : "← Back to plans"}
                </button>
              </div>

              <PricingCards
                compact
                highlightTier={selectedTier ?? "plus"}
                locale={locale}
                plans={checkoutPlans}
                onSelect={(planId) => void handleCheckout(planId)}
              />

              {auth.token ? (
                <div style={{ display: "grid", gap: "0.75rem" }}>
                  <ConsentCheckbox onConsentChange={setPaymentConsented} />
                  {checkoutError ? (
                    <p className="app-notice-banner" role="alert">
                      {checkoutError}
                    </p>
                  ) : null}
                  {checkoutNotice ? (
                    <p className="app-inline-surface" role="status">
                      {checkoutNotice}
                    </p>
                  ) : null}
                </div>
              ) : null}
            </>
          )}
        </div>
      </AppModal>
      {alipaySession && auth.token ? (
        <AlipayQrDialog
          token={auth.token}
          qrCode={alipaySession.qrCode}
          orderId={alipaySession.orderId}
          planName={alipayPlan?.name ?? alipaySession.planId}
          priceLabel={alipayPlan ? planPriceLabel(alipayPlan, locale) : ""}
          locale={locale}
          onPaid={handleAlipayPaid}
          onCancel={() => setAlipaySession(null)}
        />
      ) : null}
    </>
  );
}
