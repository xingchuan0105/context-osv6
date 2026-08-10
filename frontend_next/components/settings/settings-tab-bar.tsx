"use client";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { NavRail } from "../ui/nav-rail";
import { settingsTabIcon } from "./settings-nav-icons";
import { SETTINGS_TABS, settingsTabLabelKey, type SettingsTab } from "./settings-tabs";

export function SettingsTabBar({
  activeTab,
  onSelect,
}: {
  activeTab: SettingsTab;
  /** When set, use buttons (modal) instead of route links. */
  onSelect?: (tab: SettingsTab) => void;
}) {
  const { locale } = useUiPreferences();
  const items = SETTINGS_TABS.map((tab) => ({
    id: tab as string,
    label: formatUiMessage(locale, settingsTabLabelKey(tab)),
    icon: settingsTabIcon(tab),
    href: onSelect ? undefined : `/settings?tab=${tab}`,
  }));

  return (
    <NavRail
      activeId={activeTab}
      ariaLabel={formatUiMessage(locale, "settings.tabsLabel")}
      items={items}
      searchAriaLabel={formatUiMessage(locale, "settingsTabBar.searchLabel")}
      searchPlaceholder={formatUiMessage(locale, "settingsTabBar.searchPlaceholder")}
      searchTestId="settings-nav-search"
      testId="settings-nav-rail"
      onSelect={onSelect ? (id) => onSelect(id as SettingsTab) : undefined}
    />
  );
}
