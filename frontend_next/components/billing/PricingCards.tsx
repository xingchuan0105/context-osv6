"use client";

import styles from "./PricingCards.module.css";
import type { BillingPlan } from "../../lib/billing/api";
import { getPlanShareSlots } from "../../lib/billing/planLimits";
import {
  planPriceLabelForProvider,
  type ActiveBillingProvider,
} from "../../lib/billing/provider";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiMessageKey } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";

export type PricingCardsProps = {
  plans: BillingPlan[];
  highlightTier: "free" | "plus" | "pro";
  locale: UiLocale;
  onSelect: (planId: string) => void;
  compact?: boolean;
  /** Marketing dialog: CTA opens formal pricing page instead of in-place checkout. */
  actionMode?: "checkout" | "details";
  /**
   * When set (checkout surfaces), cards show only the selected channel's
   * currency. Unset = marketing dual CNY+USD display.
   */
  priceProvider?: ActiveBillingProvider;
};

function baseTier(planId: string): string {
  return planId.replace(/_annual$/, "");
}

export function PricingCards({
  plans,
  highlightTier,
  locale,
  onSelect,
  compact = false,
  actionMode = "checkout",
  priceProvider,
}: PricingCardsProps) {
  const detailsMode = actionMode === "details";
  const descriptionKeyByPlan: Record<string, UiMessageKey> = {
    free: "pricingPlanFreeDescription",
    plus: "pricingPlanPlusDescription",
    pro: "pricingPlanProDescription",
    plus_annual: "pricingPlanPlusDescription",
    pro_annual: "pricingPlanProDescription",
  };
  return (
    <div className={`${styles.grid} ${compact ? styles.compactGrid : ""}`}>
      {plans.map((plan) => {
        const tier = baseTier(plan.plan_id);
        const isHighlight = tier === highlightTier;
        const isCurrent = plan.current;
        const isPro = tier === "pro";
        const shareSlots = getPlanShareSlots(plan.plan_id);
        const descriptionKey = descriptionKeyByPlan[plan.plan_id] ?? descriptionKeyByPlan[tier];
        const description = descriptionKey
          ? formatUiMessage(locale, descriptionKey)
          : plan.description;
        const isAnnual = plan.plan_id.endsWith("_annual") || plan.interval === "year";
        const buttonLabel = detailsMode
          ? formatUiMessage(locale, "pricingViewDetails", { name: plan.name })
          : isCurrent
            ? formatUiMessage(locale, "currentPlan")
            : plan.plan_id === "free"
              ? formatUiMessage(locale, "upgradeContinueFree")
              : formatUiMessage(locale, "pricingUpgradeTo", { name: plan.name });
        return (
          <div
            key={plan.plan_id}
            className={`${styles.card} ${isHighlight ? styles.highlight : ""} ${isPro && !isHighlight ? styles.pro : ""} ${compact ? styles.compact : ""}`}
            data-plan-id={plan.plan_id}
          >
            {isHighlight && (
              <div className={`${styles.badge} ${isPro ? styles.badgePro : ""}`}>
                {formatUiMessage(locale, "pricingTierPlusBadge")}
              </div>
            )}
            <h3 className={styles.name}>{plan.name}</h3>
            <div className={styles.prices}>
              {priceProvider ? (
                <div className={styles.priceCny} data-testid={`price-${plan.plan_id}`}>
                  {planPriceLabelForProvider(plan, priceProvider)}
                </div>
              ) : (
                <>
                  <div className={styles.priceCny}>{plan.price_label_cny}</div>
                  <div className={styles.priceUsd}>{plan.price_label_usd}</div>
                </>
              )}
            </div>
            {shareSlots != null && !compact && (
              <ul className={styles.limits}>
                <li data-testid={`share-slots-${plan.plan_id}`}>
                  {formatUiMessage(locale, "pricingShareSlots", { n: String(shareSlots) })}
                </li>
              </ul>
            )}
            <div className={styles.description}>{description}</div>
            {!compact && (
              <div className={styles.interval}>
                {formatUiMessage(
                  locale,
                  isAnnual ? "pricingYearlyInterval" : "pricingMonthlyInterval",
                )}
              </div>
            )}
            <button
              type="button"
              className={isHighlight ? styles.primaryButton : styles.secondaryButton}
              onClick={() => onSelect(plan.plan_id)}
              disabled={!detailsMode && (isCurrent || !plan.checkout_available)}
            >
              {buttonLabel}
            </button>
          </div>
        );
      })}
    </div>
  );
}
