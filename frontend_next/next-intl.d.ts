import type { UiLocale } from "./lib/i18n/config";
import type { UiMessageKey } from "./lib/i18n/messages";

declare module "next-intl" {
  interface AppConfig {
    Locale: UiLocale;
    Messages: Record<UiMessageKey, string>;
  }
}
