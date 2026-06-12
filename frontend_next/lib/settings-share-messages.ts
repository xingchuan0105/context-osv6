import type { UiLocale } from "./i18n/config";
import { formatUiMessage, type UiMessageKey } from "./i18n/messages";

/** @deprecated Prefer `formatUiMessage` from `./i18n/messages`. Kept for existing settings/share surfaces. */
export type SettingsShareUiMessageKey = UiMessageKey;

/** @deprecated Prefer `formatUiMessage` from `./i18n/messages`. */
export function formatSettingsShareMessage(
  locale: UiLocale,
  key: SettingsShareUiMessageKey,
  values?: Record<string, string | number>,
) {
  return formatUiMessage(locale, key, values);
}
