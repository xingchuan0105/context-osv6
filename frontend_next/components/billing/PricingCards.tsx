"use client";

import styles from "./PricingCards.module.css";
import type { BillingPlan } from "../../lib/billing/api";
import { formatUiMessage, type UiLocale } from "../../lib/i18n/messages";

export type PricingCardsProps = {
  plans: BillingPlan[];
  highlightTier: "free" | "plus" | "pro";
  locale: UiLocale;
  onSelect: (planId: string) => void;
  compact?: boolean;
};

export function PricingCards({ plans, highlightTier, locale, onSelect, compact = false }: PricingCardsProps) {
  return (
    <div className={`${styles.grid} ${compact ? styles.compactGrid : ""}`}>
      {plans.map((plan) => {
        const isHighlight = plan.plan_id === highlightTier;
        const isCurrent = plan.current;
        const buttonLabel = isCurrent
          ? formatUiMessage(locale, "currentPlan")
          : plan.plan_id === "free"
            ? formatUiMessage(locale, "upgradeContinueFree")
            : formatUiMessage(locale, "pricingUpgradeTo", { name: plan.name });
        return (
          <div
            key={plan.plan_id}
            className={`${styles.card} ${isHighlight ? styles.highlight : ""} ${compact ? styles.compact : ""}`}
          >
            {isHighlight && (
              <div className={styles.badge}>{formatUiMessage(locale, "pricingTierPlusBadge")}</div>
            )}
            <h3 className={styles.name}>{plan.name}</h3>
            <div className={styles.prices}>
              <div className={styles.priceCny}>{plan.price_label_cny}</div>
              <div className={styles.priceUsd}>{plan.price_label_usd}</div>
            </div>
            <div className={styles.description}>{plan.description}</div>
            {!compact && (
              <div className={styles.interval}>{formatUiMessage(locale, "pricingMonthlyInterval")}</div>
            )}
            <button
              type="button"
              className={isHighlight ? styles.primaryButton : styles.secondaryButton}
              onClick={() => onSelect(plan.plan_id)}
              disabled={isCurrent || !plan.checkout_available}
            >
              {buttonLabel}
            </button>
          </div>
        );
      })}
    </div>
  );
}
