"use client";

import Link from "next/link";

import styles from "./PaywallModal.module.css";
import { UsageMeter } from "./UsageMeter";
import type { UsageWindowBucket } from "../../lib/billing/api";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";
import { appNavHref } from "../../lib/navigation/nav-config";

export type PaywallModalProps = {
  reason: "5h" | "7d";
  locale: UiLocale;
  rolling5h: UsageWindowBucket;
  rolling7d: UsageWindowBucket;
  onContinueFree: () => void;
};

/**
 * Rate-limit recovery explainer (PRODUCT_IA §4): shows the usage window and
 * routes upgrades to the canonical checkout at /pricing — never hosts its own
 * checkout (anti-pattern §7-2: 第三完成页).
 */
export function PaywallModal({
  reason,
  locale,
  rolling5h,
  rolling7d,
  onContinueFree,
}: PaywallModalProps) {
  return (
    <div className={styles.overlay}>
      <div className={styles.modal} role="dialog" aria-modal="true">
        <h1 className={styles.title}>
          {formatUiMessage(locale, reason === "5h" ? "paywallTitle5h" : "paywallTitle7d")}
        </h1>
        <UsageMeter
          variant="compact"
          locale={locale}
          planId="free"
          rolling5h={rolling5h}
          rolling7d={rolling7d}
          softLimitHit={{ rolling_5h: true, rolling_7d: false }}
          hardLimitHit={{ rolling_5h: reason === "5h", rolling_7d: reason === "7d" }}
        />
        <p className={styles.subtitle}>
          {formatUiMessage(locale, reason === "5h" ? "paywallSubtitle5h" : "paywallSubtitle7d")}
        </p>
        <Link
          className="app-button-primary"
          data-testid="paywall-view-plans"
          href={appNavHref("pricing")}
        >
          {formatUiMessage(locale, "paywallViewPlans")}
        </Link>
        <div className={styles.footer}>
          <button
            type="button"
            className={styles.continueButton}
            data-testid="paywall-continue-free"
            onClick={onContinueFree}
          >
            {formatUiMessage(locale, "paywallContinueFree")}
          </button>
          <span className={styles.resetHint}>{formatUiMessage(locale, "paywallResetHint")}</span>
        </div>
      </div>
    </div>
  );
}
