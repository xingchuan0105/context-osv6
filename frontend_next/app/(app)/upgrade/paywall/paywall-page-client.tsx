"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { AlipayQrDialog } from "@/components/billing/AlipayQrDialog";
import { PaywallModal } from "@/components/billing/PaywallModal";
import ConsentCheckbox from "@/components/legal/ConsentCheckbox";
import { billingApi } from "@/lib/billing/api";
import type { BillingPlan, UsageWindowResponse } from "@/lib/billing/api";
import { ApiError } from "@/lib/auth/client";
import { isPricingRevampFeatureDisabledError } from "@/lib/billing/featureFlag";
import { usePricingRevampGateResult } from "@/components/billing/PricingRevampGate";
import { createCheckoutSession } from "@/lib/settings/client";
import { recordPaymentLegalAcceptance } from "@/lib/legal/client";
import { describeAuthError } from "@/lib/auth/errors";
import { useAuth } from "@/lib/auth/context";
import { billingProviderForLocale, planPriceLabel } from "@/lib/billing/provider";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";
import styles from "./paywall-page.module.css";

type PaywallLoadState =
  | { kind: "loading" }
  | { kind: "ready"; window: UsageWindowResponse; plans: BillingPlan[] }
  | { kind: "error" };

type AlipaySession = {
  qrCode: string;
  orderId: string;
  planId: string;
};

export function PaywallPageClient({ reason }: { reason: "5h" | "7d" }) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const { ssrEnabled, enabled, ready } = usePricingRevampGateResult();
  const [state, setState] = useState<PaywallLoadState>({ kind: "loading" });
  const [paymentConsented, setPaymentConsented] = useState(false);
  const [checkoutError, setCheckoutError] = useState("");
  const [alipaySession, setAlipaySession] = useState<AlipaySession | null>(null);

  useEffect(() => {
    if (!ready || !enabled || !auth.token) {
      return;
    }

    let cancelled = false;

    async function loadPaywall() {
      try {
        const [windowData, plansData] = await Promise.all([
          billingApi.getUsageWindow(auth.token),
          billingApi.getPlans(auth.token),
        ]);
        if (cancelled) {
          return;
        }
        setState({ kind: "ready", window: windowData, plans: plansData.plans });
      } catch (error) {
        if (cancelled) {
          return;
        }
        if (
          (error instanceof ApiError && error.code === "feature_disabled") ||
          isPricingRevampFeatureDisabledError(error)
        ) {
          router.replace("/dashboard");
          return;
        }
        setState({ kind: "error" });
      }
    }

    void loadPaywall();

    return () => {
      cancelled = true;
    };
  }, [auth.token, enabled, ready, router]);

  if (!ssrEnabled) {
    return null;
  }

  if (state.kind === "loading") {
    return (
      <div className={styles.statePage}>
        <p>{formatUiMessage(locale, "paywallLoading")}</p>
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div className={styles.statePage}>
        <p className={styles.errorText}>{formatUiMessage(locale, "paywallErrorLoad")}</p>
        <button type="button" className={styles.retryButton} onClick={() => router.push("/dashboard")}>
          {formatUiMessage(locale, "paywallErrorBackDashboard")}
        </button>
      </div>
    );
  }

  const { window, plans } = state;

  async function handleSelect(planId: string) {
    if (planId === "free" || !auth.token) {
      return;
    }
    setCheckoutError("");
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

  function handleAlipayPaid() {
    setAlipaySession(null);
    router.push("/dashboard");
  }

  function handleContinueFree() {
    router.push("/dashboard");
  }

  const alipayPlan = alipaySession
    ? plans.find((plan) => plan.plan_id === alipaySession.planId)
    : undefined;

  return (
    <>
      <PaywallModal
        reason={reason}
        locale={locale}
        plans={plans}
        rolling5h={window.rolling_5h}
        rolling7d={window.rolling_7d}
        onSelect={handleSelect}
        onContinueFree={handleContinueFree}
      />
      <div className={styles.statePage} style={{ marginTop: "1rem" }}>
        <ConsentCheckbox onConsentChange={setPaymentConsented} />
        {checkoutError ? <p className={styles.errorText}>{checkoutError}</p> : null}
      </div>
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
