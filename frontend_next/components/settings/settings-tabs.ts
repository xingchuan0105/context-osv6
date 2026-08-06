/**
 * Settings IA tabs (PRODUCT_IA P1-3).
 * Order: account first, then model, then money — not billing-as-home.
 */
export const SETTINGS_TABS = [
  "profile",
  "providers",
  "billing",
  "preferences",
  "security",
] as const;

export type SettingsTab = (typeof SETTINGS_TABS)[number];

/** Default when /settings has no ?tab= (account, not paywall). */
export const DEFAULT_SETTINGS_TAB: SettingsTab = "profile";

/** Map legacy query values onto current tabs. */
export function normalizeSettingsTab(tab: string | string[] | undefined): SettingsTab {
  const value = Array.isArray(tab) ? tab[0] : tab;
  if (!value) {
    return DEFAULT_SETTINGS_TAB;
  }
  if (value === "appearance") {
    return "preferences";
  }
  // Notifications left settings for account bell; unknown deep links → profile.
  if (value === "notifications") {
    return DEFAULT_SETTINGS_TAB;
  }
  if (value === "byok" || value === "provider") {
    return "providers";
  }
  if ((SETTINGS_TABS as readonly string[]).includes(value)) {
    return value as SettingsTab;
  }
  return DEFAULT_SETTINGS_TAB;
}
