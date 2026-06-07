"use client";

import styles from "./PaywallModal.module.css";
import { UsageMeter } from "./UsageMeter";
import { PricingCards } from "./PricingCards";
import type { BillingPlan, UsageWindowBucket } from "../../lib/billing/api";
import { formatUiMessage, type UiLocale } from "../../lib/i18n/messages";

export type PaywallModalProps = {
  reason: "5h" | "7d";
  locale: UiLocale;
  plans: BillingPlan[];
  rolling5h: UsageWindowBucket;
  rolling7d: UsageWindowBucket;
  onSelect: (planId: string) => void;
  onContinueFree: () => void;
};

export function PaywallModal({
  reason,
  locale,
  plans,
  rolling5h,
  rolling7d,
  onSelect,
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
        <p className={styles.subtitle}>{formatUiMessage(locale, "paywallSubtitle")}</p>
        <PricingCards plans={plans} highlightTier="plus" locale={locale} onSelect={onSelect} compact />
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
