"use client";

import Link from "next/link";

import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SETTINGS_TABS, type SettingsTab } from "./settings-tabs";

export function SettingsTabBar({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();
  const tabKeyMap: Record<SettingsTab, Parameters<typeof formatSettingsShareMessage>[1]> = {
    billing: "settings.tabs.billing",
    profile: "settings.tabs.profile",
    appearance: "settings.tabs.appearance",
    security: "settings.tabs.security",
    notifications: "settings.tabs.notifications",
  };

  return (
    <nav
      aria-label={formatSettingsShareMessage(locale, "settings.tabsLabel")}
      className="app-tab-bar"
    >
      {SETTINGS_TABS.map((tab) => (
        <Link
          aria-current={tab === activeTab ? "page" : undefined}
          className={`app-tab-button${tab === activeTab ? " app-tab-button-active" : ""}`}
          href={`/settings?tab=${tab}`}
          key={tab}
        >
          {formatSettingsShareMessage(locale, tabKeyMap[tab])}
        </Link>
      ))}
    </nav>
  );
}

