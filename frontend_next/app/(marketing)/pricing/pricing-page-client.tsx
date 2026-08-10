"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";

import { AlipayQrDialog } from "@/components/billing/AlipayQrDialog";
import { PricingCards } from "@/components/billing/PricingCards";
import ConsentCheckbox from "@/components/legal/ConsentCheckbox";
import {
  createCheckoutSession,
  getWalletBalance,
  listTopupPacks,
  type TopupPack,
} from "@/lib/settings/client";
import { recordPaymentLegalAcceptance } from "@/lib/legal/client";
import { describeAuthError } from "@/lib/auth/errors";
import { useAuth } from "@/lib/auth/context";
import type { BillingPlan } from "@/lib/billing/api";
import { billingApi } from "@/lib/billing/api";
import { MARKETING_BILLING_PLANS, plansForInterval } from "@/lib/billing/publicPlans";
import { billingProviderForLocale, planPriceLabelForProvider, type ActiveBillingProvider } from "@/lib/billing/provider";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";
import styles from "./pricing.module.css";

type AlipaySession = {
  qrCode: string;
  orderId: string;
  planName: string;
  priceLabel: string;
  /** After paid: refresh subscription plans and/or wallet. */
  kind: "subscription" | "wallet_topup";
};

type BillingInterval = "month" | "year";

/** Marketing fallback when packs API is unavailable (logged-out or error). */
const DEFAULT_TOPUP_PACKS: TopupPack[] = [
  { pack_id: "topup_50", amount_fen: 5000, amount_yuan: 50, label_cny: "¥50" },
  { pack_id: "topup_100", amount_fen: 10000, amount_yuan: 100, label_cny: "¥100" },
  { pack_id: "topup_200", amount_fen: 20000, amount_yuan: 200, label_cny: "¥200" },
];

function formatFenAsYuan(fen: number, locale: string): string {
  const yuan = fen / 100;
  try {
    return new Intl.NumberFormat(locale === "zh-CN" ? "zh-CN" : "en-US", {
      style: "currency",
      currency: "CNY",
      minimumFractionDigits: 2,
    }).format(yuan);
  } catch {
    return `¥${yuan.toFixed(2)}`;
  }
}

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
  // Payment channel is decoupled from locale: locale only sets the default
  // (zh-CN → alipay, en → creem); both channels stay selectable.
  const [payProvider, setPayProvider] = useState<ActiveBillingProvider>(() =>
    billingProviderForLocale(locale),
  );

  useEffect(() => {
    setPayProvider(billingProviderForLocale(locale));
  }, [locale]);

  const [packs, setPacks] = useState<TopupPack[]>(DEFAULT_TOPUP_PACKS);
  const [balanceFen, setBalanceFen] = useState<number | null>(null);
  const [topupBusyPack, setTopupBusyPack] = useState<string | null>(null);
  const [topupError, setTopupError] = useState("");

  useEffect(() => {
    void billingApi
      .getPlans()
      .then((response) => {
        const remote = response.plans ?? [];
        const ids = new Set(remote.map((p) => p.plan_id));
        const extras = MARKETING_BILLING_PLANS.filter((p) => !ids.has(p.plan_id));
        setAllPlans(extras.length ? [...remote, ...extras] : remote);
      })
      .catch(() => setAllPlans(MARKETING_BILLING_PLANS));
  }, []);

  const refreshWallet = useCallback(async () => {
    if (!auth.token) {
      setBalanceFen(null);
      setPacks(DEFAULT_TOPUP_PACKS);
      return;
    }
    try {
      const [wallet, remotePacks] = await Promise.all([
        getWalletBalance(auth.token),
        listTopupPacks(auth.token),
      ]);
      setBalanceFen(wallet.balance_fen);
      if (remotePacks.length > 0) {
        setPacks(remotePacks);
      }
    } catch {
      /* keep marketing packs; balance optional */
    }
  }, [auth.token]);

  useEffect(() => {
    void refreshWallet();
  }, [refreshWallet]);

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
        provider: payProvider,
      });
      if (checkout.qr_code && checkout.order_id) {
        const plan = allPlans.find((p) => p.plan_id === planId);
        setAlipaySession({
          qrCode: checkout.qr_code,
          orderId: checkout.order_id,
          planName: plan?.name ?? planId,
          // Alipay bills CNY — show the CNY label even under en locale.
          priceLabel: plan ? planPriceLabelForProvider(plan, "alipay") : "",
          kind: "subscription",
        });
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

  async function handleTopup(pack: TopupPack) {
    if (!auth.token) {
      router.push(`/login?next=${encodeURIComponent("/pricing#topup")}`);
      return;
    }

    setTopupError("");
    setCheckoutNotice("");
    setTopupBusyPack(pack.pack_id);
    try {
      // Wallet top-up matches settings billing: no extra consent gate beyond auth.
      const checkout = await createCheckoutSession(auth.token, {
        kind: "wallet_topup",
        topup_pack_id: pack.pack_id,
        provider: payProvider,
      });
      if (checkout.qr_code && checkout.order_id) {
        setAlipaySession({
          qrCode: checkout.qr_code,
          orderId: checkout.order_id,
          planName: pack.label_cny,
          priceLabel: pack.label_cny,
          kind: "wallet_topup",
        });
        return;
      }
      if (checkout.url) {
        router.push(checkout.url);
        return;
      }
      setTopupError(formatUiMessage(locale, "pricingTopupFailed", { message: "empty checkout" }));
    } catch (error) {
      setTopupError(
        formatUiMessage(locale, "pricingTopupFailed", {
          message: describeAuthError("top-up failed", error, locale),
        }),
      );
    } finally {
      setTopupBusyPack(null);
    }
  }

  function handleAlipayPaid() {
    const kind = alipaySession?.kind;
    setAlipaySession(null);
    setCheckoutNotice(formatUiMessage(locale, "alipayQrPaid"));
    if (kind === "subscription" || !kind) {
      void billingApi
        .getPlans()
        .then((response) => setAllPlans(response.plans))
        .catch(() => {});
    }
    if (kind === "wallet_topup" || kind === "subscription") {
      void refreshWallet();
    }
  }

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
        <div
          className={styles.billingToggle}
          role="group"
          aria-label={formatUiMessage(locale, "pricingPayMethodLabel")}
          data-testid="pay-method-selector"
        >
          <span className={styles.payMethodLabel}>
            {formatUiMessage(locale, "pricingPayMethodLabel")}
          </span>
          {(["alipay", "creem"] as const).map((channel) => {
            const isActive = payProvider === channel;
            const isRecommended = billingProviderForLocale(locale) === channel;
            return (
              <button
                key={channel}
                type="button"
                className={`${styles.toggleButton} ${isActive ? styles.toggleActive : ""}`}
                aria-pressed={isActive}
                data-testid={`pay-method-${channel}`}
                onClick={() => setPayProvider(channel)}
              >
                {channel === "alipay"
                  ? formatUiMessage(locale, "pricingPayMethodAlipay")
                  : "Creem"}
                <span className={styles.toggleHintInline}>
                  {formatUiMessage(
                    locale,
                    channel === "alipay" ? "pricingPayMethodAlipayHint" : "pricingPayMethodCreemHint",
                  )}
                </span>
                {isRecommended ? (
                  <span className={styles.recoBadge}>
                    {formatUiMessage(locale, "pricingPayMethodRecommended")}
                  </span>
                ) : null}
              </button>
            );
          })}
        </div>
      </header>

      <section aria-labelledby="pricing-membership-heading">
        <h2 id="pricing-membership-heading" className={styles.sectionTitle}>
          {formatUiMessage(locale, "pricingMembershipTitle")}
        </h2>
        <p className={styles.sectionLead}>{formatUiMessage(locale, "pricingMembershipLead")}</p>
        <PricingCards
          plans={plans}
          highlightTier="plus"
          locale={locale}
          onSelect={handleSelect}
          priceProvider={payProvider}
        />
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
          planName={alipaySession.planName}
          priceLabel={alipaySession.priceLabel}
          locale={locale}
          onPaid={handleAlipayPaid}
          onCancel={() => setAlipaySession(null)}
        />
      ) : null}

      <section
        id="topup"
        className={styles.topupSection}
        data-testid="pricing-topup-section"
        aria-labelledby="pricing-topup-heading"
      >
        <div className={styles.topupCopy}>
          <h2 id="pricing-topup-heading" className={styles.sectionTitle}>
            {formatUiMessage(locale, "pricingTopupTitle")}
          </h2>
          <p className={styles.sectionLead}>{formatUiMessage(locale, "pricingTopupBody")}</p>
          <ul className={styles.topupList}>
            <li>{formatUiMessage(locale, "pricingTopupPoint1")}</li>
            <li>{formatUiMessage(locale, "pricingTopupPoint2")}</li>
            <li>{formatUiMessage(locale, "pricingTopupPoint3")}</li>
          </ul>
          {auth.token && balanceFen != null ? (
            <p className={styles.balanceLine} data-testid="pricing-wallet-balance">
              {formatUiMessage(locale, "pricingWalletBalance", {
                balance: formatFenAsYuan(balanceFen, locale),
              })}
            </p>
          ) : null}
          {topupError ? (
            <p className={styles.checkoutError} role="alert">
              {topupError}
            </p>
          ) : null}
        </div>

        <div className={styles.topupActions}>
          <p className={styles.packLabel}>{formatUiMessage(locale, "pricingTopupPacksLabel")}</p>
          {payProvider === "creem" ? (
            <p className={styles.topupChannelHint} data-testid="topup-channel-hint">
              {formatUiMessage(locale, "pricingTopupCreemHint")}
            </p>
          ) : null}
          <div className={styles.packGrid} data-testid="pricing-topup-packs">
            {packs.map((pack) => (
              <button
                key={pack.pack_id}
                type="button"
                className="app-button-secondary"
                data-testid={`pricing-topup-${pack.pack_id}`}
                disabled={topupBusyPack === pack.pack_id}
                onClick={() => void handleTopup(pack)}
              >
                {topupBusyPack === pack.pack_id
                  ? formatUiMessage(locale, "pricingTopupLoading")
                  : formatUiMessage(locale, "pricingTopupPackAction", {
                      label: pack.label_cny,
                    })}
              </button>
            ))}
          </div>
          {!auth.token ? (
            <p className={styles.topupLoginHint}>
              {formatUiMessage(locale, "pricingTopupLoginHint")}
            </p>
          ) : null}
          <Link className="app-button-ghost" href="/settings?tab=providers">
            {formatUiMessage(locale, "pricingTopupByokCta")}
          </Link>
        </div>
      </section>

      <section className={styles.faq}>
        <h2 className={styles.faqTitle}>{formatUiMessage(locale, "pricingFaqTitle")}</h2>
        <div className={styles.faqGrid}>
          <details className={styles.faqItem}>
            <summary>{formatUiMessage(locale, "pricingFaqToken")}</summary>
            <p>{formatUiMessage(locale, "pricingFaqTokenAnswer")}</p>
          </details>
          <details className={styles.faqItem}>
            <summary>{formatUiMessage(locale, "pricingFaqTopup")}</summary>
            <p>{formatUiMessage(locale, "pricingFaqTopupAnswer")}</p>
          </details>
          <details className={styles.faqItem}>
            <summary>{formatUiMessage(locale, "pricingFaqReset")}</summary>
            <p>{formatUiMessage(locale, "pricingFaqResetAnswer")}</p>
          </details>
          <details className={styles.faqItem}>
            <summary>{formatUiMessage(locale, "pricingFaqUpgrade")}</summary>
            <p>{formatUiMessage(locale, "pricingFaqUpgradeAnswer")}</p>
          </details>
        </div>
      </section>
    </div>
  );
}
