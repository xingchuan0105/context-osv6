"use client";

import styles from "./PaywallModal.module.css";
import { UsageMeter } from "./UsageMeter";
import { PricingCards } from "./PricingCards";
import type { BillingPlan, UsageWindowBucket } from "../../lib/billing/api";

export type PaywallModalProps = {
  reason: "5h" | "7d";
  plans: BillingPlan[];
  rolling5h: UsageWindowBucket;
  rolling7d: UsageWindowBucket;
  onSelect: (planId: string) => void;
  onContinueFree: () => void;
};

export function PaywallModal({
  reason,
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
          {reason === "5h" ? "5h 用量已达上限" : "7d 用量已达上限"}
        </h1>
        <UsageMeter
          variant="compact"
          planId="free"
          rolling5h={rolling5h}
          rolling7d={rolling7d}
          softLimitHit={{ rolling_5h: true, rolling_7d: false }}
          hardLimitHit={{ rolling_5h: reason === "5h", rolling_7d: reason === "7d" }}
        />
        <p className={styles.subtitle}>Free → Plus，解锁 10× 用量</p>
        <PricingCards plans={plans} highlightTier="plus" onSelect={onSelect} compact />
        <div className={styles.footer}>
          <button
            type="button"
            className={styles.continueButton}
            data-testid="paywall-continue-free"
            onClick={onContinueFree}
          >
            继续 Free
          </button>
          <span className={styles.resetHint}>限额自动重置，请关注使用节奏</span>
        </div>
      </div>
    </div>
  );
}
