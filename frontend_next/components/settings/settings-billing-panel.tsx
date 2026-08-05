"use client";

import Link from "next/link";
import { useCallback, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { AlipayQrDialog } from "../billing/AlipayQrDialog";
import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { billingProviderForLocale } from "../../lib/billing/provider";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  createCheckoutSession,
  getSubscription,
  getWalletBalance,
  listTopupPacks,
  type TopupPack,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  formatDate,
  settingsKeys,
  subscriptionStatusLabel,
} from "./settings-shared";
import { UsageLimitPanel } from "./settings-usage-limit-panel";
import styles from "./settings-billing-panel.module.css";
import shared from "./settings-ui-shared.module.css";

function planLabel(planId: string | null | undefined): string | null {
  if (!planId) {
    return null;
  }
  const known: Record<string, string> = {
    free: "Free",
    plus: "Plus",
    pro: "Pro",
  };
  return known[planId.toLowerCase()] ?? planId;
}

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

export function BillingPanel({ hideManagePlan = false }: { hideManagePlan?: boolean } = {}) {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const queryClient = useQueryClient();
  const [topupBusyPack, setTopupBusyPack] = useState<string | null>(null);
  const [topupError, setTopupError] = useState("");
  const [alipayQr, setAlipayQr] = useState<{
    qrCode: string;
    orderId: string;
    planName: string;
    priceLabel: string;
  } | null>(null);

  const billingQuery = useQuery({
    queryKey: settingsKeys.billing(token),
    enabled: Boolean(token),
    queryFn: async () => {
      // Product metering truth is UsageLimitPanel (5h/7d). Plan catalog lives on /pricing only.
      try {
        const subscription = await getSubscription(token as string);
        return { subscription, partialError: "" };
      } catch (error) {
        return {
          subscription: null,
          partialError: formatUiMessage(locale, "settings.billing.failedData", {
            items: formatUiMessage(locale, "settings.billing.failedItem.subscription"),
          }),
        };
      }
    },
  });

  const walletQuery = useQuery({
    queryKey: [...settingsKeys.billing(token), "wallet"],
    enabled: Boolean(token),
    queryFn: async () => {
      const [wallet, packs] = await Promise.all([
        getWalletBalance(token as string),
        listTopupPacks(token as string),
      ]);
      return { wallet, packs };
    },
  });

  const errorMessage = billingQuery.error
    ? describeAuthError(
        formatUiMessage(locale, "settings.loadError"),
        billingQuery.error,
      )
    : (billingQuery.data?.partialError ?? "");

  const walletError = walletQuery.error
    ? describeAuthError(
        formatUiMessage(locale, "settings.billing.failedData", {
          items: formatUiMessage(locale, "settings.billing.failedItem.wallet"),
        }),
        walletQuery.error,
      )
    : "";

  const currentPlanName = planLabel(billingQuery.data?.subscription?.plan_id);

  const refreshWallet = useCallback(() => {
    void queryClient.invalidateQueries({
      queryKey: [...settingsKeys.billing(token), "wallet"],
    });
  }, [queryClient, token]);

  const startTopup = useCallback(
    async (pack: TopupPack) => {
      if (!token) {
        return;
      }
      setTopupError("");
      setTopupBusyPack(pack.pack_id);
      try {
        const provider = billingProviderForLocale(locale);
        const checkout = await createCheckoutSession(token, {
          kind: "wallet_topup",
          topup_pack_id: pack.pack_id,
          provider,
        });
        if (checkout.qr_code && checkout.order_id) {
          setAlipayQr({
            qrCode: checkout.qr_code,
            orderId: checkout.order_id,
            planName: pack.label_cny,
            priceLabel: pack.label_cny,
          });
          return;
        }
        if (checkout.url) {
          window.location.assign(checkout.url);
          return;
        }
        setTopupError(
          formatUiMessage(locale, "settings.billing.walletTopupFailed", {
            message: "empty checkout response",
          }),
        );
      } catch (error) {
        setTopupError(
          formatUiMessage(locale, "settings.billing.walletTopupFailed", {
            message: describeAuthError("top-up failed", error),
          }),
        );
      } finally {
        setTopupBusyPack(null);
      }
    },
    [locale, token],
  );

  return (
    <section className={shared.section}>
      <UsageLimitPanel />

      <section className={`app-inline-surface ${styles.planSection}`}>
        <div className={`app-inline-row ${styles.headerRow}`}>
          <div className={shared.headerText}>
            <h2 className={shared.flushTitle}>
              {formatUiMessage(locale, "settings.billing.walletTitle")}
            </h2>
            <p className={shared.mutedText}>
              {formatUiMessage(locale, "settings.billing.walletSubtitle")}
            </p>
          </div>
        </div>
        {walletError ? <p className="app-notice-banner">{walletError}</p> : null}
        {topupError ? <p className="app-notice-banner">{topupError}</p> : null}
        {walletQuery.isLoading ? (
          <p className={shared.mutedText}>
            {formatUiMessage(locale, "settings.billing.walletLoading")}
          </p>
        ) : walletQuery.data ? (
          <>
            <div className={`app-inline-surface ${styles.planCard}`} data-testid="wallet-balance">
              <div className={`app-inline-row ${shared.summaryRow}`}>
                <span>{formatUiMessage(locale, "settings.billing.walletBalanceLabel")}</span>
                <strong>
                  {formatFenAsYuan(walletQuery.data.wallet.balance_fen, locale)}
                </strong>
              </div>
              <div className={`app-inline-row ${shared.summaryRow}`}>
                <span>{formatUiMessage(locale, "settings.billing.walletLifetimePaidLabel")}</span>
                <strong>
                  {formatFenAsYuan(
                    walletQuery.data.wallet.lifetime_paid_topup_fen,
                    locale,
                  )}
                </strong>
              </div>
            </div>
            <div className={styles.topupRow} data-testid="wallet-topup-packs">
              <p className={shared.mutedText}>
                {formatUiMessage(locale, "settings.billing.walletTopupTitle")}
              </p>
              <div className={styles.topupButtons}>
                {walletQuery.data.packs.map((pack) => (
                  <button
                    key={pack.pack_id}
                    type="button"
                    className="app-button-secondary"
                    data-testid={`wallet-topup-${pack.pack_id}`}
                    disabled={topupBusyPack === pack.pack_id}
                    onClick={() => void startTopup(pack)}
                  >
                    {topupBusyPack === pack.pack_id
                      ? formatUiMessage(locale, "settings.billing.walletTopupLoading")
                      : formatUiMessage(locale, "settings.billing.walletTopupAction", {
                          label: pack.label_cny,
                        })}
                  </button>
                ))}
              </div>
            </div>
          </>
        ) : null}
      </section>

      <section className={`app-inline-surface ${styles.planSection}`}>
        <div className={`app-inline-row ${styles.headerRow}`}>
          <div className={shared.headerText}>
            <h2 className={shared.flushTitle}>
              {formatUiMessage(locale, "settings.billing.sectionTitle")}
            </h2>
            <p className={shared.mutedText}>
              {formatUiMessage(locale, "settings.billing.sectionSubtitle")}
            </p>
          </div>
          {hideManagePlan ? null : (
            <Link
              className="app-button-primary app-button-accent"
              data-testid="settings-manage-subscription"
              href="/pricing"
            >
              {formatUiMessage(locale, "settings.billing.managePlanAction")}
            </Link>
          )}
        </div>
        {errorMessage ? <p className="app-notice-banner">{errorMessage}</p> : null}
        {billingQuery.isLoading ? (
          <p className={shared.mutedText}>
            {formatUiMessage(locale, "settings.billing.loading")}
          </p>
        ) : (
          <div
            className={`app-inline-surface ${styles.planCard}`}
            data-testid="plan-display"
          >
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>
                {formatUiMessage(locale, "settings.billing.currentPlanLabel")}
              </span>
              <strong>
                {currentPlanName ??
                  formatUiMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>{formatUiMessage(locale, "settings.billing.statusLabel")}</span>
              <strong>
                {billingQuery.data?.subscription
                  ? subscriptionStatusLabel(locale, billingQuery.data.subscription.status)
                  : formatUiMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
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

      {alipayQr && token ? (
        <AlipayQrDialog
          token={token}
          qrCode={alipayQr.qrCode}
          orderId={alipayQr.orderId}
          planName={alipayQr.planName}
          priceLabel={alipayQr.priceLabel}
          locale={locale}
          onPaid={() => {
            setAlipayQr(null);
            refreshWallet();
          }}
          onCancel={() => setAlipayQr(null)}
        />
      ) : null}
    </section>
  );
}
