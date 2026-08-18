"use client";

import Link from "next/link";
import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { getShareQuota } from "../../lib/share/client";
import {
  getReferralStats,
  getSubscription,
  getWalletBalance,
  listProviderSecrets,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  formatDate,
  settingsKeys,
  subscriptionStatusLabel,
} from "./settings-shared";
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

/**
 * Billing summary. Canonical checkout lives on /pricing (membership) and
 * /pricing#topup (wallet) per PRODUCT_IA.md — this panel does not host a second top-up checkout.
 *
 * Manage-plan CTA sits inside the plan card (not modal footer). Pass
 * `onManagePlan` to open the marketing upgrade explainer; otherwise link to /pricing.
 */
export function BillingPanel({
  onManagePlan,
}: {
  onManagePlan?: () => void;
} = {}) {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const [referralCopied, setReferralCopied] = useState(false);

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
    queryFn: () => getWalletBalance(token as string),
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
      {/* 用量信息 summary — ADR-0010 primary surface; DeepSeek-style stat cards lead */}
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
        </div>
        {errorMessage ? <p className="app-notice-banner">{errorMessage}</p> : null}
        {/* DeepSeek-style stat cards lead: balance (with top-up CTA) + lifetime top-ups. */}
        {walletError ? <p className="app-notice-banner">{walletError}</p> : null}
        <div className={styles.statRow} data-testid="usage-stat-cards">
          <div className={`app-inline-surface ${styles.statCard}`} data-testid="wallet-balance">
            <span className={styles.statLabel}>
              {formatUiMessage(locale, "settings.billing.walletBalanceLabel")}
            </span>
            {walletQuery.isLoading ? (
              <span className={shared.mutedText}>
                {formatUiMessage(locale, "settings.billing.walletLoading")}
              </span>
            ) : walletQuery.data ? (
              <strong className={styles.statValue}>
                {formatFenAsYuan(walletQuery.data.balance_fen, locale)}
              </strong>
            ) : (
              <strong className={styles.statValue}>—</strong>
            )}
            <Link
              className="app-button-secondary"
              data-testid="settings-wallet-topup-link"
              href="/pricing#topup"
            >
              {formatUiMessage(locale, "settings.billing.walletTopupCta")}
            </Link>
          </div>
          <div className={`app-inline-surface ${styles.statCard}`} data-testid="wallet-lifetime-topup">
            <span className={styles.statLabel}>
              {formatUiMessage(locale, "settings.billing.walletLifetimePaidLabel")}
            </span>
            {walletQuery.isLoading ? (
              <span className={shared.mutedText}>
                {formatUiMessage(locale, "settings.billing.walletLoading")}
              </span>
            ) : walletQuery.data ? (
              <strong className={styles.statValue}>
                {formatFenAsYuan(walletQuery.data.lifetime_paid_topup_fen, locale)}
              </strong>
            ) : (
              <strong className={styles.statValue}>—</strong>
            )}
          </div>
        </div>
        <p className={shared.mutedText} data-testid="wallet-topup-canonical-hint">
          {formatUiMessage(locale, "settings.billing.walletTopupHint")}
        </p>
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
              <span>{formatUiMessage(locale, "settings.billing.byokTitle")}</span>
              <strong>
                {primarySecret
                  ? formatUiMessage(locale, "settings.billing.providerSummaryActive", {
                      provider: primarySecret.provider,
                    })
                  : formatUiMessage(locale, "settings.billing.providerSummaryNone")}
              </strong>
              <Link className="app-link" href="/settings?tab=providers">
                {formatUiMessage(locale, "settingsBillingManage")}
              </Link>
            </div>
            <div className={`app-inline-row ${shared.summaryRow}`}>
              <Link className="app-link" href="/settings/usage" data-testid="settings-usage-details-link">
                {formatUiMessage(locale, "settings.billing.usageDetailsLink")}
              </Link>
            </div>
            {/* Plan-card CTA: upgrade / manage → /pricing (or parent upgrade explainer). */}
            <div className={styles.planActions} data-testid="plan-manage-actions">
              {onManagePlan ? (
                <button
                  className="app-button-primary app-button-accent"
                  data-testid="settings-manage-subscription"
                  type="button"
                  onClick={onManagePlan}
                >
                  {formatUiMessage(locale, "settings.billing.managePlanAction")}
                </button>
              ) : (
                <Link
                  className="app-button-primary app-button-accent"
                  data-testid="settings-manage-subscription"
                  href="/pricing"
                >
                  {formatUiMessage(locale, "settings.billing.managePlanAction")}
                </Link>
              )}
            </div>
          </div>
        )}
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
    </section>
  );
}
