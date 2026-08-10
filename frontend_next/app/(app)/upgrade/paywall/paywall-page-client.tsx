"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { PaywallModal } from "@/components/billing/PaywallModal";
import { billingApi } from "@/lib/billing/api";
import type { UsageWindowResponse } from "@/lib/billing/api";
import { ApiError } from "@/lib/auth/client";
import { isPricingRevampFeatureDisabledError } from "@/lib/billing/featureFlag";
import { usePricingRevampGateResult } from "@/components/billing/PricingRevampGate";
import { useAuth } from "@/lib/auth/context";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";
import styles from "./paywall-page.module.css";

type PaywallLoadState =
  | { kind: "loading" }
  | { kind: "ready"; window: UsageWindowResponse }
  | { kind: "error" };

/**
 * Rate-limit recovery surface (PRODUCT_IA §4): explainer + usage meter, routes
 * upgrades to the canonical /pricing checkout. No payment UI lives here.
 */
export function PaywallPageClient({ reason }: { reason: "5h" | "7d" }) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const { ssrEnabled, enabled, ready } = usePricingRevampGateResult();
  const [state, setState] = useState<PaywallLoadState>({ kind: "loading" });

  useEffect(() => {
    if (!ready || !enabled || !auth.token) {
      return;
    }

    let cancelled = false;

    async function loadPaywall() {
      try {
        const windowData = await billingApi.getUsageWindow(auth.token);
        if (cancelled) {
          return;
        }
        setState({ kind: "ready", window: windowData });
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

  const { window } = state;

  function handleContinueFree() {
    router.push("/dashboard");
  }

  return (
    <PaywallModal
      reason={reason}
      locale={locale}
      rolling5h={window.rolling_5h}
      rolling7d={window.rolling_7d}
      onContinueFree={handleContinueFree}
    />
  );
}
