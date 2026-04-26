export const APP_LOCALES = ["zh-CN", "en"] as const;

export type UiLocale = (typeof APP_LOCALES)[number];

export const DEFAULT_LOCALE: UiLocale = "zh-CN";
export const LOCALE_COOKIE_NAME = "avrag.ui.locale";

export function isUiLocale(value: string | null | undefined): value is UiLocale {
  return value === "zh-CN" || value === "en";
}

export function normalizeLocale(value: string | null | undefined): UiLocale {
  if (isUiLocale(value)) {
    return value;
  }

  return DEFAULT_LOCALE;
}
