"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { PaywallModal } from "@/components/billing/PaywallModal";
import { billingApi } from "@/lib/billing/api";
import type { BillingPlan, UsageWindowResponse } from "@/lib/billing/api";
import { ApiError } from "@/lib/auth/client";
import {
  isPricingRevampEnabled,
  isPricingRevampEnabledSSR,
  isPricingRevampFeatureDisabledError,
} from "@/lib/billing/featureFlag";
import { createCheckoutSession } from "@/lib/settings/client";
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
  const [state, setState] = useState<PaywallLoadState>({ kind: "loading" });

  useEffect(() => {
    if (!isPricingRevampEnabledSSR()) {
      router.replace("/dashboard");
      return;
    }

    let cancelled = false;

    async function loadPaywall() {
      const enabled = await isPricingRevampEnabled();
      if (cancelled) {
        return;
      }
      if (!enabled) {
        router.replace("/dashboard");
        return;
      }

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
  }, [router]);

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
    const checkout = await createCheckoutSession(auth.token, { plan_id: planId });
    if (checkout.url) {
      router.push(checkout.url);
    }
  }

  function handleContinueFree() {
    router.push("/dashboard");
  }

  return (
    <PaywallModal
      reason={reason}
      locale={locale}
      plans={plans}
      rolling5h={window.rolling_5h}
      rolling7d={window.rolling_7d}
      onSelect={handleSelect}
      onContinueFree={handleContinueFree}
    />
  );
}
