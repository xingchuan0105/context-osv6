"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { AppModal } from "../ui/app-modal";
import ConsentCheckbox from "../legal/ConsentCheckbox";
import { AlipayQrDialog } from "./AlipayQrDialog";
import { PricingCards } from "./PricingCards";
import { useAuth } from "../../lib/auth/context";
import { describeAuthError } from "../../lib/auth/errors";
import type { BillingPlan } from "../../lib/billing/api";
import { billingApi } from "../../lib/billing/api";
import { MARKETING_BILLING_PLANS } from "../../lib/billing/publicPlans";
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

export function UpgradeModal({ open, onClose }: UpgradeModalProps) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [plans, setPlans] = useState<BillingPlan[]>(MARKETING_BILLING_PLANS);
  const [paymentConsented, setPaymentConsented] = useState(false);
  const [checkoutError, setCheckoutError] = useState("");
  const [checkoutNotice, setCheckoutNotice] = useState("");
  const [alipaySession, setAlipaySession] = useState<AlipaySession | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    void billingApi
      .getPlans()
      .then((response) => setPlans(response.plans))
      .catch(() => setPlans(MARKETING_BILLING_PLANS));
  }, [open]);

  async function handleSelect(planId: string) {
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

  function handleAlipayPaid() {
    setAlipaySession(null);
    setCheckoutNotice(formatUiMessage(locale, "alipayQrPaid"));
    void billingApi
      .getPlans()
      .then((response) => setPlans(response.plans))
      .catch(() => {});
  }

  const alipayPlan = alipaySession
    ? plans.find((plan) => plan.plan_id === alipaySession.planId)
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
            {formatUiMessage(locale, "upgradeModal.subtitle")}
          </p>
          <PricingCards
            compact
            highlightTier="plus"
            locale={locale}
            plans={plans}
            onSelect={(planId) => void handleSelect(planId)}
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
