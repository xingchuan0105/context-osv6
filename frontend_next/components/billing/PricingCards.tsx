"use client";

import styles from "./PricingCards.module.css";
import type { BillingPlan } from "../../lib/billing/api";

export type PricingCardsProps = {
  plans: BillingPlan[];
  highlightTier: "free" | "plus" | "pro";
  onSelect: (planId: string) => void;
  compact?: boolean;
};

export function PricingCards({ plans, highlightTier, onSelect, compact = false }: PricingCardsProps) {
  return (
    <div className={`${styles.grid} ${compact ? styles.compactGrid : ""}`}>
      {plans.map((plan) => {
        const isHighlight = plan.plan_id === highlightTier;
        const isCurrent = plan.current;
        return (
          <div
            key={plan.plan_id}
            className={`${styles.card} ${isHighlight ? styles.highlight : ""} ${compact ? styles.compact : ""}`}
          >
            {isHighlight && <div className={styles.badge}>推荐</div>}
            <h3 className={styles.name}>{plan.name}</h3>
            <div className={styles.prices}>
              <div className={styles.priceCny}>{plan.price_label_cny}</div>
              <div className={styles.priceUsd}>{plan.price_label_usd}</div>
            </div>
            <div className={styles.description}>{plan.description}</div>
            {!compact && <div className={styles.interval}>月付</div>}
            <button
              type="button"
              className={isHighlight ? styles.primaryButton : styles.secondaryButton}
              onClick={() => onSelect(plan.plan_id)}
              disabled={isCurrent || !plan.checkout_available}
            >
              {isCurrent ? "当前套餐" : plan.plan_id === "free" ? "继续 Free" : `升级 ${plan.name}`}
            </button>
          </div>
        );
      })}
    </div>
  );
}
