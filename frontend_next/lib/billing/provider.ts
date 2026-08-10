import type { UiLocale } from "../i18n/config";

export type ActiveBillingProvider = "creem" | "alipay";

/**
 * Recommended default provider per locale. Both channels stay selectable on
 * /pricing regardless of locale — this only decides the pre-selected option.
 */
export function billingProviderForLocale(locale: UiLocale): ActiveBillingProvider {
  return locale === "zh-CN" ? "alipay" : "creem";
}

export function planPriceLabelForProvider(
  plan: {
    price_label_cny?: string;
    price_label_usd?: string;
    price_label?: string;
  },
  provider: ActiveBillingProvider,
): string {
  if (provider === "alipay") {
    return plan.price_label_cny?.trim() || plan.price_label?.trim() || "";
  }

  return plan.price_label_usd?.trim() || plan.price_label?.trim() || "";
}

export function planPriceLabel(
  plan: {
    price_label_cny?: string;
    price_label_usd?: string;
    price_label?: string;
  },
  locale: UiLocale,
): string {
  return planPriceLabelForProvider(plan, billingProviderForLocale(locale));
}
