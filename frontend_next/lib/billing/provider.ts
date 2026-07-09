import type { UiLocale } from "../i18n/config";

export type ActiveBillingProvider = "creem" | "alipay";

export function billingProviderForLocale(locale: UiLocale): ActiveBillingProvider {
  return locale === "zh-CN" ? "alipay" : "creem";
}

export function planPriceLabel(
  plan: {
    price_label_cny?: string;
    price_label_usd?: string;
    price_label?: string;
  },
  locale: UiLocale,
): string {
  if (locale === "zh-CN") {
    return plan.price_label_cny?.trim() || plan.price_label?.trim() || "";
  }

  return plan.price_label_usd?.trim() || plan.price_label?.trim() || "";
}
