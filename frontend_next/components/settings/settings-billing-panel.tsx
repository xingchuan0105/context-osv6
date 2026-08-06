"use client";

import Link from "next/link";
import { useCallback, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { AlipayQrDialog } from "../billing/AlipayQrDialog";
import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { billingProviderForLocale } from "../../lib/billing/provider";
import { formatUiMessage } from "../../lib/i18n/messages";
import { getShareQuota } from "../../lib/share/client";
import {
  createCheckoutSession,
  getReferralStats,
  getSubscription,
  getWalletBalance,
  listProviderSecrets,
  listTopupPacks,
  revokeProviderSecret,
  upsertProviderSecret,
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
    plus_annual: "Plus (annual)",
    pro_annual: "Pro (annual)",
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

  const quotaQuery = useQuery({
    queryKey: [...settingsKeys.billing(token), "share-quota"],
    enabled: Boolean(token),
    queryFn: () => getShareQuota(token as string),
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

  const referralQuery = useQuery({
    queryKey: [...settingsKeys.billing(token), "referral"],
    enabled: Boolean(token),
    queryFn: () => getReferralStats(token as string),
  });

  const secretsQuery = useQuery({
    queryKey: [...settingsKeys.billing(token), "provider-secrets"],
    enabled: Boolean(token),
    queryFn: () => listProviderSecrets(token as string),
  });

  const [byokProvider, setByokProvider] = useState("deepseek");
  const [byokKey, setByokKey] = useState("");
  const [byokBaseUrl, setByokBaseUrl] = useState("");
  const [byokModelHint, setByokModelHint] = useState("");
  const [byokBusy, setByokBusy] = useState(false);
  const [byokError, setByokError] = useState("");
  const [referralCopied, setReferralCopied] = useState(false);

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
  const activeSecrets = (secretsQuery.data?.secrets ?? []).filter((s) => !s.revoked_at);
  const primarySecret = activeSecrets[0];

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

  const saveByok = useCallback(async () => {
    if (!token || !byokKey.trim()) return;
    setByokBusy(true);
    setByokError("");
    try {
      await upsertProviderSecret(token, {
        purpose: "llm",
        provider: byokProvider.trim() || "deepseek",
        api_key: byokKey.trim(),
        base_url: byokBaseUrl.trim() || undefined,
        model_hint: byokModelHint.trim() || undefined,
      });
      setByokKey("");
      void queryClient.invalidateQueries({
        queryKey: [...settingsKeys.billing(token), "provider-secrets"],
      });
    } catch (error) {
      setByokError(describeAuthError("save failed", error, locale));
    } finally {
      setByokBusy(false);
    }
  }, [byokBaseUrl, byokKey, byokModelHint, byokProvider, locale, queryClient, token]);

  const copyReferralCode = useCallback(async () => {
    const code = referralQuery.data?.code;
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code);
      setReferralCopied(true);
      window.setTimeout(() => setReferralCopied(false), 1500);
    } catch {
      /* ignore clipboard errors */
    }
  }, [referralQuery.data?.code]);

  return (
    <section className={shared.section}>
      {/* Membership summary — ADR-0010 primary surface */}
      <section className={`app-inline-surface ${styles.planSection}`} data-testid="membership-summary">
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
          <div className={`app-inline-surface ${styles.planCard}`} data-testid="plan-display">
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>{formatUiMessage(locale, "settings.billing.currentPlanLabel")}</span>
              <strong>
                {currentPlanName ?? formatUiMessage(locale, "settings.billing.notActive")}
              </strong>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`} data-testid="share-quota-row">
              <span>{formatUiMessage(locale, "settings.billing.shareQuotaLabel")}</span>
              <strong>
                {quotaQuery.data
                  ? formatUiMessage(locale, "settings.billing.shareQuotaValue", {
                      used: String(quotaQuery.data.used),
                      max: String(quotaQuery.data.max),
                    })
                  : quotaQuery.isLoading
                    ? "…"
                    : "—"}
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
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>{formatUiMessage(locale, "settings.billing.walletBalanceLabel")}</span>
              <strong>
                {walletQuery.data
                  ? formatFenAsYuan(walletQuery.data.wallet.balance_fen, locale)
                  : walletQuery.isLoading
                    ? "…"
                    : "—"}
              </strong>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>{formatUiMessage(locale, "settings.billing.byokTitle")}</span>
              <strong>
                {primarySecret
                  ? formatUiMessage(locale, "settings.billing.providerSummaryActive", {
                      provider: primarySecret.provider,
                    })
                  : formatUiMessage(locale, "settings.billing.providerSummaryNone")}
              </strong>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <Link className="app-link" href="/settings?tab=billing#usage-details">
                {formatUiMessage(locale, "settings.billing.usageDetailsLink")}
              </Link>
            </div>
          </div>
        )}
      </section>

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
              {formatUiMessage(locale, "settings.billing.byokTitle")}
            </h2>
            <p className={shared.mutedText}>
              {formatUiMessage(locale, "settings.billing.byokSubtitle")}
            </p>
          </div>
        </div>
        {byokError ? <p className="app-notice-banner">{byokError}</p> : null}
        <div className={`app-inline-surface ${styles.planCard}`}>
          <label className={shared.mutedText}>
            {formatUiMessage(locale, "settings.billing.byokProvider")}
            <input
              className="app-input"
              value={byokProvider}
              onChange={(e) => setByokProvider(e.target.value)}
              placeholder="deepseek"
            />
          </label>
          <label className={shared.mutedText}>
            {formatUiMessage(locale, "settings.billing.byokBaseUrl")}
            <input
              className="app-input"
              value={byokBaseUrl}
              onChange={(e) => setByokBaseUrl(e.target.value)}
              placeholder="https://api.deepseek.com"
            />
          </label>
          <label className={shared.mutedText}>
            {formatUiMessage(locale, "settings.billing.byokModelHint")}
            <input
              className="app-input"
              value={byokModelHint}
              onChange={(e) => setByokModelHint(e.target.value)}
              placeholder="deepseek-chat"
            />
          </label>
          <label className={shared.mutedText}>
            {formatUiMessage(locale, "settings.billing.byokApiKey")}
            <input
              className="app-input"
              type="password"
              value={byokKey}
              onChange={(e) => setByokKey(e.target.value)}
              placeholder="sk-…"
              autoComplete="off"
            />
          </label>
          <button
            type="button"
            className="app-button-primary"
            disabled={byokBusy || !byokKey.trim()}
            onClick={() => void saveByok()}
          >
            {byokBusy ? "…" : formatUiMessage(locale, "settings.billing.byokSave")}
          </button>
        </div>
        {(secretsQuery.data?.secrets ?? []).length > 0 ? (
          <ul className={shared.mutedText}>
            {secretsQuery.data!.secrets.map((s) => (
              <li key={s.id} style={s.revoked_at ? { opacity: 0.55 } : undefined}>
                {s.provider} · {s.purpose}
                {s.model_hint ? ` · ${s.model_hint}` : ""} · {s.key_fingerprint}{" "}
                {s.revoked_at ? (
                  <span>{formatUiMessage(locale, "settings.billing.byokRevoked")}</span>
                ) : (
                  <button
                    type="button"
                    className="app-link"
                    onClick={() => {
                      if (!token) return;
                      void revokeProviderSecret(token, s.id).then(() =>
                        queryClient.invalidateQueries({
                          queryKey: [...settingsKeys.billing(token), "provider-secrets"],
                        }),
                      );
                    }}
                  >
                    {formatUiMessage(locale, "settings.billing.byokRevoke")}
                  </button>
                )}
              </li>
            ))}
          </ul>
        ) : null}
      </section>

      <section className={`app-inline-surface ${styles.planSection}`}>
        <div className={`app-inline-row ${styles.headerRow}`}>
          <div className={shared.headerText}>
            <h2 className={shared.flushTitle}>
              {formatUiMessage(locale, "settings.billing.referralTitle")}
            </h2>
            <p className={shared.mutedText}>
              {formatUiMessage(locale, "settings.billing.referralSubtitle")}
            </p>
          </div>
        </div>
        {referralQuery.data ? (
          <div className={`app-inline-surface ${styles.planCard}`} data-testid="referral-stats">
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>{formatUiMessage(locale, "settings.billing.referralCodeLabel")}</span>
              <strong>{referralQuery.data.code}</strong>
              <button type="button" className="app-link" onClick={() => void copyReferralCode()}>
                {referralCopied
                  ? formatUiMessage(locale, "settings.billing.referralCopied")
                  : formatUiMessage(locale, "settings.billing.referralCopy")}
              </button>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <span>{formatUiMessage(locale, "settings.billing.referralRewardedLabel")}</span>
              <strong>
                {referralQuery.data.rewarded_count} / {referralQuery.data.quota}
              </strong>
            </div>
          </div>
        ) : referralQuery.isLoading ? (
          <p className={shared.mutedText}>…</p>
        ) : null}
      </section>

      <div id="usage-details">
        <UsageLimitPanel />
      </div>

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
