"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

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
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";
import styles from "./paywall-page.module.css";

type PaywallLoadState =
  | { kind: "loading" }
  | { kind: "ready"; window: UsageWindowResponse; plans: BillingPlan[] }
  | { kind: "error" };

export function PaywallPageClient({ reason }: { reason: "5h" | "7d" }) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const { ssrEnabled, enabled, ready } = usePricingRevampGateResult();
  const [state, setState] = useState<PaywallLoadState>({ kind: "loading" });
  const [paymentConsented, setPaymentConsented] = useState(false);
  const [checkoutError, setCheckoutError] = useState("");

  useEffect(() => {
    if (!ready || !enabled) {
      return;
    }

    let cancelled = false;

    async function loadPaywall() {
      try {
        const [windowData, plansData] = await Promise.all([
          billingApi.getUsageWindow(),
          billingApi.getPlans(),
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
  }, [enabled, ready, router]);

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
      const checkout = await createCheckoutSession(auth.token, { plan_id: planId });
      if (checkout.url) {
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

  function handleContinueFree() {
    router.push("/dashboard");
  }

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
    </>
  );
}
