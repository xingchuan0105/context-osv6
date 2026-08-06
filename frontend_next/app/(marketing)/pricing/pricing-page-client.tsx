"use client";

import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

import { AlipayQrDialog } from "@/components/billing/AlipayQrDialog";
import { PricingCards } from "@/components/billing/PricingCards";
import ConsentCheckbox from "@/components/legal/ConsentCheckbox";
import { createCheckoutSession } from "@/lib/settings/client";
import { recordPaymentLegalAcceptance } from "@/lib/legal/client";
import { describeAuthError } from "@/lib/auth/errors";
import { useAuth } from "@/lib/auth/context";
import type { BillingPlan } from "@/lib/billing/api";
import { billingApi } from "@/lib/billing/api";
import { MARKETING_BILLING_PLANS, plansForInterval } from "@/lib/billing/publicPlans";
import { billingProviderForLocale, planPriceLabel } from "@/lib/billing/provider";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";
import styles from "./pricing.module.css";

type AlipaySession = {
  qrCode: string;
  orderId: string;
  planId: string;
};

type BillingInterval = "month" | "year";

export function PricingPageClient() {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [allPlans, setAllPlans] = useState<BillingPlan[]>(MARKETING_BILLING_PLANS);
  const [interval, setInterval] = useState<BillingInterval>("month");
  const [paymentConsented, setPaymentConsented] = useState(false);
  const [checkoutError, setCheckoutError] = useState("");
  const [checkoutNotice, setCheckoutNotice] = useState("");
  const [alipaySession, setAlipaySession] = useState<AlipaySession | null>(null);

  useEffect(() => {
    void billingApi
      .getPlans()
      .then((response) => {
        const remote = response.plans ?? [];
        // Merge marketing annual SKUs if API omits them (checkout still accepts plan_id).
        const ids = new Set(remote.map((p) => p.plan_id));
        const extras = MARKETING_BILLING_PLANS.filter((p) => !ids.has(p.plan_id));
        setAllPlans(extras.length ? [...remote, ...extras] : remote);
      })
      .catch(() => setAllPlans(MARKETING_BILLING_PLANS));
  }, []);

  const plans = useMemo(() => plansForInterval(allPlans, interval), [allPlans, interval]);

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
      .then((response) => setAllPlans(response.plans))
      .catch(() => {});
  }

  const alipayPlan = alipaySession
    ? allPlans.find((plan) => plan.plan_id === alipaySession.planId)
    : undefined;

  return (
    <div className={styles.page}>
      <header className={styles.header}>
        <h1 className={styles.title}>{formatUiMessage(locale, "pricingTitle")}</h1>
        <p className={styles.subtitle}>{formatUiMessage(locale, "pricingSubtitle")}</p>
        <div className={styles.billingToggle} role="group" aria-label="billing interval">
          <button
            type="button"
            className={`${styles.toggleButton} ${interval === "month" ? styles.toggleActive : ""}`}
            onClick={() => setInterval("month")}
          >
            {formatUiMessage(locale, "pricingMonthly")}
          </button>
          <button
            type="button"
            className={`${styles.toggleButton} ${interval === "year" ? styles.toggleActive : ""}`}
            onClick={() => setInterval("year")}
            data-testid="pricing-interval-yearly"
          >
            {formatUiMessage(locale, "pricingYearly")}
            <span className={styles.toggleHintInline}>
              {formatUiMessage(locale, "pricingYearlyHint")}
            </span>
          </button>
        </div>
      </header>

      <PricingCards plans={plans} highlightTier="plus" locale={locale} onSelect={handleSelect} />

      <section className={styles.addon} data-testid="pricing-wallet-addon">
        <h2 className={styles.faqTitle}>{formatUiMessage(locale, "pricingWalletAddonTitle")}</h2>
        <p className={styles.addonBody}>{formatUiMessage(locale, "pricingWalletAddonBody")}</p>
      </section>

      {auth.token ? (
        <div className={styles.consentSection}>
          <ConsentCheckbox onConsentChange={setPaymentConsented} />
          {checkoutError ? (
            <p className={styles.checkoutError} role="alert">
              {checkoutError}
            </p>
          ) : null}
          {checkoutNotice ? (
            <p className={styles.checkoutNotice} role="status">
              {checkoutNotice}
            </p>
          ) : null}
        </div>
      ) : null}

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

      <section className={styles.faq}>
        <h2 className={styles.faqTitle}>{formatUiMessage(locale, "pricingFaqTitle")}</h2>
        <details className={styles.faqItem}>
          <summary>{formatUiMessage(locale, "pricingFaqToken")}</summary>
          <p>{formatUiMessage(locale, "pricingFaqTokenAnswer")}</p>
        </details>
        <details className={styles.faqItem}>
          <summary>{formatUiMessage(locale, "pricingFaqReset")}</summary>
          <p>{formatUiMessage(locale, "pricingFaqResetAnswer")}</p>
        </details>
        <details className={styles.faqItem}>
          <summary>{formatUiMessage(locale, "pricingFaqUpgrade")}</summary>
          <p>{formatUiMessage(locale, "pricingFaqUpgradeAnswer")}</p>
        </details>
      </section>

      <section
        className={styles.faq}
        style={{ marginTop: "1.5rem" }}
        data-testid="pricing-desktop-crosslink"
      >
        <h2 className={styles.faqTitle}>{formatUiMessage(locale, "pricingDesktopCrossTitle")}</h2>
        <p style={{ color: "hsl(var(--muted-foreground))", marginBottom: "0.75rem" }}>
          {formatUiMessage(locale, "pricingDesktopCrossBody")}
        </p>
        <a className="app-button-secondary" href="/desktop">
          {formatUiMessage(locale, "pricingDesktopCrossCta")}
        </a>
      </section>
    </div>
  );
}
