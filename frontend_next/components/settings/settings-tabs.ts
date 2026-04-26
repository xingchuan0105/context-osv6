export const SETTINGS_TABS = ["billing", "profile", "appearance", "security", "notifications"] as const;

export type SettingsTab = (typeof SETTINGS_TABS)[number];

export function normalizeSettingsTab(tab: string | string[] | undefined): SettingsTab {
  const value = Array.isArray(tab) ? tab[0] : tab;

  if (value && (SETTINGS_TABS as readonly string[]).includes(value)) {
    return value as SettingsTab;
  }

  return "billing";
}
