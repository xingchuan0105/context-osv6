/** Settings IA tabs after ADR-0010 W3 (notifications → account bell in W4). */
export const SETTINGS_TABS = ["billing", "profile", "providers", "preferences", "security"] as const;

export type SettingsTab = (typeof SETTINGS_TABS)[number];

/** Map legacy query values onto current tabs. */
export function normalizeSettingsTab(tab: string | string[] | undefined): SettingsTab {
  const value = Array.isArray(tab) ? tab[0] : tab;
  if (!value) {
    return "billing";
  }
  if (value === "appearance") {
    return "preferences";
  }
  // Notifications leave settings for W4 bell; fall through to membership.
  if (value === "notifications") {
    return "billing";
  }
  if (value === "byok" || value === "provider") {
    return "providers";
  }
  if ((SETTINGS_TABS as readonly string[]).includes(value)) {
    return value as SettingsTab;
  }
  return "billing";
}
